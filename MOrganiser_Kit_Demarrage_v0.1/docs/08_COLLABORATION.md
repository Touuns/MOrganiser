# 08 — Organisation du travail : propriétaire, ChatGPT, Claude, Codex

## Règle absolue

**Claude est l'unique auteur des modifications de code. Codex ne touche jamais au code ni aux fichiers du projet.** Codex peut lire, analyser, auditer et remettre ses conclusions dans un rapport texte ; Claude décide ensuite comment implémenter les corrections retenues à la demande du propriétaire.

## Rôles

- **Propriétaire :** besoins, arbitrages, tests utilisateurs, validations et décision de publier/mettre à jour Stable.
- **ChatGPT :** vision, UI/UX, cahier des charges, documentation, rédaction de prompts ciblés et coordination. Pas de présomption de changement du dépôt local.
- **Claude dans VS Code :** seul intervenant en écriture sur le code ; petits lots, tests, explications des modifications.
- **Codex :** **lecture seule stricte** ; code review, diagnostic, recommandations, rapport textuel. Ne pas appliquer de patch, ne pas corriger un fichier, ne pas créer de fichier/commit, ne pas exécuter de commande modifiant le projet.

## Usage économe des tokens

1. Une mission précise et bornée par conversation.
2. Fournir le chemin des documents ciblés plutôt que recopier l'intégralité du contexte.
3. Pour Claude, citer la fiche de brique et les critères d'acceptation ; demander de n'ouvrir que les fichiers nécessaires.
4. Pour Codex, demander un audit focalisé + liste hiérarchisée des observations (fichier/ligne/gravité/preuve/solution proposée), **sans écriture**.
5. Éviter de faire relire toute l'application à chaque petit changement ; privilégier revues aux jalons.
6. Les bilans vont dans les Markdown afin de reprendre le travail sans dépendre de la mémoire d'un seul assistant.

## Workflow d'une brique

1. Définir et valider un contrat de brique.
2. Claude implémente dans la zone Dev et teste avec données fictives.
3. Propriétaire vérifie dans l'application.
4. Si utile, Codex analyse le code **sans le modifier**.
5. Le propriétaire transmet les observations pertinentes à Claude pour correction.
6. Claude met à jour tests, fiche de brique et journal.
7. Intégration dans Stable uniquement après revue et sauvegarde appropriée.

## Interdictions

- Ne jamais donner à Codex la consigne « corrige », « applique », « refactorise » ou « lance un formatage ».
- Aucune exécution de scripts de migration sur la base Stable depuis Dev.
- Aucun push GitHub, release, suppression, reset Git ou commande destructive sans décision explicite du propriétaire.
- Ne pas stocker les discussions privées, les données personnelles réelles ou les clés dans le dépôt.
