# 02 — Direction UI/UX

## Identité

- Direction **centre de contrôle personnel** avec touches analytiques. Obsidian/notes en référence secondaire, **pas** en modèle d'accueil.
- Moderne, accueillant, lisible, professionnel ; jamais une interface « IA générique » faite d'énormes cartes et de dégradés décoratifs.
- Alignements, typographie, contrastes et espacement consistants. Icônes compréhensibles, animations rares et motivées par un déplacement réel d'information.
- Les modules peuvent avoir des présentations spécialisées (inventaire illustré, tâches structurées, finances tabulaires) mais les interactions essentielles restent cohérentes.

> **Évolution décidée le 2026-10-14 (cadrage 003-0).** Le tableau de bord complet, ses deux présentations (Épurée / Connectée), « Ma synthèse », l'espace « Capture & Idées » et les cartes de rubriques sont spécifiés dans `10_TABLEAU_DE_BORD.md`, qui fait foi pour l'accueil complet. Les sections « Accueil compact » et « Tableau de bord étendu » ci-dessous sont **conservées pour l'historique** et annotées là où elles évoluent. Rien de cela n'est développé : l'accueil actuel reste le prototype de la brique 001.

## Accueil compact au démarrage

- Fenêtre compacte Windows affichée à l'ouverture de session ; zone de capture directement visible.
- Liste personnelle : 2–5 prochaines actions pertinentes, éventuellement un sous-ensemble des engagements externes et alertes. *(À faire évoluer : le compact privilégie désormais la capture rapide et l'accès aux éléments à traiter ; la présence de prochaines actions est À DÉCIDER — `10_TABLEAU_DE_BORD.md`, section 10.)*
- Accès immédiat au mode étendu et aux modules ; ne pas occuper inutilement l'écran.

## Tableau de bord étendu

Ordre de lecture suggéré, à tester sur prototype :

1. Bandeau discret : recherche, contexte, paramètres.
2. **Boîte « À organiser » juste au-dessus de la capture rapide** (ensemble vertical ; décision du 2026-10-10, remplace « côte à côte »). Sous le champ : « Destination (facultatif) », jamais appelé « Tags ».
3. **Mes tâches personnelles** : vraies tâches identifiables, statuts et prochaine action.
4. **Engagements externes** : vraies tâches distinctes.
   *(Points 3 et 4 précisés le 2026-10-14 : Moi / Externe forment un filtre transversal Tous / Moi / Externe, pas deux colonnes permanentes ; voir `10_TABLEAU_DE_BORD.md`, section 4.)*
5. En attente / échéances critiques et informations utiles ; statistiques discrètes.
6. Accès aux modules : tâches, personnes, inventaire, notes, finances, projets et loisirs futur.

En fenêtre moins large, réorganiser les sections en colonnes verticales sans perdre le champ de capture.

## Navigation

- Une navigation latérale stable, compacte et éventuellement repliable.
- Les espaces « Moi » et « Externe » filtrent un socle commun ; les modules ouvrent des vues spécialisées.
- Sections de tableau de bord repliables/masquables, réglages mémorisés.
- Deux styles de création : saisie libre instantanée et formulaire volontairement ouvert à la demande.
- **Grande fiche modale (décidé le 2026-10-14, brique 003) :** la fiche complète d'une tâche s'ouvre dans une grande fenêtre modale centrée, avec arrière-plan assombri et légèrement flouté, et non comme une page qui remplace l'application. Ce n'est pas un « popup permanent » : elle est ouverte volontairement et se ferme à tout moment, sous réserve de la protection du brouillon. Spécification : `briques/BRIQUE_003_FICHE_TACHE_ENRICHIE.md`, section 5.

## Animation de capture (spécification d'intention)

1. Saisie et validation.
2. Persistance locale réussie (ou indication de traitement pending sans faux succès).
3. Naissance visuelle d'une petite **carte** près du champ.
4. Trajectoire fluide, **ascendante**, vers la boîte « À organiser » située au-dessus ; insertion **en bas de la liste** (ordre chronologique, la plus récente au plus près du champ) (sous-brique 001-C).
5. Boîte mise à jour, focus rendu au champ.

Animation discrète et rapide ; bloquer les doubles envois ; ne pas faire disparaître une saisie non sauvegardée. `reduced motion` : transition instantanée sans trajectoire. Les contraintes de performance et de clavier priment sur l'effet.

## Confort / accessibilité

- Clavier utilisable ; focus visible ; navigation prévisible ; support des préférences de mouvement réduit ; contrastes suffisants.
- Aucun popup permanent, aucune fenêtre épinglée au premier plan.
- Raccourcis clavier futurs pour capture rapide depuis le tray ou globalement, avec traitement des conflits.
- Journal et statistiques sans injonctions culpabilisantes ; tâche terminée archivée discrètement, non effacée.

## Prototype et validation utilisateur

Avant de figer un composant, tester en mode compact, fenêtre normale et grand écran. Vérifier la lisibilité des vraies tâches, la densité, les menus masqués, les animations et la reprise de travail. **Ne pas confondre un schéma documentaire avec une maquette validée.**
