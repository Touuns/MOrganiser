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
  > **Précisé le 2026-10-14 (cadrage 003-0) :** Moi et Externe ne sont **pas** deux colonnes permanentes ; ce sont un filtre transversal **Tous / Moi / Externe** (priorité indépendante de l'univers). Voir `10_TABLEAU_DE_BORD.md`, sections 2 et 4.
- Autres espaces contextuels : attente, échéances, capture, boîte « À organiser », informations financières utiles.
- Grand écran : sections lisibles, alignement rigoureux, espacement généreux ; sections repliables ou masquables ; personnalisation progressive.
- Petit écran : les mêmes données dans une disposition compacte et adaptée.
- **Champ de capture rapide visible immédiatement**, avec la boîte « À organiser » placée **juste au-dessus** (décision du 2026-10-10, remplace « côte à côte »).
- Les indicateurs restent discrets et ne remplacent pas les listes.

## C. Capture et boîte de réception — Décidé

- Texte libre ; saisie quasi instantanée ; possibilité alternative de fiche structurée.
- Une capture non classée entre dans une boîte « À organiser », **pas automatiquement dans les tâches**.
- **Destination facultative unique** (ce n'est **pas** un tag) : l'espace de responsabilité (Moi, Externe) ou la rubrique (Administratif, Finances, Inventaire) où l'élément sera traité. La capture reste dans « À organiser », filtrable par destination ; un seul enregistrement, jamais de copie. Détails : `briques/BRIQUE_001_CAPTURE_RAPIDE.md`.
- L'élément peut ensuite devenir tâche, note, projet, objet d'inventaire, etc. sans perte de contenu.
- **Capture & Idées (précisé le 2026-10-14)** : espace commun qui regroupe la capture rapide, les informations en attente de traitement et l'accès aux captures récentes ; une idée non traitée n'est **pas** une rubrique métier, et une capture n'est pas toujours une « idée ». Une capture transformée quitte la liste des éléments à traiter ; à la clôture de sa tâche, elle disparaît des vues courantes sans suppression physique. Détail : `10_TABLEAU_DE_BORD.md` section 3 et `briques/BRIQUE_003_FICHE_TACHE_ENRICHIE.md` section 7.
- Un élément peut être conservé tel quel pour plus tard.
- Animation courte et utile : une carte part du champ et **monte** rejoindre la boîte de réception située au-dessus ; ne confirmer l'arrivée qu'après sauvegarde réussie ; version sans animation selon préférence d'accessibilité.
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
- **Précisé le 2026-10-14 :** « Moi » n'implique pas une priorité élevée, « Externe » n'implique pas une priorité faible ; la priorité d'une tâche est une information distincte de l'univers.

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

## N. Calendrier externe — Vision future, HORS PÉRIMÈTRE de la brique 003

> Statuts : **VALIDÉ** = volonté confirmée par le propriétaire ; **À DÉCIDER** = nécessite une étude technique dédiée ; **HORS PÉRIMÈTRE** = non développé. Rien ici n'est implémenté.

**VALIDÉ (volonté) :** intégrer d'abord le **calendrier utilisé sur l'iPhone du propriétaire**, en identifiant le **fournisseur réel** (iCloud, Google, Microsoft ou autre) avant tout choix technique.

**Synchronisation souhaitée, bidirectionnelle :**

- consultation des événements ;
- création et modification ;
- suppressions ;
- fonctionnement hors ligne ;
- détection des conflits, **sans écrasement silencieux** ;
- gestion des doublons ;
- confidentialité et sécurité des accès.

**À DÉCIDER (étude dédiée) :** fournisseur réel, protocole, méthode d'authentification, stockage des identifiants, politique de conflit, périmètre des données synchronisées, éventuel agenda interne préalable. **Aucun protocole ni mode d'authentification n'est choisi à ce stade.**

**Contraintes à lever avant tout développement :**

- le fonctionnement initial est **sans réseau** et la politique de contenu interdit tout appel externe (`04_SECURITE.md`) : une telle fonction serait **facultative, explicitement activée par l'utilisateur** et accompagnée d'une revue de sécurité ;
- elle ne relève pas de la brique 003 ; elle alimentera plus tard la zone « événements et échéances » du tableau de bord (`10_TABLEAU_DE_BORD.md`, section 9).
