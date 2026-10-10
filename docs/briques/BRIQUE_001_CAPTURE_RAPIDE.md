# Brique 001 — Capture rapide et boîte « À organiser »

**Document de référence unique de la brique 001** (fusion, le 2026-10-10, de l'ancienne fiche et de la spécification V2 validée par le propriétaire).
**État : 001-A validée et fusionnée dans `main` (PR #2) ; 001-B implémentée sur la branche `brique-001b-gestion`, en attente de validation ; 001-C à 001-E spécifiées, non commencées.**
**Dépendance :** brique 000 (fondations) fusionnée dans `main` (`d527798`).
**À lire avec :** `01_CAHIER_DES_CHARGES.md`, `02_UI_UX.md`, `03_ARCHITECTURE.md`, `04_SECURITE.md`, `05_ROADMAP.md`, `06_INITIATION.md`.

## Besoin utilisateur

« J'ouvre M'Organiser et je vois un champ. J'écris ce qui me passe par la tête, je valide et cela rejoint, juste au-dessus, une boîte de réception. Je peux le retrouver après redémarrage. »

## 1. Intention

Rendre utile M'Organiser dès sa première fonctionnalité : saisir une idée, une obligation ou une information en quelques secondes ; la voir rejoindre, **vers le haut**, la boîte **« À organiser »** ; la retrouver après redémarrage. L'application ne transforme **pas automatiquement** une capture en tâche, note, facture ou objet d'inventaire. Le contenu est du **texte** : jamais exécuté comme du code, jamais interprété par une IA.

### Destination ≠ tag (correction conceptuelle importante)

Le contrôle facultatif de la capture **ne sélectionne pas des tags descriptifs**. Il choisit **une destination**, c'est-à-dire **où l'élément sera consultable et traité**.

- **Destinations initiales (décision du 2026-10-10)** :
  - espaces de **responsabilité** : **Moi**, **Externe** ;
  - **rubriques** fonctionnelles : **Administratif**, **Finances**, **Inventaire**.
  « Vente » n'est pas retenue pour l'instant. Ces destinations **ne créent aucun module** : choisir « Finances » ou « Inventaire » ne développe pas ces modules. La liste sera harmonisée avec la navigation du tableau de bord quand celle-ci sera construite (brique 006).
- La nature de chaque destination (responsabilité ou rubrique) est enregistrée, pour permettre plus tard des associations plus riches (ex. une capture « Moi » **et** « Finances ») sans altérer les captures existantes. En 001-A : **une seule destination principale, facultative**.
- Sans destination : l'élément est dans « À organiser », signalé « À classer ». Une capture sans destination est parfaitement valide.
- Avec destination : l'élément reste dans « À organiser » **et** peut y être retrouvé par un **filtre** sur cette destination (exigence minimale, même sans module définitif). Il s'agit **d'un seul enregistrement**, jamais de copies.
- Une destination est **un classement initial**, pas le traitement complet de l'information.
- **Ne pas faire disparaître** une capture de la boîte parce qu'une destination a été choisie : sa sortie dépendra d'une action explicite de traitement, développée plus tard.
- Des tags descriptifs ou associations multiples pourront venir avec une fiche enrichie future. Le sélecteur ne doit **jamais** s'appeler « Tags » dans l'interface.
- Les destinations ont des **identifiants stables** : un renommage futur ne doit effacer ni l'historique ni les associations. Une destination n'est jamais supprimée, seulement archivée.

## 2. Implantation UI / UX

```text
┌──────────────────────────────────────────────────────┐
│ À organiser                [Filtre : Toutes ▾]       │
│ ┌──────────────────────────────────────────────────┐ │
│ │ Captures plus anciennes…                         │ │
│ │ Dernière capture — éventuellement sa destination │ │
│ └──────────────────────────────────────────────────┘ │
│                     ↑ (transfert animé en 001-C)     │
│ Capture rapide                                       │
│ [ Écrire une idée, obligation ou note…             ] │
│ Destination (facultatif) : [Aucune ▾]   [ Envoyer ↑ ]│
└──────────────────────────────────────────────────────┘
```

- **La boîte « À organiser » se trouve AU-DESSUS du champ**, pas sur son côté (remplace l'ancienne disposition « côte à côte »).
- Champ de capture **toujours visible** sur l'accueil, sans bouton préalable.
- Mode compact : les mêmes deux éléments dans un espace adapté ; la saisie reste accessible ; seule la liste défile.
- L'accueil de la brique 001 reste un **prototype fonctionnel limité à capture + boîte** : ne pas développer prématurément le tableau de bord de la brique 006.
- Interface nette, sans grands compteurs décoratifs, entièrement utilisable au clavier.

## 3. Parcours de saisie

1. L'utilisateur écrit un texte libre ; il choisit ou non une destination.
2. `Entrée` envoie immédiatement ; `Maj+Entrée` insère un retour à la ligne. Le bouton « Envoyer » fait exactement la même chose.
3. Texte vide ou composé uniquement d'espaces : rien n'est créé, aucune animation de succès.
4. L'interface appelle une commande Rust de persistance ; le texte est traité exclusivement comme **une donnée**.
5. **Après confirmation de la sauvegarde**, l'élément apparaît **tout en bas de la liste visible**, juste au-dessus du champ : la boîte est en ordre chronologique croissant (les plus anciennes en haut). La liste défile pour montrer la nouvelle capture. Cette disposition sert de référence à l'animation ascendante de 001-C. *(Décision du 2026-10-10 : remplace « en tête, récent d'abord » de la spécification V2.)*
   - Au-delà de la limite d'affichage (20), ce sont les **plus récentes** qui sont montrées, en ordre chronologique ; les plus anciennes restent en base (accès complet en 001-B).
6. Le champ se vide seulement après succès et récupère le focus ; la destination revient à « Aucune ».
7. En cas d'échec : **conserver le texte et la destination**, afficher une erreur non bloquante, permettre de réessayer, aucun faux succès.
8. Pendant un envoi, empêcher les doublons (clics ou `Entrée` répétés). Préserver ce qui est saisi pendant l'envoi : le champ n'est vidé que s'il contient encore exactement le texte envoyé.

## 4. Animations (001-C)

- Trajectoire courte, principalement **verticale ascendante**, de la capture vers la boîte au-dessus : comme une carte/lettre envoyée.
- Utile et discrète ; durée initiale de l'ordre de 300–450 ms, à ajuster ; **sans bloquer la nouvelle saisie**.
- Insertion lisible, sans déplacements brusques.
- Respecter `prefers-reduced-motion` : apparition immédiate à la place du mouvement.
- L'animation **n'est pas** une condition de la sauvegarde ; la base de données reste la source de vérité.

## 5. Gestion des éléments reçus (001-B)

- Affichage du texte et de la destination facultative ; la fiche peut être consultée.
- **Modifier et supprimer** une capture, y compris après envoi. La suppression doit être récupérable (corbeille logique via `deleted_at`) ou offrir une annulation claire ; jamais de destruction irréversible involontaire.
- Boîte utilisable avec de nombreuses entrées : nombre limité de cartes à l'accueil, « Voir tout » ; ne pas charger des centaines de lignes.
- Pas de transformation en tâche ou facture tant que les modules correspondants n'existent pas (brique 002 et suivantes).

## 6. Destination temporairement conservée (001-D, amélioration différée)

- **Défaut :** remise à « Aucune » après chaque sauvegarde réussie.
- **Idée retenue, non requise pour 001-A :** après deux captures consécutives vers la même destination, une suggestion discrète et **non bloquante** peut proposer de la conserver pendant une courte session (~10 minutes), refusable et annulable à tout moment.
- Pas de rappel répétitif, de popup intrusif ni d'IA : une simple règle locale.
- À développer **seulement si le bénéfice est confirmé à l'usage**.

## 7. Persistance, identité, sécurité

- SQLite locale **uniquement via Rust** et des commandes Tauri déclarées dans `build.rs` et autorisées une à une dans la capacité ; React ne manipule jamais la base.
- Base Dev dans le dossier Dev existant ; aucune lecture ni écriture dans le dossier Stable ; tests automatisés sur bases **temporaires**.
- Migrations versionnées dès la création du schéma, transactions, identifiants stables (UUID v7), dates de création et de modification, destination facultative.
- **Nom du fichier : `morganiser.db`** (décision du 2026-10-10 ; « Pantao » n'était pas une décision).
- Structure minimale : `InboxItem(id, content, destination_id nullable, created_at, updated_at, deleted_at nullable)` et référentiel `destinations` ; ne pas créer les autres modules.
- Pas de `innerHTML` (ni équivalent) pour le contenu utilisateur ; requêtes SQL paramétrées ; tailles et erreurs d'entrée traitées proprement.
- Aucun fichier de base, journal personnel, sauvegarde, secret ou donnée réelle dans GitHub.
- **Mode WAL et fichiers auxiliaires :** la base se compose de **trois fichiers indissociables** : `morganiser.db`, `morganiser.db-wal` et `morganiser.db-shm`. Toute écriture validée est d'abord placée dans `-wal`, puis reportée plus tard dans `.db` (« checkpoint »). Les captures récentes peuvent donc se trouver **uniquement** dans `-wal`, y compris application fermée (arrêt brutal, extinction de Windows, checkpoint bloqué). Conséquences :
  - **ne jamais copier `morganiser.db` seul**, que l'application soit ouverte ou fermée : la copie peut être incomplète, voire vide ;
  - **ne jamais supprimer** `-wal` ou `-shm` à la main : `-wal` peut contenir les dernières captures ; `-shm` est reconstruit automatiquement ;
  - en attendant le vrai système de sauvegarde, une copie manuelle ne se fait **qu'application fermée, en copiant les trois fichiers ensemble** ;
  - la future sauvegarde utilisera un mécanisme cohérent de SQLite (API de sauvegarde ou `VACUUM INTO`), qui fonctionne même base ouverte, puis sera **testée en restauration** ;
  - les trois fichiers sont exclus de Git (`*.db`, `*.db-*`).
- **Avant les données personnelles réelles ou une version Stable** : confirmer la stratégie de sauvegarde/restauration et de migration ; en Dev, données fictives uniquement.

## 8. Déroulement par sous-étapes

Chaque sous-étape est validée par le propriétaire avant la suivante ; ce ne sont pas des ordres de tout implémenter d'un coup.

- **001-A — Capture et persistance :** « À organiser » au-dessus du champ, saisie immédiate, destination facultative simple, filtre par destination, SQLite locale, relecture après relance, gestion d'erreurs, tests. Pas d'animation complexe.
- **001-B — Gestion :** ouvrir une capture, modifier texte et destination, suppression récupérable ou annulation claire, « Voir tout », tests.
- **001-C — Mouvement :** animation ascendante et insertion visuelle, `reduced-motion`, robustesse au redimensionnement.
- **001-D — Confort facultatif :** suggestion de conserver temporairement la destination, après validation du besoin.
- **001-E — Initiation minimale :** spotlight sur le **vrai champ**, validation d'une capture, focus sur **la vraie boîte** ; « Passer » toujours disponible. Exemple proposé « Vendre ma PlayStation 5 » ou texte libre ; ne pas polluer des données réelles par une démo sans consentement. Le moteur complet d'initiation reste pour une brique ultérieure.

**Répartition avec la brique 002 (décision du 2026-10-10) :** l'édition et la suppression élémentaires d'une capture relèvent de 001-B ; la **conversion** en tâche/note/projet et le cycle de vie avancé restent en brique 002.

## 9. Critères d'acceptation et tests

- L'application Dev démarre sans toucher à Stable ; `pnpm test` conserve tous les tests existants.
- Le champ de capture est immédiatement visible à l'ouverture.
- « Vendre ma PlayStation 5 » sans destination → une carte dans la boîte ; **une seule** occurrence même avec validation répétée rapidement.
- Le même exemple avec une destination → l'élément reste dans la boîte, affiche sa destination, n'est pas dupliqué, et apparaît avec le filtre de cette destination.
- Plusieurs captures successives → chacune apparaît en bas de la liste, dans l'ordre d'envoi ; au-delà de 20, les 20 plus récentes restent affichées (jamais les 20 plus anciennes).
- Relancer l'application → texte et destination inchangés.
- Capture vide ou espaces seuls → aucun élément créé.
- Accents, apostrophes, longues lignes, `'; DROP TABLE inbox_items; --` → texte affiché tel quel, sans exécution.
- Échec simulé de SQLite → texte et destination préservés, aucun succès affiché.
- Les éléments non traités restent visibles même avec une destination.
- Après création : destination réinitialisée (sa conservation temporaire est hors 001-A).
- 001-B : modifier, relancer → nouvelle valeur persistée ; supprimer puis restaurer/annuler.
- 001-C : animation réellement **du bas vers le haut** ; mouvement réduit respecté ; clavier conservé.
- Aucune commande Rust non déclarée/autorisée, aucune fuite vers Stable, aucune donnée réelle dans le dépôt.

## 10. Livrables de chaque sous-étape

- Démonstration Windows réelle, tests automatisés adaptés, commandes reproductibles, fichiers modifiés.
- Explication accessible : chemin d'une capture (UI → commande Rust → SQLite → UI), emplacement des données Dev, protections ajoutées, tests réellement passés ou échoués, limites restantes.
- Mise à jour de cette fiche, des documents transversaux et de `07_JOURNAL_DEVELOPPEMENT.md`.
- Travail sur une branche issue de `main` propre ; **aucun push ni fusion sans autorisation**. Claude seul modifie le code ; Codex audite en lecture seule.

## 11. Réalisation 001-A (implémentée le 2026-10-10, en attente de validation)

### Chemin d'une capture

1. **Interface** (`CaptureForm`) : `Entrée` → le texte et la destination sont envoyés ; un verrou empêche un second envoi tant que le premier n'est pas terminé.
2. **Pont Tauri** (`src/features/inbox/api.ts`) : appel de la commande `create_inbox_item`, autorisée nommément.
3. **Rust** (`commands.rs` → `inbox.rs`) : nettoyage et validation du texte, vérification de la destination, création d'un identifiant UUID v7, insertion par requête paramétrée.
4. **SQLite** (`morganiser.db`) : l'enregistrement est écrit ; s'il échoue, une erreur `{code, message}` revient à l'interface.
5. **Interface** : seulement après succès, le champ est vidé, la destination remise à « Aucune », puis la liste est **relue depuis la base** (source de vérité). En cas d'échec, rien ne change dans le champ.

### Fichiers

| Fichier | Rôle |
|---|---|
| `src-tauri/migrations/0001_initial.sql` | Schéma v1 : `destinations` (5 destinations initiales, nature `responsibility`/`section`) et `inbox_items`. |
| `src-tauri/src/storage/mod.rs` | Ouverture de la base, réglages (`foreign_keys`, WAL, `synchronous = FULL`, attente 5 s si verrouillée), migrations versionnées, checkpoint à la fermeture (résultat complet/incomplet). |
| `src-tauri/src/inbox.rs` | Logique « À organiser » : validation, création, lecture filtrée, erreurs avec code stable. |
| `src-tauri/src/commands.rs` | Commandes `list_destinations`, `list_inbox_items`, `create_inbox_item` (asynchrones, accès un par un à la base). |
| `src-tauri/src/lib.rs` | Ouverture de la base dans le dossier validé par `prepare()` ; checkpoint WAL à la fermeture. |
| `src-tauri/build.rs`, `capabilities/default.json` | Déclaration et autorisation nominative des 4 commandes. |
| `src/features/inbox/api.ts` | Seul point d'appel vers Rust pour cette fonctionnalité ; types alignés sur Rust. |
| `src/features/inbox/InboxPanel.tsx` | Boîte « À organiser » : les 20 plus récentes en ordre chronologique (nouvelle en bas, défilement automatique), destination ou « À classer », filtre, mention du total. |
| `src/features/inbox/CaptureForm.tsx` | Capture : Entrée / Maj+Entrée, destination facultative, anti-doublon, erreur sans perte. |
| `src/features/inbox/InboxHome.tsx` | Assemble boîte (au-dessus) et capture (en dessous) ; relit la base après chaque capture. |
| `src/features/inbox/DestinationOptions.tsx` | Options groupées « Espaces » / « Rubriques ». |

### Choix techniques

- **Base :** `%LOCALAPPDATA%\com.morganiser.desktop.dev\data\morganiser.db` en Dev ; numéro de schéma dans `PRAGMA user_version` ; base d'une version plus récente refusée **sans être modifiée** : la version est lue en premier, avant tout réglage (le passage en WAL est écrit dans le fichier et ne doit jamais toucher une base future). Ordre à l'ouverture : contrôle de version → réglages (WAL, FULL, clés étrangères) → migrations.
- **Durabilité :** `synchronous = FULL` : chaque capture confirmée est forcée sur le disque avant que l'interface n'affiche le succès ; elle survit à un arrêt brutal, y compris une coupure de courant.
- **Checkpoint à la fermeture normale :** `PRAGMA wal_checkpoint(TRUNCATE)` reporte le contenu de `-wal` dans `.db`. C'est une **commodité, pas une garantie ni une sauvegarde** :
  - l'opération est sûre par construction : SQLite ne vide `-wal` qu'après avoir écrit et synchronisé `.db` ; interrompue, elle laisse `-wal` intact ;
  - si elle est bloquée (« incomplète ») ou échoue, rien n'est perdu : les données restent dans `-wal` et un message est écrit dans le terminal ;
  - elle n'a pas lieu en cas d'arrêt brutal : les données validées restent dans `-wal` et SQLite les relit automatiquement à l'ouverture suivante (vérifié par test et en réel).
- **Règle de copie :** voir section 7 : jamais `morganiser.db` seul ; les trois fichiers ensemble, application fermée, en attendant le système de sauvegarde.
- **Validation côté Rust :** texte sans espaces de début/fin, non vide, 10 000 caractères maximum (aussi imposé par le schéma) ; destination existante et non archivée.
- **Messages d'erreur :** jamais le texte saisi ; la cause technique SQLite est écrite dans le terminal de développement seulement.
- **Ouverte dans un navigateur** (`pnpm dev`), l'interface n'appelle pas Rust et indique que la capture n'est disponible que dans l'application.

### Tests

- Rust (47 au total, plus 1 test « sous-processus » ignoré en exécution normale) : 9 sur le stockage (migrations, base plus récente refusée, base future en journal classique laissée **octet pour octet** intacte, réglages dont `synchronous = FULL`, checkpoint complet, **arrêt brutal réel d'un sous-processus après validation sans perte**, copie de `.db` seul insuffisante contre `.db` + `-wal` complète, checkpoint bloqué signalé sans perte), 17 sur la logique « À organiser », 9 de configuration (dont cohérence `build.rs` ↔ capacité ↔ `lib.rs`), et ceux de la brique 000.
- Interface (25) : 11 sur la capture, 10 sur l'accueil (avec une fausse base en mémoire, dont l'ordre chronologique et la sélection des plus récentes au-delà de 20), 4 sur l'application.
- Essais réels dans la fenêtre Dev (pilotée par le protocole DevTools local) : voir le journal du 2026-10-10.

### Limites connues

- L'accueil affiche les 20 plus récentes (Rust les sélectionne, l'interface les remet en ordre chronologique) ; « Voir tout » et l'historique complet viendront en 001-B.
- **Fermeture :** fermer la fenêtre par `×` (fermeture normale, code de sortie 0, report WAL effectué). Arrêter `pnpm app:dev` par `Ctrl+C` ou en fermant le terminal interrompt le programme (`STATUS_CONTROL_C_EXIT`, `0xc000013a`) et peut faire afficher par WebView2 `Failed to unregister class Chrome_WidgetWin_0` : messages sans conséquence pour les données (le report WAL n'a simplement pas lieu ; les captures validées sont relues depuis `-wal` au lancement suivant). La ligne `ELIFECYCLE … exit code 4294967295` affichée à la fermeture est émise par le `pnpm dev` (serveur Vite) lancé par Tauri et arrêté de force à la sortie de l'application ; l'application et le CLI Tauri renvoient bien 0 (mesuré, voir le journal du 2026-10-10).
- Si la base est verrouillée, l'envoi attend jusqu'à 5 secondes avant d'afficher l'erreur.
- En fenêtre très étroite, la boîte devient petite (à revoir avec le mode compact de la brique 006).
- Si la base ne peut pas être ouverte au démarrage, l'erreur n'apparaît que dans le terminal (limite héritée de la brique 000).

## 12. Réalisation 001-B (implémentée le 2026-10-10, en attente de validation)

### Parcours

- **Consulter :** un clic ou `Entrée` sur une carte ouvre la **fiche**. Au-dessus de 900 px de largeur, elle s'affiche dans un panneau à droite ; en dessous, elle remplace temporairement la vue principale (bouton « ← Retour »). La carte ouverte est repérée (`aria-current`). `Échap` ferme la fiche et rend le focus à la carte d'origine.
- **Modifier :** texte et destination (dont « Aucune ») sont directement éditables ; « Enregistrer » (ou `Ctrl+Entrée`) n'est actif que s'il y a une modification valide ; « Annuler les modifications » rétablit l'original. Dates de création et de dernière modification affichées (« Jamais modifiée » si le texte n'a jamais changé).
- **Brouillon protégé :** fermer la fiche, ouvrir une autre capture ou mettre à la corbeille avec des modifications non enregistrées affiche un avertissement : *Enregistrer* / *Abandonner les modifications* / *Continuer à modifier*. Un échec d'enregistrement (base verrouillée, etc.) conserve texte et destination et permet de réessayer.
- **Conflit de version :** si la capture a changé depuis son ouverture, rien n'est écrasé ; le brouillon est conservé, la version actuelle devient la référence, et un nouvel « Enregistrer » est un choix explicite de la remplacer.
- **Corbeille :** « Mettre à la corbeille » (sans confirmation lourde) ferme la fiche et affiche une notification **« Annuler »** pendant 8 secondes. La vue **Corbeille** (lien dans la boîte) liste les captures supprimées, de la plus ancienne à la plus récente suppression, avec « Restaurer » ; une capture supprimée s'ouvre en lecture seule. Aucune suppression définitive ni vidage automatique en 001-B.
- **Voir tout :** lien affiché lorsque la boîte contient plus de captures que l'accueil n'en montre. Historique complet par lots de 50, en ordre chronologique ; « Charger les N plus anciennes » ajoute les éléments **au-dessus** sans déplacer la lecture. Le filtre de destination est conservé ; en changer repart de zéro.

### Commandes et données

| Commande | Rôle |
|---|---|
| `get_inbox_item(id)` | Lecture d'une capture (corbeille comprise) |
| `update_inbox_item(id, content, destinationId, expectedUpdatedAt)` | Modification avec verrou optimiste |
| `trash_inbox_item(id)` / `restore_inbox_item(id)` | Corbeille logique (`deleted_at`) et restauration |
| `list_inbox_items(filter, limit, before)` | Étendue : pagination par curseur |
| `list_trashed_items(limit, before)` | Corbeille |

- **Aucune migration** : `deleted_at` et `updated_at` existaient déjà (schéma v1).
- **Verrouillage optimiste, atomique :** `UPDATE … WHERE id = ? AND updated_at = ? AND deleted_at IS NULL`. Chaque modification effective fait croître **strictement** `updated_at` (`max(maintenant, attendu + 1)`), même si l'horloge n'avance pas ; sans changement réel, rien n'est écrit. Erreurs distinguées : `not_found`, `trashed`, `version_conflict`, `not_trashed`.
- **`updated_at` ne bouge ni à la mise à la corbeille ni à la restauration** : il désigne la dernière modification du contenu. La restauration remet donc exactement la capture d'origine (identifiant, texte, destination, dates). *Conséquence à garder en tête pour la synchronisation mobile : un changement d'état (corbeille/restauration) ne se détecte que par `deleted_at` ; une future synchronisation devra prévoir un marqueur de version d'état.*
- **Pagination par curseur `(date, id)`** (`created_at` pour la boîte, `deleted_at` pour la corbeille) : insensible aux captures ajoutées pendant le parcours. L'accueil conserve « les 20 plus récentes en ordre chronologique ».

### Fichiers

| Fichier | Rôle |
|---|---|
| `src-tauri/src/inbox.rs` | `get_item`, `update_item`, `trash_item`, `restore_item`, `list_items` (curseur, portée boîte/corbeille), nouvelles erreurs |
| `src-tauri/src/commands.rs`, `lib.rs`, `build.rs`, `capabilities/default.json`, `config_tests.rs` | 5 nouvelles commandes déclarées, enregistrées et autorisées nommément |
| `src/features/inbox/CaptureDetail.tsx` | Fiche : édition, brouillon, conflit, corbeille, restauration |
| `src/features/inbox/CaptureCard.tsx` | Carte cliquable partagée (boîte, Voir tout, Corbeille) |
| `src/features/inbox/PagedCaptureView.tsx` | Vue « Voir tout » et Corbeille : lots, défilement conservé |
| `src/features/inbox/UndoToast.tsx` | Notification d'annulation (8 s) |
| `src/features/inbox/InboxHome.tsx` | Orchestration : vues, fiche, garde de brouillon |
| `src/features/inbox/InboxPanel.tsx`, `api.ts`, `format.ts`, styles | Adaptations |
| `src/test/fakeBackend.ts` | Faux « Rust + SQLite » pour les tests de l'interface |

### Tests

- Rust : 69 (+1 sous-processus ignoré) ; **22 nouveaux** : modification et dates, horloge figée, deux modifications à la même milliseconde, garde atomique du `UPDATE`, introuvable/corbeille/conflit, texte vide ou trop long, destination archivée inchangée, corbeille et restauration à l'identique, tri de la corbeille, pagination sans doublon à dates égales, ajouts pendant la pagination, filtres, persistance après réouverture, échecs d'écriture, format JSON.
- Interface : 71 (dont 46 nouveaux : fiche, édition, annulation, avertissements, échec, conflit, corbeille, restauration, notification, lots de 50, défilement, filtres, ajouts pendant la consultation).

### Limites connues

- Après une modification, « Voir tout » repart de la page la plus récente (les lots plus anciens déjà chargés sont rechargés à la demande).
- Aucune suppression définitive ; la corbeille grossit tant qu'elle n'est pas gérée (à décider plus tard).
- Seuil du panneau latéral : 900 px (à ajuster après essais d'usage).

### Fermeture de la fenêtre protégée (ajout du 2026-10-10)

La fermeture **normale** (bouton `×`, `Alt+F4`, fermeture depuis la barre des tâches) est interceptée par l'événement officiel Tauri 2 `onCloseRequested` (`src/lib/closeGuard.ts`) :

| Situation | Comportement |
|---|---|
| Rien à perdre (y compris une fiche ouverte mais non modifiée) | Fermeture normale, sans confirmation |
| Fiche modifiée | Avertissement dans la fiche : *Enregistrer* / *Abandonner les modifications* / *Continuer à modifier* (même composant que pour les autres départs de fiche) |
| Texte non envoyé dans la capture rapide | Avertissement au-dessus des colonnes : *Envoyer* / *Abandonner ce texte* / *Continuer à écrire* |
| Les deux | La fiche d'abord, puis la capture rapide ; « Continuer » à n'importe quelle étape annule la fermeture |
| Échec d'enregistrement ou d'envoi | Brouillon et fenêtre conservés, message d'erreur, nouvel essai possible |
| « Abandonner » / enregistrement réussi | Fermeture effective par `destroy()` : aucune nouvelle demande de fermeture, donc aucune boucle de confirmation |

- Le focus est donné au choix le plus sûr (« Continuer ») et revient au texte quand il est choisi ; tout est utilisable au clavier.
- **Permissions ajoutées (minimales) :** `core:event:allow-listen`, `core:event:allow-unlisten`, `core:window:allow-destroy`. Rien d'autre (pas de `core:default`, pas de `window:allow-close`).
- **Fermeture pendant un enregistrement en cours :** la fenêtre n'est jamais détruite avant la décision. Une fiche en cours d'enregistrement est encore « non enregistrée » : l'avertissement s'affiche, puis la fermeture a lieu automatiquement, une seule fois, quand l'enregistrement réussit ; s'il échoue, fenêtre et brouillon sont conservés. Dans la capture rapide, « Envoyer » attend l'envoi en cours (pas de doublon, pas de faux échec). SQLite étant transactionnel, une écriture est soit complète, soit absente.
- **Limite :** la protection ne concerne que la fermeture normale. Un arrêt forcé du processus (Gestionnaire des tâches, `Ctrl+C` dans le terminal de développement), l'extinction de Windows ou une coupure de courant ne sont pas interceptables ; les captures **déjà enregistrées** restent protégées par SQLite (WAL, `synchronous = FULL`), seul un brouillon non enregistré peut être perdu.
- **Notification d'annulation :** elle recouvre désormais la ligne d'état, dans un emplacement réservé : le champ de capture ne bouge plus (mesuré : 0 px de décalage).
