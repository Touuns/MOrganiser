# Brique 001 — Capture rapide et boîte « À organiser »

**État : spécifiée, pas encore implémentée.**  
**Dépendance : étape 0 — fondations + choix de technologie.**

## Besoin utilisateur

« J'ouvre M'Organiser et je vois un champ. J'écris ce qui me passe par la tête, je valide et cela arrive dans une boîte de réception juste à côté ou à proximité. Je peux le retrouver après redémarrage. »

## Portée minimale

- Champ de saisie visible sans ouvrir de dialogue additionnel.
- Validation par Entrée ou bouton (comportement multiligne à convenir ; ne jamais sacrifier l'accessibilité).
- Une capture possède un identifiant stable, un contenu non vide et un horodatage local.
- Persistance locale ; affichage immédiat dans la boîte « À organiser » ; ordre récent d'abord initialement.
- Un échec de sauvegarde laisse le texte récupérable et affiche une erreur claire.
- Le contenu est du **texte**, non du code à exécuter, non interprété par IA.

## Enrichissements après preuve de fonctionnement

- Carte animée entre capture et boîte ; mouvement discret et option réduit/sans animation.
- Capture rapide depuis tray, raccourcis, conversion en tâche, édition/suppression : briques ultérieures, sauf arbitrage explicite.

## Tests manuels d'acceptation

1. Ouvrir Dev. Le champ de capture est immédiatement visible.
2. Saisir « Vendre ma PlayStation 5 » puis valider : apparaît **une fois** dans « À organiser ».
3. Fermer puis relancer : l'élément est toujours présent.
4. Tenter une capture vide : rien n'est ajouté.
5. Entrer un texte contenant apostrophes, accents, caractères spéciaux et `'; DROP TABLE ...` : il est affiché comme texte sans exécution.
6. Simuler une erreur d'enregistrement : le contenu du champ n'est pas perdu.
7. Vérifier que Dev n'écrit pas dans les données de Stable.

## Mini-parcours pédagogique lié

Focus sur le vrai champ, exemple proposé (« Vendre ma PlayStation 5 ») ou texte libre ; validation ; pointer la boîte d'arrivée ; inviter à consulter la capture. Ne pas polluer des données réelles par une démo fictive sans consentement.

## Compte rendu exigé après implémentation

Fichiers créés/modifiés, fonctionnement de la persistance, tests réellement passés/échoués, limites restantes, explication accessible au propriétaire.
