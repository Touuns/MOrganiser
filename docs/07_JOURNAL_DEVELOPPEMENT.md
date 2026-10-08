# 07 — Journal de développement

## 2026-10-08 — Cadrage et documentation initiale

- Vision du produit et comportements Windows explicités.
- Rôles clarifiés : **Claude écrit et modifie le code uniquement ; Codex réalise des diagnostics textuels en lecture seule**.
- Documentation et feuille de route préparées avant le premier code.
- **Aucune fonctionnalité applicative ni pile technique validée à ce stade.**

## 2026-10-08 — Brique 000 : Fondations techniques

- **Objectif :** squelette Tauri exécutable sous Windows, séparation Dev/Stable, base de styles, tests reproductibles. Aucune fonctionnalité utilisateur.
- **Décisions prises :** pile Tauri 2 + React + TypeScript + Vite, Rust, SQLite plus tard (consignée dans `09`) ; données dans `%LOCALAPPDATA%\<identifiant>\data` (proposition à confirmer) ; branche `brique-000-fondations`.
- **Fichiers modifiés (Claude) :** voir la liste dans `docs/briques/BRIQUE_000_FONDATIONS.md` ; `.gitignore` complété (`target/`, `src-tauri/gen/`, `*.exe`, `*.msi`) ; docs `03`, `05`, `07`, `09`, `README.md`.
- **Fonctionnement expliqué simplement :** au lancement, la partie Rust lit l'identifiant de l'application pour savoir si l'on est en Dev ou en Stable, refuse toute incohérence, crée le dossier `data` de cet environnement, puis ouvre la fenêtre. L'interface React demande ces informations à Rust et les affiche (badge DEV, version, dossier).
- **Tests exécutés / résultats réels :**
  - `pnpm typecheck` : OK.
  - `pnpm test` : 5/5 tests interface et 7/7 tests Rust réussis.
  - `pnpm app:dev` : fenêtre « M'Organiser — DEV » ouverte, badge DEV et dossier `…\com.morganiser.desktop.dev\data` affichés (capture vérifiée) ; fermeture par `×`, sortie code 0.
  - `pnpm app:build:dev` : `morganiser-dev.exe` (~3 Mo) compilé, lancé avec la CSP active, affichage identique, fermeture code 0.
  - Garde-fou : exécutable debug avec l'identifiant Stable refusé (code 101) sans créer de dossier Stable.
  - `pnpm audit` : aucune vulnérabilité connue.
- **Problème rencontré et corrigé :** au premier essai du garde-fou, la fenêtre était créée avant la vérification, et WebView2 avait créé un cache `%LOCALAPPDATA%\com.morganiser.desktop\EBWebView` (aucune donnée applicative). Correction : fenêtre créée seulement après les vérifications (`"create": false`) ; cache de test supprimé ; nouveau test : aucun dossier Stable créé.
- **Diagnostic Codex (si demandé, lecture seule) :** non demandé.
- **Problèmes connus / limites :** erreur de démarrage visible seulement dans le terminal ; `×` quitte (tray en brique 006) ; icône provisoire ; pas de linter ; non testé : thème clair et réglage « animations réduites » de Windows.
- **Validation du propriétaire :** en attente.
- **Prochaine brique :** 001 — capture rapide (après arbitrage du nom de base et de la stratégie de sauvegarde).

## 2026-10-08 — Brique 000 : corrections après audit Codex

- **Objectif :** traiter les trois observations de l'audit Codex (lecture seule) ; aucune anomalie critique ou élevée.
- **Décisions prises :** activer dès maintenant le manifeste de commandes Tauri (contrôle d'accès effectif) ; jonctions Windows : documentation seulement.
- **Fichiers modifiés (Claude) :** `src-tauri/src/environment.rs` (fonction `prepare` + tests), `src-tauri/src/config_tests.rs` (nouveau), `src-tauri/src/lib.rs`, `src-tauri/build.rs`, `src-tauri/capabilities/default.json`, `src-tauri/Cargo.toml` (`tempfile` en dépendance de test), docs `03`, `04`, `07`, fiche brique 000.
- **Fonctionnement expliqué simplement :** les vérifications et la création du dossier sont regroupées dans `prepare()`, testable sur un dossier temporaire. Des tests lisent les vrais fichiers de configuration. Toute commande Rust doit désormais être déclarée et autorisée nommément, sinon Tauri la refuse.
- **Vérification de la documentation Tauri :** dans le code source de Tauri 2.12.1 (`webview/mod.rs`), le contrôle d'accès des commandes de l'application n'est appliqué que si un manifeste existe ou si l'origine est distante ; l'observation de Codex est confirmée.
- **Tests exécutés / résultats réels :**
  - `pnpm test` : 5/5 tests de l'interface et 20/20 tests Rust réussis.
  - `pnpm app:dev` : fenêtre Dev OK, `app_info` autorisée.
  - Contre-épreuve : avec `allow-app-info` retiré temporairement, l'appel est refusé (« app_info not allowed »), puis le fichier est restauré à l'identique.
  - Lancement debug avec la configuration Stable : refusé (code 101).
  - Dossier `%LOCALAPPDATA%\com.morganiser.desktop` absent avant et après tous les tests et lancements.
- **Problème rencontré :** un lancement `pnpm app:dev` a échoué une fois (« port 1420 already in use ») et laissé une fenêtre orpheline ; le port était libre juste après, la relance a fonctionné. Cause non identifiée (conflit passager) ; à surveiller.
- **Diagnostic Codex :** audit en lecture seule transmis par le propriétaire ; les trois observations ont été traitées.
- **Problèmes connus / limites :** jonctions non détectées par le code (documenté) ; erreur de démarrage visible seulement dans le terminal.
- **Validation du propriétaire :** en attente.
- **Prochaine brique :** 001, après validation.

## Modèle à recopier après chaque brique

### AAAA-MM-JJ — Brique XXX : [nom]

- **Objectif :**
- **Décisions prises :**
- **Fichiers modifiés (Claude) :**
- **Fonctionnement expliqué simplement :**
- **Tests exécutés / résultats réels :**
- **Diagnostic Codex (si demandé, lecture seule) :**
- **Problèmes connus / limites :**
- **Validation du propriétaire :**
- **Prochaine brique :**

Ne pas écrire « tests réussis » si aucun test n'a été exécuté.
