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

## 6. Destination conservée à la demande (001-D, option manuelle)

**Décision du 2026-10-10 :** la suggestion automatique après deux captures identiques (minuterie de ~10 minutes) est **abandonnée**. Elle est remplacée par une option manuelle, « Conserver ce choix », près du sélecteur de destination.

- **Défaut :** option décochée ; la destination revient à « Aucune » après chaque sauvegarde réussie.
- **Cochée avec une destination :** cette destination reste sélectionnée après les envois réussis suivants.
- L'utilisateur peut changer de destination à tout moment ; si l'option reste cochée, le nouveau choix est conservé.
- Sélectionner « Aucune » désactive la conservation (l'option est alors grisée et décochée).
- Décocher l'option ne supprime pas la destination de la capture en cours : elle est réinitialisée après le prochain envoi réussi.
- **Échec de sauvegarde :** texte, destination et état de l'option sont conservés.
- **Relance de l'application :** conservation désactivée, destination « Aucune » (rien n'est enregistré).
- Aucune détection de captures identiques, aucune suggestion, aucune minuterie, aucune IA, aucun stockage, aucune modification Rust ni SQLite.

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
- **001-C — Mouvement (implémentée, voir section 13) :** animation ascendante et insertion visuelle, `reduced-motion`, robustesse au redimensionnement.
- **001-D — Confort facultatif (option manuelle, voir section 6) :** case « Conserver ce choix » près de la destination ; la suggestion automatique est abandonnée.
- **001-E — Initiation minimale (implémentée, voir section 14) :** mise en évidence du **vrai champ**, progression sur un enregistrement réussi, mise en évidence de **la vraie boîte** et de la capture créée ; « Passer » toujours disponible ; aucun texte ni donnée imposés. Le moteur complet d'initiation (reprise, parcours thématiques) reste pour la brique 007.

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
- Après création : destination réinitialisée, sauf si « Conserver ce choix » est coché (001-D) ; la case est décochée à chaque lancement.
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
- Interface : 89 (dont 64 nouveaux : fiche, édition, annulation, avertissements, échec, conflit, corbeille, restauration, notification, lots de 50, défilement, filtres, ajouts pendant la consultation).

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

### Coordination des opérations asynchrones (correction après audit Codex, 2026-10-10)

Principe : trois notions distinctes dans la fiche : la **référence enregistrée** (`baseline`), le **brouillon** (texte et destination) et les **opérations en cours**. Un modèle « immédiat » (références mises à jour au même instant que l'état React) permet à une réponse tardive ou à une demande de fermeture de lire l'état réel, jamais celui d'un rendu périmé.

| # | Défaut | Règle appliquée |
|---|---|---|
| A | La réponse tardive d'un enregistrement remplaçait la saisie suivante | La référence passe à la version enregistrée ; le brouillon n'est remplacé que s'il n'a pas changé depuis l'envoi. Le départ en attente n'a lieu que si plus rien n'est non enregistré. |
| B | Une fermeture différée aboutissait malgré une nouvelle saisie ou « Continuer à écrire » | Chaque demande de fermeture est un objet annulable ; chaque décision relance l'examen de l'état ACTUEL (fiche, capture rapide, écritures en cours) avant toute destruction ; une nouvelle demande annule la précédente ; un abandon est mémorisé **par contenu** (empreinte du brouillon) : une saisie différente faite ensuite, même pendant l'attente d'une écriture, est proposée à son tour, alors qu'un brouillon inchangé et déjà abandonné n'est pas redemandé ; l'avertissement périmé reprend la fermeture dès que l'envoi a abouti. |
| C | La corbeille de A fermait la fiche B | Le résultat porte l'identifiant de A : il ne ferme que la fiche de A, si elle est encore ouverte et sans saisie récente ; sinon notification seule. Une saisie faite pendant l'attente est conservée. |
| D | Un brouillon n'était plus protégé quand la capture était supprimée ailleurs | `hasUnsaved()` = divergence du brouillon, indépendamment du droit d'enregistrer. « Enregistrer » explique qu'il faut d'abord restaurer ; « Annuler les modifications » est proposé aussi dans la fiche d'une capture supprimée. |
| E | « Enregistrer puis corbeille » ne mettait pas à la corbeille | L'action enchaînée est lancée après la libération effective du verrou, et seulement si l'enregistrement a réussi. |
| F | La fiche restait « supprimée » après une restauration depuis la liste | La fiche ouverte de la capture est relue (`refresh()`) ; son brouillon n'est pas touché. |

**Fermeture pendant une corbeille, une restauration ou un envoi :** toutes les écritures sont suivies (`track`). La fermeture normale les attend (aucune demande en transit n'est interrompue), puis réexamine l'état avant de détruire la fenêtre. Pas de gestionnaire global : un simple ensemble de promesses.

**Limites :** un arrêt forcé du processus, l'extinction de Windows ou une coupure de courant restent hors de portée (voir ci-dessus). Quand une fermeture normale attend une écriture bloquée, la fenêtre reste ouverte au plus le délai d'attente SQLite (5 s) puis le résultat est traité comme n'importe quelle réponse.

## 13. Réalisation 001-C : animation d'arrivée (implémentée le 2026-10-10, en attente de validation)

### Comportement

Après l'enregistrement confirmé, une **carte fantôme** monte de la zone de saisie (bord supérieur du champ) jusqu'à la place de la vraie carte, en bas de la boîte. Durée **350 ms**, trajet vertical, opacité de 0,55 à 1 : aucun halo ni effet décoratif.

### Principe (validé par le propriétaire)

- **L'animation ne conditionne rien** : la capture est enregistrée, intégrée à la liste et utilisable normalement ; le mouvement est purement visuel et arrive après.
- La vraie carte n'est que **masquée visuellement** (`visibility`) pendant le trajet et **toujours rétablie** (fin normale, annulation, erreur d'animation, délai de sécurité de 350 + 300 ms).
- La carte fantôme est un **clone** de la vraie carte : mêmes dimensions et apparence, `position: fixed`, `aria-hidden`, `inert`, sans événements de pointeur, retirée à la fin.
- Les cartes déjà présentes **glissent** de leur ancienne place vers la nouvelle (technique FLIP) au lieu de sauter quand la liste défile jusqu'en bas.
- Technique : **Web Animations API**, sans dépendance. Code : `src/features/inbox/arrivalAnimation.ts`, branché dans `InboxPanel` (déclencheur `Arrival`) et `InboxHome` (mesure du point de départ après l'enregistrement). Aucune modification Rust, SQLite ou commande.

### Cas particuliers

| Situation | Comportement |
|---|---|
| Échec d'enregistrement | Aucune animation, texte et destination conservés |
| Filtre excluant la capture | Aucune animation ; message « masquée par le filtre » conservé |
| `prefers-reduced-motion: reduce` | Aucun trajet : insertion immédiate |
| Liste ou zone de saisie non visible (fenêtre étroite avec fiche ouverte) | Insertion directe sans mouvement |
| Deux envois rapprochés | La première arrivée est terminée net (sa carte est visible), la seconde s'anime ; aucune perte ni doublon |
| Nouvelle saisie, clic dans la liste, ouverture d'une fiche pendant le trajet | Jamais bloqués |
| Redimensionnement, changement de filtre, de vue ou démontage | Animation annulée, vraie carte visible |
| API d'animation indisponible ou en erreur | Insertion directe, carte visible |
| Vues « Voir tout » et Corbeille | Pas d'animation (insertion directe) |

### Tests

- Interface : 104 (dont 15 nouveaux dans `InboxHome.animation.test.tsx`) ; jsdom n'ayant ni API d'animation ni mise en page, des simulations en tiennent lieu (trajet, durée, fantôme `aria-hidden`/`inert`, vraie carte masquée puis visible, FLIP, filtre, mouvement réduit, fenêtre étroite, vue, redimensionnement, filtre, démontage, échec et délai de sécurité).
- Fenêtre Windows Dev réelle (échantillonnage image par image, protocole DevTools local) : voir le journal.

### Limites

- Le mouvement n'a pas pu être capturé en une image fixe (latence de capture supérieure à 350 ms) ; sa validation repose sur l'échantillonnage des positions. À juger à l'œil lors des essais manuels.
- Pas d'animation de retrait (corbeille) ni d'arrivée dans « Voir tout » : hors périmètre 001-C.

### Mise à la corbeille : disparition et notification (ajout du 2026-10-10)

L'arrivée animée est validée par le propriétaire et reste inchangée. Constat : après « Mettre à la corbeille », la fiche se refermait sur la liste sans retour assez perceptible.

- **Transition de sortie :** après confirmation de la base (jamais avant), la carte concernée disparaît en fondu avec un léger rétrécissement, **220 ms**, ease-in. Carte fantôme = clone non interactif (`aria-hidden`, `inert`) ; la vraie carte n'est que masquée ; les cartes restantes glissent vers leur place (FLIP déclenché quand la liste a retiré la carte, via `MutationObserver`). Code : `playDeparture` dans `arrivalAnimation.ts`, déclenchée par `InboxHome` juste après le rendu où la fiche s'est refermée (la liste est alors visible, même en fenêtre étroite).
- **Jamais de réactivation :** la transition ne peut pas rendre la capture de nouveau active ; la suppression logique n'est pas retardée.
- **Notification :** « Capture déplacée dans la corbeille. » + « Annuler » (≈ 8 s). Elle est désormais **flottante** (`position: fixed`, en bas, centrée, hors du flux : aucun déplacement du champ de capture), contrastée (bordure d'accent, ombre) et accueillie par une **région vivante persistante** (`aria-live="polite"`) pour que l'ajout du message soit annoncé. La minuterie ne repart plus à chaque rendu du parent.
- **Focus :** il passe à la carte voisine (suivante, sinon précédente), ou au champ de capture s'il n'y en a plus ; il n'est plus perdu avec la carte supprimée.
- **Cas particuliers :**

| Situation | Comportement |
|---|---|
| Échec de la suppression | Aucune animation ni notification, message d'erreur dans la fiche |
| Capture absente de la liste actuelle (filtre) | Confirmation seule, aucun trajet |
| Depuis « Voir tout » | Disparition en fondu aussi |
| Deux suppressions rapprochées | Une seule notification, qui vise la dernière ; la première reste restaurable depuis la Corbeille |
| « Annuler » pendant la transition | Fantôme retiré tout de suite, carte restaurée visible |
| Mouvement réduit | Disparition immédiate, notification affichée |
| Redimensionnement, changement de vue ou de filtre | Transition annulée proprement |

- **Tests :** `InboxHome.departure.test.tsx` (13 tests) ; interface : 117 au total.

### Règles UX : géométrie stable et défilement conservé (ajout du 2026-10-10)

**Règle : un panneau secondaire ne déplace jamais le contenu principal.**

- La colonne principale (boîte « À organiser » + capture rapide) a une position et une taille **indépendantes** de l'ouverture d'une fiche. Elle reste centrée (largeur maximale 760 px).
- La fiche est un **panneau flottant** placé dans la zone de la boîte (jamais sur la capture rapide) :

| Largeur de fenêtre | Disposition de la fiche |
|---|---|
| ≥ 1580 px | À droite de la colonne, sans recouvrement (380 px) |
| 900 à 1579 px | Flottante sur la droite de la boîte (360 px), la capture rapide reste libre |
| < 900 px | Elle remplace temporairement la vue principale (« ← Retour ») |

- Technique : la fiche est un enfant en `position: absolute` d'une **scène** (`.inbox-home__stage`) qui ne contient que la vue (boîte) ; la capture rapide est hors de la scène, donc le panneau ne peut ni la recouvrir ni rien pousser. Le seuil de 900 px est conservé. *(Un premier essai d'ancrage sur la « zone de grille » n'était pas respecté par le moteur de rendu : la fiche descendait sur la capture rapide ; la scène explicite l'a corrigé.)*

**Défilement : « en bas » seulement quand c'est voulu.**

| Événement | Défilement |
|---|---|
| Premier affichage, changement de filtre | En bas |
| Nouvelle capture | En bas (la carte arrive et s'anime) |
| Ouverture ou fermeture d'une fiche, édition | Aucun |
| Mise à la corbeille, restauration, relecture | Zone de lecture conservée |
| Chargement d'anciennes captures (« Voir tout ») | Contexte conservé (déjà en place) |

- Mécanisme : **ancrage par identifiant** (`scrollAnchor.ts`). Quelques cartes visibles sont mémorisées avec leur position ; après un changement, la première encore présente est remise à la même position visuelle. Si la carte supprimée était l'ancre, la suivante la remplace. L'entrée d'une capture plus ancienne dans l'ensemble des 20 affichées est compensée exactement (pas de saut).
- « Voir tout » et la Corbeille rechargent désormais autant de captures qu'il y en avait d'affichées (au lieu de repartir de la page la plus récente).
- Les appels à `focus()` (carte voisine, fiche, champ de capture) utilisent `preventScroll`.

**Tests :** `InboxHome.scroll.test.tsx` (8) ; interface : 125 au total.

**Mesures dans la fenêtre Windows Dev réelle** (rectangles `getBoundingClientRect` de la boîte, de la capture rapide et de la colonne ; avant, pendant et après ; 1700, 1200 et 960 px) :

| Mesure | Résultat |
|---|---|
| Écart de géométrie à l'ouverture, pendant la disparition, après fermeture | **0 px** aux trois largeurs |
| Défilement à l'ouverture de la fiche | Inchangé (601 → 601) |
| Après suppression au milieu de la liste | Carte voisine à 238 → 237 px (compensation de l'entrée d'une capture plus ancienne) ; liste non ramenée en bas |
| « Annuler » | Capture restaurée, liste non ramenée en bas, géométrie identique |
| Fiche (hauteur) | 582 px = hauteur de la boîte, sans recouvrir la capture rapide ; à droite de la colonne à 1700 px |
| Mouvement réduit | Géométrie stable, aucun fantôme, lecture conservée |
| « Voir tout » | Géométrie stable ; voisine 142 → 142 px ; lecture conservée |
| Changement de filtre, fiche ouverte | Boîte et capture immobiles |
| Fenêtre de 600 px | La fiche remplace la vue ; liste et capture de retour à la fermeture |

### Carte masquée par la transition de sortie (correctif du 2026-10-10)

La carte réelle n'est masquée que pendant le trajet et **n'est jamais laissée invisible** : quand le mouvement s'arrête (fin, annulation par redimensionnement, délai de sécurité) et que la liste ne l'a pas encore retirée (relecture tardive ou en échec), elle est rendue visible mais **marquée « sortie »** (`data-departed` : atténuée, `inert`). Une relecture réussie la retire ; « Annuler » lui rend son état normal et interactif, même si la transition était déjà terminée. Les durées et styles des animations ne changent pas. Tests : 4 de plus, interface : 129.

### Notification d'annulation compacte (ajustement du 2026-10-10)

- Texte « Déplacée dans la corbeille » + bouton « Annuler », en pilule compacte ; durée **4 s** (au lieu de 8).
- Position : **en bas à droite de la fenêtre, sur la ligne du pied de page**, hors flux : elle ne recouvre ni le champ de capture, ni « Envoyer », ni la boîte, et ne déplace rien ; même emplacement en fenêtre étroite.
- Le compte à rebours est **suspendu** au survol de la souris ou tant que le bouton a le focus, puis **reprend avec le temps restant** : le bouton ne disparaît pas sous le curseur.
- L'annonce par les lecteurs d'écran est inchangée (région vivante persistante). Après expiration, la capture reste récupérable depuis la vue Corbeille.
- Composant réutilisé : `UndoToast` (aucun second système). Tests : 7 sur la notification ; interface : 132.
- **Mesures dans la fenêtre Windows réelle (validées) :** position en bas à droite (`t` 764, `b` 796) à 1700, 960 et 600 px ; boîte, champ et « Envoyer » immobiles ; aucun recouvrement du champ, de « Envoyer » ni de la boîte ; disparition ≈ 4,4 s après l'action ; pause au survol (affichée 6 s pendant que la souris est dessus, disparition ≈ 3,8 s après la sortie du curseur, soit le temps restant) ; « Annuler » restaure la capture (20 → 20 cartes) et retire la notification.

## 14. Réalisation 001-E : initiation minimale (implémentée le 2026-10-10, en attente de validation)

### Décisions du propriétaire
- Proposition automatique **uniquement lors d'une véritable première utilisation**, présentée **une seule fois** : mémorisée dès son premier affichage, même si l'application est fermée sans réponse.
- Accès permanent « Découvrir » dans l'en-tête ; la visite est rejouable à volonté.
- Aucune donnée fictive enregistrée ; aucun voile bloquant ; « Passer » à toutes les étapes.
- Préférences **Dev et Stable séparées** ; installations existantes jamais interrompues.

### Parcours
1. **Proposition** (carte discrète) : « Découvrir » / « Plus tard ».
2. **La capture rapide :** le vrai champ est entouré ; le focus y est placé ; l'étape n'avance **que** sur un enregistrement confirmé (jamais sur un envoi vide ou échoué).
3. **La boîte « À organiser » (étape finale) :** « 2/2 · Votre capture est dans À organiser. » avec « Précédent » et « Terminer » ; la vraie boîte et **la capture réellement créée** (repérée par son identifiant) sont mises en évidence. Si un filtre ou une autre vue la masque, la carte l'explique et propose une action **explicite** (« Tout afficher », « Voir la boîte ») : rien n'est changé silencieusement, le brouillon du champ est conservé.

Pas d'écran de conclusion séparé : « Terminer » clôt la visite à l'étape 2/2 ; le bouton « Découvrir » de l'en-tête reste visible pour la relancer. « Précédent » revient à l'étape 1 ; Échap (focus dans la carte) arrête la visite.

### Mémorisation et détection
- `localStorage` de la WebView, clé `morganiser.initiation.v1`, valeur `{ "version": 1, "parcours": { "general": { "proposee": true } } }` (rangée par parcours pour la brique 007). Aucune donnée personnelle ; rien dans SQLite ni Rust.
- **Isolation Dev/Stable :** identifiants Tauri différents (`com.morganiser.desktop.dev` / `com.morganiser.desktop`), donc dossiers WebView2 distincts ; vérifié : seul le dossier Dev existe sur la machine de développement.
- **Première utilisation** = clé absente **et** aucune capture, ni active ni dans la corbeille (lecture par `list_inbox_items` et `list_trashed_items`, sans nouvelle commande).
- Clé absente mais captures présentes (installation Dev existante, cache WebView effacé) : réglage inscrit **en silence**, rien proposé.
- Erreur de lecture, réponse incomplète ou incohérente, stockage indisponible, contenu illisible, écriture non confirmée : **aucune proposition** ; jamais traité comme « base vide ».

### Limites connues
- Une base Dev vidée à la main ressemble à une première utilisation : la proposition apparaît une fois.
- Le `localStorage` n'est pas dans la base ni dans sa sauvegarde : un effacement du cache WebView2 avec une base vide reproposerait la visite une fois.
- Une erreur de lecture lors d'un tout premier lancement n'inscrit rien : la proposition peut venir au lancement suivant.
- Le bouton « Découvrir » n'existe pas dans l'aperçu navigateur (pas d'application Windows).
- Échap ne fonctionne que si le focus est dans la carte (pour ne pas entrer en conflit avec Échap de la fiche).

### Fichiers
`src/features/initiation/` : `initiationStore.ts` (mémoire locale), `firstUse.ts` (détection), `useInitiation.ts` (état de la visite), `InitiationGuide.tsx` + `.css` (carte et contours). Points de contact : `App.tsx` (bouton), `InboxHome.tsx` (repères, notification d'un enregistrement réussi), `InboxPanel.tsx` et `CaptureCard.tsx` (repère de la capture). Animations 001-C, Rust et SQLite inchangés.

### Règles UX
- **Une ligne de message par étape**, toujours visible (aucun texte réservé aux lecteurs d'écran), avec ses boutons sur la même ligne quand la largeur le permet (≈ 49 px de haut), sinon sur deux lignes (≈ 78 px).
- La carte flotte (hors flux, elle ne déplace rien) **sous l'en-tête de la vue** ; sa position est mesurée, donc le filtre, la corbeille et « Voir tout » ne sont pas recouverts. La liste réserve la même place (5 rem) aux étapes 1/2 et 2/2, dès l'ouverture de la carte et donc avant toute capture : aucun décalage à l'arrivée.
- **Boîte trop basse** (moins de 175 px sous l'en-tête, fenêtre étroite ou basse) : la carte se pose sur l'en-tête : le filtre et la Corbeille sont **recouverts** (non masqués) pendant la visite, et la réserve est supprimée pour que la capture mise en évidence reste visible. Dès « Terminer » ou « Passer », la carte disparaît et ces contrôles redeviennent accessibles et non recouverts (vérifié à 360×480 et 380×560).
- Contour intérieur uniquement : rien ne change de taille ni de place (boîte et champ de capture mesurés identiques avant et après l'étape).
- `prefers-reduced-motion` : durée de transition à 0 (jetons).
- Limite préexistante : sous ≈ 480 px de haut et ≈ 360 px de large, la liste de la boîte est presque vide de hauteur (16 px mesurés sans la visite) ; la visite n'y change rien.
