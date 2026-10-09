# 05 — Feuille de route par briques

**Méthode : construire, tester, comprendre, documenter, valider, puis seulement passer à la brique suivante.** Les numéros identifient les capacités, non les dates ni des livraisons garanties.

## Étape 0 — Fondations (implémentée le 2026-10-08, validation du propriétaire en attente — voir `docs/briques/BRIQUE_000_FONDATIONS.md`)

- Valider la pile technologique et ses compromis.
- Créer squelette exécutable Windows, fenêtre de base, environnement de test, conventions et stratégie de stockage local.
- Séparer Dev / Stable / données ; préparer lancement local et gestion Git.
- **Critère :** l'application démarre dans Dev sans réseau, se ferme proprement et une commande de test reproductible fonctionne.

## Brique 001 — Capture rapide + boîte « À organiser »

- Champ libre visible, ajout, affichage dans boîte de réception, persistance locale, redémarrage sans perte, premiers tests.
- Animation et sophistication visuelle **après fiabilisation** de l'enregistrement.
- **Critère :** une capture créée reste visible après fermeture/réouverture, sans être automatiquement classée comme tâche.
- Voir `docs/briques/BRIQUE_001_CAPTURE_RAPIDE.md`.

## Brique 002 — Vie d'un élément capturé

- Consulter, éditer, classer/convertir en tâche, annuler, supprimer avec garde-fous ; conserver la source.
- **Critère :** aucune perte de texte pendant la conversion.

## Brique 003 — Actions de base et historique

- Création directe de tâche, statuts, terminer, archivage, tri simple, sauvegarde, journal des changements utiles.
- **Critère :** terminer n'efface jamais la tâche ni ses liens.

## Brique 004 — Univers Moi / Externe

- Affectation de responsabilité, contacts simples, vues séparées et tableau de bord montrant les deux sections.
- **Critère :** même tâche visible dans les filtres appropriés, sans duplication.

## Brique 005 — Sous-tâches et prochaine action

- Hiérarchie, dépendances, affichage de la prochaine action, mode liste/arborescence puis workflow.
- **Critère :** relation parent/enfant et dépendance ne sont pas confondues.

## Brique 006 — Tableau de bord, fenêtre compact/étendu et tray

- Sections réorganisées selon taille, options de visibilité, classement configurable, démarrage Windows, réduction/minimisation, tray et instance unique.
- **Critère :** `×` masque et ne quitte pas, `—` minimise, Quitter termine ; réglages mémorisés.
- Le **squelette** de fenêtre existe dès l'étape 0, mais ses comportements avancés peuvent être intégrés progressivement.

## Brique 007 — Initiation interactive

- Spotlight sur composants réels, étapes pédagogiques déclaratives, reprise, passage, réduction des animations, données de démonstration séparées.
- **Critère :** la découverte ne bloque pas l'utilisateur et peut être rejouée.
- Préparer les métadonnées d'initiation dès chaque brique, même si le moteur complet vient plus tard.

## Extensions par phases (ordre à réévaluer)

- Notes / documents ; inventaire ; finances ; projets ; statistiques ; capture mobile et synchronisation ; Vie & Loisirs ; IA facultative en dernier.
- **La capture mobile doit arriver relativement tôt** quand les fondations de synchronisation et de sécurité sont prêtes, et non nécessairement à la fin de toutes les extensions.

## Validation de chaque brique

- Démo réelle sur Windows ; tests et résultats listés ; aucune régression connue dans les fonctions existantes ; revue sécurité proportionnée ; fiche de brique et journal mis à jour ; accord du propriétaire avant de passer à la suite.
