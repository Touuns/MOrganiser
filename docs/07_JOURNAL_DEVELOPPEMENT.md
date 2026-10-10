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

## 2026-10-09 — Brique 000 : clôture (nettoyage du dépôt)

- **Suppression de `MOrganiser_Kit_Demarrage_v0.1.zip`** (validée par le propriétaire) : ses 17 fichiers ont été comparés un à un à leur version suivie par Git (commit `96fe340`) ; ils sont tous présents et identiques.
- **`.gitignore` corrigé** : les règles de dossier (`data`, `build`, `dist`, `target`, `private`, `backups`…) sont ancrées à la racine ou à un chemin précis, pour ne plus masquer un dossier de code du même nom ; `*secret*` est remplacé par `*.secret`, `*secret*.json`, `*secret*.txt` et `/secrets/`. Les règles par extension (bases SQLite, sauvegardes, clés, `.env`, exécutables) restent globales ; les règles `bin/` et `obj/` (.NET, sans usage) sont retirées.
- **Vérifié avec `git check-ignore`** : 10 chemins de code plausibles ne sont pas ignorés (ex. `src-tauri/src/data/mod.rs`) ; 18 chemins sensibles ou générés le sont ; la liste des fichiers ignorés du dépôt est inchangée ; aucun fichier suivi n'est devenu ignoré.
- **Aucune modification fonctionnelle.**

## 2026-10-10 — Brique 001-A : capture et persistance

- **Objectif :** boîte « À organiser » au-dessus du champ, capture immédiate, destination facultative (pas un tag) et filtre, SQLite locale, relecture après relance, erreurs sans perte, tests.
- **Décisions prises (validées par le propriétaire) :** destinations Moi, Externe (responsabilité), Administratif, Finances, Inventaire (rubriques) ; base `morganiser.db` ; spécification V2 fusionnée dans `BRIQUE_001_CAPTURE_RAPIDE.md` (référence unique, V2 supprimée) ; édition/suppression en 001-B, conversion en brique 002. Ajout en cours de route : checkpoint WAL à la fermeture.
- **Fichiers modifiés (Claude) :** voir la fiche brique 001, section 11 ; docs `01`, `02`, `03`, `04`, `05`, `06`, `09` ; `prompts/Claude/` vérifié (aucun secret) et à versionner.
- **Fonctionnement expliqué simplement :** l'interface envoie le texte à une commande Rust autorisée ; Rust le valide et l'écrit dans SQLite ; seulement après confirmation, l'interface vide le champ et relit la liste depuis la base.
- **Tests exécutés / résultats réels :**
  - `pnpm typecheck` : OK. `pnpm test` : 24/24 interface, 43/43 Rust.
  - Fenêtre Dev réelle, pilotée par le protocole DevTools de WebView2 (port 9222 sur `127.0.0.1` uniquement, pendant les essais, sans modification du code) :
    - champ actif à l'ouverture, boîte au-dessus ;
    - « Vendre ma PlayStation 5 » + 3 × Entrée → **une** carte « À classer », champ vidé, focus conservé ;
    - Maj+Entrée → retour à la ligne ; destination Inventaire affichée puis remise à « Aucune » ; filtres Inventaire et « À classer » corrects ;
    - saisie vide ou d'espaces → rien créé ;
    - `'; DROP TABLE inbox_items; --`, accents, balises HTML, mot très long → affichés tels quels, aucune balise interprétée, table intacte ;
    - fenêtre étroite 380 × 560 (émulée) → capture visible, seule la liste défile ;
    - fermeture puis relance → captures et destinations conservées ;
    - **échec réel** (base verrouillée par un script pendant 12 s) → « Envoi… », puis erreur, texte et destination conservés, aucune carte ; nouvel essai après levée du verrou → enregistré ;
    - contrôle en lecture seule de la base Dev : 5 captures distinctes, schéma v1, 5 destinations.
  - Dossier Stable `%LOCALAPPDATA%\com.morganiser.desktop` : absent avant et après.
- **Problèmes rencontrés :**
  - après la première fermeture, toutes les captures n'étaient que dans `morganiser.db-wal` (`.db` = 4 Ko) → ajout d'un checkpoint à la fermeture ; vérifié : `.db` = 28 Ko, `-wal` = 0 ;
  - une capture d'écran par Windows a montré un autre contenu que la fenêtre (fenêtre non mise au premier plan) : image supprimée aussitôt, aucune touche envoyée ; les essais ont ensuite été menés par le protocole DevTools, indépendant de l'écran.
- **Diagnostic Codex :** non demandé à ce stade.
- **Problèmes connus / limites :** voir fiche brique 001, section 11.
- **Validation du propriétaire :** en attente.
- **Prochaine étape :** 001-B (consulter, modifier, suppression récupérable, « Voir tout »), après validation.

## 2026-10-10 — Brique 001-A : vérification WAL et interruption brutale (prévalidation)

- **Demande du propriétaire :** vérifier que le checkpoint à la fermeture ne compromet ni la récupération après interruption brutale, ni les données validées ; gestion des fichiers auxiliaires ; absence de perte si le checkpoint échoue ; documentation sans ambiguïté sur la copie de `morganiser.db`.
- **Constats :**
  - le checkpoint SQLite est sûr par construction (`-wal` vidé seulement après écriture et synchronisation de `.db`) ;
  - le réglage `synchronous` n'était pas fixé explicitement → fixé à `FULL` (chaque capture confirmée est forcée sur disque) ;
  - un checkpoint bloqué renvoie « occupé » sans erreur, ce que le code ignorait → résultat désormais lu (`Complete` / `Incomplete`) et signalé dans le terminal ; aucune perte dans les deux cas ;
  - la documentation et un commentaire laissaient entendre que `.db` seul suffisait une fois l'application fermée → corrigé : jamais `.db` seul ; les trois fichiers ensemble, application fermée, en attendant le système de sauvegarde.
- **Fichiers modifiés :** `src-tauri/src/storage/mod.rs`, `src-tauri/src/lib.rs`, fiche brique 001 (sections 7 et 11).
- **Tests exécutés / résultats réels :**
  - `pnpm test` : 24/24 interface ; 46/46 Rust (+1 ignoré, lancé comme sous-processus) ; `pnpm typecheck` OK ;
  - nouveau test : un sous-processus valide une capture puis est **tué** (TerminateProcess) → `-wal` non vide, réouverture : capture présente ;
  - nouveau test : après arrêt sans checkpoint, une copie de `.db` seul ne contient pas la capture ; `.db` + `-wal` (sans `-shm`) la contient ;
  - nouveau test : checkpoint bloqué par une lecture → `Incomplete`, aucune perte après réouverture ;
  - **en réel** : capture dans l'application Dev puis arrêt forcé du processus → `.db` inchangé, données uniquement dans `-wal` ; relance → toutes les captures présentes ; fermeture normale → checkpoint complet (`-wal` = 0), 9 captures lues dans `.db`.
  - Dossier Stable : absent avant et après.
- **Observation :** 3 captures de test (« Faire un test », « DEuxieme test », « test », 03:06) ont été ajoutées par le propriétaire pendant sa validation manuelle ; elles ont été conservées. Son instance `pnpm app:dev` occupait le port 1420, ce qui explique l'échec « port 1420 already in use » d'un de mes lancements (même cause probable que l'incident noté en brique 000).

## 2026-10-10 — Brique 001-A : corrections finales (ordre d'affichage, fermeture)

- **Ordre d'affichage (demande du propriétaire) :** ordre chronologique croissant, nouvelle capture en bas de la liste, au-dessus du champ. Rust continue de sélectionner les 20 **plus récentes** (tri décroissant + limite) ; l'interface inverse cette sélection pour l'affichage. Une simple inversion du tri SQL aurait affiché les 20 plus anciennes : écarté. La liste défile vers le bas au chargement, au changement de filtre et après une capture réussie ; en cas d'échec, la liste n'est pas modifiée. La mention « Les 20 plus récentes sur N » est placée en haut de la liste.
- **Fichiers modifiés :** `src/features/inbox/InboxHome.tsx`, `InboxPanel.tsx`, `InboxPanel.css`, `InboxHome.test.tsx` ; fiche brique 001, `02`, `09`, README.
- **Fermeture (signalement du propriétaire : `Failed to unregister class Chrome_WidgetWin_0. Error = 1411` et `STATUS_CONTROL_C_EXIT (0xc000013a)`) :**
  - fermeture normale vérifiée deux fois par `WM_CLOSE` (le message envoyé par le bouton `×`), sans `Ctrl+C` : code de sortie **0**, aucun message `Chrome_WidgetWin`, aucun `0xc000013a`, checkpoint WAL complet (`-wal` = 0), captures retrouvées à la relance ;
  - `0xc000013a` signifie par définition « arrêté par Ctrl+C ou fermeture de la console » : `Ctrl+C` dans le terminal de `pnpm app:dev` interrompt aussi l'application (compilation debug liée à la console). Le message WebView2 accompagne cette interruption (déjà observé lors d'un arrêt anormal en brique 000). Sans conséquence pour les données (récupération depuis `-wal`, vérifiée par test et en réel) ;
  - aucun contournement ajouté ; consigne documentée : fermer par `×` ;
  - la ligne `ELIFECYCLE … exit code 4294967295` présente à chaque fermeture provient de l'arrêt du serveur Vite par Tauri, pas de l'application.
- **Tests exécutés / résultats réels :**
  - `pnpm typecheck` OK ; `pnpm test` : 25/25 interface, 46/46 Rust (+1 ignoré, sous-processus).
  - En réel : ordre initial (plus ancienne en haut, plus récente en bas, liste défilée en bas) ; 3 captures successives ajoutées en bas dans l'ordre ; 10 captures supplémentaires (22 au total) → 20 affichées, les 2 plus anciennes exclues, mention « Les 20 plus récentes sur 22 » ; filtres Finances (2) et « À classer » (15) en ordre chronologique ; fermeture `×` puis relance : même sélection, même ordre.
  - Dossier Stable : absent.
- **Données Dev :** 22 captures fictives (les miennes et les 3 du propriétaire).

## 2026-10-10 — Brique 001-A : correctif après revue GitHub (bases de version future)

- **Observation de la revue (ChatGPT, commit `cd9da28`) :** `open()` appliquait les réglages (dont `journal_mode = WAL`) avant de vérifier `user_version` ; une base de version future pouvait donc être modifiée avant d'être refusée.
- **Vérification :** confirmée par un test écrit avant la correction : sur une base v99 en journal classique, le refus faisait passer l'en-tête du fichier en WAL (octets 18-19 : `1,1` → `2,2`) et incrémentait son compteur de modifications.
- **Correction (sans refonte) :** `check_schema_version()` lit la version (lecture seule) en premier dans `open()` ; les réglages et migrations ne viennent qu'ensuite ; `migrate()` réutilise le même contrôle.
- **Fichiers modifiés :** `src-tauri/src/storage/mod.rs`, fiche brique 001, `03_ARCHITECTURE.md`.
- **Tests exécutés / résultats réels :** nouveau test `une_base_future_en_journal_classique_reste_strictement_intacte` (fichier identique octet pour octet, pas de `-wal` créé, mode `delete` conservé, `user_version` = 99) ; `pnpm typecheck` OK ; `pnpm test` : 25/25 interface, 47/47 Rust (+1 ignoré) ; le test des réglages WAL/FULL des bases compatibles reste vert.

## 2026-10-10 — Brique 001-B : gestion des captures

- **Objectif :** fiche de consultation, modification, corbeille récupérable, « Voir tout » paginé (périmètre validé par le propriétaire).
- **Décisions prises :** voir `09_DECISIONS_OUVERTES.md` (panneau 900 px, corbeille sans confirmation + annulation 8 s, verrou optimiste atomique, lots de 50).
- **Fichiers :** voir fiche brique 001, section 12.
- **Fonctionnement expliqué simplement :** cliquer une carte ouvre sa fiche ; l'enregistrement n'a lieu que si la capture n'a pas changé entre-temps ; supprimer ne fait que la masquer (champ `deleted_at`) et la corbeille permet de la remettre exactement comme avant ; « Voir tout » charge l'historique 50 par 50 en s'appuyant sur la date et l'identifiant du dernier élément reçu.
- **Tests exécutés / résultats réels :**
  - `pnpm typecheck` OK ; `pnpm test` : 55/55 interface, 69/69 Rust (+1 ignoré) ; `pnpm audit` : aucune vulnérabilité.
  - Fenêtre Dev réelle (pilotée par le protocole DevTools local, 127.0.0.1) :
    - fiche : ouverture au clic et par `Entrée` (focus dans le texte), fermeture par `Échap` (focus rendu à la carte) ;
    - modification texte + destination, date « Dernière modification » mise à jour ; avertissement de brouillon et « Continuer à modifier » ;
    - corbeille depuis la fiche, notification « Annuler », restauration à sa place chronologique ; vue Corbeille ; fiche en lecture seule ;
    - **échec réel** (base verrouillée par un script) : brouillon conservé, nouvel essai réussi ; **conflit réel** (écriture externe en base) : version externe non écrasée, brouillon conservé, remplacement seulement au second enregistrement (base vérifiée à chaque étape) ;
    - « Voir tout » avec 82 captures : lot de 50, lot plus ancien de 32 ajouté au-dessus, **position de lecture conservée à l'identique** (la carte lue n'a pas bougé), 0 doublon ; filtres Finances (22), À classer (55), Toutes (82) **égaux aux comptes de la base** ;
    - fermeture normale (code 0, `-wal` vidé) puis relance : modification et corbeille conservées ; restauration : mêmes identifiant, date de création et `updated_at` qu'avant la suppression ;
    - fenêtre étroite (600 px émulés) : la fiche remplace la vue principale, « ← Retour » affiché.
  - Dossier Stable : absent avant et après.
- **Problèmes rencontrés :**
  - premier rendu avec fiche ouverte : l'aide « Entrée pour envoyer… » se repliait lettre par lettre dans la colonne rétrécie → masquée par une requête de conteneur (corrigé, vérifié) ;
  - `git merge --ff-only` lancé par erreur depuis `brique-001-capture` (qui a avancé localement) → branche remise à `a0f6a42` (identique à GitHub), `main` synchronisée, aucune perte ;
  - mes scripts de pilotage ont eu deux défauts de sélecteur (ancien texte filtré, première carte « Restaurer » cliquée) ; l'application n'était pas en cause, les contrôles ont été refaits.
- **Diagnostic Codex :** non demandé à ce stade.
- **Problèmes connus / limites :** voir fiche brique 001, section 12 (fermeture de fenêtre avec brouillon, « Voir tout » rechargé après modification, pas de suppression définitive, seuil 900 px à ajuster).
- **Validation du propriétaire :** en attente.
- **Prochaine étape :** 001-C (animation ascendante), après validation.

## 2026-10-10 — Brique 001-B : protection de la fermeture de la fenêtre

- **Demande du propriétaire :** ne plus perdre un brouillon (fiche ou capture rapide) à la fermeture normale ; examiner le message ELIFECYCLE ; stabiliser l'emplacement de la notification d'annulation.
- **Mise en œuvre :** `useCloseGuard` (`src/lib/closeGuard.ts`) sur `onCloseRequested` ; composant partagé `LeaveBanner` (fiche et capture rapide) ; `hasUnsaved()` (fiche) et `hasUnsent()`/`submit()` (capture rapide) ; fermeture par `destroy()` après décision (aucune boucle). Permissions ajoutées : `core:event:allow-listen`, `core:event:allow-unlisten`, `core:window:allow-destroy` (vérifiées dans le code source de Tauri 2.12.1 : le gestionnaire est attendu, la fenêtre est détruite automatiquement s'il n'appelle pas `preventDefault()`).
- **Corrigé en cours de route :** après « Continuer », le focus tombait sur la page ; il revient au texte (fiche) ou au champ (capture rapide), testé.
- **Tests :** `pnpm typecheck` OK ; `pnpm test` : 68/68 interface (13 nouveaux dans `InboxHome.close.test.tsx`), 69/69 Rust (+1 ignoré) ; `pnpm audit` : aucune vulnérabilité.
- **Vérifié dans la fenêtre Dev réelle (fermeture par WM_CLOSE, équivalent du `×`) :**

  | Scénario | Résultat |
  |---|---|
  | Sans brouillon | Fermée en 0,2 s, code de sortie 0, `-wal` vide |
  | Fiche modifiée → Continuer | Fenêtre ouverte, brouillon intact, focus dans le texte, base inchangée |
  | Fiche modifiée → Abandonner | Application arrêtée, base inchangée, brouillon non écrit |
  | Fiche modifiée → Enregistrer | Enregistré, fenêtre fermée en 0,3 s |
  | Fiche modifiée, base verrouillée → Enregistrer | Erreur affichée, fenêtre et brouillon conservés, base inchangée ; réussi une fois le verrou levé |
  | Texte non envoyé → Continuer | Fenêtre ouverte, texte conservé, focus dans le champ |
  | Texte non envoyé → Envoyer | Capture enregistrée puis fermeture |
  | Relance | Toutes les captures retrouvées |
  | Dossier Stable | Absent |

- **Notification d'annulation :** emplacement réservé sous le champ ; position du champ mesurée avant, pendant et après la notification : 379 px, 379 px, 379 px.
- **Diagnostic ELIFECYCLE (`Command failed with exit code 4294967295`) :** *pas un défaut de l'application.*
  - code de sortie de `morganiser.exe` : **0** (toutes les fermetures) ;
  - code de sortie du CLI Tauri lancé par `node tauri.js dev` (sans pnpm autour) : **0**, et le message est quand même présent dans son journal : il ne vient donc pas du pnpm externe ;
  - code de sortie de `pnpm app:dev`, `pnpm exec tauri dev` et `node tauri.js dev` (trois chaînes, mesurées séquentiellement) : **0** ;
  - le message est écrit par le `pnpm dev` interne (`beforeDevCommand`, qui exécute Vite) : à la sortie de l'application, le CLI Tauri arrête ce processus de force, qui se termine avec -1 (`0xFFFFFFFF` = 4294967295) et le signale ;
  - après la fermeture : aucun processus `morganiser`, port 1420 libre, SQLite fermé (`-wal` = 0), captures retrouvées à la relance.
  - Aucun contournement ajouté. Le message n'existe qu'en mode développement (`pnpm app:dev`).
- **Erreurs de mesure corrigées pendant le diagnostic :** journaux PowerShell lus en UTF-8 alors qu'ils sont en UTF-16 (faux « ELIFECYCLE absent ») ; `bash` résolu vers WSL dans un script (essais chevauchés, mesures écartées puis refaites séquentiellement) ; un scénario (D) rejoué car sa carte avait été renommée plus tôt.
- **Problèmes connus / limites :** arrêt forcé du processus, extinction de Windows et coupure de courant ne sont pas interceptables (documenté) ; seul un brouillon non enregistré peut alors être perdu.
- **Validation du propriétaire :** en attente.

## 2026-10-10 — Brique 001-B : fermeture pendant un enregistrement (clôture)

- **Constat :** aucune perte possible (fenêtre non détruite avant décision, écriture SQLite atomique), mais deux comportements faux : avertissement périmé dans la fiche après la fin de l'enregistrement, et faux message « L'envoi a échoué » dans la capture rapide si l'envoi était en cours.
- **Correction :** la fiche exécute le départ en attente une seule fois dès que l'enregistrement réussit ; la capture rapide fait attendre toute nouvelle demande d'envoi sur l'envoi en cours.
- **Tests :** 3 nouveaux (enregistrement long puis fermeture ; enregistrement qui échoue ; envoi long puis « Envoyer »). `pnpm typecheck` OK ; `pnpm test:ui` 71/71 ; Rust inchangé (69/69 au dernier passage complet).
- **Limite :** une mise à la corbeille ou une restauration en cours n'est pas bloquante pour la fermeture (opération atomique côté Rust, sans brouillon à perdre).

## 2026-10-10 — Brique 001-B : corrections après audit Codex de `d7edd22`

- **Contexte :** audit en lecture seule, 6 défauts avérés (4 élevés) de coordination asynchrone ; publication suspendue. Aucun commit correctif avant validation.
- **Méthode :** 15 tests déterministes à promesses différées écrits **avant** les corrections (`InboxHome.async.test.tsx`) : 11 échouaient sur `d7edd22`, 4 étaient des garde-fous.
- **Défauts, causes et corrections :** voir la fiche brique 001, section « Coordination des opérations asynchrones ». Cause commune : décisions prises sur l'état d'un rendu périmé (closure) et résultats d'opérations non rattachés à leur capture d'origine.
- **Deux défauts supplémentaires trouvés pendant la correction :** (1) juste après un enregistrement réussi, `hasUnsaved()` lisait encore l'ancien rendu et ré-affichait l'avertissement ; même défaut côté capture rapide (`hasUnsent()`) → modèle immédiat par références ; (2) un avertissement « texte non envoyé » devenait périmé si l'envoi aboutissait pendant son affichage → la fermeture reprend (`onSent`).
- **Tests :** `pnpm typecheck` OK ; interface 86/86 (+15) ; Rust 69/69 (+1 ignoré, inchangé) ; `pnpm audit` : aucune vulnérabilité.
- **Fenêtre Dev réelle, retards créés par un verrou SQLite (3,5 s) :** 24 vérifications sur 24 :
  - A : saisie B conservée, A en base, B s'enregistre ensuite sans conflit ;
  - B : × pendant l'envoi → avertissement, « Envoyer » puis « Continuer à écrire » → fenêtre ouverte, capture enregistrée une seule fois ;
  - C : corbeille de A différée pendant l'ouverture de B → fiche et brouillon de B intacts, notification affichée, brouillon protégé à la fermeture ;
  - D : capture supprimée ailleurs → brouillon conservé, fermeture suspendue, restauration en gardant le brouillon ;
  - E : enregistrer puis corbeille → texte enregistré et capture à la corbeille ;
  - F : restauration depuis la liste → fiche modifiable, titre et commandes cohérents ;
  - G : × pendant une corbeille bloquée → la fenêtre attend, puis se ferme (2,6 s), mise à la corbeille aboutie.
- **SQLite :** `integrity_check` ok, aucune violation de clé étrangère, schéma v1, aucun doublon ; `-wal` vide après fermeture. Dossier Stable : absent.
- **Fichiers modifiés :** `CaptureDetail.tsx`, `CaptureForm.tsx`, `InboxHome.tsx`, nouveau `InboxHome.async.test.tsx`, fiche brique 001, journal.
- **Limites :** voir la fiche (arrêt forcé, coupure de courant). **Validation du propriétaire :** en attente ; commit correctif non créé.

## 2026-10-10 — Brique 001-B : abandon limité au brouillon concerné (revue GitHub de `b1f3cc2`)

- **Défaut :** `skipDetail`/`skipForm` restaient vrais jusqu'à la destruction de la fenêtre : un texte B saisi pendant l'attente d'une écriture, après l'abandon de A, était considéré abandonné et détruit à la fermeture.
- **Reproduction :** 2 tests déterministes (fiche, capture rapide) échouaient avant correction ; un troisième (brouillon inchangé abandonné → fermeture sans nouvel avertissement) servait de garde-fou.
- **Correction :** l'abandon mémorise l'empreinte du brouillon (`draftKey()`), pas un drapeau ; la fermeture réexamine l'état actuel à la reprise.
- **Tests :** `pnpm typecheck` OK ; interface 89/89 ; Rust non modifié (69/69 au dernier passage).

## 2026-10-10 — Brique 001-C : animation d'arrivée d'une capture

- **Objectif :** carte fantôme montant de la zone de saisie vers le bas de la boîte, 350 ms, sans jamais retarder ni conditionner l'enregistrement.
- **Décisions (propriétaire) :** départ = zone de saisie (pas le bouton) ; vraie carte insérée normalement puis seulement masquée visuellement pendant le trajet ; animation non bloquante ; aucune dépendance ; aucune modification Rust/SQLite.
- **Technique :** Web Animations API ; fantôme = clone de la carte (`aria-hidden`, `inert`, non interactif) ; FLIP des cartes présentes ; annulation sûre sur resize, filtre, vue, démontage ; délai de sécurité.
- **Fichiers :** `arrivalAnimation.ts` (nouveau), `InboxPanel.tsx`, `InboxHome.tsx`, `CaptureForm.tsx`, `CaptureCard.tsx/.css`, `InboxHome.animation.test.tsx` (nouveau), fiche brique 001 (section 13).
- **Tests :** `pnpm typecheck` OK ; interface 104/104 (+15) ; Rust non modifié (69/69 au dernier passage).
- **Fenêtre Dev réelle (échantillonnage par `requestAnimationFrame`) :**
  - envoi normal : fantôme de 424 px (haut du champ) à 281 px (carte d'arrivée) en ≈ 345 ms, 22 images, trajet monotone, opacité 0,55 → 1 ; vraie carte masquée pendant, visible après, 0 fantôme restant ;
  - deux envois à 120 ms d'intervalle : 3 cartes visibles, aucun fantôme ;
  - mouvement réduit (émulé) : aucun fantôme, carte visible ;
  - fenêtre de 600 px : trajet normal de 504 à 361 px en ≈ 350 ms ;
  - redimensionnement et changement de vue en cours de trajet : fantôme retiré, vraie carte visible ;
  - fermeture normale (code 0), `-wal` vide, `integrity_check` ok, aucun doublon, dossier Stable absent.
- **Difficultés :** une capture d'écran en plein vol n'a pas pu être obtenue (latence > 350 ms ; le ralentissement DevTools n'agit pas sur les animations créées ensuite) ; validation par positions mesurées.
- **Validation du propriétaire :** en attente (jugement visuel de la fluidité à faire à l'usage).

## 2026-10-10 — Brique 001-C : disparition à la mise à la corbeille et notification flottante

- **Demande (propriétaire, après essais) :** retour plus explicite à la suppression depuis la fiche ; arrivée animée validée et inchangée.
- **Réalisé :** `playDeparture` (fondu 220 ms, FLIP des cartes restantes), notification flottante hors flux avec région vivante persistante, message « Capture déplacée dans la corbeille. », focus conservé sur la carte voisine, minuterie de la notification stabilisée. Aucun changement Rust, SQLite ou commande.
- **Tests :** `pnpm typecheck` OK ; interface 117/117 (+13 : disparition, confirmation, annulation pendant la transition, suppressions rapprochées, « Voir tout », filtre, mouvement réduit, redimensionnement, vue, focus, fenêtre étroite) ; Rust non modifié.
- **Vérification Windows réelle :** **non effectuée à ce jour** : une instance de l'application lancée par le propriétaire (démarrée à 16:11) occupait le port 1420 ; elle n'a pas été arrêtée. À faire dès que l'instance est fermée.
- **Validation du propriétaire :** en attente.

## 2026-10-10 — Brique 001-C : géométrie stable et défilement conservé

- **Défauts signalés (propriétaire) :** (1) la liste revenait en bas après une suppression ; (2) ouvrir une fiche déplaçait et redimensionnait la boîte « À organiser ».
- **Causes :** (1) un `useLayoutEffect` descendait en bas à chaque changement de `items` ; (2) la fiche était une colonne de grille qui poussait la boîte.
- **Corrections :** ancrage de défilement par identifiant (`scrollAnchor.ts`), « en bas » réservé au premier affichage, au changement de filtre et à l'arrivée d'une capture ; « Voir tout » conserve sa profondeur à la relecture ; `preventScroll` sur les focus ; fiche en panneau flottant hors flux (règle : un panneau secondaire ne déplace jamais le contenu principal). Détails : fiche brique 001, sous-section « Règles UX ».
- **Constat annexe :** le défilement en bas d'une nouvelle capture dépendait de la mesure de géométrie de l'animation (zone de saisie non mesurable → pas d'arrivée) ; il en est désormais indépendant.
- **Tests :** `pnpm typecheck` OK ; interface 125/125 (+8 : conservation après suppression, ancre supprimée, édition, restauration, nouvelle capture, filtre, focus, structure du panneau) ; Rust non modifié.
- **Vérification Windows réelle (instance du propriétaire fermée) :** rectangles de la boîte, de la capture et de la colonne mesurés avant, pendant et après, à 1700, 1200 et 960 px : **écart 0 px** partout ; défilement conservé après suppression (voisine 238 → 237 px) et après « Annuler » ; mouvement réduit, « Voir tout », filtre avec fiche ouverte et fenêtre de 600 px conformes (voir fiche brique 001).
- **Défaut révélé par la mesure réelle :** la fiche mesurait 786 px et descendait sur la capture rapide (l'ancrage CSS sur la zone de grille n'était pas respecté). Les tests simulés ne pouvaient pas le voir. Corrigé par une scène explicite ; fiche à 582 px.
- **Retour du propriétaire :** essais manuels satisfaisants (« ça m'a l'air parfait »).
- **Données :** les captures « Dép … » de test restent dans la base Dev (pas de nettoyage, sur instruction).
- **Validation du propriétaire :** en attente.

## 2026-10-10 — Brique 001-C : carte laissée invisible après la transition de sortie (revue de `1cb3487`)

- **Défaut (reproduit) :** `playDeparture` masquait la carte (`visibility: hidden`) sans jamais la rétablir à la fin ou à l'annulation ; si la liste ne la retirait pas (relecture en échec ou tardive), elle restait dans le DOM, invisible. 3 des 4 tests de reproduction échouaient avant correction.
- **Correction :** à l'arrêt du mouvement, la carte encore présente est rendue visible et marquée « sortie » (atténuée, `inert`) ; `Annuler` retire la marque, même après la fin de la transition. Aucun changement de durée ni de style des animations, ni de Rust, SQLite, fiche flottante ou ancrage du défilement.
- **Tests :** `pnpm typecheck` OK ; interface 129/129 (+4).

## 2026-10-10 — Brique 001-C : notification d'annulation compacte et discrète

- **Demande :** notification trop longue (8 s) et trop visible près de la capture rapide.
- **Réalisé :** 4 s ; texte « Déplacée dans la corbeille » ; pilule compacte en bas à droite sur la ligne du pied de page ; pause du compte à rebours au survol et au focus, reprise avec le temps restant ; mêmes composant et région vivante.
- **Inchangés :** animations d'arrivée et de départ, correctif `data-departed` et ses 4 tests, défilement, fiche flottante, SQLite.
- **Tests :** `pnpm typecheck` OK ; interface 132/132 (notification : 6 tests dont survol, focus, cumul, reprise du temps restant).
- **Vérification Windows réelle (instance du propriétaire fermée) :** position, non-recouvrement, durée de 4,4 s, pause au survol, reprise du temps restant et « Annuler » conformes à 1700, 960 et 600 px. Un échec apparent (3,7 s au lieu d'une disparition plus rapide) venait du script de mesure, et non de l'application : un survol réel de 5 s maintient la pilule, sa sortie la fait disparaître en 3,8 s (temps restant). Corrigé dans le script, pas dans l'application.
- **Fermeture normale :** code 0, `-wal` vide, `integrity_check` ok, dossier Stable absent, aucun processus ni port resté ouvert.

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
