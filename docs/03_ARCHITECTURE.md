# 03 — Architecture : principes, pas encore choix technologique

**Statut : architecture conceptuelle.** La pile de développement Windows reste à choisir avant toute programmation.

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

## Choix de technologie à examiner

Critères : bon support Windows **tray, démarrage automatique, instance unique**, interface moderne, performance en arrière-plan, SQLite locale, expérience d'animation, maintenabilité avec Claude/VS Code, packaging, mises à jour et future interface mobile.

À comparer de manière concise avant de coder : **.NET (WPF/WinUI 3)** versus **Tauri + interface web moderne** (Electron comme alternative si une contrainte forte le justifie). Consigner la décision, ses avantages et ses coûts ; ne pas commencer plusieurs versions concurrentes.
