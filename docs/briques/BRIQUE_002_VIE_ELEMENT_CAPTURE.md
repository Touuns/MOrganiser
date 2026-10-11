# Brique 002 — Vie d'un élément capturé : transformation d'une capture en tâche

**Document de référence unique de la brique 002.**
**État : 002-A (PR #7) et 002-B (PR #8) validées et fusionnées dans `main` ; 002-C implémentée sur la branche `brique-002c-traitees` (2026-10-11), en attente de validation.**
**Dépendances :** brique 001 (A à E) validée et fusionnée dans `main`.
**À lire avec :** `01_CAHIER_DES_CHARGES.md` (sections C et D), `03_ARCHITECTURE.md`, `04_SECURITE.md`, `05_ROADMAP.md`, `BRIQUE_001_CAPTURE_RAPIDE.md`.

## 1. Intention

Permettre de **transformer une capture en tâche** sans jamais perdre son contenu ni créer de doublon. La capture reste intégralement conservée (son origine) ; elle quitte seulement « À organiser » et se retrouve dans « Traitées ».

Notes et projets ne sont **pas** proposés : leurs modules n'existent pas (règle de `06_INITIATION.md` : ne pas afficher comme disponible ce qui ne l'est pas). Ils s'ajouteront à la même mécanique.

### Déjà couvert par 001-B (non refait)
Fiche, édition du texte et de la destination, brouillon protégé, verrou optimiste, corbeille récupérable avec « Annuler », « Voir tout » paginé, filtre par destination. « Conserver une capture telle quelle » est le comportement par défaut.

## 2. Décisions validées (2026-10-10)

1. Conversion en tâche uniquement.
2. Tâche minimale : identifiant stable, titre, détails, statut initial « à faire », destination facultative, capture d'origine, horodatages.
3. Origine conservée, retirée de « À organiser », retrouvable dans « Traitées » et depuis la tâche.
4. Titre proposé : première ligne, 120 caractères au plus, modifiable avant validation ; le texte complet est dans les détails.
5. Annulation : notification « Annuler » de 4 s (composant existant) ; éléments retrouvables dans « Traitées ».
6. Destination copiée telle quelle, sans déduction.
7. Essais sur bases fictives isolées ; aucune migration de Stable ; toute migration d'une base existante est précédée d'une sauvegarde cohérente dont la restauration est testée.
8. `updated_at` de la capture inchangé par la conversion ; `converted_at` porte le changement de cycle de vie.

## 3. Découpage

| Sous-brique | Contenu | État |
|---|---|---|
| **002-A** | Migration 0002, modèle minimal, commandes Rust, conversion et annulation atomiques, sauvegarde préalable aux migrations, tests | Validée et fusionnée (PR #7) |
| **002-B** | « Transformer en tâche » dans la fiche (aperçu du titre, notification « Annuler »), première liste des tâches en lecture seule, liens « Tâches » dans l'en-tête de la boîte | Validée et fusionnée (PR #8) |
| **002-C** | Vue « Traitées », provenance depuis la tâche, restauration contrôlée (« Remettre dans la boîte »), finitions | Implémentée, en attente de validation |

002-A n'ajoute aucune interface ; 002-B ajoute la conversion et la liste des tâches (section 11). Hors périmètre de 002 : statuts avancés, terminer une tâche, sous-tâches, notes, projets, moteur d'initiation.

## 4. Modèle de données (migration `0002_tasks.sql`)

### Table `tasks`
| Colonne | Rôle |
|---|---|
| `id` | UUID v7, stable |
| `title` | 1 à 120 caractères (CHECK) |
| `details` | copie intégrale du texte de la capture (1 à 10 000 caractères) |
| `status` | `todo`, `in_progress`, `waiting`, `blocked`, `done` (CHECK) ; **002 n'écrit que `todo`** |
| `destination_id` | copiée telle quelle, facultative, référence `destinations` |
| `origin_inbox_item_id` | capture d'origine ; `NULL` pour une future tâche créée directement (003) |
| `created_at`, `updated_at` | ms UTC ; égaux à la création |
| `deleted_at` | annulation / corbeille logique ; **aucune suppression physique** |

### Colonnes ajoutées à `inbox_items` (facultatives)
`converted_at` (changement de cycle de vie) et `converted_task_id` (tâche active issue de la capture).

### Index et déclencheurs
| Objet | Rôle |
|---|---|
| `tasks_one_active_per_origin` (unique **partiel**, `deleted_at IS NULL`) | au plus une tâche active par capture ; une tâche annulée libère la place (reconversion possible) |
| `inbox_items_converted_task` (unique partiel) | une tâche n'est la conversion active que d'une capture |
| `tasks_recent`, `inbox_items_converted` | listes paginées (tâches, « Traitées ») |
| déclencheur `tasks_updated_at_must_grow` | toute modification **effective** de `title`, `details`, `status` ou `destination_id` doit faire croître **strictement** `updated_at` ; sinon `ABORT`. Une mise à jour sans changement de valeur, ou de `deleted_at` seul, passe |
| déclencheur `tasks_origin_is_immutable` | `id`, `origin_inbox_item_id` et `created_at` ne se réécrivent jamais |

Pourquoi le déclencheur : la garde d'annulation lit `updated_at = created_at`. Sans lui, une évolution future (brique 003 et suivantes) pourrait modifier une tâche sans que l'annulation le voie, et masquerait un travail déjà fait.

## 5. Invariants

1. Une capture a **au plus une tâche active**.
2. Une capture est « convertie » **si et seulement si** `converted_task_id` désigne une tâche active dont `origin_inbox_item_id` est cette capture.
3. **Rien n'est supprimé** : annuler = `deleted_at` sur la tâche ; la capture revient avec le même texte, la même destination, les mêmes dates (`updated_at` inclus).
4. Une tâche modifiée ou avancée depuis sa création ne s'annule plus.
5. Une capture convertie est **en lecture seule** et ne va pas à la corbeille (annuler d'abord).
6. Le texte de la capture n'est jamais modifié par la conversion ni par l'annulation.

## 6. Transactions

Toutes en `BEGIN IMMEDIATE` (verrou d'écriture pris **avant** les contrôles : aucune lecture périmée possible, y compris depuis une autre connexion). Tout échec annule l'ensemble.

| Opération | Étapes | Refus |
|---|---|---|
| **Convertir** `(id, expected_updated_at, titre?)` | lire la capture → contrôles → `INSERT tasks` → `UPDATE inbox_items … WHERE converted_at IS NULL AND deleted_at IS NULL AND updated_at = attendu` (exactement 1 ligne) | `not_found`, `trashed`, `already_converted` (+ `taskId`), `version_conflict`, `empty_title`, `title_too_long` |
| **Annuler** `(taskId)` | `UPDATE tasks SET deleted_at … WHERE deleted_at IS NULL AND updated_at = created_at AND status = 'todo'` → `UPDATE inbox_items SET converted_* = NULL WHERE converted_task_id = tâche` (exactement 1 ligne) | `task_not_found`, `task_not_active`, `no_origin`, `task_modified`, `inconsistent_state` |
| **Reconvertir** | comme « Convertir » ; l'ancienne tâche reste, une **nouvelle** est créée | — |

Le titre fourni est normalisé (espaces et retours à la ligne ramenés à un espace, 1 à 120 caractères). Sans titre, la proposition est utilisée : première ligne non vide, coupée à 120 caractères avec « … ».

## 7. Commandes Rust (déclarées dans `build.rs`, autorisées nommément, enregistrées dans `lib.rs`)

| Commande | Rôle |
|---|---|
| `convert_inbox_item_to_task(id, expectedUpdatedAt, title?)` | conversion ; renvoie `{ task, item }` |
| `cancel_task_conversion(id)` | annulation ; renvoie `{ task, item }` |
| `suggest_task_title(id)` | titre proposé (lecture seule) |
| `list_tasks(limit, before?)` | tâches actives, plus récentes d'abord, curseur `(created_at, id)` |
| `get_task(id)` | une tâche (annulée comprise) |
| `list_converted_items(limit, before?)` | captures converties (« Traitées »), curseur `(converted_at, id)` |

Les captures converties sont exclues de `list_inbox_items` (boîte). `InboxItem` expose désormais `convertedAt` et `convertedTaskId` (additif). Nouveau code d'erreur de la boîte : `converted`. Les messages d'erreur ne reprennent jamais le texte de la capture.

## 8. Sauvegarde préalable aux migrations

Module `src-tauri/src/storage/backup.rs`, appelé par `storage::open` **avant** `configure` et `migrate`, seulement pour une base existante (`0 < version < SCHEMA_VERSION`).

1. **Copie cohérente** par SQLite (`VACUUM INTO`), compatible WAL : instantané incluant `-wal`, base ouverte ou non. Jamais de copie du fichier `.db` seul.
2. Écriture dans `backups/morganiser-v<version>-<ms>.db.partial`, vérifiée, puis **renommage** : un `.partial` n'est jamais une sauvegarde. Un reste d'interruption est supprimé au prochain démarrage.
3. **Vérification :** `integrity_check`, `foreign_key_check`, `user_version` attendue sur la copie (toujours) ; puis mêmes tables et **mêmes lignes** que la source (comparaison valeur par valeur) **si la source est restée au repos**.
   - **Écriture concurrente :** `PRAGMA data_version` est lu avant la copie et après ses contrôles. Si une autre connexion ou un autre processus a validé une écriture entre-temps, la source n'est plus celle de l'instantané : la comparaison ligne à ligne n'aurait pas de sens et ne rejette donc **pas** une sauvegarde valide.
   - La copie est alors **refaite** (jusqu'à 3 essais, cible neuve à chaque fois, la copie abandonnée est supprimée). Une copie cohérente peut en effet ne pas contenir toutes les transactions présentes au moment où la migration va commencer.
   - **Si la source change encore pendant le troisième essai, la sauvegarde échoue avec une erreur explicite et la migration n'a pas lieu** : un point de restauration non validé n'est pas accepté. Les sauvegardes existantes sont conservées, aucun `.partial` ne subsiste, la base source n'est pas modifiée par ce mécanisme. Une source qui se stabilise à la 2ᵉ ou 3ᵉ tentative fonctionne normalement.
   - Sans écriture concurrente, toute anomalie (version, contenu, intégrité) reste un refus, donc **pas de migration**.
4. **Aucun écrasement :** nom déjà pris → suffixe `-1`, `-2`…
5. **Échec de sauvegarde ou de vérification = pas de migration** : le démarrage s'arrête avec une erreur explicite, la base reste strictement intacte.
6. **Conservation :** les trois plus récentes **utilisables** ; nettoyage seulement après le succès de la nouvelle sauvegarde. Un fichier illisible n'est ni compté ni supprimé ; les fichiers étrangers ne sont jamais touchés.

### Restauration (application FERMÉE)
1. **Déplacer** (ne pas supprimer) `morganiser.db`, `morganiser.db-wal` et `morganiser.db-shm` dans un dossier de mise de côté.
2. Copier la sauvegarde choisie (`backups/morganiser-v<N>-<horodatage>.db`) sous le nom `morganiser.db` dans le dossier de données.
3. Relancer : la base est rouverte, remigrée si besoin (avec une nouvelle sauvegarde) et remise en mode WAL.

Le test `restauration_depuis_une_sauvegarde` exécute exactement ces étapes sur une base fictive : captures et destinations d'origine retrouvées à l'identique, tâche créée après migration absente, ancienne base conservée dans le dossier mis de côté.

## 9. Tests (Rust : 117 réussis, 1 sous-processus ignoré par conception ; 69 avant 002-A)

- **Conversion (tasks) :** création et conservation intégrale du texte (texte hostile, accents, 10 000 caractères), destination copiée, titre (suggestion, normalisation, 120 caractères, caractères multi-octets), refus (déjà convertie, corbeille, version périmée, introuvable) **sans aucune modification**, atomicité par panne simulée entre les deux écritures.
- **Annulation :** capture remise à l'identique, tâche conservée, reconversion (plusieurs cycles), refus si modifiée ou avancée, cas limites, atomicité.
- **Interaction avec la boîte :** capture convertie hors boîte, en lecture seule, non supprimable ; pagination sans doublon.
- **Garde SQL :** déclencheur (chaque champ, valeur égale ou en recul, valeur supérieure acceptée, mise à jour sans changement acceptée, `deleted_at` libre), provenance immuable, index partiel, CHECK du schéma.
- **Invariants :** 400 opérations aléatoires reproductibles (conversion, annulation, corbeille, restauration, édition, enrichissement) avec contrôle des invariants et du texte après chaque pas ; 8 conversions simultanées de la même capture (une seule réussit) ; conversion/annulation simultanées ; 6 conversions simultanées distinctes.
- **Migration et sauvegarde :** v1 → v2 sans perte (captures, métadonnées, destinations, WAL non vidé), intégrité et clés étrangères, échec partiel sans modification durable, échec de sauvegarde = pas de migration (fichier de base identique octet pour octet), base de version future refusée sans sauvegarde, reste `.partial`, non-écrasement, conservation de trois, échec d'une nouvelle sauvegarde sans suppression, vérification qui refuse une copie différente, sauvegarde cohérente sous écritures concurrentes, copie naïve prouvée incomplète, restauration complète.
- **Configuration Tauri :** `chaque_commande_est_declaree_autorisee_et_enregistree` et `la_capacite_n_accorde_que_les_permissions_revues` (quinze commandes).

## 10. Limites et reporté

- **Aucune interface en 002-A** (historique) : la conversion était utilisable seulement par les commandes ; l'interface est livrée en 002-B (section 11).
- La base **Dev réelle** n'a pas été migrée : la première migration réelle se fera à un lancement ultérieur, après un essai distinct sur une copie isolée.
- Les commandes n'ont pas été exercées dans une fenêtre réelle (pas d'interface) ; leur enregistrement est vérifié par la compilation et les tests de configuration.
- Une tâche annulée est conservée mais n'a pas encore de vue de récupération : elle relève de la future corbeille des tâches (003).
- Restauration : procédure manuelle documentée et testée, sans interface.
- Sauvegarde **utilisateur** (planifiée ou exportable) toujours à décider avant toute donnée réelle dans Stable (voir `09_DECISIONS_OUVERTES.md`).
- Contrat pour 003 et suivantes : toute modification d'une tâche fait croître `updated_at` (imposé par le déclencheur) ; tout lien rattaché à une tâche (sous-tâche, note, relation) doit ajouter sa condition à la garde d'annulation.

## 11. Réalisation 002-B : interface de conversion et liste des tâches (2026-10-11, validée et fusionnée : PR #8)

### Parcours
1. **Fiche d'une capture :** bouton « Transformer en tâche » entre « Annuler les modifications » et « Mettre à la corbeille » (celle-ci reste isolée à droite). Absent pour une capture supprimée, supprimée ailleurs ou déjà convertie. Les actions de la fiche sont désormais **collantes** en bas du panneau : à 640 px de haut, elles ne sortent plus de la zone visible.
2. **Panneau de conversion**, intégré en haut de la fiche (pas de fenêtre modale) : titre proposé par `suggest_task_title` (sélectionné, modifiable, 120 caractères), rappel « texte complet conservé, destination reprise, capture conservée ». Pendant le panneau, le texte et la destination sont **figés** : on convertit la version enregistrée.
3. **Validation :** Entrée ou « Créer la tâche ». Le succès n'est affiché qu'après la réponse de Rust.
4. **Après la conversion :** la fiche se ferme, la carte part avec la transition de sortie 001-C **inchangée** (mouvement réduit respecté), le focus va à la carte voisine, et la notification « **Transformée en tâche** » + « Annuler » (4 s, pause au survol et au focus) reprend le composant existant.
5. **« Annuler » :** `cancel_task_conversion` ; la capture revient avec « Conversion annulée : la capture est de retour dans « À organiser » » ; la tâche est conservée (annulée) et sort de la liste ; si elle était ouverte, sa fiche se ferme.
6. **« Tâches » :** lien dans l'en-tête de la boîte, à côté de « Corbeille ». La vue remplace la scène (la capture rapide reste visible) ; **la plus ancienne en haut, la plus récente en bas**, lots de 50, anciennes chargées au-dessus avec défilement conservé. Chaque carte : titre, « À faire », destination, date.
7. **Fiche d'une tâche :** panneau flottant, **lecture seule** (titre, détails complets, statut, destination, date de création, « Issue de la capture du … »). Pas d'édition (brique 003).

### Protection du titre (vrai brouillon)
- Le titre modifié est protégé comme un texte non enregistré : fermeture de la fiche, ouverture d'une autre capture ou d'une autre vue, fermeture de la fenêtre et Échap passent par `requestLeave` / `LeaveBanner`. L'avertissement propose « Abandonner la conversion » ou « Continuer » (pas d'« Enregistrer » : il n'y a rien à enregistrer).
- Un titre **non modifié** se ferme sans avertissement ; « Annuler la conversion » est l'abandon volontaire explicite.
- La proposition de titre arrivée tardivement n'écrase jamais un titre déjà modifié ; une proposition d'une fiche refermée est ignorée.
- Texte modifié avant de transformer : Enregistrer (le panneau s'ouvre sur la version enregistrée), Abandonner (texte rétabli) ou Continuer, comme en 001-B.

### Erreurs et conflits (titre toujours conservé)
| Code | Comportement |
|---|---|
| `version_conflict` | version actuelle affichée, titre conservé, **nouvelle confirmation** sur la version actualisée |
| `already_converted` | fiche en lecture seule, « Voir la tâche » (`taskId`), titre rappelé dans le message |
| `trashed` / `not_found` | message avec le titre ; fiche en mode corbeille ou « n'existe plus » |
| `empty_title` / `title_too_long` | erreur sous le champ, panneau ouvert |
| autres (`storage`) | « Votre titre est conservé : réessayez », panneau ouvert |
| `converted` (enregistrer, corbeille) | notice, fiche rafraîchie en lecture seule, brouillon visible |

### Doubles validations et opérations
Verrou `inFlight` partagé avec l'enregistrement et la corbeille ; opération déclarée à `track` (la fermeture de la fenêtre l'attend) ; bouton désactivé « Création… », champ en lecture seule pendant l'envoi.

### Fichiers
`src/features/inbox/` : `api.ts` (types `Task`, `Conversion`, wrappers, `taskId` dans l'erreur), `CaptureDetail.tsx` + `.css` (panneau, protections, fiche convertie), `InboxHome.tsx` (notification généralisée, vue et fiche des tâches), `InboxPanel.tsx` (lien), `PagedCaptureView.tsx` (générique, rendu personnalisable ; usage historique inchangé), `TaskCard.tsx`, `TaskDetail.tsx` (nouveaux) ; `src/components/LeaveBanner.tsx` (« Enregistrer » facultatif) ; `src/test/fakeBackend.ts` (règles de 002-A). Rust : aucun code de production modifié ; test de répétition `storage/rehearsal_tests.rs`.

### Tests
- Interface : 213 réussis (171 existants + 28 conversion/protections + 14 tâches).
- Rust : 121 réussis, 2 ignorés par conception (sous-processus ; répétition sur copie).
- Vérification visuelle Edge sur base fictive (1000, 700 et 380 px) : panneau, avertissement, conversion, annulation, liste, fiche ; aucun défilement horizontal, boutons visibles, notification sans recouvrement du champ, mouvement réduit.

### Répétition de migration sur copie isolée (préparée, non exécutée sur des données personnelles)
Test Rust `repetition_sur_copie_isolee`, `#[ignore]`, paramétré par `MORGANISER_REHEARSAL_DB` : refuse tout chemin sous `%LOCALAPPDATA%` / `%APPDATA%` ou dans un dossier `com.morganiser*` ; **n'ouvre jamais** le fichier fourni (travaille sur une copie temporaire) ; migre avec sauvegarde, compare tables et empreintes, joue conversion/annulation/reconversion, restaure depuis la sauvegarde ; vérifie que les fichiers source n'ont pas changé ; n'affiche que des comptes et des empreintes. Le dispositif lui-même est testé sur une base fictive. *(Garde-fous durcis et procédure corrigée : section 13.)*

### Limites
- La conversion n'a pas été essayée dans l'application Windows avec la base Dev (interdit tant que la copie isolée n'a pas été répétée).
- « Traitées » et « Remettre dans la boîte » : livrés en 002-C (section 12).
- Fiche de tâche en lecture seule ; aucune édition ni changement de statut (003).
- À 360×480 la boîte reste très petite (limite préexistante, voir 001-E).

## 12. Réalisation 002-C : captures traitées et traçabilité (2026-10-11, en attente de validation)

Aucune nouvelle commande Rust, aucune migration : `list_converted_items`, `cancel_task_conversion`, `get_inbox_item` et `get_task` de 002-A suffisent. Seul un wrapper TypeScript (`listConvertedItems`) est ajouté.

### « Traitées » = captures **actuellement** converties
La vue liste les captures dont la conversion est **active**. Ce n'est **pas un historique** de toutes les conversions passées : une conversion annulée (par « Annuler » ou « Remettre dans la boîte ») fait sortir la capture de « Traitées » ; une reconversion l'y fait revenir une seule fois. Les tâches annulées restent conservées en base (`deleted_at`) mais n'ont pas de vue avant la brique 003.

### Parcours
1. **Accès :** lien « Traitées » dans l'en-tête de « À organiser », entre « Tâches » et « Corbeille ». La vue remplace la scène, la capture rapide reste visible, « ← Retour ».
2. **Liste :** `PagedCaptureView` réutilisé (lots de 50, plus ancienne en haut, plus récente en bas, anciennes chargées au-dessus avec défilement conservé), classée par **date de conversion** (« Traitée le … »), destination conservée, pas de badge « Tâche ». État vide : « Aucune capture traitée pour le moment. Une capture transformée en tâche apparaît ici. »
3. **Fiche de l'origine :** texte intégral, destination et dates d'origine, date de conversion, tout en lecture seule (aucun « Enregistrer », « Corbeille » ni « Transformer »). Commandes : « Voir la tâche » et « Remettre dans la boîte ».
4. **Navigation tâche ↔ capture :** « Voir la tâche » (existant) ; « Voir la capture » ajouté dans la fiche de tâche quand l'origine existe. Une seule fiche à la fois ; l'objet est relu dans Rust ; les gardes de brouillon existantes s'appliquent (tout passe par `openItem` / `openTask`).
5. **« Remettre dans la boîte » — uniquement dans la fiche de capture traitée** (ni sur les cartes de la liste, ni depuis la fiche de tâche) :
   - une confirmation intégrée, non modale, remplace la bannière : « La tâche liée sera annulée mais conservée… » ; boutons « Confirmer la remise » et « Garder en Traitées » ; **le focus est sur « Garder en Traitées »** ; Échap ferme la confirmation (la fiche reste) et rend le focus à la commande ;
   - `cancel_task_conversion(convertedTaskId)` n'est appelé qu'après confirmation, une seule fois (verrou `inFlight`), et le succès n'est affiché qu'après la réponse de Rust ;
   - succès : la fiche se ferme **seulement si c'est toujours la fiche de cette capture**, la capture sort de « Traitées », la boîte et les tâches sont relues, message d'état d'une ligne « Capture remise dans « À organiser ». », **aucune animation d'arrivée, aucune notification « Annuler »**, focus sur la carte voisine, sinon sur le champ de capture ;
   - la capture retrouve identité, texte, destination et `updatedAt` d'origine (garanti par Rust, vérifié par test) ; elle reprend sa place chronologique, donc peut ne pas figurer parmi les 20 de l'accueil : la confirmation l'indique et « Voir tout » la retrouve (testé avec 26 captures). `highlightId` (parcours pédagogique) n'est pas réutilisé.

### Erreurs et concurrence
| Situation | Comportement |
|---|---|
| `task_modified` | rien n'est annulé ; la capture reste dans « Traitées » ; message en haut de la fiche ; « Voir la tâche » disponible |
| `task_not_active` | fiche relue (capture de nouveau active), message « déjà annulée », listes relues |
| `task_not_found` | message d'erreur, fiche relue, listes relues |
| `not_found` (capture) | message, fiche en lecture seule, listes relues |
| `storage`, `inconsistent_state`, inconnu | état affiché inchangé, confirmation conservée, nouvel essai possible |
| « Voir la tâche » sur tâche déjà annulée ailleurs | message, aucune fiche de tâche, listes relues |
| « Voir la tâche » / « Voir la capture » : objet introuvable | message d'erreur, fiche courante conservée |
| Réponse tardive | un compteur de navigation périme toute ouverture par identifiant dès qu'une autre ouverture, fermeture ou changement de vue a lieu ; la réponse tardive d'une remise ne ferme jamais une autre fiche |

Les protections Rust restent l'autorité finale : l'interface ne contourne rien (le faux backend reproduit les mêmes règles).

### Anomalies trouvées à la vérification et corrigées
1. À 380 et 1000 px de large la confirmation, placée sous la bannière, sortait de la zone visible du panneau défilant, et le focus (sans défilement) y était invisible : la confirmation remplace maintenant la bannière, en haut, avec le même mode compact que le panneau de conversion.
2. En fenêtre étroite, le focus était perdu après la remise (la carte voisine est masquée tant que la fiche est ouverte) : le focus est posé après le rendu qui réaffiche la liste.
3. Le message d'état de deux lignes faisait remonter la capture rapide : message ramené à une ligne, détail déplacé dans la confirmation.
4. Le message de refus (`task_modified`), en bas de la fiche, n'était pas visible sans défilement : il est affiché en haut pour une capture traitée.

### Fichiers
`src/features/inbox/` : `api.ts` (`listConvertedItems`), `InboxHome.tsx` (vue, navigation par identifiant protégée, `handleReturned`), `InboxPanel.tsx` (lien), `CaptureDetail.tsx` + `.css` (confirmation, erreurs), `TaskDetail.tsx` (« Voir la capture »), `InboxHome.treated.test.tsx` (nouveau) ; `src/test/fakeBackend.ts` (`list_converted_items`). Rust : aucun fichier modifié.

### Tests
- Interface : 247 réussis (216 existants + 31 nouveaux : liste, ordre par conversion, pagination, lecture seule, navigation dans les deux sens, réponses tardives, objets introuvables ou inactifs, confirmation et Échap, remise à l'identique, double validation, refus tâche modifiée, conversion déjà annulée, erreurs et nouvel essai, fenêtre compacte, non-régression).
- Les tests de réponses tardives ont été validés par mutation (suppression du contrôle de péremption : 3 échecs).
- Revue avant publication : tri et curseur de « Traitées » vérifiés sur la date de **conversion** (55 captures converties dans l'ordre inverse de leur création, dates d'origine inchangées) ; la remise d'une capture ne retire plus la notification « Annuler » d'une autre capture (correctif ciblé, test validé par mutation).
- Vérification visuelle Edge sur base fictive (380, 700, 1000 px ; mouvement réduit) : liste, fiche, confirmation, Échap, navigation, remise, refus ; aucun défilement horizontal, boutons visibles, capture rapide immobile, aucune notification.

### Limites
- Pas de vue de récupération des tâches annulées (brique 003).
- Pas d'édition ni de statut de tâche (003) : une tâche modifiée ne peut plus être annulée, donc sa capture reste dans « Traitées » sans moyen de la remettre dans la boîte avant 003.
- En fenêtre de 380 px, les trois liens de l'en-tête passent sur deux lignes (« Corbeille » seule sur la seconde).
- Essai Windows réel avec la base Dev : toujours interdit tant que la répétition de migration sur copie isolée n'a pas été autorisée et exécutée.

## 13. Répétition de migration : garde-fous renforcés et procédure (2026-10-12, en attente de validation)

Branche `chore/rehearsal-safety`. Seul le dispositif de test (`storage/rehearsal_tests.rs`) est modifié, plus la dépendance de test `sha2` (déjà présente dans le graphe via Tauri : aucune nouvelle crate, rien dans l'exécutable livré). Aucun code de production, aucune migration, `environment.rs` et `backup.rs` inchangés.

### Garde-fous
| Garde-fou | Comportement |
|---|---|
| Variable `MORGANISER_REHEARSAL_DB` | absente, vide ou composée d'espaces : **échec explicite** (avant : réussite trompeuse) |
| Chemin | absolu, sans `..`, nommé `morganiser.db`, fichier ordinaire non lien |
| Zones protégées | `%LOCALAPPDATA%`, `%APPDATA%`, dossiers de données Dev et Stable (calculés avec `environment::data_dir`), composant `com.morganiser*` (préfixe tiré de `STABLE_IDENTIFIER`) |
| Chemin saisi ET canonique | les deux sont contrôlés ; comparaison par composants, insensible à la casse, préfixe `\\?\` retiré |
| Liens, jonctions, noms 8.3 | refus de tout chemin dont la forme canonique diffère de la forme saisie ; un lien vers une zone protégée est refusé comme tel |
| Zone protégée non résolue | la vérification **échoue** (refus par sécurité) ; variables `LOCALAPPDATA` / `APPDATA` absentes : échec |
| `-wal` et `-shm` | s'ils existent : ni lien, ni dossier, même dossier réel, hors zones protégées ; absents : normal |
| Avant validation | aucun fichier créé, aucune base ouverte (testé) |
| Source | jamais ouverte par SQLite ; lecture binaire seule (copie et SHA-256) |
| SHA-256 des fichiers source | présence, taille, date, empreinte de `.db`, `-wal`, `-shm` : avant la copie, après la copie, après l'opération, **même en cas d'erreur** ; la copie temporaire est comparée à la source avant toute ouverture SQLite |
| Rapport du test | ni texte, ni chemin, ni empreinte ; une **ligne-marqueur à vocabulaire fixe** (`REHEARSAL-RESULT status=… from=… to=… integrity=… backup=… roundtrip=… restore=… source=…`) dont chaque `OK` correspond à une étape réellement vérifiée ; les messages de refus ne contiennent jamais de chemin personnel |

Les empreintes SHA-256 de **fichiers** (contrôle de la source et de la copie) sont distinctes des empreintes **logiques** FNV des lignes SQLite (comparaison avant/après migration), qui ne sont plus affichées.

### Nettoyage du dossier de travail (limites réelles)
Le dossier de travail est préfixé `morganiser-rehearsal-` dans `%TEMP%`.
- **Réussite** et **erreur normale** : supprimé (testé ; les connexions SQLite sont fermées avant la suppression).
- **Panique** : supprimé (le déroulement exécute `Drop` ; testé).
- **Arrêt brutal** (`taskkill /F`, fermeture du terminal, coupure de courant) : **aucun nettoyage** ; le dossier et les données copiées restent. Démontré par un test avec sous-processus tué.
- **Suppression partielle** : `TempDir` ignore les erreurs (fichier tenu par un antivirus, un indexeur) ; un reliquat est possible même après une réussite.
- **Recommandation** : lister puis supprimer seulement `morganiser-rehearsal-*`, avec confirmation dossier par dossier, jamais `%TEMP%` en bloc (étape 8 ci-dessous).

### Aller-retour conversion/annulation : jamais déclaré sans avoir eu lieu
Si la copie ne contient **aucune capture active** (base vide, ou captures toutes à la corbeille), le test crée une capture **fictive** dans la copie temporaire, exécute réellement les deux aller-retour, puis supprime la capture et ses tâches avant de comparer le contenu de référence (restauration depuis la sauvegarde ensuite). Choix : plus sûr et plus informatif que « étape non exécutée », car le schéma v2, ses index et ses déclencheurs sont alors exercés sur la base réellement migrée ; `roundtrip=OK` n'est positionné qu'après l'exécution effective des conversions et annulations. Le rapport indique si l'essai a eu lieu sur une capture existante ou fictive.

### Tests ajoutés (bases fictives, dossiers temporaires)
Variable absente, vide, espaces ; chemin relatif, `..`, mauvais nom, inexistant (rien n'est créé) ; zones protégées (dossiers Dev et Stable fictifs, `com.morganiser*`, casse, préfixe verbatim) ; zone protégée non résolue ; absence de chemin personnel dans les refus ; jonction vers une zone protégée (chemin canonique) et vers un dossier ordinaire (redirection) ; lien symbolique (si le privilège existe) ; `-wal` absent, présent, dossier ; SHA-256 de référence ; changement de contenu à taille identique ; changement avec **date restaurée** ; échec de copie ; source modifiée pendant la copie ; copie infidèle ; source illisible ; source inchangée après réussite et après échec ; nettoyage après réussite, erreur, panique et arrêt brutal.

### Procédure PowerShell en deux phases indépendantes (à exécuter par le propriétaire)
Fichiers à coller : le **bloc commun** en tête de chaque phase. La phase B **ne démarre jamais** à la fin de la phase A : elle exige une nouvelle autorisation, une copie déjà validée et la confirmation `EXECUTER`.

Principes :
- **Aucune sortie brute n'est republiée.** Le rapport est un tableau synthétique construit à partir de résultats vérifiés (code de sortie, nombre de tests, ligne `test result`, ligne-marqueur). La sortie complète de `cargo` est écrite dans un **journal local** (`journal-phaseB-….log`, dans le dossier d'essais), jamais affichée.
- **Une réussite trompeuse est impossible :** code de sortie 0 **et** exactement un test exécuté **et** `1 passed; 0 failed; 0 ignored` **et** marqueur `status=OK` valide. Zéro test exécuté = échec.
- **Messages fixes** : les arrêts contrôlés affichent des phrases sans chemin ; toute autre erreur affiche seulement qu'une vérification manuelle des journaux locaux est nécessaire.
- **Source Dev validée avant toute lecture de contenu** (`Assert-DevSource`, métadonnées seulement, phases A et B) : le dossier doit être `…\com.morganiser.desktop.dev\data` ; il ne doit pas désigner Stable ; ni lui ni aucun de ses parents ne doit être un lien ou une jonction ; une redirection vers Stable est refusée explicitement ; chacun des trois fichiers présents doit être un fichier ordinaire (ni dossier, ni lien). Le dossier Stable n'est jamais lu : `$StableDataDir` ne sert qu'à une comparaison de chaînes.
- **Cohérence A/B :** la phase B recalcule les empreintes de la source Dev actuelle et les compare au manifeste de la phase A (nom, présence, SHA-256, fichier par fichier). Toute différence refuse l'essai **avant** la répétition ; la copie de référence est conservée et une nouvelle phase A est requise.
- **Aucune suppression automatique.** La copie de référence n'est jamais supprimée ; les dossiers `morganiser-rehearsal-*` restants sont listés puis supprimés un par un, sur confirmation `oui`, jamais `%TEMP%` en bloc.

#### Bloc commun
```powershell
# ===== BLOC COMMUN (identique en tête des phases A et B) ============================
$ErrorActionPreference = 'Stop'
$AppDataDir = Join-Path $env:LOCALAPPDATA 'com.morganiser.desktop.dev\data'   # Dev UNIQUEMENT
$StableDataDir = Join-Path $env:LOCALAPPDATA 'com.morganiser.desktop\data'      # comparaison de chaînes SEULEMENT : jamais lu
$Root       = 'C:\MOrganiserEssais'                   # dossier local NON synchronisé
$RepoTauri  = 'I:\ProjetDEv\Morganiser\src-tauri'
$Names      = @('morganiser.db', 'morganiser.db-wal', 'morganiser.db-shm')

function Fail([string]$Message) { throw ('CTRL:' + $Message) }   # messages fixes, sans chemin

function Get-Snapshot([string]$Dir) {             # présence + SHA-256 de chaque fichier
    $h = @{}
    foreach ($n in $Names) {
        $p = Join-Path $Dir $n
        if (Test-Path -LiteralPath $p) { $h[$n] = (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash } else { $h[$n] = $null }
    }
    return $h
}
function Test-SameSnapshot($A, $B) {              # même présence, mêmes empreintes, fichier par fichier
    foreach ($n in $Names) { if ($A[$n] -ne $B[$n]) { return $false } }
    return $true
}
function Test-FileFree([string]$Path) {           # lecture exclusive : échoue si un processus tient le fichier
    try { $s = [IO.File]::Open($Path, 'Open', 'Read', 'None'); $s.Close(); return $true } catch { return $false }
}
function Assert-SafeFolder([string]$Path) {       # hors zones protégées et synchronisées, sans jonction ni lien
    $full = [IO.Path]::GetFullPath($Path)
    foreach ($zone in @($env:LOCALAPPDATA, $env:APPDATA, $env:OneDrive, $env:OneDriveConsumer, $env:OneDriveCommercial)) {
        if ($zone -and $full.StartsWith($zone, [StringComparison]::OrdinalIgnoreCase)) { Fail 'Dossier interdit (zone protégée ou synchronisée).' }
    }
    if ($full -match 'OneDrive|Dropbox|Google Drive|iCloud|com\.morganiser') { Fail 'Dossier interdit (synchronisé ou données de l''application).' }
    $probe = $full
    while ($probe) {
        if (Test-Path -LiteralPath $probe) {
            if ((Get-Item -LiteralPath $probe -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { Fail 'Le chemin contient une jonction ou un lien.' }
        }
        $parent = Split-Path -Parent $probe
        if (-not $parent -or $parent -eq $probe) { break }
        $probe = $parent
    }
}
function Assert-DevSource([string]$Dir) {         # MÉTADONNÉES seulement : aucun contenu lu avant validation
    $full = [IO.Path]::GetFullPath($Dir).TrimEnd('\')
    $stable = [IO.Path]::GetFullPath($StableDataDir).TrimEnd('\')
    if ($full -notlike '*\com.morganiser.desktop.dev\data') { Fail 'Le dossier source n''est pas le dossier de données Dev attendu.' }
    if ($full.StartsWith($stable, [StringComparison]::OrdinalIgnoreCase)) { Fail 'Le dossier source désigne Stable : refus.' }
    $probe = $full                                  # le dossier ET chacun de ses parents : ni lien, ni jonction
    while ($probe) {
        $item = Get-Item -LiteralPath $probe -Force -ErrorAction SilentlyContinue
        if ($item -and ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            $target = ($item.Target -join ';')
            if ($target -match '\\com\.morganiser\.desktop(\\|;|$)' -or $target.StartsWith($stable, [StringComparison]::OrdinalIgnoreCase)) {
                Fail 'Le dossier Dev est redirigé vers Stable : refus.'
            }
            Fail 'Le dossier source ou l''un de ses parents est un lien ou une jonction : refus.'
        }
        $parent = Split-Path -Parent $probe
        if (-not $parent -or $parent -eq $probe) { break }
        $probe = $parent
    }
    if (-not (Test-Path -LiteralPath $full -PathType Container)) { Fail 'Dossier de données Dev introuvable.' }
    foreach ($n in $Names) {                        # chaque fichier présent : ordinaire, ni lien, ni dossier
        $f = Get-Item -LiteralPath (Join-Path $full $n) -Force -ErrorAction SilentlyContinue
        if ($f) {
            if ($f.PSIsContainer) { Fail "$n n'est pas un fichier ordinaire : refus." }
            if ($f.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                $target = ($f.Target -join ';')
                if ($target -match '\\com\.morganiser\.desktop(\\|;|$)') { Fail "$n est redirigé vers Stable : refus." }
                Fail "$n est un lien : refus."
            }
        }
    }
}
function Assert-AppStopped {                      # « Quitter » depuis la zone de notification ; le × ne fait que masquer
    if (Get-Process -Name 'morganiser-dev' -ErrorAction SilentlyContinue) { Fail 'M''Organiser Dev tourne encore : quittez-le puis relancez.' }
    $held = @(Get-Process | Where-Object { try { $_.Path -like '*morganiser-dev*' } catch { $false } })
    if ($held.Count -gt 0) { Fail 'Un processus morganiser-dev subsiste.' }
    if (-not (Test-Path -LiteralPath (Join-Path $AppDataDir 'morganiser.db'))) { Fail 'morganiser.db introuvable dans les données Dev.' }
    foreach ($n in $Names) {
        $p = Join-Path $AppDataDir $n
        if ((Test-Path -LiteralPath $p) -and -not (Test-FileFree $p)) { Fail "$n est tenu par un processus : application non complètement arrêtée." }
    }
}
function Write-Failure($ErrorRecord, [string]$Log) {   # le détail brut va au journal LOCAL, jamais à l'écran
    $message = [string]$ErrorRecord.Exception.Message
    if ($Log) { try { ($ErrorRecord | Out-String) | Add-Content -LiteralPath $Log -Encoding UTF8 } catch { } }
    if ($message.StartsWith('CTRL:')) { Write-Host ('ÉCHEC : ' + $message.Substring(5)) }
    else { Write-Host 'ÉCHEC inattendu : vérification manuelle des journaux locaux nécessaire.' }
}
function Write-Report($Report) {
    foreach ($entry in $Report.GetEnumerator()) { Write-Host ('{0,-36}: {1}' -f $entry.Key, $entry.Value) }
}
# ===== FIN DU BLOC COMMUN ============================================================
```

#### Phase A — création et vérification de la copie Dev
Vérifie l'arrêt de l'application, contrôle les dossiers, calcule les empreintes, copie `.db`, `-wal`, `-shm`, compare nom par nom et empreinte par empreinte, écrit un **manifeste** (hors de la copie), met la copie en lecture seule, puis **s'arrête**. En cas de différence, la copie est conservée comme INVALIDE et aucun manifeste n'est écrit (la phase B la refusera).

```powershell
# ===== PHASE A — Création et vérification de la copie Dev ===========================
# S'arrête à la fin. La phase B exige une NOUVELLE autorisation et ne démarre jamais seule.
$Stamp    = Get-Date -Format 'yyyyMMdd-HHmmss'
$Dest     = Join-Path $Root ("dev-$Stamp")
$Manifest = "$Dest.manifest.json"
$Log      = Join-Path $Root ("journal-phaseA-$Stamp.log")
$Report   = [ordered]@{
    'Dossier Dev ordinaire (sans lien)'  = 'ÉCHEC'
    'Application Dev arrêtée'            = 'ÉCHEC'
    'Copie fidèle (.db, -wal, -shm)'     = 'ÉCHEC'
    'Source Dev inchangée'               = 'ÉCHEC'
    'Manifeste des empreintes écrit'     = 'ÉCHEC'
}
try {
    Assert-SafeFolder $Root
    if (-not (Test-Path -LiteralPath $Root)) { New-Item -ItemType Directory -Path $Root | Out-Null }
    Assert-SafeFolder $Root
    if (Test-Path -LiteralPath $Dest) { Fail 'La destination existe déjà.' }

    Assert-DevSource $AppDataDir                         # chemins validés AVANT toute lecture de contenu
    $Report['Dossier Dev ordinaire (sans lien)'] = 'OK'
    Assert-AppStopped
    $Report['Application Dev arrêtée'] = 'OK'

    $S0 = Get-Snapshot $AppDataDir                       # référence : nom, présence, SHA-256
    New-Item -ItemType Directory -Path $Dest | Out-Null
    foreach ($n in $Names) {
        if ($S0[$n]) { Copy-Item -LiteralPath (Join-Path $AppDataDir $n) -Destination (Join-Path $Dest $n) }
    }
    $S1 = Get-Snapshot $Dest                             # la copie est-elle identique ?
    $S2 = Get-Snapshot $AppDataDir                       # la source a-t-elle changé pendant la copie ?
    $Report['Copie fidèle (.db, -wal, -shm)'] = if (Test-SameSnapshot $S0 $S1) { 'OK' } else { 'DIFFÉRENT' }
    $Report['Source Dev inchangée']           = if (Test-SameSnapshot $S0 $S2) { 'OK' } else { 'DIFFÉRENT' }
    if ($Report['Source Dev inchangée'] -ne 'OK')           { Fail 'Source modifiée pendant la copie : copie INVALIDE, conservée pour examen, ne pas l''utiliser.' }
    if ($Report['Copie fidèle (.db, -wal, -shm)'] -ne 'OK') { Fail 'Copie différente de la source : copie INVALIDE, conservée pour examen, ne pas l''utiliser.' }

    $files = [ordered]@{}
    foreach ($n in $Names) { $files[$n] = $S0[$n] }
    (@{ schema = 1; files = $files } | ConvertTo-Json) | Set-Content -LiteralPath $Manifest -Encoding UTF8
    foreach ($n in $Names) {
        if ($S0[$n]) { (Get-Item -LiteralPath (Join-Path $Dest $n)).IsReadOnly = $true }
    }
    $Report['Manifeste des empreintes écrit'] = 'OK'
}
catch { Write-Failure $_ $Log }
finally {
    Write-Host ''
    Write-Host '===== RAPPORT SYNTHÉTIQUE (phase A) ====='
    Write-Report $Report
    $allOk = -not ($Report.Values -contains 'ÉCHEC' -or $Report.Values -contains 'DIFFÉRENT')
    if ($allOk) {
        Write-Host ('Dossier de copie validé           : ' + (Split-Path -Leaf $Dest))
        Write-Host 'PHASE A TERMINÉE. Ne lancez PAS la phase B sans nouvelle autorisation.'
    } else {
        Write-Host 'PHASE A EN ÉCHEC : aucune répétition possible. Rien n''a été supprimé ni exécuté.'
    }
}
```

#### Phase B — répétition de migration (nouvelle autorisation requise)
Reprend le dossier validé par la phase A, **revérifie ses empreintes** contre le manifeste, revérifie l'arrêt de l'application, n'exécute que le test exact, contrôle le résultat et l'absence de modification de la source Dev et de la copie, puis affiche le rapport synthétique.

```powershell
# ===== PHASE B — Répétition de migration sur une copie DÉJÀ validée par la phase A ===
$DestLeaf = ''          # À RENSEIGNER : nom du dossier validé par la phase A, ex. dev-20261012-101500
$Stamp    = Get-Date -Format 'yyyyMMdd-HHmmss'
$Log      = Join-Path $Root ("journal-phaseB-$Stamp.log")
$Report   = [ordered]@{
    'Source Dev identique à la phase A'  = 'ÉCHEC'
    'Tests réellement exécutés'          = 'inconnu'
    'Répétition'                         = 'ÉCHEC'
    'Version'                            = 'non vérifiée'
    'Intégrité'                          = 'ÉCHEC'
    'Sauvegarde et restauration'         = 'ÉCHEC'
    'Source Dev inchangée'               = 'ÉCHEC'
    'Copie de référence inchangée'       = 'ÉCHEC'
}
$Verdict = 'ÉCHEC'
$Started = $false
try {
    # --- Reprise d'une copie validée, empreintes de référence revérifiées ---------
    if (-not $DestLeaf) { Fail 'Renseignez $DestLeaf avec le dossier validé par la phase A.' }
    if ($DestLeaf -notmatch '^dev-\d{8}-\d{6}$') { Fail 'Nom de dossier de copie invalide.' }
    $Dest     = Join-Path $Root $DestLeaf
    $Manifest = "$Dest.manifest.json"
    Assert-SafeFolder $Root
    Assert-SafeFolder $Dest
    if (-not (Test-Path -LiteralPath $Manifest)) { Fail 'Manifeste de la phase A introuvable : refaites la phase A.' }
    if (-not (Test-Path -LiteralPath (Join-Path $Dest 'morganiser.db'))) { Fail 'Copie introuvable : refaites la phase A.' }
    $manifestData = Get-Content -LiteralPath $Manifest -Raw | ConvertFrom-Json
    $Expected = @{}
    foreach ($n in $Names) { $Expected[$n] = $manifestData.files.$n }
    $C0 = Get-Snapshot $Dest
    if (-not (Test-SameSnapshot $Expected $C0)) { Fail 'La copie ne correspond plus au manifeste de la phase A : essai refusé.' }

    Assert-DevSource $AppDataDir                         # chemins validés AVANT toute lecture de contenu
    Assert-AppStopped
    $B0 = Get-Snapshot $AppDataDir                       # source Dev avant l'essai
    # La source Dev doit être celle de la phase A : mêmes fichiers (nom, présence), mêmes SHA-256.
    if (-not (Test-SameSnapshot $Expected $B0)) {
        $Report['Source Dev identique à la phase A'] = 'DIFFÉRENT'
        Fail 'La source Dev a changé depuis la phase A : copie conservée, essai refusé. Refaites la phase A.'
    }
    $Report['Source Dev identique à la phase A'] = 'OK'

    # --- Autorisation explicite, puis UNIQUEMENT le test exact --------------------
    if ((Read-Host 'Tapez EXECUTER pour lancer la répétition sur la COPIE') -ne 'EXECUTER') { Fail 'Phase B annulée : rien n''a été exécuté.' }
    $Started = $true
    $env:MORGANISER_REHEARSAL_DB = Join-Path $Dest 'morganiser.db'
    $ErrorActionPreference = 'Continue'                  # stderr de cargo ne doit pas lever d'exception
    Push-Location $RepoTauri
    try {
        $lines = @(& cargo test --lib -- --exact storage::rehearsal_tests::repetition_sur_copie_isolee --ignored --nocapture 2>&1 | ForEach-Object { ("$_").TrimEnd() })
        $exit  = $LASTEXITCODE
    } finally {
        Pop-Location
        Remove-Item Env:MORGANISER_REHEARSAL_DB -ErrorAction SilentlyContinue
        $ErrorActionPreference = 'Stop'
    }
    $lines | Set-Content -LiteralPath $Log -Encoding UTF8   # sortie brute : journal LOCAL, jamais affiché ni republié

    # --- Résultats VÉRIFIÉS, jamais recopiés ---------------------------------------
    $ran = -1
    $runLines = @($lines | Where-Object { $_ -match '^running \d+ tests?$' })
    if ($runLines.Count -eq 1) { $ran = [int]([regex]::Match($runLines[0], '\d+').Value) }
    if ($ran -ge 0) { $Report['Tests réellement exécutés'] = [string]$ran }
    $passed  = (@($lines | Where-Object { $_ -match '^test result: ok\. 1 passed; 0 failed; 0 ignored;' })).Count -eq 1
    $markers = @($lines | Where-Object { $_ -match '^REHEARSAL-RESULT ' })
    $m = $null
    if ($markers.Count -eq 1) {
        $match = [regex]::Match($markers[0], '^REHEARSAL-RESULT status=(OK|FAIL) from=(\d|\?) to=(\d|\?) integrity=(OK|FAIL) backup=(OK|FAIL) roundtrip=(OK|FAIL) restore=(OK|FAIL) source=(OK|DIFFERENT|UNVERIFIABLE|NOTCHECKED)$')
        if ($match.Success) { $m = $match.Groups }
    }
    # Réussite = code 0 ET exactement un test ET « 1 passed » ET marqueur valide status=OK.
    $testOk = ($exit -eq 0) -and ($ran -eq 1) -and $passed -and ($null -ne $m) -and ($m[1].Value -eq 'OK')
    if ($testOk) {
        $Report['Répétition'] = 'OK'
        if ($m[2].Value -eq '1' -and $m[3].Value -eq '2') { $Report['Version'] = 'v1 -> v2 (vérifiée)' }
        $Report['Intégrité'] = if ($m[4].Value -eq 'OK') { 'OK' } else { 'ÉCHEC' }
        $Report['Sauvegarde et restauration'] = if ($m[5].Value -eq 'OK' -and $m[7].Value -eq 'OK') { 'OK' } else { 'ÉCHEC' }
    }

    # --- La source Dev et la copie de référence n'ont pas bougé --------------------
    $B1 = Get-Snapshot $AppDataDir
    $C1 = Get-Snapshot $Dest
    $Report['Source Dev inchangée']     = if (Test-SameSnapshot $B0 $B1) { 'OK' } else { 'DIFFÉRENT' }
    $copyOk = (Test-SameSnapshot $C0 $C1) -and (Test-SameSnapshot $Expected $C1) -and (($null -eq $m) -or ($m[8].Value -eq 'OK'))
    $Report['Copie de référence inchangée'] = if ($copyOk) { 'OK' } else { 'DIFFÉRENT' }

    if ($testOk -and -not ($Report.Values -contains 'ÉCHEC') -and -not ($Report.Values -contains 'DIFFÉRENT') -and $Report['Version'] -like 'v1*') { $Verdict = 'RÉUSSITE' }
    elseif ($null -eq $m) { Write-Host 'Résultat inconnu : vérification manuelle des journaux locaux nécessaire.' }
}
catch { Write-Failure $_ $Log }
finally {
    Write-Host ''
    Write-Host '===== RAPPORT SYNTHÉTIQUE (phase B) ====='
    Write-Report $Report
    Write-Host ('{0,-36}: {1}' -f 'Verdict', $Verdict)
    if ($Verdict -ne 'RÉUSSITE' -and $Started) { Write-Host ('Journal local à consulter : ' + (Split-Path -Leaf $Log)) }
    Write-Host 'La copie de référence est CONSERVÉE (aucune suppression automatique).'
}

# --- Dossiers de travail restants : nettoyage ciblé, confirmé un par un -----------
$left = @(Get-ChildItem -LiteralPath $env:TEMP -Directory -Filter 'morganiser-rehearsal-*' -ErrorAction SilentlyContinue |
          Where-Object { -not ($_.Attributes -band [IO.FileAttributes]::ReparsePoint) })
Write-Host ("Dossiers de travail restants : {0}" -f $left.Count)
foreach ($d in $left) {
    if ((Read-Host ("Supprimer {0} ? (oui/non)" -f $d.Name)) -eq 'oui') { Remove-Item -LiteralPath $d.FullName -Recurse -Force }
}
```

Rapport attendu en cas de réussite : `Source Dev identique à la phase A : OK`, `Tests réellement exécutés : 1`, `Répétition : OK`, `Version : v1 -> v2 (vérifiée)`, `Intégrité : OK`, `Sauvegarde et restauration : OK`, `Source Dev inchangée : OK`, `Copie de référence inchangée : OK`, `Verdict : RÉUSSITE`.

#### Vérification de la procédure elle-même
Les deux phases ont été exécutées sur un **faux dossier de données** (base v1 fictive avec `-wal` et `-shm`, générée à partir de la vraie migration 0001, hors `%LOCALAPPDATA%`), jamais sur la base Dev :
- réussite complète (phases A puis B) ;
- zéro test exécuté (filtre inexistant) : `Verdict : ÉCHEC`, « vérification manuelle des journaux locaux nécessaire » ;
- copie falsifiée d'un octet : phase B refusée **avant** tout test ;
- `-wal` tenu par un autre processus : phase A refusée, aucune copie créée ;
- **source Dev fictive modifiée après la phase A : phase B refusée avant tout test** (`Source Dev identique à la phase A : DIFFÉRENT`, aucun test exécuté, copie conservée) ;
- **dossier Dev fictif = jonction vers Stable fictif** (fichier Stable verrouillé : toute tentative de lecture aurait échoué) : refus « redirigé vers Stable », Stable intact ;
- **parent du dossier Dev fictif = jonction** : refus ;
- **fichier associé (`-shm`) remplacé par un dossier** : refus ;
- retour à la normale : phase A de nouveau réussie ;
- ces scénarios tournent avec `LOCALAPPDATA` redirigée vers un faux dossier (processus enfants seulement), ce qui permet d'exécuter les scripts **non modifiés** ;
- défaut découvert et corrigé par cet essai : la copie de référence en lecture seule se propageait à la copie de travail (`attempt to write a readonly database`) ; l'attribut est maintenant levé sur la copie de travail uniquement (test ajouté).

### Limites résiduelles
- Les liens symboliques de **fichiers** n'ont pas pu être créés sur ce poste (privilège absent) : leur refus repose sur le code et n'a pas été exercé ; les jonctions de **dossiers** l'ont été.
- La procédure PowerShell a été exécutée **uniquement sur des données fictives** ; jamais sur la base Dev réelle.
- Les liens symboliques de **fichiers** n'ont pas pu être créés sur ce poste (privilège absent) : leur refus par `Assert-DevSource` repose sur le code ; jonctions de dossiers et dossiers à la place d'un fichier sont exercés.
- Les liens physiques (« hard links ») ne sont pas détectés ; ils ne pourraient mener qu'à une lecture seule, car la source n'est jamais ouverte en écriture ni par SQLite.
- Le test refuse une base qui n'est pas en v1 : si l'application a déjà migré Dev, il n'y a rien à répéter.
- `Test-FileFree` ouvre les fichiers en lecture exclusive ; il détecte un processus qui les tient, pas un processus qui les ouvrirait plus tard.
