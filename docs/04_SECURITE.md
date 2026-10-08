# 04 — Sécurité et confidentialité

## Modèle de menace pragmatique

Menaces prioritaires : perte de fichiers, corruption de base, modification accidentelle, accès par un autre compte local, paquet compromis, injection via saisies/imports, mise à jour non fiable, fuite accidentelle dans Git. Le risque d'un Windows déjà compromis ne peut pas être annulé par un simple mot de passe d'application.

## Règles initiales

- Données **locales** ; aucune télémétrie ni appel réseau nécessaire au fonctionnement initial.
- Répertoire de données privé dans le profil Windows, droits adaptés ; fichiers de code et données séparés.
- Requêtes de base paramétrées, validation des champs et des chemins ; aucune exécution de texte utilisateur en tant que commande ou code.
- Logs minimisés et nettoyés des données sensibles ; aucune trace complète des notes dans les rapports de bug.
- Dépendances connues et versions contrôlées, vérifications de vulnérabilités ; distribution et mises à jour vérifiables.
- Sauvegardes locales cohérentes **testées en restauration** ; procédure de récupération documentée.
- Dev et Stable isolés ; n'utiliser que des données d'essai dans les tests et les tutoriels.
- `.gitignore` est un filet de sécurité, **pas** une garantie : examiner les fichiers avant commit et éviter tout secret réel dans le répertoire du dépôt.

## Verrouillage et chiffrement

- **Verrouillage automatique optionnel, désactivé par défaut.** Possibilité future de déverrouiller par mot de passe ou mécanisme Windows, après inactivité.
- Le verrouillage de l'interface ne remplace **pas** le chiffrement des données au repos.
- Chiffrement de base, sauvegardes et fichiers sensibles : décision à prendre avant stockage régulier de données confidentielles ; examiner protection Windows/DPAPI, chiffrement SQLite adapté et plan de récupération des clés.
- Éviter toute promesse « inviolable » ou « protection contre toutes les injections ».

## Développement et mise à jour

- Tests et revue de sécurité par brique ; **Codex en lecture seule**, Claude implémente les corrections validées.
- Une mise à jour ne doit pas écraser les données utilisateur ; avant migration, sauvegarder puis vérifier le chemin de restauration.
- Pas de `git push` automatique par les assistants ; ne pas publier de base `.db`, pièces jointes, token ou identité personnelle.
- Mobile et IA futurs : modèle de permission et de consentement explicite, chiffrement en transit, authentification des appareils et possibilité de révoquer.

## Avant une diffusion open source

Revue des secrets, des licences/dépendances, de la télémétrie, des valeurs de configuration, des scripts de release et des parcours de suppression/export des données.
