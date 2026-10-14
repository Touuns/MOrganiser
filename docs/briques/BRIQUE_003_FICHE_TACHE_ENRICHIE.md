# Brique 003 — Fiche de tâche enrichie, grande modale et cycle de vie

**Document de cadrage 003-0 (référence unique de la brique 003).**
**État : cadrage 003-0 du 2026-10-14 ; sous-brique 003-A implémentée sur la branche `brique-003a-modale-brouillon` (2026-10-14), tests rapportés réussis, **validation manuelle positive du propriétaire sur Dev le 14 octobre 2026** ; intégration Git en attente (non fusionnée). 003-B, C et D non commencées. Aucune migration.**
**Dépendances :** briques 001 et 002 validées et fusionnées dans `main`.
**À lire avec :** `02_UI_UX.md`, `03_ARCHITECTURE.md`, `04_SECURITE.md`, `05_ROADMAP.md`, `10_TABLEAU_DE_BORD.md`, `briques/BRIQUE_002_VIE_ELEMENT_CAPTURE.md`.

**Légende** — chaque affirmation de ce document porte un des quatre statuts :

| Statut | Sens |
|---|---|
| **VALIDÉ** | Décidé par le propriétaire ; ne se rediscute pas sans décision explicite |
| **PROPOSÉ** | Recommandation de l'assistant, étudiée, en attente d'accord |
| **À DÉCIDER** | Question ouverte, arbitrage du propriétaire nécessaire |
| **HORS PÉRIMÈTRE** | Explicitement exclu de la brique 003 |

Seul **003-A** est développé (section 14). Le reste de ce document décrit des intentions : la fiche d'une tâche existante reste en lecture seule (fiche 002, section 11) tant que 003-B n'est pas livrée.

## 1. Intention et périmètre

Faire de la transformation d'une capture en tâche un geste **à la fois rapide et complet** : une grande fiche modale préremplie, que l'on peut compléter librement ou valider telle quelle ; puis permettre de faire vivre la tâche (modifier, dater, changer de statut, terminer) sans jamais perdre la capture d'origine, la tâche, ni les liens.

**Dans la brique 003 (VALIDÉ) :** fiche de tâche enrichie ; grande modale immersive ; édition ; dates ; statuts ; historique des modifications utiles ; comportement des captures transformées.

**Hors de la brique 003 (HORS PÉRIMÈTRE) :** tableau de bord complet, « Ma synthèse » et cartes de rubriques (`10_TABLEAU_DE_BORD.md`) ; moteur de relations et vue Connexions ; modules Finance, Administratif, Inventaire et autres rubriques ; sous-tâches ; calendrier externe ; filtres Tous / Moi / Externe sur l'accueil (brique 004) ; toute dépendance à l'IA.

## 2. Décisions validées

| # | Décision | Statut |
|---|---|---|
| D1 | « Transformer en tâche » ouvre **directement** une grande fiche modale immersive | VALIDÉ |
| D2 | La fiche est un **brouillon** : aucune tâche n'existe en SQLite avant une validation explicite ; abandonner la fiche laisse la capture dans « À organiser » | VALIDÉ |
| D3 | Préremplissage : titre proposé (modifiable) ; **texte intégral** de la capture repris dans une **description entièrement modifiable** ; classement existant repris, modifiable | VALIDÉ |
| D4 | Les champs facultatifs sont présentés **progressivement** ; tous les champs ne sont pas obligatoires | VALIDÉ |
| D5 | Le brouillon est protégé contre la fermeture accidentelle et contre un échec d'enregistrement | VALIDÉ |
| D6 | La capture d'origine reste **discrètement conservée** tant que la tâche est active | VALIDÉ |
| D7 | Quand la tâche est **terminée**, la capture historique disparaît des vues courantes et des archives visibles ; **aucune suppression physique automatique** ; la tâche terminée reste conservée | VALIDÉ |
| D8 | Les règles de traçabilité et de sécurité de la brique 002 sont préservées jusqu'à décision explicite sur leur évolution | VALIDÉ |
| D9 | La priorité est une information **distincte** de l'univers : « Moi » n'implique pas une priorité élevée, « Externe » pas une priorité faible | VALIDÉ |
| D10 | Rubrique, type d'élément, responsabilité (Moi / Externe) et état d'avancement sont **quatre notions indépendantes** (`03_ARCHITECTURE.md`) | VALIDÉ |
| D11 | Les dates, le statut et les champs complémentaires n'apparaissent dans la fiche que **selon les capacités réellement implémentées** : aucune zone fictive | VALIDÉ |

## 3. Spécifications existantes qui évoluent

Ces textes restent dans leurs documents (historique utile) ; ils sont annotés comme remplacés ou précisés.

| Où | Texte actuel | Évolution | Statut |
|---|---|---|---|
| Fiche 002, section 11 (« Panneau de conversion, intégré en haut de la fiche, pas de fenêtre modale ») | Panneau dans la fiche capture | Remplacé par la grande modale (D1) | VALIDÉ |
| Fiche 002, section 11 (« Pendant le panneau, le texte et la destination sont figés ») | On convertit la version enregistrée | Description et classement modifiables dans la modale (D3) | VALIDÉ |
| Fiche 002, section 4 (`details` : « copie intégrale du texte de la capture ») | Copie non modifiable en pratique | La description devient un texte propre de la tâche ; la copie intégrale reste dans la capture | VALIDÉ ; contrainte « non vide » : À DÉCIDER (Q2) |
| Fiche 002, section 12 (« Traitées » = captures actuellement converties) | Toute conversion active | + les captures dont la tâche est terminée n'y figurent plus (D7) | VALIDÉ |
| `09`, 001-B (fiche = panneau latéral à droite) | Capture et tâche en panneau | Les tâches passent à la modale ; la fiche d'une **capture** reste un panneau tant qu'aucune décision contraire | À DÉCIDER (Q12) |
| `09` / `01` §C (destination unique, ni tag) | Un seul champ de classement | La **capture** garde une destination unique ; la **tâche** porte à terme deux dimensions : univers (Moi / Externe) et rubrique | Principe VALIDÉ ; schéma À DÉCIDER (Q3) |
| `00` principe 4, `01` §B, `02` accueil (« Moi » et « Externe » visibles en deux sections) | Deux sections fixes | Filtre transversal Tous / Moi / Externe (`10_TABLEAU_DE_BORD.md`) | VALIDÉ |
| `05` brique 003 (« création directe de tâche… ») | Liste d'actions de base | Précisée par cette fiche | PROPOSÉ |

## 4. Parcours (capture → tâche)

**VALIDÉ** (étapes 1 à 6) ; les détails d'interface sont **PROPOSÉS**.

1. L'utilisateur ouvre une capture et clique « Transformer en tâche ». *(PROPOSÉ : si la capture a des modifications non enregistrées, l'avertissement actuel Enregistrer / Abandonner / Continuer est conservé : Q5.)*
2. La grande fiche s'ouvre directement. Rien n'a été écrit.
3. Elle contient : titre proposé, description (texte intégral), classement repris ; les champs complémentaires disponibles sont repliés.
4. L'utilisateur complète ou valide telle quelle (clavier : Ctrl+Entrée).
5. Un seul appel Rust, atomique, crée la tâche et marque la capture comme traitée ; le succès n'est affiché qu'**après** la réponse de Rust.
6. La modale se ferme, la capture quitte « À organiser », le focus va à la carte voisine.

**Abandon, conflit et échec (PROPOSÉ) :**

| Situation | Comportement |
|---|---|
| Fermeture avec brouillon **non modifié** | Fermeture silencieuse (règle déjà appliquée au titre en 002-B) |
| Fermeture avec brouillon **modifié** | Bandeau « Abandonner le brouillon / Continuer » dans la modale (jamais une seconde modale) |
| Capture modifiée, supprimée ou convertie ailleurs | Refus de Rust ; brouillon conservé ; version actuelle affichée ; nouvelle confirmation demandée |
| Échec d'écriture | Modale ouverte, champs conservés, message sans contenu utilisateur |
| Fermeture de la fenêtre | Passe par la protection existante (`closeGuard`) |
| Clic sur le fond | Sans effet (PROPOSÉ, Q11) |

## 5. La grande modale

### Exigences (VALIDÉ)

- Fenêtre modale **centrée**, assez large pour travailler, **marges visibles** de chaque côté ; ce n'est pas une page qui remplace l'application.
- Arrière-plan de M'Organiser **visible, assombri et légèrement flouté**.
- Contenu défilable à l'intérieur ; **en-tête et actions principales toujours accessibles**.
- Adaptée aux petits écrans ; navigation clavier, gestion du focus, Échap, lecteur d'écran.
- Protection contre la perte de brouillon ; aucun effet visuel inutile ou fatigant ; cohérente avec le style existant (jetons de `src/styles/tokens.css`).

### Structure et choix techniques (PROPOSÉ)

| Zone | Contenu |
|---|---|
| En-tête collant | Titre accessible, état « Brouillon » / « Enregistrée », fermeture |
| Corps défilant | Titre, description, classement, puis (selon capacités) dates et statut ; champs complémentaires repliés |
| Pied collant | « Créer la tâche » (création) / « Enregistrer » (édition), « Annuler », raccourcis |

- **Largeur :** environ 880 px au plus, marges d'au moins 24 px ; sous ~600 px de large, feuille plein cadre avec « ← Retour » (comme les fiches actuelles sous 900 px). Valeurs à valider sur la vraie fenêtre (**À DÉCIDER**, Q11).
- **Composant natif `<dialog>` + `showModal()`** recommandé : couche supérieure, arrière-plan inerte, piège à focus natif, Échap intercepté pour passer par la protection de brouillon. Deux points vérifiés dans le code : la zone `.inbox-home__main` porte `container-type: inline-size` (une modale `fixed` rendue à l'intérieur y serait confinée), et jsdom (tests) n'implémente pas `showModal` (un petit shim de test sera nécessaire).
- **Flou d'arrière-plan :** `backdrop-filter` léger avec repli en aplat assombri ; son coût dans WebView2 n'est **pas mesuré** et doit l'être avant validation.
- **Mouvement :** fondu court via les jetons existants, nul en mouvement réduit.
- **Fenêtre basse** (640 px par défaut, 480 minimum) : en-tête et pied collants ne doivent pas laisser une zone de saisie minuscule ; à vérifier à 480 px.

## 6. Règles d'annulation, de restauration, de concurrence et d'intégrité

Les invariants de la brique 002 (fiche 002, sections 5 et 6) sont **VALIDÉS et conservés** (D8) ; ils ne sont pas recopiés ici. Rappel de ceux que la modale touche :

| Règle | Application à la nouvelle fiche |
|---|---|
| Une capture a au plus une tâche active | Inchangée (index unique partiel) |
| Convertie ⇔ `converted_task_id` désigne une tâche active d'origine cette capture | Inchangée |
| Rien n'est supprimé physiquement | Inchangée ; abandonner un brouillon n'écrit rien |
| Transactions `BEGIN IMMEDIATE`, un seul appel atomique de création | Création **riche** en une transaction (PROPOSÉ) |
| Verrou optimiste sur la capture (`expected_updated_at`) | La version lue à l'ouverture de la modale est celle contrôlée à la validation |
| `updated_at` strictement croissant à chaque modification effective d'un champ métier (déclencheur SQL) | Le déclencheur devra **couvrir les nouveaux champs** (sinon la garde d'annulation devient aveugle) |
| Provenance (`origin_inbox_item_id`, `created_at`) immuable | Inchangée |
| Sauvegarde cohérente et vérifiée avant toute migration d'une base existante | S'applique à toute migration de la brique 003 ; échec de sauvegarde = pas de migration |
| Capture convertie : lecture seule, pas de corbeille | Inchangée |
| Messages d'erreur sans contenu utilisateur | Inchangée |

**Annulation (« Annuler » 4 s, « Remettre dans la boîte »).** Distinguer :

- *abandonner un brouillon* : rien n'existe, rien à annuler (VALIDÉ, D2) ;
- *annuler une conversion* : opération existante, refusée si la tâche a été modifiée ou avancée.

Deux tensions sont consignées, **non résolues** :

- La garde actuelle exige `status = 'todo'` **et** `updated_at = created_at`. Une tâche créée directement avec un autre statut ne serait jamais annulable, et la notification « Annuler » serait trompeuse. Options : ne pas proposer la notification dans ce cas ; assouplir la garde à `updated_at = created_at` ; supprimer la notification puisque l'enregistrement est déjà une confirmation volontaire. **À DÉCIDER (Q4).**
- Contrat hérité de la brique 002 : tout lien rattaché à une tâche (relations futures, sous-tâches) doit ajouter sa condition à la garde d'annulation. **VALIDÉ**, à appliquer lors de ces briques.

**Restauration.** La corbeille logique des captures est inchangée. Les tâches annulées restent conservées sans vue de récupération (limite connue de 002) ; une vue de récupération n'est **pas** décidée (**À DÉCIDER**, Q14).

**Concurrence.** Un seul processus, une connexion protégée par un verrou : les écritures sont déjà sérialisées. Les contrôles de version protègent contre la lecture périmée de l'interface, pas contre un second processus (l'instance unique est prévue en brique 006). Pour l'édition d'une tâche existante, le même modèle s'applique (**PROPOSÉ**) : version attendue vérifiée dans le `UPDATE`, conflit = version actuelle affichée et brouillon conservé, jamais d'écrasement silencieux.

## 7. Capture d'origine : conservation et masquage

**VALIDÉ** : conserver discrètement tant que la tâche est active ; masquer des vues courantes et des archives visibles quand elle est terminée ; aucune suppression physique automatique.

**PROPOSÉ — masquage dérivé :** la visibilité de l'origine se **déduit** du statut de la tâche (rien n'est stocké en plus). Une tâche rouverte rend donc sa capture de nouveau visible ; une erreur de manipulation reste réversible.

| État | « À organiser » | « Traitées » | Fiche de la tâche | Base de données |
|---|---|---|---|---|
| À traiter | Visible | — | — | Conservée |
| Convertie, tâche active (à faire, en cours, en attente, bloquée) | Absente | Visible, discrète | Lien d'origine discret | Conservée |
| Convertie, tâche **terminée** | Absente | **Absente** | Accès à l'origine : **À DÉCIDER (Q7)** | **Conservée** (aucune suppression) |
| Tâche rouverte | Absente | Visible de nouveau | Lien d'origine | Conservée |
| Conversion annulée | Visible (à l'identique) | — | — | Conservée |

Points d'attention :

- **Texte dupliqué :** la description (modifiable) et la capture (intacte) contiennent le même texte. Toute future purge, export ou règle de confidentialité devra traiter les deux ; c'est pourquoi aucune suppression automatique n'est décidée.
- La définition de « Traitées » passe de « captures actuellement converties » à « captures dont la conversion est active **et** dont la tâche n'est pas terminée ». Cela exige une jointure sur la tâche dans la lecture de la liste : à prévoir dans 003-D.
- Le nom d'espace **« Capture & Idées »** (`10_TABLEAU_DE_BORD.md`) ne renomme pas l'interface actuelle : le libellé « À organiser » reste en vigueur tant qu'aucune brique d'interface ne le change (Q15).

## 8. Modèle conceptuel de la tâche enrichie (PROPOSÉ)

Orientations, **pas** de définition de schéma SQL : le schéma est spécifié au démarrage de la sous-brique qui le porte.

### Quatre notions indépendantes

| Notion | Rôle | Exemples | Aujourd'hui |
|---|---|---|---|
| Type d'élément | Ce qu'est l'élément | tâche, (futur) rendez-vous, note, objet, dossier, opération, document | Tout est tâche |
| Rubrique | Où on le classe | Administratif, Finances, Inventaire, (futur) Projets, Santé… | `destinations.kind = 'section'` |
| Univers de responsabilité | Pour qui | Moi, Externe | `destinations.kind = 'responsibility'` |
| État d'avancement | Où en est-il | à faire, en cours, en attente, bloqué, terminé | `tasks.status` (seul « à faire » écrit) |

Aujourd'hui rubrique et univers partagent **un seul champ** (`destination_id`) : une tâche « Administratif » ne peut pas être aussi « Externe ». Le principe de deux dimensions est VALIDÉ (exemple du propriétaire : une tâche administrative appartient à la fois à Moi et à Administratif) ; la représentation est **À DÉCIDER (Q3)**.

### Champs

| Champ | Nature | Obligatoire | Remarque |
|---|---|---|---|
| Identifiant, création, dernière modification | Fondamental, stocké | Oui | Existants |
| Titre (≤ 120 caractères) | Fondamental | **Oui — seul champ saisi obligatoire** | |
| Statut | Fondamental | Défaut « à faire » | Cinq valeurs déjà admises par le schéma |
| Description (≤ 10 000) | Facultatif | À DÉCIDER (Q2) | Reprend le texte de la capture |
| Univers, rubrique | Facultatifs | Non | Q3 |
| Date de début, échéance | Facultatifs | Non | Jours calendaires, sans fuseau (PROPOSÉ ; Q8) |
| Date effective de réalisation | Facultatif | Non | Renseignée à « terminée », **modifiable** (Q8) |
| Priorité | Facultatif | Non | Distincte de l'univers (D9) ; absence ≠ « normale » (PROPOSÉ ; Q9) |
| Origine (capture), corbeille/annulation | Existants | Non | Inchangés |
| Éléments liés | Facultatif | Non | **Hors 003** (relations) |

**Données dérivées, jamais stockées (PROPOSÉ) :** « en retard » (échéance dépassée et statut ≠ terminé) ; « origine visible ou masquée » ; « prochaine action » ; nombre d'éléments liés ; dernière activité.

### Historique des modifications utiles (PROPOSÉ)

Journal en ajout seul, écrit **dans la même transaction** que le changement :

- événements : création (avec provenance), changement de statut, d'échéance, d'univers ou de rubrique, terminée, rouverte, mise à la corbeille ou restaurée ;
- **jamais le texte** du titre ni de la description (pas de duplication de contenu privé) ;
- finalité : statistiques honnêtes (la durée entre création et clôture n'est pas du temps de travail), base de la détection de « progression significative » de Ma synthèse.

Liste exacte des événements et durée de conservation : **À DÉCIDER (Q10).**

### Conséquences pour SQLite (PROPOSÉ)

1. Relâcher la contrainte « description non vide » ou ajouter des champs impose une **reconstruction de la table `tasks`** (SQLite ne modifie pas un CHECK). `inbox_items` référence `tasks` et les clés étrangères sont actives pendant les migrations : opération délicate, la plus risquée du projet.
2. Le déclencheur « `updated_at` croissant » doit être recréé pour couvrir les nouveaux champs.
3. La sauvegarde automatique avant migration (002) s'applique ; la conservation de trois sauvegardes évincera à terme l'instantané antérieur à la v2 de la base Dev réelle.
4. **Le dispositif de répétition de migration actuel refuse toute base qui n'est pas en v1.** La base Dev réelle a déjà été migrée (information du propriétaire) : il devra être étendu à v2 → v3 **avant** toute migration réelle (test seulement).
5. Aucune migration de la version Stable sans décision explicite.

## 9. Découpage proposé

Ordre **PROPOSÉ** (À DÉCIDER, Q1). Chaque sous-brique garde les invariants 002 comme tests de non-régression.

### 003-0 — Cadrage documentaire (ce document)
Documentation seulement. Aucun code, aucune base.

### 003-A — Grande modale et conversion par brouillon, **sans migration** *(implémentée : voir section 14)*
- **Périmètre :** composant de modale ; éditeur de brouillon (titre, description, classement) ; remplacement du panneau de conversion de la fiche capture ; la commande de conversion reçoit en plus une description et un classement facultatifs.
- **Dépendances :** décisions Q4, Q5, Q11.
- **Migration :** aucune. Contrainte provisoire : description non vide.
- **Tests :** accessibilité de la modale (focus, Échap, retour du focus), protections de brouillon (abandon, conflit, échec, double validation, réponses tardives), conversion avec description et classement modifiés, invariants 002, permissions de commandes, shim de test.
- **Critères d'acceptation :** le parcours du §4 fonctionne ; la capture reste dans « À organiser » en cas d'abandon ; aucune écriture avant validation ; aucune perte de texte.
- **Risques :** confinement CSS, coût du flou, complexité de `CaptureDetail`, protection de fermeture de fenêtre.

### 003-B — Modèle de tâche enrichi et édition
- **Périmètre :** migration 0003 ; description facultative (si Q2) ; dates ; univers et rubrique (si Q3) ; priorité (si Q9) ; édition d'une tâche existante dans la même modale ; création directe (Q13).
- **Dépendances :** 003-A ; Q2, Q3, Q8, Q9 ; extension du dispositif de répétition (v2 → v3).
- **Migration :** oui (reconstruction de `tasks`, déclencheur recréé), précédée d'une répétition sur copie isolée, **autorisée séparément**.
- **Tests :** migration sans perte (captures, tâches, liens d'origine), intégrité et clés étrangères, déclencheurs, séquences aléatoires étendues, concurrence, sauvegarde préalable.
- **Critères :** aucune ligne perdue ; invariants 002 intacts ; édition protégée par le verrou optimiste.
- **Risques :** reconstruction sous clés étrangères ; sémantique de l'ancienne colonne de détails.

### 003-C — Cycle de vie, dates effectives, historique
- **Périmètre :** statuts utilisables ; terminer et rouvrir ; date effective ; journal des événements ; tri et filtre simples de la liste des tâches.
- **Dépendances :** 003-B ; Q8, Q10, Q13.
- **Migration :** probablement (journal des événements, nouveaux champs).
- **Critère de la roadmap :** terminer n'efface jamais la tâche ni ses liens.
- **Tests :** événement écrit atomiquement avec le changement ; aucun texte dans le journal ; transitions ; réouverture.
- **Risques :** surcharge d'interface (statuts) ; ambiguïté « terminée / archivée ».

### 003-D — Archivage discret des captures et finitions
- **Périmètre :** masquage dérivé de l'origine quand la tâche est terminée ; redéfinition de « Traitées » ; retrait du code devenu mort (ancien panneau de conversion) ; mise à jour de la documentation et de l'initiation si nécessaire.
- **Dépendances :** 003-C (statut « terminée ») ; Q7.
- **Migration :** non attendue.
- **Tests :** visibilité par état (tableau du §7), réouverture, capture jamais supprimée, non-régression de « Remettre dans la boîte ».
- **Risques :** texte dupliqué, accès à l'origine depuis une tâche terminée.

## 10. Critères UX observables (PROPOSÉ)

À valider par le propriétaire ; mesurables sans hypothèse non fondée.

| Critère | Cible proposée |
|---|---|
| Interactions pour créer une tâche depuis une capture, cas nominal | ≤ 3 (ouvrir, valider) |
| Création directe, titre seul, au clavier | ≤ 15 secondes |
| Champs obligatoires | 1 (le titre) |
| Compréhension de la transformation | Après création, la personne retrouve sa capture sans aide (observation sur 3 à 5 essais) |
| Reprise après interruption | Zéro saisie perdue dans les scénarios d'abandon, conflit, échec d'écriture et fermeture de fenêtre |
| Navigation clavier | Parcours complet sans souris, aucun piège à focus |
| Perte de données | 0 dans la batterie de tests d'intégrité |

## 11. Questions ouvertes

Questions à trancher **avant de spécifier 003-A** : Q1, Q4, Q5, Q11 (et Q12 si la fiche capture change). Avant 003-B : Q2, Q3, Q8, Q9, Q13. Avant 003-C : Q10, Q16. Avant 003-D : Q7. Les autres peuvent attendre.

| # | Question | Proposition |
|---|---|---|
| Q1 | Ordre des sous-briques : modale d'abord (sans migration) ou modèle d'abord ? | Modale d'abord : valeur visible tôt, réversible |
| Q2 | Description vide autorisée (impose la reconstruction de `tasks`) ? | Oui, en 003-B, après répétition de migration |
| Q3 | Univers + rubrique en deux dimensions : modèle dans 003-B, ou à la brique 004 ? | Les deux colonnes en 003-B si la modale les expose ; filtres et vues en 004 |
| Q4 | Notification « Annuler » après enregistrement d'une fiche complète ? Quelle garde ? | La conserver ; retirer la condition sur le statut |
| Q5 | Capture modifiée non enregistrée au clic « Transformer » : enregistrer/abandonner d'abord ou reprendre le brouillon ? | Garder le comportement actuel |
| Q6 | Brouillon : mémoire seule, ou mémorisation locale (reprise après arrêt forcé) ? | Mémoire seule en 003 |
| Q7 | Accès à la capture d'origine depuis une tâche terminée ? | Accès conservé mais discret |
| Q8 | Dates : jour calendaire ou instant ? Date de réalisation modifiable ? | Jour calendaire ; modifiable |
| Q9 | Priorité en 003 ? Combien de niveaux ? | Colonne dès 003-B, trois niveaux, repliée |
| Q10 | Événements d'historique retenus et durée de conservation ? | Liste du §8 ; conservation illimitée |
| Q11 | Clic sur le fond, largeur, plein cadre sous 600 px ? | Voir §5 ; à valider sur la vraie fenêtre |
| Q12 | La fiche d'une **capture** reste-t-elle un panneau ? | Oui, jusqu'à décision contraire |
| Q13 | Création directe de tâche dans 003 (inscrite à la roadmap) ? | Oui, en 003-B, par la même modale |
| Q14 | Vue de récupération des tâches annulées ? | Hors 003 |
| Q15 | Renommage des libellés (« À organiser », « Tâches ») vers « Capture & Idées » | Avec la brique d'interface du tableau de bord |
| Q16 | Transitions de statut libres, ou parcours contraint ? | Libres ; terminer renseigne la date de réalisation |

## 12. Risques et contradictions identifiés

**Techniques**
1. Reconstruction de `tasks` sous clés étrangères actives (§8) : seule migration susceptible de dégrader des données ; mitigée par la sauvegarde existante et la répétition sur copie.
2. Dispositif de répétition limité à v1 → v2.
3. `CaptureDetail` (~920 lignes, état dupliqué en `useState` et en référence) : recopier ce modèle doublerait la complexité ; extraire la logique de brouillon dans du code **nouveau** d'abord.
4. Protection de fermeture de fenêtre mono-brouillon : à généraliser sans régression.
5. Le titre proposé existe en double (Rust et faux backend de test) ; toute règle ajoutée doit l'être aux deux.
6. Coût du flou d'arrière-plan non mesuré.
7. Une version Stable construite depuis `main` migrerait automatiquement une base Stable existante : à confirmer avant toute livraison.

**UX**
1. Surcharge du formulaire malgré la divulgation progressive.
2. Libellés ambigus « Enregistrer » (capture) / « Créer la tâche » : d'où des libellés distincts.
3. Utilisateur croyant sa capture perdue : « Traitées » et la notification doivent le rassurer.
4. Brouillon perdu sur arrêt forcé de Windows (limite connue).
5. Fenêtre basse : zone de saisie réduite.

**Contradictions avec l'existant** : voir le tableau du §3 (aucune n'est résolue silencieusement) et les questions Q3, Q4 et Q7.

## 13. Hors périmètre

HORS PÉRIMÈTRE de la brique 003 : tableau de bord et Ma synthèse ; cartes de rubriques ; moteur de relations et vue Connexions ; modules Finance, Administratif, Inventaire ; sous-tâches et dépendances ; calendrier externe ; filtres d'univers globaux ; IA ; synchronisation ; chiffrement ; export `.xlsx`.

## 14. Réalisation 003-A : grande modale et conversion par brouillon (2026-10-14, validée manuellement ; intégration Git en attente)

**Brique 003-A : validation manuelle positive par le propriétaire sur M'Organiser Dev, le 14 octobre 2026.** Cette validation porte sur l'ergonomie et le parcours testés ; elle ne vaut ni validation de 003-B, ni revue indépendante complète du code.

Branche `brique-003a-modale-brouillon`. **Aucune migration, aucune table ni colonne, aucun fichier SQL modifié.** Aucune nouvelle commande Tauri : permissions, `build.rs` et capacité inchangés.

### Arbitrages validés par le propriétaire pour 003-A

| # | Décision | Application |
|---|---|---|
| Q1 | Modale d'abord, modèle enrichi ensuite | 003-A fonctionne avec le schéma v2 ; dates, statuts et édition viennent en 003-B/C |
| Q4 | Mécanisme de sécurité de 002 conservé | Notification « Annuler » 4 s inchangée ; tâches créées en `todo` ; condition de la garde d'annulation **non assouplie** ; révision différée |
| Q5 | Étape explicite avant transformation | Capture modifiée non enregistrée : Enregistrer / Abandonner / Continuer, comme avant ; jamais enregistrée en silence |
| Q11 | Comportement de la modale | Centrée, marges, fond assombri et flouté, **un clic sur le fond n'abandonne rien**, défilement interne, en-tête et actions accessibles, clavier, focus, protection du brouillon |

### Parcours réellement développé

1. « Transformer en tâche » (fiche d'une capture) ouvre **directement** la grande modale **Nouvelle tâche · Brouillon**. Le panneau intégré de 002-B n'existe plus.
2. Préremplissage **sans aucune écriture** : titre proposé (sélectionné, modifiable, 120 caractères), **description = texte intégral enregistré de la capture** (modifiable), **classement** = destination de la capture (modifiable, facultatif).
3. Le brouillon est indépendant de la capture : changer la description ou le classement de la tâche ne modifie jamais la capture d'origine.
4. « **Créer la tâche** » (ou Ctrl+Entrée, ou Entrée dans le titre) : un seul appel Rust atomique ; le succès n'est affiché qu'après sa réponse ; la fiche se ferme, la carte quitte « À organiser », notification « Annuler » (4 s) comme en 002.
5. Description vide ou blanche : message permanent sous le champ, bouton désactivé, aucune validation possible ; **jamais remplacée en silence** par le texte d'origine (contrainte de schéma actuelle : `details` non vide).

### Brouillon, fermeture et conflits

| Situation | Comportement |
|---|---|
| Brouillon **intact** (titre, description, classement comme à l'ouverture) | Échap, ✕ et « Abandonner » ferment sans avertissement ; le focus revient à « Transformer en tâche » |
| Brouillon **modifié** | Avertissement **dans la modale** (jamais une seconde modale) : « Abandonner le brouillon » / « Continuer » ; Échap sur l'avertissement = « Continuer » ; ouvrir une autre capture, changer de vue et fermer la fenêtre Windows passent par la même protection |
| Clic sur l'arrière-plan | Aucun effet |
| Création en cours | Champs en lecture seule, « Création… », fermeture refusée, aucune double validation |
| Échec Rust | Modale ouverte, brouillon intact, message ; nouvel essai possible |
| Conflit de version | Version actuelle prise comme référence ; les champs **non touchés** suivent la capture actuelle, les champs modifiés sont conservés ; nouvelle confirmation |
| Déjà convertie / corbeille / supprimée ailleurs | Brouillon **intact** : la modale se ferme avec un message dans la fiche ; brouillon **modifié** : la modale reste ouverte avec le message et le brouillon, création désactivée, abandon explicite |
| Réponse tardive (titre proposé) | Ne remplace jamais un titre modifié ; ignorée si la modale est fermée ; ne touche jamais à la description ni au classement |

### Rust

- `convert_inbox_item_to_task(id, expectedUpdatedAt, title?, details?, destination?)` : `details` absent = texte de la capture ; `destination` absente = destination de la capture. Énumération `{ "type": "keep" | "clear" | "set", "id" }` (trois états explicites : « absent » et « aucune » ne se distinguent pas dans une commande).
- Validé côté Rust : description (extrémités nettoyées, 1 à 10 000 caractères comptés en caractères), titre (inchangé), destination (existante et active ; une destination **inchangée** reste acceptée même archivée depuis, comme en 001-B). Nouveaux codes : `empty_details`, `details_too_long`, `unknown_destination`. Messages sans contenu utilisateur.
- `convert_item_with` porte la logique ; `convert_item` (signature historique) n'existe plus que pour les tests (`#[cfg(test)]`).
- Transaction `BEGIN IMMEDIATE`, verrou optimiste, index unique partiel, déclencheurs et annulation : **inchangés**.

### Interface

| Fichier | Rôle |
|---|---|
| `src/components/Modal.tsx` + `.css` | Modale **générique** : `<dialog>` + `showModal()` (couche supérieure, fond inerte, piège à focus natif), Échap routé vers l'appelant, retour du focus, aucun effet du clic sur le fond |
| `src/features/tasks/TaskEditor.tsx` + `.css` | Champs **contrôlés** (titre, description, classement), réutilisables pour l'édition future |
| `src/features/tasks/TaskDraftModal.tsx` + `.css` | Cadre de création : en-tête, avertissement, pied collant |
| `src/features/inbox/CaptureDetail.tsx` | Le brouillon de tâche (titre, description, classement) remplace le panneau ; protections étendues ; reste du composant inchangé |
| `src/features/inbox/api.ts`, `src/test/fakeBackend.ts` | Contrat et mêmes règles que Rust |
| `src/styles/tokens.css` | Jetons de recouvrement (`--color-overlay`, `--color-overlay-solid`, `--blur-overlay`, `--shadow-modal`) |
| `src/test/setup.ts` | Shim `<dialog>` **limité aux tests** (jsdom n'implémente pas `showModal`) |

`InboxHome` n'a pas été modifié ; aucune refonte de `CaptureDetail` : seul l'état du brouillon de conversion a été étendu.

### Tests (exécutés le 2026-10-14)

- Rust : `cargo test` 164 réussis, 0 échec, 3 ignorés par conception (2 sous-processus, 1 répétition sur copie réelle qui exige `MORGANISER_REHEARSAL_DB` et n'a pas été lancée) ; dont 14 nouveaux tests de `tasks` ; `cargo clippy --all-targets` : 0 avertissement.
- Interface : `vitest` 270 réussis (247 avant ; 23 de plus : réécriture de `InboxHome.convert.test.tsx` et `Modal.test.tsx`) ; `pnpm typecheck` propre ; `pnpm build` (tsc + vite) réussi.
- Mutations vérifiées : fermeture sur clic de fond (2 tests échouent), proposition de titre tardive qui écrase un titre modifié (1 test échoue).
- Vérification visuelle dans Chromium sur **banc d'essai jetable** (données fictives en mémoire, hors dépôt, application Tauri non lancée) à 1000×640 et en petite fenêtre : modale centrée 880 px, fond assombri et flouté, en-tête et pied visibles, clic sur le fond sans effet, Échap sur brouillon intact et modifié, Ctrl+Entrée, focus rendu, aucune erreur console. **Non vérifié** dans la vraie fenêtre WebView2 Windows.

### Limites et reporté

- **003-B** : modèle enrichi, description facultative, dates, univers + rubrique, priorité, édition d'une tâche existante, création directe, migration 0003 et extension du dispositif de répétition de migration (v2 → v3).
- **003-C** : statuts utilisables, terminer, date effective, historique. **003-D** : masquage des captures de tâches terminées, redéfinition de « Traitées ».
- Garde d'annulation, notification et conditions de création riche avec d'autres statuts : inchangées (Q4 différée).
- Coût réel du flou dans WebView2 non mesuré ; comportement à 480 px de haut sur la vraie fenêtre à confirmer.
- Le brouillon n'est conservé qu'en mémoire (Q6) : un arrêt forcé de Windows le perd.
- Le formatage `rustfmt` n'est pas appliqué par le dépôt (code existant non conforme) : non lancé en mode écriture.

## 15. Réalisation 003-B1b : migration 0003 et modèle de tâche enrichi (branche `brique-003b1a-preuve-migration`, non intégrée)

**Aucune migration réelle n'a été exécutée** : la migration 0003 n'a été jouée que sur des bases synthétiques de test. Elle s'appliquera automatiquement à une base v2 **au prochain lancement de l'application** (sauvegarde vérifiée préalable, voir fiche 002 §8) : ne pas lancer Dev avant la répétition v2 → v3 sur copie isolée (003-B1c) et l'accord du propriétaire.

### Schéma v3 (`migrations/0003_task_model.sql`)

| Élément | Décision |
|---|---|
| `details` | `TEXT NOT NULL DEFAULT ''`, ≤ 10 000 caractères ; **vide accepté** ; anciennes descriptions copiées à l'identique |
| `responsibility_id`, `section_id` | Deux FK facultatives vers `destinations` ; nature imposée par déclencheurs ; l'ancien `destination_id` est réparti selon `destinations.kind` (aucune tâche non classée n'est rattachée à « Moi ») ; `destination_id` n'existe plus sur `tasks` |
| `start_date`, `due_date`, `completed_on` | Jours calendaires `AAAA-MM-JJ`, `CHECK (date(x) IS x)` ; `CHECK (start_date <= due_date)` ; `completed_on` préparé, sans comportement avant 003-C |
| `priority` | `INTEGER` NULL, `CHECK BETWEEN 1 AND 5` ; 1 à 3 acceptés par Rust (basse, normale, haute) ; 4 et 5 réservés |
| Index et déclencheurs | `tasks_one_active_per_origin`, `tasks_recent`, `tasks_origin_is_immutable` recréés à l'identique ; `tasks_updated_at_must_grow` étendu aux neuf champs métier ; 4 déclencheurs de nature |
| `inbox_items`, `destinations` | **Inchangés** (la capture garde sa destination unique) |

### Transposition de la preuve 003-B1a
Le fichier SQL de production reprend les étapes prouvées (garde-fous → liens mis de côté → `tasks_new` → copie et vérification champ par champ → détachement des liens → `DROP` + `RENAME` → objets → restauration des liens → contrôles finaux). Les lignes `-- @stage: nom` sont lues par les tests, qui **exécutent ce fichier réel** et y injectent une panne avant chaque étape. Interdits et vérifiés : renommer l'ancienne table (`tasks_old` réécrit les clés étrangères), désactiver les clés étrangères, `writable_schema`.

### Exécuteur de migrations
Chaque migration s'exécute en transaction `IMMEDIATE`, relit `user_version` sous le verrou (une migration déjà appliquée par un autre processus n'est pas rejouée), puis contrôle `foreign_key_check` **avant** de valider (violation = annulation complète, erreur `ForeignKeys`). Les règles de sauvegarde automatique sont inchangées.

### Commandes et contrats
- `convert_inbox_item_to_task(id, expectedUpdatedAt, title?, details?, destination?, startDate?, dueDate?, priority?)` : `keep`/`clear`/`set` de 003-A conservés (`keep` = destination de la capture reprise dans sa dimension ; `set` = placée dans sa dimension) ; nouvelle variante `split { responsibilityId, sectionId }`.
- `create_task(fields)` : sans capture d'origine ni provenance inventée, au statut « à faire ».
- `update_task(id, expectedUpdatedAt, fields)` : verrou optimiste dans le `UPDATE`, refus d'une tâche annulée, aucune écriture sans changement réel, `updated_at` strictement croissant ; ne modifie jamais le statut, l'origine ni `completed_on`. Conflit : code `task_version_conflict`.
- Nouveaux codes : `invalid_date`, `dates_incoherent`, `invalid_priority`, `destination_kind`, `details_too_long`, `unknown_destination`. Le code `empty_details` n'existe plus (description facultative).
- Permissions : `allow-create-task` et `allow-update-task` ajoutées à la capacité et au test des permissions.

### Interface (compatibilité minimale, aucune refonte)
Types et wrappers (`Task` enrichi, `createTask`, `updateTask`), faux backend aligné sur Rust, libellé de classement des listes (« Univers · Rubrique »), détail de tâche (Univers, Rubrique). **La modale 003-A est inchangée** : un champ de classement unique, description encore exigée côté interface (limite de 003-A) ; les deux listes, les dates, la priorité, la création directe et l'édition sont l'objet de 003-B2.

### Tests (2026-10-14)
`cargo test` 211 réussis, 3 ignorés par conception ; `cargo clippy --all-targets` sans avertissement ; `vitest` 278/278 ; `pnpm typecheck` et `pnpm build` propres. Aucune base réelle ouverte ; application non lancée.

### Limites et suite
- Dispositif de répétition de migration : étendu à v2 → v3 en 003-B1c (voir §16).
- Corbeille des tâches créées directement : orientation validée, **non implémentée** ; politique des tâches converties : 003-C.
- La migration 0003 est prouvée sur SQLite 3.53.2 embarquée : toute montée de version de `rusqlite` impose de rejouer les tests de migration.

## 16. Réalisation 003-B1c : répétition de migration v2 → v3 (2026-10-14, branche `brique-003b1a-preuve-migration`, non intégrée)

**Aucune donnée réelle utilisée** : ni base Dev ou Stable, ni WAL/SHM, ni sauvegarde, ni copie de référence personnelle. Tout s'appuie sur des bases fictives créées par les tests ou, pour la procédure PowerShell, sur un dossier d'essais synthétique **supprimé ensuite**.

### Ce que permet le dispositif
- Répéter, sur une **copie isolée**, une étape **explicite** : v1 → v2, v2 → v3 ou v1 → v3 (versions données par `MORGANISER_REHEARSAL_FROM` et `_TO`, jamais déduites ; toute autre combinaison, version réelle différente ou valeur invalide = échec avant la moindre création).
- Comparer l'état avant/après **valeur par valeur** : captures, destinations, liens capture ↔ tâche, tâches ; la projection attendue (répartition de l'ancienne destination selon sa nature, colonnes nouvelles vides, « non classée » conservée) est calculée **en Rust, indépendamment du SQL de migration**.
- Contrôler intégrité, clés étrangères, version, objets de schéma (index, déclencheurs), absence de table temporaire, sauvegarde, aller-retour conversion/annulation sans résidu, restauration.
- Refuser une **source incohérente** avant toute migration, et signaler toute divergence par `table#identifiant.champ`, **sans texte**.
- `open_with(path, migrations)` : `open` en est inchangé ; il permet de répéter une étape précise.

### Marqueur
`REHEARSAL-RESULT status=… from=… to=… integrity=… schema=… projection=… backup=… roundtrip=… restore=… source=…` — `from` et `to` sont les versions **demandées** ; chaque `OK` n'est émis qu'après vérification effective.

### Testé (synthétique)
v2 → v3 sans WAL, avec WAL/SHM (les transactions n'existent que dans `-wal` ; la copie du seul `.db` est démontrée incohérente), sans capture active ; v1 → v3 ; v1 → v2 (non-régression) ; version ou plan refusé ; source incohérente ; panne en cours de migration ; valeur, classement, lien ou ligne altérés (détectés) ; source modifiée pendant la copie ; absence de texte et de chemin dans les messages. **Procédure PowerShell** (phases A puis B, fiche 002 §13 ; script modifié en B1c : versions `$From`/`$To`, nouveau marqueur, lignes Schéma et Projection ; complété en B1d) exécutée sur une base fictive : v2 → v3 en réussite, et refus de v1 → v2 / v1 → v3 sur une copie v2 et de la combinaison 3 → 2.

### Non vérifié, autorisations distinctes requises
Aucune migration, copie ni répétition sur la base Dev réelle. À obtenir séparément : (1) phase A sur Dev (application entièrement fermée) ; (2) phase B sur cette copie. La base Dev réelle (v2) reste à migrer à l'ouverture de l'application, après sauvegarde vérifiée : **ne pas lancer Dev avant ces étapes**.

## 17. Réalisation 003-B1d : durcissement après audit indépendant (2026-10-14, non commité)

**Aucune donnée réelle utilisée ; aucune migration réelle.**

### Garde-fou contre la migration automatique (implémenté)
- `storage::open_guarded(path, permit)` est le seul point d'ouverture de production. Une **base neuve** (fichier absent ou sans aucune table) est créée librement. Une **base existante** de version inférieure à la cible n'est migrée **que si** une autorisation désigne exactement sa version actuelle et la version cible ; sinon `MigrationNotAuthorized`, **avant** tout réglage, sauvegarde ou écriture (sonde en lecture seule).
- Autorisation : fichier `migration-authorization.txt` dans le dossier de données, trois lignes exactes `channel=dev`, `from=2`, `to=3`. Lue **uniquement pour Dev**, jamais pour Stable. Elle ne vaut que pour cette paire de versions : une fois la base migrée, elle est inopérante. Rien n'est activé par défaut ; l'application ne crée jamais ce fichier.
- Un fichier sans version mais contenant des tables, ou une version future, restent refusés. Sauvegarde préalable, contrôles d'intégrité et de clés étrangères : inchangés.
- **Procédure proposée** (à n'utiliser qu'après répétition réussie sur copie et accord du propriétaire) : application fermée, créer le fichier à la main dans le dossier de données Dev, lancer Dev, puis supprimer le fichier.
- Limite : un build Stable ne migre aucune base existante tant qu'une décision explicite n'étend pas ce mécanisme.

### Répétition
- `status=OK` exige désormais **toutes** les étapes (test dédié). PowerShell : ligne `Aller-retour conversion/annulation`, mapping des groupes documenté, révision Git (`RÉUSSITE NON TRAÇABLE` si code non commité), contrôle des processus élargi.
