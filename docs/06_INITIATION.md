# 06 — Initiation interactive dans la véritable application

## Intention

L'apprentissage doit ressembler à un jeu de découverte, **sans gamification obligatoire**. L'utilisateur agit sur les vrais boutons, champs et listes du logiciel ; un focus visuel guide son regard en douceur de gauche à droite, de haut en bas et entre les modules.

## Principes

- Première ouverture : proposer « Découvrir M'Organiser », avec **Passer** toujours accessible.
- Surbrillance (« spotlight ») d'un composant réel ; le reste est légèrement atténué ; déplacement fluide du focus si possible.
- Étapes petites et actives : saisir → voir la capture arriver → consulter → convertir en tâche → ajouter une sous-tâche → relier un objet quand l'inventaire existe.
- Pas de faux succès ; une étape avance lorsque l'action de l'utilisateur a **réussi**.
- Parcours rejouables depuis « Centre de découverte » / Paramètres ; sous-parcours par brique, y compris après mise à jour.
- Toujours laisser accéder directement aux fonctionnalités complètes ; aucune fonction verrouillée si le tutoriel est ignoré.
- Données de démonstration isolées ou exemples explicitement conservables ; ne pas créer une fausse dette ou un objet dans l'inventaire réel sans consentement.
- Clavier, lecteur d'écran, focus visible, réduction du mouvement et commandes « Précédent », « Passer » et « Reprendre ».

## Métadonnées à prévoir pour les briques

Chaque étape dispose conceptuellement : `id`, `feature_id`, `title`, `instruction`, `target_component_id`, `required_action`, `success_condition`, `next_step`, `skippable`, `demo_data_policy`.

L'implémentation exacte (JSON, code typé, base locale) sera décidée avec la pile technique. Il n'est **pas nécessaire de construire tout le moteur** avec la première brique ; commencer par une instruction contextualisée, documenter la suite.

## Parcours initial indicatif

1. Bienvenue : expliquer l'esprit « capturer, organiser, agir ».
2. Focus sur capture rapide : proposer « Vendre ma PlayStation 5 » ou saisie libre.
3. Animation de la carte vers « À organiser » ; constater son arrivée.
4. Focus sur l'élément ; ouvrir et convertir en tâche.
5. Ajouter une petite étape « Prendre une photo » lorsque les sous-tâches existent.
6. Focus sur l'inventaire plus tard ; relier une console existante ou exemple isolé.
7. Montrer le tableau de bord, Moi/Externe, et la possibilité de rejouer le parcours.

Première mise en œuvre prévue : sous-brique **001-E** (spotlight sur le vrai champ de capture et la vraie boîte « À organiser », « Passer » disponible), sans le moteur complet.

**Réalisé en 001-E (2026-10-10) :** proposition unique à la première utilisation (mémorisée dès son affichage), bouton « Découvrir » permanent dans l'en-tête, parcours capture → boîte (« Terminer » à la dernière étape), étapes avancées seulement sur un enregistrement réussi, aucune donnée de démonstration. Mémoire : `localStorage` (`morganiser.initiation.v1`, rangé par parcours). Pas encore : reprise d'un parcours interrompu, parcours thématiques, progression par parcours, Centre de découverte — brique 007. Détail et limites : fiche brique 001, section 14.

## Quand une fonctionnalité n'existe pas encore

Ne pas l'afficher comme disponible ; le parcours actif ne référence que les briques implémentées et vérifiées. Les nouvelles séquences sont ajoutées après chaque version stable.
