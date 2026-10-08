# Brique 000 — Fondations techniques

**État : implémentée le 2026-10-08, en attente de validation du propriétaire.**
**Branche : `brique-000-fondations`.**

## Objectif

Disposer d'une application Windows qui démarre, affiche une fenêtre, sait dans quel environnement (Dev ou Stable) elle tourne, utilise un dossier de données propre à cet environnement, et se teste avec une seule commande. **Aucune fonctionnalité utilisateur.**

## Hors périmètre (volontairement)

SQLite, tâches, capture, tray, démarrage automatique, instance unique, mises à jour, installeur, tableau de bord définitif.

## Ce qui a été construit

| Élément | Rôle |
|---|---|
| `package.json`, `pnpm-lock.yaml` | Dépendances JavaScript (versions exactes) et commandes du projet. |
| `vite.config.ts`, `index.html`, `tsconfig.json` | Serveur de développement de l'interface (port 1420), compilation, configuration des tests. |
| `src/main.tsx`, `src/App.tsx`, `src/App.css` | Fenêtre de fondation : nom, badge d'environnement, version, dossier de données. |
| `src/styles/tokens.css` | Jetons de design (couleurs clair/sombre, espacements, typographie, durées d'animation à 0 si « animations réduites »). |
| `src/styles/base.css` | Règles globales : focus clavier toujours visible, mouvement réduit. |
| `src/components/Badge`, `src/components/Panel` | Deux composants de base réutilisables. |
| `src/lib/appInfo.ts` | Seul point d'appel vers Rust (`invoke("app_info")`) ; ne fait rien dans un simple navigateur. |
| `src/App.test.tsx`, `src/test/setup.ts` | 5 tests de l'interface (Vitest + Testing Library), sans fenêtre réelle. |
| `src-tauri/src/environment.rs` | Logique Dev/Stable ; `prepare()` = vérifications puis création du dossier `data` (12 tests, sur dossiers temporaires). |
| `src-tauri/src/config_tests.rs` | 8 tests qui lisent les **vrais** fichiers de configuration (identifiants, scripts, fenêtres, permissions, CSP). |
| `src-tauri/src/lib.rs` | Démarrage : `prepare()`, **puis** création de la fenêtre. |
| `src-tauri/build.rs` | Manifeste des commandes (`APP_COMMANDS`) : active le contrôle d'accès de Tauri. |
| `src-tauri/src/commands.rs` | Commande `app_info` (lecture seule). |
| `src-tauri/tauri.conf.json` | Configuration **Stable** (par défaut) : identifiant, fenêtre, politique de sécurité du contenu (CSP). |
| `src-tauri/tauri.dev.conf.json` | Surcharge **Dev** : autre identifiant, autre nom d'exécutable, titre « — DEV ». |
| `src-tauri/capabilities/default.json` | Permissions de la fenêtre : uniquement `allow-app-info` ; aucune permission système. |
| `src-tauri/icons/` | Icône provisoire (`icon-source.svg`) et ses dérivés Windows. |
| `.editorconfig`, `.vscode/extensions.json`, `.gitignore` | Conventions d'édition, extensions conseillées, exclusions Git (dont `target/`, installeurs). |

## Séparation Dev / Stable

| | Dev | Stable |
|---|---|---|
| Identifiant | `com.morganiser.desktop.dev` | `com.morganiser.desktop` |
| Dossier de données | `%LOCALAPPDATA%\com.morganiser.desktop.dev\data` | `%LOCALAPPDATA%\com.morganiser.desktop\data` |
| Cache WebView2 | `%LOCALAPPDATA%\com.morganiser.desktop.dev\EBWebView` | `%LOCALAPPDATA%\com.morganiser.desktop\EBWebView` |
| Exécutable compilé | `morganiser-dev.exe` | `morganiser.exe` |
| Lancement | `pnpm app:dev` | (plus tard, version installée) |

Garde-fous :
1. Un identifiant inconnu est refusé (l'application ne démarre pas).
2. Une compilation de développement (debug) avec l'identifiant Stable est refusée **avant** toute création de fichier ou de fenêtre.
3. Les deux dossiers sont distincts et non imbriqués (testé).
4. Un lancement refusé ne crée rien ; un lancement Dev ne modifie pas un dossier Stable existant (testé sur dossiers temporaires, jamais sur le vrai `%LOCALAPPDATA%`).
5. Les fichiers de configuration réels sont vérifiés par les tests : un identifiant ou un script `app:dev` modifié par erreur fait échouer `pnpm test`.

Risque résiduel documenté : une jonction Windows pourrait rediriger Dev vers Stable (voir `04_SECURITE.md`, vérifications à faire avant d'utiliser des données réelles).

## Permissions

Les commandes Rust sont soumises au contrôle d'accès de Tauri (manifeste dans `build.rs`). Politique pour les futures commandes : `04_SECURITE.md`, section « Commandes Rust et permissions ».

## Commandes

| Commande | Effet |
|---|---|
| `pnpm install` | Installe les dépendances JavaScript (une fois, ou après un changement de `package.json`). |
| `pnpm app:dev` | **Lance la fenêtre Windows en Dev**, avec rechargement automatique de l'interface. |
| `pnpm test` | Tous les tests : interface (`test:ui`) puis Rust (`test:rust`). |
| `pnpm typecheck` | Vérification des types TypeScript. |
| `pnpm dev` | Interface seule dans un navigateur (`http://127.0.0.1:1420`), sans Rust ni données. |
| `pnpm app:build:dev` | Exécutable Dev optimisé : `src-tauri/target/release/morganiser-dev.exe`. |
| `pnpm app:build:stable` | Exécutable Stable optimisé (à n'utiliser qu'après validation). |

## Critères d'acceptation (roadmap, étape 0)

- [x] L'application démarre dans Dev sans réseau.
- [x] Elle se ferme proprement (fermeture par `×`, code de sortie 0).
- [x] Une commande de test reproductible fonctionne (`pnpm test`).
- [x] Audit Codex (lecture seule) traité : tests de configuration, manifeste de permissions, risque des jonctions documenté.
- [ ] Validation visuelle par le propriétaire.

## Limites connues

- Si le démarrage est refusé (garde-fou), l'erreur n'apparaît que dans le terminal : pas encore de boîte de dialogue.
- `×` ferme l'application : le comportement « masquer vers le tray » viendra avec la brique 006.
- Icône provisoire.
- Pas encore de linter (ESLint) : TypeScript 7 est récent et la compatibilité des outils reste à vérifier.
