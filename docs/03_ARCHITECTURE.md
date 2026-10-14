# 03 — Architecture

**Statut : pile technique actée le 2026-10-08 (voir `09_DECISIONS_OUVERTES.md`) ; fondations en place (brique 000).**

## Pile technique

| Couche | Technologie | Emplacement |
|---|---|---|
| Interface | React 19 + TypeScript, Vite | `src/` |
| Styles | CSS natif + jetons de design (variables CSS) | `src/styles/` |
| Système, métier, persistance | Rust, Tauri 2 | `src-tauri/src/` |
| Base locale (dès la brique 001) | SQLite (`rusqlite`, compilé dans l'exécutable), accédée uniquement depuis Rust | `%LOCALAPPDATA%\<identifiant>\data\morganiser.db` |
| Tests | Vitest + Testing Library (interface), `cargo test` (Rust) | `pnpm test` |

Communication : l'interface appelle des **commandes Rust** nommées (`invoke("…")`) regroupées dans `src-tauri/src/commands.rs`, et passe par un module unique côté TypeScript (`src/lib/`). L'interface n'accède jamais directement aux fichiers ni à la base.

Sécurité de la fenêtre : politique de sécurité du contenu (CSP) stricte dans `tauri.conf.json` (aucune ressource externe, aucun appel réseau). Les commandes Rust sont déclarées dans `build.rs` et autorisées une par une dans `capabilities/default.json` ; aucune permission système. Politique complète : `04_SECURITE.md`, section « Commandes Rust et permissions ».

Environnements : voir `docs/briques/BRIQUE_000_FONDATIONS.md` (identifiants, dossiers, garde-fous Dev/Stable).

Persistance (depuis 001-A) :
- `src-tauri/src/storage/` : ouverture de la base, réglages (`foreign_keys`, WAL), migrations SQL versionnées (`src-tauri/migrations/NNNN_*.sql`, numéro dans `PRAGMA user_version`, une transaction par migration ; base plus récente que l'application refusée avant tout réglage, donc sans être modifiée).
- Un module par domaine (`src-tauri/src/inbox.rs` pour « À organiser », `src-tauri/src/tasks.rs` pour les tâches et la conversion depuis la brique 002) : validation, requêtes paramétrées, testé sur bases temporaires.
- Listes paginées par curseur `(date, id)` ; modifications protégées par un verrou optimiste (`updated_at` attendu, vérifié dans le `UPDATE`) ;
- Une seule connexion partagée, protégée par un verrou ; les commandes Tauri (`commands.rs`) ne font que relayer vers les modules de domaine.
- Côté interface, une fonctionnalité regroupe ses composants et son unique module d'appel Rust (`src/features/<fonction>/`) : `src/features/inbox/` (capture, boîte) et `src/features/initiation/` (visite d'initiation minimale, mémoire `localStorage` locale à chaque environnement).
- **Sauvegarde avant migration (002-A) :** avant de migrer une base existante, `storage::open` crée une copie cohérente (`VACUUM INTO`, jamais une copie de fichier), la vérifie (intégrité, clés étrangères, version, contenu table par table), la finalise par renommage et ne conserve que les trois plus récentes, dans `<dossier de données>/backups/`. Un échec interdit la migration. Détail : `docs/briques/BRIQUE_002_VIE_ELEMENT_CAPTURE.md`.

## Séparation indispensable

- **UI** : vues, navigation, capture, animations, onboarding.
- **Métier** : tâches, sous-tâches, statuts, liens, responsabilités, tri.
- **Persistance** : base locale (SQLite envisagé), transactions, migrations versionnées, export, sauvegardes.
- **Intégrations futures** : téléphone, API médias, IA — facultatives et découplées.

La construction doit être **modulaire sans sur-ingénierie** : créer seulement les abstractions nécessaires à la brique courante, tout en maintenant des identifiants stables et des évolutions de schéma possibles.

## Objets conceptuels

- `InboxItem` : texte brut, créé le, destination facultative (pas un tag), éventuellement transformé sans perte de source.
- `Destination` : espace de responsabilité (Moi, Externe) ou rubrique (Administratif, Finances, Inventaire) ; identifiant stable, renommable, archivable, jamais supprimée. *(Précisé le 2026-10-14 : responsabilité et rubrique sont des notions indépendantes, voir « Notions indépendantes » ci-dessous ; la liste des rubriques est ouverte.)*
- `Task` : identifiant, titre, statut, priorité facultative, échéance facultative, responsabilité, liens, date création/clôture. *(Implémenté en 002 : identifiant, titre, détails, statut, destination, origine, dates. L'enrichissement est cadré dans `briques/BRIQUE_003_FICHE_TACHE_ENRICHIE.md`.)*
- `TaskRelation` : parent/enfant OU dépendance explicite ; ne pas confondre ces deux relations.
- `Person` : fiche de contexte et relations, sans doublons inutiles.
- `InventoryItem` : objet/collection et propriétés typées facultatives.
- `Note` : contenu et relations ; pièces jointes facultatives.
- `FinancialRecord` : montant/devise, type, catégorie, date, statut de paiement et relations.
- `Project` : ensemble d'actions et contexte.
- `EventHistory` : événements pertinents avec horodatages (pas forcément du temps de travail).
- `UserPreferences` : configuration d'affichage, tri, compact/étendu, onboarding, sécurité facultative.

Ces entités sont une **carte conceptuelle**, pas un ordre de créer dix tables dès la première brique.

## Notions indépendantes (cadrage 003-0, 2026-10-14 — VALIDÉ pour le principe)

M'Organiser distingue des notions **indépendantes** ; aucune ne se déduit d'une autre. État d'implémentation à ce jour entre parenthèses.

| Notion | Rôle | Exemples |
|---|---|---|
| **Entrée** (Capture & Idées) | Où arrivent les informations non traitées | capture rapide, captures récentes, éléments à traiter (implémenté : boîte « À organiser », « Traitées ») |
| **Univers de responsabilité** | Pour qui : dimension transversale de classement et de filtrage | Tous, Moi, Externe (implémenté seulement comme « destination » unique d'une capture) |
| **Rubrique** | Domaine métier, **liste ouverte** | Administratif, Finance(s), Inventaire ; plus tard Projets, Santé, Apprentissage, Maison, Voyages… (liste par défaut : À DÉCIDER) |
| **Type d'élément** | Ce qu'est l'élément ; **tout n'est pas une tâche** | tâche (implémenté), rendez-vous, note, objet d'inventaire, dossier administratif, opération financière, document (introduits progressivement) |
| **État d'avancement** | Où en est l'élément, propre aux types qui progressent | à faire, en cours, en attente, bloqué, terminé |

Conséquences :

- Une tâche administrative peut appartenir **simultanément** à l'univers Moi et à la rubrique Administratif. Représentation de ces deux dimensions pour une tâche : **À DÉCIDER** (`briques/BRIQUE_003_FICHE_TACHE_ENRICHIE.md`, Q3). Une **capture** conserve une destination unique et facultative (décision du 2026-10-10).
- Les vues et filtres **ne créent jamais de doublon physique** : un élément existe une fois, plusieurs vues le montrent.
- Ajouter une rubrique ou un type ne doit pas imposer une architecture complète nouvelle.
- Un objet d'inventaire peut exister sans tâche et être associé plus tard à plusieurs tâches.
- La **priorité** est une information propre à l'élément, indépendante de l'univers.
- La **visibilité** d'une capture traitée se **déduit** du statut de sa tâche ; elle n'entraîne aucune suppression physique.

## Relations entre éléments (principe VALIDÉ ; schéma À DÉCIDER ; HORS PÉRIMÈTRE de la brique 003)

- Un élément peut être lié à plusieurs autres ; la relation est **consultable depuis chaque extrémité**, enregistrée **une seule fois**, sans duplication logique.
- Exemples : tâche ↔ objet, tâche ↔ dossier, facture ↔ opération, rendez-vous ↔ dossier.
- Les relations **symétriques** et **dirigées** (ex. parent/enfant, dépendance) doivent être distinguées ; la hiérarchie de tâches (brique 005) ne se confond pas avec une relation générique.
- Points à étudier avant le schéma : identifiants (les UUID v7 existants sont déjà globalement uniques), intégrité entre types différents, suppression logique et réapparition, prévention des doublons, garde d'annulation des conversions (contrat de la fiche 002, section 10).
- Aucun schéma SQL n'est décidé. Le moteur de relations et la vue « Connexions » sont une brique ultérieure (`10_TABLEAU_DE_BORD.md`, sections 8 et 11).

## Local-first

- Base et fichiers réels dans un répertoire utilisateur Windows approprié, hors dossier de code et hors Git.
- **Stable** : base personnelle active ; **Dev** : données fictives ou copie isolée, jamais d'accès en écriture à Stable.
- Migrations testées sur copies, sauvegarde préalable et contrôle de compatibilité du retour arrière.
- Synchronisation mobile plus tard : nécessitera résolution de conflits et modèle d'identifiants stables ; aucun port ouvert initialement.

## Git et publication

- Un dépôt source GitHub, branches dédiées aux nouvelles fonctionnalités, intégration après revue et tests.
- Version stable installée distinctement de Dev ; les données personnelles ne résident pas dans le dossier de l'exécutable.
- Les mises à jour complètes suffisent au départ ; les patchs différentiels ne sont pas nécessaires pour une petite application.
- Aucune publication automatique de données, clés ou logs sensibles.

## Choix de technologie

Tranché le 2026-10-08 en faveur de Tauri 2 + React + TypeScript (comparaison, avantages et coûts consignés dans `09_DECISIONS_OUVERTES.md`). Ne pas commencer de version concurrente.
