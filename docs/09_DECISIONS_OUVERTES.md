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

## À arbitrer avant le développement de la brique 001

1. **Pile technologique Windows** : .NET/WPF/WinUI ou Tauri (comparaison concise et choix unique).
2. **Emplacement technique des données** : répertoire Windows exact et stratégie de sauvegarde.
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
