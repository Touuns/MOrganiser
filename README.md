# M'Organiser

**Statut : brique 000 validée ; brique 001-A (capture rapide + « À organiser », SQLite locale) en cours de validation.**  
**Plateforme cible : Windows, en premier.**  
**Pile : Tauri 2 + React + TypeScript + Rust.**  
**Méthode : une brique fonctionnelle à la fois.**

## Lancer l'application (Dev)

Prérequis : Node.js 24, pnpm 10, Rust (chaîne MSVC), Visual Studio Build Tools (C++), WebView2.

```powershell
pnpm install      # une seule fois
pnpm app:dev      # ouvre la fenêtre « M'Organiser — DEV »
pnpm test         # tous les tests (interface + Rust)
```

Pour arrêter, fermez la fenêtre par `×` (fermeture normale) plutôt que `Ctrl+C` dans le terminal. La première compilation Rust prend quelques minutes. Les données Dev (base `morganiser.db`) sont dans `%LOCALAPPDATA%\com.morganiser.desktop.dev\data`, jamais dans celles de Stable. Détails : `docs/briques/BRIQUE_000_FONDATIONS.md`.

M'Organiser est un centre de contrôle personnel : il doit aider à capturer, organiser, exécuter et suivre les responsabilités personnelles et externes, les projets, les objets, les finances et, plus tard, les loisirs.

## Démarrage pour le propriétaire

1. Lire `docs/00_VISION.md`, `docs/01_CAHIER_DES_CHARGES.md` et `docs/05_ROADMAP.md`.
2. Lire `docs/08_COLLABORATION.md` : **Claude écrit le code ; Codex ne modifie jamais les fichiers.**
3. Garder les fichiers réels (bases SQLite, documents, secrets et sauvegardes) **hors Git**.
4. Valider le choix de technologie avant la première brique de programmation. Le prompt de cadrage pour Claude se trouve dans `prompts/01_CLAUDE_CHOIX_TECHNIQUE.md`.
5. Développer et valider **une seule brique à la fois** ; consigner les changements dans le journal et dans la fiche correspondante.

## Index documentaire

- `docs/00_VISION.md` : intention du produit et principes non négociables.
- `docs/01_CAHIER_DES_CHARGES.md` : fonctionnalités, règles métier, limites.
- `docs/02_UI_UX.md` : interface, tableau de bord, animation et navigation.
- `docs/03_ARCHITECTURE.md` : principes techniques et modèle conceptuel.
- `docs/04_SECURITE.md` : confidentialité, sauvegardes et prévention des risques.
- `docs/05_ROADMAP.md` : construction progressive et critères de validation.
- `docs/06_INITIATION.md` : découverte guidée à travers l'interface réelle.
- `docs/07_JOURNAL_DEVELOPPEMENT.md` : journal chronologique.
- `docs/08_COLLABORATION.md` : règles de Claude, Codex et revue humaine.
- `docs/09_DECISIONS_OUVERTES.md` : décisions actées et sujets à arbitrer.
- `docs/briques/BRIQUE_000_FONDATIONS.md` : fondations techniques (structure, Dev/Stable, commandes).
- `docs/briques/BRIQUE_001_CAPTURE_RAPIDE.md` : spécification de la première fonctionnalité.

**Ne pas présenter une maquette ou un exemple comme une fonctionnalité déjà implémentée.**
