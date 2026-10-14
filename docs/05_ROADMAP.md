# 05 — Feuille de route par briques

**Méthode : construire, tester, comprendre, documenter, valider, puis seulement passer à la brique suivante.** Les numéros identifient les capacités, non les dates ni des livraisons garanties.

## Étape 0 — Fondations (implémentée le 2026-10-08, validation du propriétaire en attente — voir `docs/briques/BRIQUE_000_FONDATIONS.md`)

- Valider la pile technologique et ses compromis.
- Créer squelette exécutable Windows, fenêtre de base, environnement de test, conventions et stratégie de stockage local.
- Séparer Dev / Stable / données ; préparer lancement local et gestion Git.
- **Critère :** l'application démarre dans Dev sans réseau, se ferme proprement et une commande de test reproductible fonctionne.

## Brique 001 — Capture rapide + boîte « À organiser » (découpée en sous-étapes validées une à une)

- **001-A** : boîte au-dessus du champ, saisie immédiate, destination facultative (pas un tag) et filtre, SQLite locale, relecture après relance, erreurs, tests.
- **001-A à 001-E : validées et fusionnées dans `main` (PR #2 à #6).**
- **001-B** : fiche, modification avec protection du brouillon et verrou optimiste, corbeille récupérable, « Voir tout » paginé.
- **001-C** : animation ascendante, mouvement réduit.
- **001-D** (facultatif) : option manuelle « Conserver ce choix » pour garder la destination (suggestion automatique abandonnée).
- **001-E** : initiation minimale sur le vrai champ et la vraie boîte ; proposition unique à la première utilisation, bouton « Découvrir » permanent.
- **Critère :** une capture créée reste visible après fermeture/réouverture, sans être automatiquement classée comme tâche.
- Voir `docs/briques/BRIQUE_001_CAPTURE_RAPIDE.md` (référence unique).

## Brique 002 — Vie d'un élément capturé

- **Périmètre décidé le 2026-10-10 :** convertir une capture en **tâche** uniquement (notes et projets attendent leurs modules) ; conserver la source ; annulation (4 s) ; vue « Traitées ». L'édition et la suppression élémentaires d'une capture sont traitées en 001-B.
- **002-A** *(validée et fusionnée, PR #7)* : migration 0002, tâche minimale, conversion et annulation atomiques, sauvegarde préalable aux migrations, commandes Rust. **002-B** *(validée et fusionnée, PR #8)* : interface de conversion et première liste des tâches. **002-C** *(validée et fusionnée, PR #9)* : vue « Traitées » (captures actuellement converties), navigation tâche ↔ capture, « Remettre dans la boîte » contrôlé.
- **Critère :** aucune perte de texte pendant la conversion.
- Voir `docs/briques/BRIQUE_002_VIE_ELEMENT_CAPTURE.md` (référence unique).

## Brique 003 — Fiche de tâche enrichie, actions de base et historique

*Cadrage 003-0 du 2026-10-14 (documentation seulement ; aucune implémentation). Référence unique : `docs/briques/BRIQUE_003_FICHE_TACHE_ENRICHIE.md`.*

- **Périmètre (VALIDÉ) :** fiche de tâche enrichie ; **grande modale immersive** ouverte directement par « Transformer en tâche », sous forme de brouillon (aucune tâche créée avant validation explicite) ; édition ; dates ; statuts ; historique des modifications utiles ; comportement des captures transformées (conservées discrètement tant que la tâche est active, masquées des vues et archives visibles quand elle est terminée, sans suppression physique).
- Reste de l'ancien périmètre : création directe de tâche (à confirmer, Q13), terminer, archivage, tri simple, sauvegarde, journal des changements utiles.
- **Sous-briques (ordre Q1 validé) :** 003-A modale et conversion par brouillon, sans migration *(implémentée ; validation manuelle positive du propriétaire sur Dev le 2026-10-14 ; intégration Git en attente)* ; 003-B modèle enrichi, migration 0003 et édition ; 003-C cycle de vie, dates effectives et historique ; 003-D archivage discret des captures et finitions.
- **Critère :** terminer n'efface jamais la tâche ni ses liens ; abandonner un brouillon ne laisse aucune trace ; aucune perte de texte.
- **Hors 003 :** tableau de bord, Ma synthèse, relations, modules Finance / Administratif / Inventaire, calendrier externe.

## Brique 004 — Univers Moi / Externe

- Affectation de responsabilité, contacts simples, vues séparées et tableau de bord montrant les deux sections.
- **Précisé le 2026-10-14 :** les univers forment un **filtre transversal Tous / Moi / Externe** (ni deux colonnes permanentes, ni lien avec la priorité) ; « tableau de bord montrant les deux sections » est remplacé par `docs/10_TABLEAU_DE_BORD.md`. Si le modèle à deux dimensions (univers + rubrique) n'est pas posé en 003-B, il l'est ici (À DÉCIDER).
- **Critère :** même tâche visible dans les filtres appropriés, sans duplication.

## Brique 005 — Sous-tâches et prochaine action

- Hiérarchie, dépendances, affichage de la prochaine action, mode liste/arborescence puis workflow.
- **Critère :** relation parent/enfant et dépendance ne sont pas confondues.

## Brique 006 — Tableau de bord, fenêtre compact/étendu et tray

- Sections réorganisées selon taille, options de visibilité, classement configurable, démarrage Windows, réduction/minimisation, tray et instance unique.
- **Précisé le 2026-10-14 (spécification : `docs/10_TABLEAU_DE_BORD.md`, rien n'est développé) :** accueil complet modulaire intégrant **Capture & Idées**, **Ma synthèse** (règles déterministes et explicables), **cartes de rubriques** personnalisables, filtres transversaux, **deux présentations** (Épurée / Connectée) sur les mêmes données ; mode compact distinct (capture rapide et éléments à traiter).
- **Dépendances :** brique 003 (dates, statuts, priorité, historique) et brique 004 (univers). Les cartes spécialisées attendent leurs modules ; la présentation Connectée ne trace que des relations réelles.
- **Critère :** `×` masque et ne quitte pas, `—` minimise, Quitter termine ; réglages mémorisés.
- Le **squelette** de fenêtre existe dès l'étape 0, mais ses comportements avancés peuvent être intégrés progressivement.

## Brique 007 — Initiation interactive

- Spotlight sur composants réels, étapes pédagogiques déclaratives, reprise, passage, réduction des animations, données de démonstration séparées.
- **Critère :** la découverte ne bloque pas l'utilisateur et peut être rejouée.
- Préparer les métadonnées d'initiation dès chaque brique, même si le moteur complet vient plus tard.

## Extensions par phases (ordre à réévaluer)

- Notes / documents ; inventaire ; finances ; projets ; statistiques ; capture mobile et synchronisation ; Vie & Loisirs ; IA facultative en dernier.
- **Ajouts du 2026-10-14 (numéros à attribuer, non numérotés pour ne pas bousculer la numérotation existante) :**
  - **Relations / Connexions** : relations bidirectionnelles entre éléments, puis vue « Connexions » (exploration graphique d'après des relations réelles). Principe : `docs/03_ARCHITECTURE.md`. Hors brique 003. Doit précéder les liens réels de la présentation Connectée et le rattachement tâche ↔ objet.
  - **Modules spécialisés** (Administratif, Finances, Inventaire, puis d'autres rubriques) : chacun avec ses entités propres, introduits progressivement ; un module n'est pas une extension de la tâche.
  - **Calendrier** : agenda éventuel, puis **calendrier externe** (fournisseur réel à identifier, synchronisation bidirectionnelle) : vision future, étude technique et de sécurité dédiée préalable (`docs/01_CAHIER_DES_CHARGES.md` §N).
- **Ordre logique :** 003 → 004 → (005) → 006 ; relations avant liens réels et cartes liées ; chaque module avant sa carte.
- **La capture mobile doit arriver relativement tôt** quand les fondations de synchronisation et de sécurité sont prêtes, et non nécessairement à la fin de toutes les extensions.

## Validation de chaque brique

- Démo réelle sur Windows ; tests et résultats listés ; aucune régression connue dans les fonctions existantes ; revue sécurité proportionnée ; fiche de brique et journal mis à jour ; accord du propriétaire avant de passer à la suite.
