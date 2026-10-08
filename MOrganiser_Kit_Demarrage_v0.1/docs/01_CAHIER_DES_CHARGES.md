# 01 — Cahier des charges fonctionnel (évolutif)

> Statuts : **Décidé** = orientation confirmée ; **Prévu** = retenu pour une phase future ; **À arbitrer** = non décidé. Ce fichier décrit des objectifs, **pas des fonctionnalités déjà codées**.

## A. Fenêtre Windows et cycle de vie — Décidé

- Lancement automatique à l'ouverture de session, paramétrable ; la fenêtre s'affiche normalement en **mode compact**.
- Fenêtre **non toujours-au-premier-plan**, pouvant être réduite et déplacée.
- Bouton « Agrandir » : tableau de bord étendu et espacé.
- Bouton `—` : minimiser dans la barre des tâches Windows.
- Bouton `×` : masquer la fenêtre tout en maintenant le programme actif dans la **zone de notification (System Tray)**.
- Icône de notification : rouvrir, capture rapide, paramètres, **Quitter réellement**.
- **Une seule instance** de l'application ; activation de la fenêtre existante si relancement.
- Rétablissement de la taille, du mode et des préférences d'affichage sans perte de données.
- Faible consommation en arrière-plan et sauvegarde automatique.

## B. Tableau de bord — Décidé

- Priorité au contenu des tâches et à la prochaine action, **pas** aux tuiles de compteurs.
- Deux sections simultanément visibles : **Mes tâches personnelles** et **Engagements externes**.
- Autres espaces contextuels : attente, échéances, capture, boîte « À organiser », informations financières utiles.
- Grand écran : sections lisibles, alignement rigoureux, espacement généreux ; sections repliables ou masquables ; personnalisation progressive.
- Petit écran : les mêmes données dans une disposition compacte et adaptée.
- **Champ de capture rapide visible immédiatement**, de préférence côte à côte avec la boîte « À organiser » en mode étendu.
- Les indicateurs restent discrets et ne remplacent pas les listes.

## C. Capture et boîte de réception — Décidé

- Texte libre ; saisie quasi instantanée ; possibilité alternative de fiche structurée.
- Une capture non classée entre dans une boîte « À organiser », **pas automatiquement dans les tâches**.
- L'élément peut ensuite devenir tâche, note, projet, objet d'inventaire, etc. sans perte de contenu.
- Un élément peut être conservé tel quel pour plus tard.
- Animation courte et utile : une carte part du champ et rejoint visuellement la boîte de réception ; ne confirmer l'arrivée qu'après sauvegarde réussie ; version sans animation selon préférence d'accessibilité.
- La capture restera à terme accessible depuis le téléphone, avec synchronisation (solution à décider).

## D. Tâches, sous-tâches et démarches — Décidé

- Créer, consulter, modifier, achever, classer, reporter, archiver ou supprimer avec garde-fous.
- Sous-tâches **imbriquées à plusieurs niveaux**. Différencier « enfant de » et « dépend de ».
- Vue par défaut d'une démarche : **prochaine action concrète**. Vue détaillée, arborescence et workflow horizontal sur demande.
- Statuts distincts : à faire, en cours, en attente, bloqué, terminé ; états complémentaires à définir plus tard.
- Une tâche peut avoir une échéance, une priorité, des notes, un contexte, des éléments nécessaires, un temps facultatif et des associations.
- Tri configurable : priorité, récent, ancien, courte durée estimée, aléatoire, ou règles personnelles. Échéances critiques toujours signalées, même en mode aléatoire.
- Le hasard ne doit pas détourner l'attention des tâches importantes ; un « À ne pas manquer » distinct reste visible.
- Les tâches terminées quittent les vues actives mais demeurent dans l'historique, avec liens et contexte.
- Ne pas assimiler la durée entre création et clôture au temps de travail. Suivi du temps actif seulement si explicitement renseigné/mesuré.

## E. Univers et personnes — Décidé

- **Moi** : ses propres démarches, besoins, finances, activités et projets.
- **Externe** : tâches pour parents, famille, amis et autres personnes/organisations.
- Les obligations personnelles envers autrui (ex. une dette à rembourser) peuvent figurer dans **Moi → Finances**, tout en étant reliées à une personne.
- Répertoire minimal de personnes ; chaque fiche rassemble les liens vers tâches, notes et opérations concernés.
- Une base commune évite les doublons ; les espaces sont des **vues et filtres de responsabilité**.
- Mes responsabilités personnelles reçoivent une priorité visuelle à importance comparable ; les urgences externes demeurent visibles.

## F. Inventaire — Prévu

- Catégories et sous-catégories modifiables (mangas, jeux, consoles, chaussures, matériel, etc.).
- Collections et ensembles ; propriétés générales + propriétés adaptées au type d'objet, principalement facultatives.
- État, quantité, emplacement, achat, valeur, garantie, photos et statut (conserver, réparer, vendre, donner, remplacer), si utiles.
- Une tâche « vendre une console » peut proposer une association locale à un objet existant ; validation utilisateur avant modification d'état ou suppression.
- Pas de duplication entre une collection de loisirs, la liste des tâches et l'inventaire.

## G. Notes et documents — Prévu, détails ouverts

- Approche **notes et connaissances d'abord**, plutôt qu'un clone de Drive.
- Notes riches, listes, tableaux et relations entre personnes, dossiers, tâches, finances et projets.
- Pièces jointes facultatives, liens locaux, import glisser-déposer envisageable mais non imposé.
- Recherche globale et consultation par contexte. Politique des copies vs liens à préciser.

## H. Finances — Prévu

- Dépenses ponctuelles/récurrentes, abonnements, dettes, remboursements, échéances, montants payés et restant à payer.
- Catégories/sous-catégories libres ; liens avec personnes, documents, objets et tâches.
- Vues mensuelles, hebdomadaires, annuelles ; tableaux et graphiques ; export de données.
- Distinguer dépenses prévues, engagées et effectivement payées. Pas de connexion bancaire obligatoire.

## I. Projets et objectifs — Prévu

- Répertoire de projets, même non démarrés ; notes de contexte, objectifs, étapes, prochaine action, progression et pause.
- Une tâche peut appartenir à un projet tout en apparaissant dans les vues globales.

## J. Vie & Loisirs — Idée retenue pour plus tard

- Sport, lecture, films/séries, jeux, activités ; statuts « à découvrir / en cours / terminé » selon catégories.
- Possibilité future de données enrichies via API (saisons/épisodes), jamais une dépendance initiale.

## K. Statistiques et mémoire — Décidé pour l'orientation, prévu par étapes

- Historique exploitable : créations, achèvements, échéances, attentes, changements d'état, opérations financières.
- Indicateurs utiles : tâches réellement achevées par semaine/mois, engagements en attente, remboursements réglés, dépenses récurrentes, projets en progression.
- Pas de métriques trompeuses (« cette tâche a duré 60 jours de travail » alors qu'elle est restée ouverte 60 jours).
- Mesures pertinentes uniquement ; éviter la gamification culpabilisante.

## L. IA et téléphone — Futur

- IA **après** un cœur autonome mature ; suggestions non intrusives et facultatives, avec contrôle strict de l'accès aux données.
- Capture rapide mobile assez tôt dans la feuille de route ; synchronisation locale ou chiffrée à décider, avec prévention des conflits et protection des données.

## M. Découverte et aide — Décidé

- Première ouverture : parcours initiatique optionnel dans **la véritable interface**, focus progressif sur les vrais composants.
- Exemple guidé ou saisie libre ; exemple ne doit pas polluer les données réelles sans consentement.
- Passer, reprendre et rejouer un parcours ; centre de découverte par fonctionnalité.
- Chaque brique documentée avec son scénario pédagogique.
