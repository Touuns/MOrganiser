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

## Commandes Rust et permissions (Tauri 2)

**Frontière de sécurité actuelle (vérifiée le 2026-10-08 dans le code source de Tauri 2.12.1).**

- Sans manifeste d'application, Tauri **n'applique pas** le contrôle d'accès aux commandes de l'application pour le contenu local : une liste `permissions` vide ne bloque rien. Seul le contenu distant est bloqué.
- Depuis la brique 000, un manifeste est déclaré dans `src-tauri/build.rs` (`APP_COMMANDS`). Le contrôle d'accès s'applique alors à **toutes** les commandes : une commande absente du manifeste, ou non autorisée dans `src-tauri/capabilities/default.json`, est refusée.
- État actuel (brique 001-B) : neuf commandes, chacune autorisée nommément pour la seule fenêtre `main` : `app_info`, `list_destinations`, `list_inbox_items` et `list_trashed_items` (lectures paginées, limite plafonnée à 200), `get_inbox_item` (lecture), `create_inbox_item`, `update_inbox_item` (verrou optimiste), `trash_inbox_item` et `restore_inbox_item` (corbeille logique, aucune suppression définitive). Toutes valident leurs arguments côté Rust. Trois permissions système, minimales, pour la protection à la fermeture : `core:event:allow-listen`, `core:event:allow-unlisten` et `core:window:allow-destroy` (aucune extension ni `core:default`). Le test `chaque_commande_est_declaree_autorisee_et_enregistree` vérifie la cohérence `build.rs` ↔ capacité ↔ `lib.rs`.
- Vérifié en réel : en retirant `allow-app-info`, l'appel est refusé (« app_info not allowed »).
- La CSP interdit tout chargement ou appel externe. La frontière à défendre est donc le passage interface → Rust : tout ce qui arrive par une commande est une **donnée non fiable**.

**Politique pour les futures commandes**

1. Chaque commande est ajoutée à `APP_COMMANDS` **et** autorisée explicitement (`allow-<commande>`) dans la capacité, pour les seules fenêtres qui en ont besoin. Ni joker, ni capacité `remote`.
2. Une commande correspond à une intention métier précise (ex. « ajouter une capture »). Interdit : les commandes génériques (exécuter du SQL, lire ou écrire un chemin arbitraire, lancer un programme).
3. L'interface ne fournit jamais de chemin de fichier ni de requête : Rust choisit les emplacements dans le dossier de données de l'environnement courant.
4. Arguments typés et validés côté Rust (longueur, format, valeurs autorisées), même si l'interface valide déjà. Requêtes SQLite toujours paramétrées.
5. Les réponses ne renvoient que ce dont l'écran a besoin ; les messages d'erreur ne contiennent pas le contenu des notes.
6. Toute permission système ou extension (tray, démarrage automatique, dialogues…) est justifiée dans la fiche de la brique qui l'introduit, avec la permission la plus étroite disponible.
7. Le test `la_capacite_n_accorde_que_les_permissions_revues` (`src-tauri/src/config_tests.rs`) doit être mis à jour en même temps, ce qui rend chaque ajout visible en revue.

## Liens et jonctions Windows (risque faible, documenté)

Un lien symbolique ou une **jonction** NTFS placé à `%LOCALAPPDATA%\com.morganiser.desktop.dev` (ou sur son sous-dossier `data`) pourrait rediriger silencieusement les écritures de Dev vers le dossier de Stable. Le code compare des chemins textuels ; il ne résout pas les redirections. Créer une jonction exige d'agir volontairement sur le compte : ce risque concerne surtout une erreur de manipulation ou un poste déjà compromis.

Aucun mécanisme n'est ajouté à ce stade. **Avant de stocker des données personnelles réelles dans Stable :**

- vérifier manuellement qu'aucun des deux dossiers n'est une redirection : `dir /AL "%LOCALAPPDATA%"` (invite de commandes) ne doit lister aucun `com.morganiser.*`, ni rien dans ces dossiers ;
- envisager une vérification au démarrage : refuser un dossier de données qui est un point d'analyse (« reparse point »), et comparer les chemins **canoniques** de Dev et Stable ;
- ne jamais « partager » les données entre Dev et Stable par un lien : copier une sauvegarde dans Dev.

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
