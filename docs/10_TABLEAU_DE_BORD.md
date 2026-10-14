# 10 — Tableau de bord, Capture & Idées, Ma synthèse (spécification UX)

**État : spécification de cadrage du 2026-10-14. Rien de ce document n'est développé.** L'accueil actuel reste le prototype « boîte « À organiser » au-dessus de la capture rapide » (brique 001).

**Brique cible :** la brique **006** de la roadmap (« Tableau de bord, fenêtre compact/étendu et tray »), qui hérite de ce contenu. Elle dépend des briques 003 et 004 (voir section 11).

**Légende** — **VALIDÉ** (décidé par le propriétaire) · **PROPOSÉ** (recommandation, en attente d'accord) · **À DÉCIDER** (arbitrage nécessaire) · **HORS PÉRIMÈTRE** (exclu pour l'instant).

**Règle générale (VALIDÉ) :** ne jamais afficher comme disponible une fonction qui n'existe pas. Une rubrique dont le module n'est pas développé n'affiche ni faux solde, ni graphique inventé, ni bouton sans effet.

## 1. Ce que ce document remplace ou précise

Les textes suivants restent dans leurs documents, annotés ; cette spécification fait foi pour l'accueil complet.

| Texte existant | Évolution | Statut |
|---|---|---|
| `00` principe 4, `01` §B, `02` « Tableau de bord étendu » : « Mes tâches personnelles » et « Engagements externes » en deux sections simultanées | Moi / Externe deviennent un **filtre transversal** (Tous / Moi / Externe), pas deux colonnes | VALIDÉ |
| `02` « Accueil compact » : « 2–5 prochaines actions » | Le compact privilégie la capture rapide et l'accès aux éléments à traiter | VALIDÉ ; présence de prochaines actions : À DÉCIDER |
| `02` « Tableau de bord étendu », ordre 2 à 6 | Remplacé par l'architecture de la section 2 | VALIDÉ ; ordre exact : À DÉCIDER |
| `01` §C : boîte « À organiser » | Intégrée à l'espace **Capture & Idées** ; le libellé de liste ne change pas tant qu'aucune brique d'interface ne le décide | VALIDÉ ; libellé : À DÉCIDER |

## 2. Architecture générale de l'accueil complet (VALIDÉ)

Le tableau de bord complet devient l'**accueil principal** lorsqu'il sera développé. Il est modulaire, lisible, personnalisable, cohérent visuellement, riche en informations pertinentes et exempt de surcharge.

1. **Navigation générale, recherche et filtres transversaux** (dont Tous / Moi / Externe).
2. **Capture & Idées** : l'espace d'entrée.
3. **Ma synthèse** : sélection contextuelle des éléments demandant de l'attention.
4. **Cartes des rubriques et domaines.**
5. **Événements et échéances** lorsque le calendrier sera disponible.

Le **mode compact** est distinct du tableau de bord complet : capture rapide et accès immédiat aux informations en attente de traitement.

## 3. Capture & Idées (VALIDÉ)

Espace commun regroupant : la capture rapide ; les idées et informations en attente de traitement ; l'accès aux captures récentes ; la transformation ultérieure en tâche, note, dossier, objet ou autre élément **lorsque les fonctionnalités correspondantes existent** (aujourd'hui : tâche uniquement).

Règles :

- Une idée non traitée **n'est pas une rubrique métier** équivalente à Finance, Administratif ou Inventaire. Il n'y a donc pas de carte « Idées » au niveau des rubriques par défaut.
- Une capture peut être une idée, une obligation, une note ou une information brute : elle n'est **pas** artificiellement catégorisée « idée ».
- Une capture transformée quitte la liste des éléments à traiter, avec les garanties de conservation et de traçabilité de la brique 002.
- Quand la tâche issue d'une capture est **terminée**, la capture historique disparaît des vues courantes et des archives visibles (aucune suppression physique) : voir `briques/BRIQUE_003_FICHE_TACHE_ENRICHIE.md`, section 7.

## 4. Univers de responsabilité (VALIDÉ)

- Trois filtres principaux : **Tous** (vue combinée), **Moi** (besoins, projets, obligations et priorités personnelles), **Externe** (services, démarches et engagements réalisés pour des tiers).
- Dimension **transversale** de classement et de filtrage ; ni colonnes permanentes ni sections figées.
- « Moi » n'implique pas une priorité élevée ; « Externe » n'implique pas une priorité faible ; **la priorité reste une information distincte**.
- Un même élément s'affiche dans les filtres appropriés **sans duplication physique** (critère déjà inscrit en brique 004).
- Mise en page d'un affichage « Tous » qui distingue visuellement Moi et Externe : **À DÉCIDER**.

## 5. Rubriques et cartes

### Principes (VALIDÉ)

- Administratif, Finance et Inventaire sont des **exemples**, pas une liste exhaustive ; d'autres rubriques pourront exister (Projets, Santé, Apprentissage, Maison, Voyages…).
- Rubrique, type d'élément et responsabilité sont des notions séparées (`03_ARCHITECTURE.md`) ; une tâche administrative peut appartenir à la fois à l'univers Moi et à la rubrique Administratif.
- Ajouter une rubrique ne demande pas de nouvelle architecture complète ; les vues ne créent pas de doublons physiques.
- Dossiers et sous-dossiers lorsque cela a du sens (ex. inventaire).

### Cartes

Cadre visuel **commun** (titre, état vide / indisponible / chargement, actions, taille), contenu **spécialisé** :

| Carte | Contenu envisagé | Dépend de |
|---|---|---|
| Administratif | Dossiers, démarches, échéances | Module Administratif |
| Finance | Tableaux, paiements, indicateurs, graphiques | Module Finance |
| Inventaire | Collections, objets, états, catégories | Module Inventaire |
| Projets | Progressions, jalons, prochaines actions | Module Projets |
| Santé | Rendez-vous, suivis | Module concerné + rendez-vous |
| Calendrier | Agenda, événements, échéances | Calendrier (section 9) |

Les cartes pourront être affichées ou masquées, réordonnées, adaptées à la disponibilité des informations et personnalisées progressivement ; leurs dimensions restent cohérentes avec leur contenu et les préférences (**VALIDÉ**).

À décider :

- liste des rubriques **activées par défaut** (**À DÉCIDER**, à étudier) ;
- harmonisation du nom « Finances » (libellé existant) et « Finance » (**À DÉCIDER**) ;
- traitement d'une carte dont le module n'existe pas : **PROPOSÉ** de ne pas l'afficher par défaut plutôt que d'afficher un état « bientôt » ;
- cartes réordonnables par glisser-déposer ou par commandes : **À DÉCIDER** (accessibilité clavier à garantir dans les deux cas).

## 6. Ma synthèse

### Nature (VALIDÉ)

Ma synthèse n'est pas la liste des dernières tâches : c'est un espace de **sélection contextuelle** des éléments demandant potentiellement une attention.

### Situations pertinentes (VALIDÉ)

| Situation | Donnée nécessaire | Disponible aujourd'hui ? |
|---|---|---|
| Tâche en retard | Échéance + statut | Non (brique 003) |
| Échéance proche | Échéance + délai d'anticipation | Non (003) |
| Tâche en cours | Statut | Non utilisé (003) |
| En attente d'une réponse | Statut « en attente » | Non utilisé (003) |
| Relance à envisager | Statut « en attente » + date de relance ou ancienneté | Non — **donnée À DÉCIDER** |
| Sans progression significative depuis une durée | Historique des événements + définition de « significatif » | Non (003, historique) |
| Explicitement prioritaire | Priorité | Non (003) |

Règles de fond (VALIDÉ) :

- une tâche **ancienne** n'est pas, en soi, urgente ;
- **ouvrir une fiche n'est pas une progression significative** ;
- quand rien ne nécessite d'intervention, **aucun faux signal** n'est généré : l'espace l'indique simplement (message neutre).

### Hiérarchisation

**VALIDÉ :** nombre raisonnable d'éléments ; ordre qui privilégie les échéances et les urgences explicites ; règles **déterministes, explicables, testables, configurables à terme** ; pas d'IA pour une fonction réalisable par des règles fiables ; pas de score d'urgence prétendument validé scientifiquement.

**PROPOSÉ :**

- ordre par **paliers successifs**, sans score numérique : 1) en retard ; 2) échéance proche ; 3) explicitement prioritaire ; 4) en attente d'une réponse / relance ; 5) sans progression depuis la durée choisie ; départage par échéance puis par date de création ;
- **chaque suggestion affiche sa raison** (« échéance dans 2 jours », « en attente depuis 14 jours ») pour que le classement reste contrôlable ;
- « progression significative » = événement du journal (changement de statut, d'échéance ou d'univers, clôture, réouverture) ; **pas** l'ouverture d'une fiche ni une simple consultation ; liste exacte **À DÉCIDER** ;
- nombre maximal par défaut : **À DÉCIDER** (ordre de grandeur : 5).

### Actions sur les suggestions (VALIDÉ pour le principe)

Masquer une suggestion ; reporter son rappel ; ne plus afficher cet élément dans la synthèse. Ces actions **ne suppriment pas la tâche et ne modifient pas son statut** : l'état de masquage est une information propre à la synthèse, distincte de la tâche (**PROPOSÉ**).

**À DÉCIDER :** politique de réapparition d'une alerte devenue **critique** après masquage (jamais, uniquement si l'échéance est dépassée, notification distincte…). Les alertes déjà masquées ne doivent pas disparaître sans que l'utilisateur sache comment les retrouver (**PROPOSÉ**).

### Configuration future (VALIDÉ comme intention)

Délai d'inactivité ; anticipation des échéances ; types de suggestions ; nombre maximal d'informations ; rubriques concernées ; univers Tous / Moi / Externe ; règles de masquage et de rappel. Aucun réglage n'est créé avant que les données correspondantes existent.

## 7. Deux présentations du tableau de bord

**VALIDÉ (principe).** Même contenu, mêmes composants métier, mêmes données ; **un seul système de données**.

| | **Épurée** | **Connectée** |
|---|---|---|
| Intention | Productivité | Ambiance plus expressive, identité relationnelle |
| Mise en page | Grille de cartes ordonnée, hiérarchie stable | Mêmes informations et fonctions |
| Effets | Limités | Connexions graphiques **pertinentes** |
| Règle | — | **Aucun lien visuel ne prétend représenter une relation qui n'existe pas** |

- Le choix se modifie **directement depuis l'accueil** et est mémorisé dans les préférences (VALIDÉ).
- **PROPOSÉ :** la présentation est une couche d'habillage et de disposition au-dessus des mêmes composants ; aucune donnée ni règle métier n'en dépend. Les mêmes exigences d'accessibilité s'appliquent (contrastes, mouvement réduit, aucune information portée par la seule décoration).
- Tant que le moteur de relations n'existe pas, la présentation Connectée **ne trace que des liens réels** (la provenance capture → tâche est la seule relation déjà enregistrée) ; si aucun lien réel n'est affichable, elle n'en dessine pas (**PROPOSÉ** ; l'usage de la provenance : **À DÉCIDER**).
- Nom définitif des deux présentations et styles précis (couleurs, formes) : **À DÉCIDER**, à valider sur maquette réelle (rappel de `02_UI_UX.md` : un schéma documentaire n'est pas une maquette validée).

## 8. Future vue « Connexions » (HORS PÉRIMÈTRE pour l'instant)

- Vue **distincte** pour explorer les relations bidirectionnelles entre éléments.
- S'appuie **exclusivement** sur des relations réelles enregistrées dans le modèle de données.
- **Ne se confond pas** avec la présentation « Connectée » du tableau de bord (un style, non une fonction d'exploration).
- Dépend de la brique des relations (section 11).

## 9. Calendrier et événements

Le calendrier externe est une vision future (`01_CAHIER_DES_CHARGES.md` §N, **HORS PÉRIMÈTRE** de la brique 003). Le tableau de bord n'affiche une carte ou une zone « Événements et échéances » que lorsque ces données existent réellement. Si un agenda **interne** précède la synchronisation externe : **À DÉCIDER**.

## 10. Mode compact (VALIDÉ pour l'intention)

Distinct du tableau de bord complet ; privilégie la **capture rapide** et l'accès immédiat aux informations en attente de traitement. Il s'appuie sur les mêmes composants. Les comportements de fenêtre (tray, démarrage, `×` / `—`) restent ceux de la brique 006 déjà décrits au cahier des charges (§A).

## 11. Dépendances et ordre logique

| Brique | Apporte | Nécessaire pour |
|---|---|---|
| 003 (fiche enrichie) | Échéances, statuts, priorité, dates effectives, historique, masquage des captures terminées | Ma synthèse (toutes les situations) |
| 004 (Univers Moi / Externe) | Filtres Tous / Moi / Externe, responsabilité affectée | Filtre transversal de l'accueil |
| 005 (sous-tâches, prochaine action) | Hiérarchie, dépendances | « Prochaine action » dans les cartes |
| 006 (tableau de bord, compact/étendu, tray) | Accueil modulaire, Capture & Idées intégré, Ma synthèse, cartes, deux présentations | — |
| Relations / Connexions *(numéro à attribuer)* | Relations bidirectionnelles, vue Connexions | Liens réels de la présentation Connectée, vue Connexions |
| Modules Finance, Administratif, Inventaire… | Contenu des cartes spécialisées | Cartes correspondantes |
| Calendrier *(numéro à attribuer)* | Événements, échéances | Zone agenda |

Ordre minimal : 003 → 004 → 006. Les cartes spécialisées et la vue Connexions s'ajoutent ensuite, chacune quand sa donnée existe.

## 12. Questions ouvertes

1. Liste des rubriques activées par défaut ; nom « Finance(s) ».
2. Libellé des espaces et des listes (« À organiser », « Traitées », « Tâches ») face à « Capture & Idées ».
3. Prochaines actions dans le mode compact.
4. Nombre maximal d'éléments de Ma synthèse ; définition de « progression significative » ; donnée de relance.
5. Politique de réapparition d'une alerte critique masquée.
6. Réorganisation des cartes : modalités et accessibilité.
7. Noms et styles définitifs des deux présentations ; usage de la provenance dans « Connectée ».
8. Agenda interne avant calendrier externe.
9. Distinction visuelle Moi / Externe dans la vue « Tous ».

## 13. Hors périmètre

HORS PÉRIMÈTRE à ce stade : toute implémentation ; score d'urgence ; IA ; vue Connexions ; calendrier externe ; cartes dont le module n'existe pas ; personnalisation « extrême » de la mise en page (`00_VISION.md`).
