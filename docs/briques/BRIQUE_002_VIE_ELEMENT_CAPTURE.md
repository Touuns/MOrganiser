# Brique 002 — Vie d'un élément capturé : transformation d'une capture en tâche

**Document de référence unique de la brique 002.**
**État : 002-A validée et fusionnée dans `main` (PR #7) ; 002-B implémentée sur la branche `brique-002b-interface`, en attente de validation ; 002-C non commencée.**
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
| **002-B** | « Transformer en tâche » dans la fiche (aperçu du titre, notification « Annuler »), première liste des tâches en lecture seule, liens « Tâches » dans l'en-tête de la boîte | À faire |
| **002-C** | Vue « Traitées », provenance depuis la tâche, restauration contrôlée (« Remettre dans la boîte »), finitions | À faire |

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

## 11. Réalisation 002-B : interface de conversion et liste des tâches (2026-10-11, en attente de validation)

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
Test Rust `repetition_sur_copie_isolee`, `#[ignore]`, paramétré par `MORGANISER_REHEARSAL_DB` : refuse tout chemin sous `%LOCALAPPDATA%` / `%APPDATA%` ou dans un dossier `com.morganiser*` ; **n'ouvre jamais** le fichier fourni (travaille sur une copie temporaire) ; migre avec sauvegarde, compare tables et empreintes, joue conversion/annulation/reconversion, restaure depuis la sauvegarde ; vérifie que les fichiers source n'ont pas changé ; n'affiche que des comptes et des empreintes. Le dispositif lui-même est testé sur une base fictive.

### Limites
- La conversion n'a pas été essayée dans l'application Windows avec la base Dev (interdit tant que la copie isolée n'a pas été répétée).
- Pas de vue « Traitées » ni de « Remettre dans la boîte » depuis la liste (002-C) ; une capture convertie ouverte par une liste périmée est en lecture seule avec « Voir la tâche ».
- Fiche de tâche en lecture seule ; aucune édition ni changement de statut (003).
- À 360×480 la boîte reste très petite (limite préexistante, voir 001-E).
