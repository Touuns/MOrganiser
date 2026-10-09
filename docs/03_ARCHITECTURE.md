# 03 — Architecture

**Statut : pile technique actée le 2026-10-08 (voir `09_DECISIONS_OUVERTES.md`) ; fondations en place (brique 000).**

## Pile technique

| Couche | Technologie | Emplacement |
|---|---|---|
| Interface | React 19 + TypeScript, Vite | `src/` |
| Styles | CSS natif + jetons de design (variables CSS) | `src/styles/` |
| Système, métier, persistance | Rust, Tauri 2 | `src-tauri/src/` |
| Base locale (dès la brique 001) | SQLite, accédée uniquement depuis Rust | `%LOCALAPPDATA%\<identifiant>\data` |
| Tests | Vitest + Testing Library (interface), `cargo test` (Rust) | `pnpm test` |

Communication : l'interface appelle des **commandes Rust** nommées (`invoke("…")`) regroupées dans `src-tauri/src/commands.rs`, et passe par un module unique côté TypeScript (`src/lib/`). L'interface n'accède jamais directement aux fichiers ni à la base.

Sécurité de la fenêtre : politique de sécurité du contenu (CSP) stricte dans `tauri.conf.json` (aucune ressource externe, aucun appel réseau). Les commandes Rust sont déclarées dans `build.rs` et autorisées une par une dans `capabilities/default.json` ; aucune permission système. Politique complète : `04_SECURITE.md`, section « Commandes Rust et permissions ».

Environnements : voir `docs/briques/BRIQUE_000_FONDATIONS.md` (identifiants, dossiers, garde-fous Dev/Stable).

## Séparation indispensable

- **UI** : vues, navigation, capture, animations, onboarding.
- **Métier** : tâches, sous-tâches, statuts, liens, responsabilités, tri.
- **Persistance** : base locale (SQLite envisagé), transactions, migrations versionnées, export, sauvegardes.
- **Intégrations futures** : téléphone, API médias, IA — facultatives et découplées.

La construction doit être **modulaire sans sur-ingénierie** : créer seulement les abstractions nécessaires à la brique courante, tout en maintenant des identifiants stables et des évolutions de schéma possibles.

## Objets conceptuels

- `InboxItem` : texte brut, créé le, classé/non classé, éventuellement transformé sans perte de source.
- `Task` : identifiant, titre, statut, priorité facultative, échéance facultative, responsabilité, liens, date création/clôture.
- `TaskRelation` : parent/enfant OU dépendance explicite ; ne pas confondre ces deux relations.
- `Person` : fiche de contexte et relations, sans doublons inutiles.
- `InventoryItem` : objet/collection et propriétés typées facultatives.
- `Note` : contenu et relations ; pièces jointes facultatives.
- `FinancialRecord` : montant/devise, type, catégorie, date, statut de paiement et relations.
- `Project` : ensemble d'actions et contexte.
- `EventHistory` : événements pertinents avec horodatages (pas forcément du temps de travail).
- `UserPreferences` : configuration d'affichage, tri, compact/étendu, onboarding, sécurité facultative.

Ces entités sont une **carte conceptuelle**, pas un ordre de créer dix tables dès la première brique.

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
