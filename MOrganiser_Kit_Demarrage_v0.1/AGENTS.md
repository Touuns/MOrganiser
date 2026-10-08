# Instructions communes aux assistants — M'Organiser

**À lire avant toute intervention.** Lire d'abord `README.md`, `docs/00_VISION.md`, `docs/05_ROADMAP.md`, `docs/08_COLLABORATION.md` et la fiche de brique concernée.

## Autorisations

- **Claude est le seul agent autorisé à créer, modifier, supprimer ou reformater le code et les fichiers de l'application**, dans le cadre d'une tâche explicitement demandée.
- **Codex : STRICTEMENT LECTURE SEULE.** Examiner, analyser, auditer, suggérer des correctifs sous forme de rapport texte dans la conversation. Ne modifier aucun fichier, ne lancer aucune commande qui change le dépôt, n'appliquer aucun patch, ne créer aucun commit. Ne pas proposer de corrections en les appliquant directement.
- ChatGPT aide au cadrage, à la documentation et à la préparation des instructions ; il ne doit pas supposer qu'une modification locale a eu lieu sans confirmation.
- Pour un agent dont l'identité ou les droits sont ambigus, appliquer par défaut le rôle **lecture seule** jusqu'à instruction explicite.

## Principes de développement

1. Une brique cohérente et testable à la fois ; pas de refonte générale non sollicitée.
2. Lire le contexte avant toute modification ; annoncer le périmètre des fichiers impactés.
3. La version **Dev** ne doit jamais écrire dans les données réelles de la version **Stable**.
4. Aucun secret, document personnel, fichier de données réel, jeton ou information privée dans Git.
5. Pas d'IA, d'appel distant, de télémétrie ou de serveur comme dépendance du fonctionnement initial.
6. Valider les données à l'entrée ; considérer les notes et imports comme des données, jamais comme du code.
7. Mettre des tests adaptés à chaque brique et documenter le résultat réel, sans prétendre avoir testé ce qui ne l'a pas été.
8. Respecter l'accessibilité (clavier, focus visible, animations réduites) et la sauvegarde fiable.
9. Ne jamais exécuter `git push`, supprimer des branches, écraser une base, migrer les données réelles ou publier une release sans demande explicite.
10. Conclure une tâche de code avec : fichiers changés, comportement ajouté, tests exécutés et résultat, risques, prochaine étape.

En cas de conflit entre ces règles et une demande vague, **demander une précision avant une action destructive ou touchant aux données réelles**.
