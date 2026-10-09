# 09 — Registre des décisions

## Décisions actées

- Application Windows d'abord ; au lancement Windows : fenêtre compacte visible, non épinglée.
- `×` masque vers le tray, `—` minimise, `Quitter` via tray arrête réellement.
- Tableau de bord développé : tâches personnelles et externes distinctes et visibles ; capture directement visible avec boîte « À organiser » adjacente quand l'espace le permet.
- La capture est une **entrée brute**, non automatiquement classée en tâche.
- Sobriété visuelle « centre de contrôle », données opérationnelles devant les gros indicateurs.
- Hiérarchie de sous-tâches et prochaine action visible par défaut ; tris configurables.
- Historique conservé et statistiques fiables, non culpabilisantes.
- Stockage local et hors Git ; IA facultative bien après le cœur ; capture mobile assez tôt.
- Initiation in situ, facultative, rejouable, progression réelle.
- Sécurité dès les fondations ; verrouillage par mot de passe **facultatif, désactivé par défaut**.
- **Claude seul modifie le code ; Codex lecture seule et rapports.**

### 2026-10-08 — Pile technique

- **Question :** quelle technologie pour une application Windows-first (tray, instance unique, démarrage automatique, SQLite locale, interface moderne et animée, mobile plus tard) ?
- **Options :** .NET WPF, .NET WinUI 3, Tauri 2 + interface web (Electron écarté : poids et mémoire).
- **Choix :** **Tauri 2 + React + TypeScript + Vite** ; **Rust** pour la partie système, métier et persistance ; **SQLite** locale (gérée côté Rust) à partir de la brique 001.
- **Justification :** modules officiels pour tray / instance unique / démarrage / mises à jour ; animations et accessibilité du web ; séparation nette interface ↔ Rust ; exécutable léger (~3 Mo) ; voie mobile possible ; bon outillage VS Code.
- **Coûts acceptés :** deux langages (TypeScript et Rust), compilation Rust initiale lente, mémoire WebView2 supérieure à WPF.
- **Révision :** si Rust devient un frein majeur, le repli envisagé est WPF (pas Electron).

### 2026-10-08 — Emplacement des données (proposition appliquée en Dev, à confirmer)

- Données dans `%LOCALAPPDATA%\<identifiant>\data` (dossier local non itinérant, propre au compte Windows).
- Dev : `com.morganiser.desktop.dev` ; Stable : `com.morganiser.desktop`.
- La **stratégie de sauvegarde** reste à arbitrer (avant usage réel de Stable).

## À arbitrer avant le développement de la brique 001

1. ~~Pile technologique Windows~~ → actée le 2026-10-08 (voir ci-dessus).
2. **Emplacement technique des données** : proposition ci-dessus à confirmer ; stratégie de sauvegarde encore ouverte.
3. **Nom technique de la base** : `organiser.db` seulement comme exemple ; la retranscription « Pantao » n'est pas une décision ferme.

## À arbitrer au fil des briques

- Placement exact, taille et ajustement des panneaux dans le grand tableau de bord.
- Heuristique initiale de tri et paramètres d'urgences.
- Détails des statuts / dépendances / échéances.
- Modèle d'attachement des notes et documents.
- Chiffrement au repos et gestion des clés avant usage de données très sensibles.
- Solution mobile, synchronisation, conflits et chiffrement en transit.
- Licence et stratégie de publication si open source.

## Politique de décision

Chaque arbitrage important : date, question, options, choix, justification, impacts, révision éventuelle. Ne pas transformer une idée discutée en fonctionnalité déjà promise pour une version précise.
