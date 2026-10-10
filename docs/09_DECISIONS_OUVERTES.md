# 09 — Registre des décisions

## Décisions actées

- Application Windows d'abord ; au lancement Windows : fenêtre compacte visible, non épinglée.
- `×` masque vers le tray, `—` minimise, `Quitter` via tray arrête réellement.
- Tableau de bord développé : tâches personnelles et externes distinctes et visibles ; capture directement visible avec boîte « À organiser » **au-dessus** du champ de capture (décision du 2026-10-10, remplace « adjacente »).
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

### 2026-10-10 — Brique 001 (validées par le propriétaire)

- **Destination ≠ tag :** une capture peut recevoir **une destination principale facultative**, qui indique où elle sera traitée. Destinations initiales : espaces de responsabilité **Moi**, **Externe** ; rubriques **Administratif**, **Finances**, **Inventaire** (« Vente » non retenue). Aucun module n'est créé par une destination. Le modèle enregistre la nature (responsabilité / rubrique) pour permettre plus tard des associations multiples. La capture reste dans « À organiser », filtrable par destination ; un seul enregistrement.
- **Disposition :** la boîte « À organiser » est placée **au-dessus** du champ de capture (remplace « côte à côte »).
- **Ordre d'affichage :** ordre chronologique croissant ; chaque nouvelle capture apparaît **en bas** de la liste, juste au-dessus du champ (remplace « en tête, récent d'abord »). Au-delà de la limite d'affichage, ce sont les plus récentes qui sont montrées. Référence pour l'animation 001-C.
- **Répartition 001-B / 002 :** édition et suppression élémentaires d'une capture en 001-B ; conversion en tâche/note/projet en brique 002.
- **Nom de la base :** `morganiser.db`, dans le dossier `data` de l'environnement (Dev : `%LOCALAPPDATA%\com.morganiser.desktop.dev\data`). Mode WAL : fichiers auxiliaires `-wal`/`-shm` à traiter avec la base (voir fiche brique 001, section 7).
- **Référence unique :** `docs/briques/BRIQUE_001_CAPTURE_RAPIDE.md` (la spécification V2 y a été fusionnée).

### 2026-10-10 — Sous-brique 001-B (validées par le propriétaire)

- **Fiche :** panneau latéral à droite à partir de 900 px (ajustable après essais), remplacement de la vue principale en dessous ; fiche directement éditable.
- **Corbeille :** suppression logique **sans confirmation systématique**, notification « Annuler » (8 s en 001-B, **4 s depuis 001-C**), vue Corbeille permanente (survit au redémarrage) ; **aucune suppression définitive ni vidage automatique** en 001-B.
- **Conflits :** verrouillage optimiste (`updated_at` attendu) vérifié atomiquement dans le `UPDATE`, `updated_at` strictement croissant, erreurs distinctes (introuvable / corbeille / conflit). Sans migration.
- **« Voir tout » :** lots de 50, pagination par curseur `(date, id)`, plus anciennes ajoutées au-dessus.
- **Choix d'implémentation à relire :** la mise à la corbeille et la restauration ne modifient pas `updated_at` (dernière modification du contenu) ; voir la fiche brique 001, section 12, pour l'impact sur la future synchronisation.

### 2026-10-10 — Brique 001 clôturée

- 001-A à 001-E validées et fusionnées (PR #2 à #6). 001-D : option manuelle « Conserver ce choix » (suggestion automatique abandonnée). 001-E : initiation minimale, proposition unique à la première utilisation (mémoire `localStorage`, `morganiser.initiation.v1`), bouton « Découvrir » permanent ; reprise et parcours thématiques reportés en brique 007.

### 2026-10-10 — Brique 002 : conversion d'une capture en tâche (validées par le propriétaire)

- **Périmètre :** conversion en **tâche** uniquement ; notes et projets attendent leurs modules.
- **Tâche minimale :** identifiant stable, titre (≤ 120 caractères, extrait modifiable de la première ligne), détails (copie intégrale), statut initial « à faire », destination facultative copiée telle quelle, capture d'origine, horodatages. Les cinq statuts du cahier des charges sont admis par le schéma ; 002 n'écrit que « à faire ».
- **Origine :** capture conservée intégralement, retirée de « À organiser », retrouvable dans « Traitées » et depuis la tâche ; lecture seule tant qu'elle est convertie.
- **Annulation :** « Annuler » 4 s ; refusée si la tâche a été modifiée ou avancée. Tâche annulée conservée (`deleted_at`), jamais supprimée. Reconversion possible.
- **Dates :** la conversion ne modifie pas `updated_at` de la capture ; `converted_at` porte le changement de cycle de vie.
- **Garde SQL :** tout changement d'un champ métier d'une tâche doit faire croître strictement `updated_at` (déclencheur).
- **Migrations :** sauvegarde cohérente (`VACUUM INTO`) vérifiée avant toute migration d'une base existante ; échec de sauvegarde = pas de migration ; trois sauvegardes conservées. Aucune migration de Stable ; la base Dev réelle n'est migrée qu'après un essai sur copie isolée, organisé séparément.
- **Reste ouvert :** stratégie de sauvegarde/restauration **utilisateur** (copie planifiée, export) avant toute donnée réelle dans Stable.

## Arbitrages d'avant la brique 001 (clos, sauf la sauvegarde de Stable)

1. ~~Pile technologique Windows~~ → actée le 2026-10-08 (voir ci-dessus).
2. **Emplacement technique des données** : appliqué en Dev ; la **stratégie de sauvegarde/restauration** reste ouverte et doit être décidée **avant toute utilisation de Stable avec des données réelles**.
3. ~~Nom technique de la base~~ → `morganiser.db`, acté le 2026-10-10.

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
