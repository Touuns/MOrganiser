# Prompt pour Claude — démarrage de la sous-brique 001-A

Nous commençons maintenant **M'Organiser — brique 001-A**, sans réaliser les autres sous-briques.

Lis d'abord `AGENTS.md`, `CLAUDE.md`, `docs/08_COLLABORATION.md`, `docs/04_SECURITE.md`, `docs/03_ARCHITECTURE.md`, `docs/05_ROADMAP.md`, `docs/06_INITIATION.md` et la fiche existante `docs/briques/BRIQUE_001_CAPTURE_RAPIDE.md`.

Je fournis également la nouvelle spécification `BRIQUE_001_SPECIFICATION_V2.md`. **Elle corrige une confusion importante : il ne s'agit pas de tags, mais d'une destination facultative dans une rubrique du tableau de bord.** Si tu ne la vois pas dans le dossier, demande-moi de l'y placer avant de coder.

## Travail préliminaire

1. Confirme `main` synchronisée avec GitHub et répertoire propre, sans modifier de données utilisateur.
2. Crée une branche locale dédiée `brique-001-capture` depuis `main`. Ne touche pas à la Stable.
3. Lis les exigences mises à jour. Propose brièvement ton découpage de 001-A, les chemins de fichiers concernés, le schéma SQLite minimal et les commandes Rust autorisées.
4. **Avant d'implémenter**, consigne les ajustements de vision dans `docs/briques/BRIQUE_001_CAPTURE_RAPIDE.md` et les références correspondantes (`01_CAHIER_DES_CHARGES.md`, `02_UI_UX.md`, `05_ROADMAP.md` et registre de décisions si nécessaire). Privilégie la mise à jour des documents existants, pas une multiplication des fichiers contradictoires.

## Implémentation 001-A — uniquement après confirmation du plan

- Capture rapide toujours visible ; **boîte « À organiser » placée juste AU-DESSUS** du champ.
- `Entrée` pour enregistrer, `Maj+Entrée` pour un retour à la ligne si multiligne ; bouton Envoyer disponible.
- **Destination facultative unique** correspondant à une rubrique de l'accueil, **pas un tag**. Sans destination, enregistrer dans « À organiser » sans classement. Avec destination, même enregistrement central, affichage de la rubrique, pas de copie ni de transformation automatique en tâche.
- Valeurs initiales sobres et extensibles, sans créer de modules inachevés.
- Enregistrement SQLite local côté Rust, schéma/migrations versionnés, commandes minimales Tauri explicitement autorisées, gestion d'erreurs et prévention des doubles saisies.
- Affichage récentes d'abord ; persistance après fermeture/réouverture ; aucune donnée réelle dans Git.
- Après sauvegarde réussie : vider la saisie et réinitialiser la destination, focus dans le champ. Sur erreur : conserver le contenu et la destination.
- **Pas encore** de fiche détaillée, modification/suppression (001-B), animation complexe (001-C), conservation intelligente de la destination (001-D), ni moteur d'initiation (001-E).

## Contraintes de méthode

- Claude est le seul assistant autorisé à modifier le code ; Codex exclusivement auditeur en lecture seule.
- Développer par petites étapes avec tests ciblés, sans refonte des fondations, sans accès au dossier Stable.
- Pas de télémétrie ni de serveur réseau, sauf serveur local Vite indispensable en mode Dev.
- Avant tout `push`, commit ou fusion, attendre mon autorisation. Aucun `--force`.
- À la fin : explique le cheminement UI → Rust → SQLite → UI en termes accessibles, les fichiers touchés, les tests exécutés et leurs résultats, comment essayer les captures sur Windows, les limitations et la suite.

**Commence par me présenter ton plan précis et les ajustements de documentation ; attends mon feu vert avant de programmer la 001-A.**
