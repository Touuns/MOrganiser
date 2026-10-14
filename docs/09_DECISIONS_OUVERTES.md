# 09 — Registre des décisions

## Décisions actées

- Application Windows d'abord ; au lancement Windows : fenêtre compacte visible, non épinglée.
- `×` masque vers le tray, `—` minimise, `Quitter` via tray arrête réellement.
- Tableau de bord développé : tâches personnelles et externes distinctes et visibles ; capture directement visible avec boîte « À organiser » **au-dessus** du champ de capture (décision du 2026-10-10, remplace « adjacente »). *(Précisé le 2026-10-14 : Moi / Externe forment un filtre transversal Tous / Moi / Externe, non deux colonnes permanentes ; voir l'entrée « Cadrage 003-0 » ci-dessous.)*
- La capture est une **entrée brute**, non automatiquement classée en tâche.
- Sobriété visuelle « centre de contrôle », données opérationnelles devant les gros indicateurs.
- Hiérarchie de sous-tâches et prochaine action visible par défaut ; tris configurables.
- Historique conservé et statistiques fiables, non culpabilisantes.
- Stockage local et hors Git ; IA facultative bien après le cœur ; capture mobile assez tôt.
- Initiation in situ, facultative, rejouable, progression réelle.
- Sécurité dès les fondations ; verrouillage par mot de passe **facultatif, désactivé par défaut**.
- **Claude seul modifie le code ; Codex lecture seule et rapports.**

### 2026-10-08 — Pile technique

- **Question :** quelle technologie pour une application Windows-first (tray, instance unique, démarrage automatique, SQLite locale, interface moderne et animée, mobile plus tard) ?
- **Options :** .NET WPF, .NET WinUI 3, Tauri 2 + interface web (Electron écarté : poids et mémoire).
- **Choix :** **Tauri 2 + React + TypeScript + Vite** ; **Rust** pour la partie système, métier et persistance ; **SQLite** locale (gérée côté Rust) à partir de la brique 001.
- **Justification :** modules officiels pour tray / instance unique / démarrage / mises à jour ; animations et accessibilité du web ; séparation nette interface ↔ Rust ; exécutable léger (~3 Mo) ; voie mobile possible ; bon outillage VS Code.
- **Coûts acceptés :** deux langages (TypeScript et Rust), compilation Rust initiale lente, mémoire WebView2 supérieure à WPF.
- **Révision :** si Rust devient un frein majeur, le repli envisagé est WPF (pas Electron).

### 2026-10-08 — Emplacement des données (proposition appliquée en Dev, à confirmer)

- Données dans `%LOCALAPPDATA%\<identifiant>\data` (dossier local non itinérant, propre au compte Windows).
- Dev : `com.morganiser.desktop.dev` ; Stable : `com.morganiser.desktop`.
- La **stratégie de sauvegarde** reste à arbitrer (avant usage réel de Stable).

### 2026-10-10 — Brique 001 (validées par le propriétaire)

- **Destination ≠ tag :** une capture peut recevoir **une destination principale facultative**, qui indique où elle sera traitée. Destinations initiales : espaces de responsabilité **Moi**, **Externe** ; rubriques **Administratif**, **Finances**, **Inventaire** (« Vente » non retenue). Aucun module n'est créé par une destination. Le modèle enregistre la nature (responsabilité / rubrique) pour permettre plus tard des associations multiples. La capture reste dans « À organiser », filtrable par destination ; un seul enregistrement.
- **Disposition :** la boîte « À organiser » est placée **au-dessus** du champ de capture (remplace « côte à côte »).
- **Ordre d'affichage :** ordre chronologique croissant ; chaque nouvelle capture apparaît **en bas** de la liste, juste au-dessus du champ (remplace « en tête, récent d'abord »). Au-delà de la limite d'affichage, ce sont les plus récentes qui sont montrées. Référence pour l'animation 001-C.
- **Répartition 001-B / 002 :** édition et suppression élémentaires d'une capture en 001-B ; conversion en tâche/note/projet en brique 002.
- **Nom de la base :** `morganiser.db`, dans le dossier `data` de l'environnement (Dev : `%LOCALAPPDATA%\com.morganiser.desktop.dev\data`). Mode WAL : fichiers auxiliaires `-wal`/`-shm` à traiter avec la base (voir fiche brique 001, section 7).
- **Référence unique :** `docs/briques/BRIQUE_001_CAPTURE_RAPIDE.md` (la spécification V2 y a été fusionnée).

### 2026-10-10 — Sous-brique 001-B (validées par le propriétaire)

- **Fiche :** panneau latéral à droite à partir de 900 px (ajustable après essais), remplacement de la vue principale en dessous ; fiche directement éditable.
- **Corbeille :** suppression logique **sans confirmation systématique**, notification « Annuler » (8 s en 001-B, **4 s depuis 001-C**), vue Corbeille permanente (survit au redémarrage) ; **aucune suppression définitive ni vidage automatique** en 001-B.
- **Conflits :** verrouillage optimiste (`updated_at` attendu) vérifié atomiquement dans le `UPDATE`, `updated_at` strictement croissant, erreurs distinctes (introuvable / corbeille / conflit). Sans migration.
- **« Voir tout » :** lots de 50, pagination par curseur `(date, id)`, plus anciennes ajoutées au-dessus.
- **Choix d'implémentation à relire :** la mise à la corbeille et la restauration ne modifient pas `updated_at` (dernière modification du contenu) ; voir la fiche brique 001, section 12, pour l'impact sur la future synchronisation.

### 2026-10-10 — Brique 001 clôturée

- 001-A à 001-E validées et fusionnées (PR #2 à #6). 001-D : option manuelle « Conserver ce choix » (suggestion automatique abandonnée). 001-E : initiation minimale, proposition unique à la première utilisation (mémoire `localStorage`, `morganiser.initiation.v1`), bouton « Découvrir » permanent ; reprise et parcours thématiques reportés en brique 007.

### 2026-10-10 — Brique 002 : conversion d'une capture en tâche (validées par le propriétaire)

- **Périmètre :** conversion en **tâche** uniquement ; notes et projets attendent leurs modules.
- **Tâche minimale :** identifiant stable, titre (≤ 120 caractères, extrait modifiable de la première ligne), détails (copie intégrale), statut initial « à faire », destination facultative copiée telle quelle, capture d'origine, horodatages. Les cinq statuts du cahier des charges sont admis par le schéma ; 002 n'écrit que « à faire ».
- **Origine :** capture conservée intégralement, retirée de « À organiser », retrouvable dans « Traitées » et depuis la tâche ; lecture seule tant qu'elle est convertie.
- **Annulation :** « Annuler » 4 s ; refusée si la tâche a été modifiée ou avancée. Tâche annulée conservée (`deleted_at`), jamais supprimée. Reconversion possible.
- **Dates :** la conversion ne modifie pas `updated_at` de la capture ; `converted_at` porte le changement de cycle de vie.
- **Garde SQL :** tout changement d'un champ métier d'une tâche doit faire croître strictement `updated_at` (déclencheur).
- **Migrations :** sauvegarde cohérente (`VACUUM INTO`) vérifiée avant toute migration d'une base existante ; échec de sauvegarde = pas de migration ; trois sauvegardes conservées. Aucune migration de Stable ; la base Dev réelle n'est migrée qu'après un essai sur copie isolée, organisé séparément.
- **Reste ouvert :** stratégie de sauvegarde/restauration **utilisateur** (copie planifiée, export) avant toute donnée réelle dans Stable.

### 2026-10-14 — Cadrage 003-0 : vision UX et architecture fonctionnelle (documentation seulement)

Références : `docs/briques/BRIQUE_003_FICHE_TACHE_ENRICHIE.md` (fiche, avec Q1 à Q16), `docs/10_TABLEAU_DE_BORD.md`, `docs/03_ARCHITECTURE.md` (notions indépendantes, relations), `docs/01_CAHIER_DES_CHARGES.md` §N. **Aucune implémentation.**

**Décidé (validé par le propriétaire)**

- **Transformer en tâche** ouvre directement une **grande fiche modale immersive** (centrée, marges, arrière-plan assombri et légèrement flouté, contenu défilable, en-tête et actions accessibles, clavier et lecteur d'écran, protection du brouillon). **Brouillon : aucune tâche en SQLite avant validation explicite** ; abandon = la capture reste dans « À organiser ».
- Préremplissage : titre proposé modifiable ; **texte intégral dans une description entièrement modifiable** ; classement repris et modifiable ; champs facultatifs présentés progressivement.
- La capture d'origine est conservée **discrètement** tant que la tâche est active ; à la clôture elle disparaît des vues courantes et des archives visibles, **sans suppression physique automatique** ; la tâche terminée est conservée. Les invariants de traçabilité et de sécurité de la brique 002 restent en vigueur.
- **Capture & Idées** : espace d'entrée commun ; une idée non traitée n'est pas une rubrique ; une capture n'est pas toujours une « idée ».
- **Univers de responsabilité** Tous / Moi / Externe : filtre **transversal**, non deux colonnes permanentes ; priorité indépendante de l'univers. **Remplace** la lecture « deux sections permanentes » des décisions antérieures (textes conservés, annotés).
- **Rubriques** : liste ouverte (Administratif, Finance, Inventaire… puis Projets, Santé, Apprentissage, Maison, Voyages…) ; **rubrique, type d'élément, responsabilité et état d'avancement sont quatre notions indépendantes** ; pas de doublon physique entre vues. Tout élément n'est pas une tâche.
- **Tableau de bord complet** = futur accueil principal (brique 006, hérite de la spécification) : navigation/recherche/filtres, Capture & Idées, **Ma synthèse**, cartes de rubriques, événements ; mode compact distinct ; **deux présentations** (Épurée, Connectée) sur les mêmes données ; vue **Connexions** distincte, fondée sur des relations réelles.
- **Ma synthèse** : sélection contextuelle, règles déterministes, explicables, testables, configurables à terme ; pas d'IA ; pas de score d'urgence ; ouvrir une fiche ≠ progression ; aucun faux signal quand rien n'est à signaler ; masquer / reporter / ne plus afficher sans toucher à la tâche.
- **Relations bidirectionnelles** : principe consigné (une relation, consultable des deux côtés, symétriques / dirigées) ; **hors brique 003**, schéma non décidé.
- **Calendrier externe** : vision future (d'abord le calendrier de l'iPhone du propriétaire, fournisseur réel à identifier, synchronisation bidirectionnelle sans écrasement silencieux) ; **hors brique 003**, aucun protocole choisi.
- Roadmap : numérotation conservée ; **003** = fiche enrichie ; le tableau de bord rejoint la **006** existante ; « Relations / Connexions », modules spécialisés et calendrier sont des briques à numéroter ultérieurement.

**Remplace ou précise (anciens textes conservés et annotés)**

| Ancien texte | Devient |
|---|---|
| Fiche 002 §11 : panneau de conversion intégré, texte figé | Modale de brouillon, description et classement modifiables |
| Fiche 002 §12 : « Traitées » = toutes les conversions actives | Idem, **sans** les captures dont la tâche est terminée |
| `00`, `01`, `02` : Moi et Externe en deux sections permanentes | Filtre transversal Tous / Moi / Externe |
| `02` accueil compact : « 2–5 prochaines actions » | Capture rapide et éléments à traiter en priorité ; prochaines actions : à décider |

**Validé le 2026-10-14 pour 003-A** (voir fiche 003, §14) : Q1 modale d'abord, sans migration · Q4 mécanisme d'annulation de 002 conservé · Q5 étape explicite avant transformation · Q11 comportement de la modale. Validation manuelle positive sur Dev ; intégration Git en attente.

**À arbitrer (proposition de l'assistant dans la fiche 003, §11)**

Ordre des sous-briques (proposé : modale sans migration d'abord) · description vide autorisée (reconstruction de `tasks`) · deux dimensions univers + rubrique : dans 003-B ou 004 · notification « Annuler » après fiche complète et garde d'annulation · capture modifiée non enregistrée au clic « Transformer » · persistance du brouillon · accès à l'origine depuis une tâche terminée · dates (jour calendaire, date de réalisation modifiable) · priorité · événements d'historique · clic sur le fond de la modale, largeur et plein cadre · fiche d'une capture : panneau ou modale · création directe de tâche dans 003 · vue de récupération des tâches annulées · libellés « Capture & Idées » · transitions de statut.

**À arbitrer (tableau de bord, `10`, section 12)**

Rubriques activées par défaut et nom « Finance(s) » · prochaines actions en compact · nombre d'éléments, « progression significative » et donnée de relance de Ma synthèse · **réapparition d'une alerte critique masquée** (question ouverte conservée) · réorganisation accessible des cartes · noms et styles des présentations · agenda interne avant calendrier externe.

**Contradictions et risques consignés (non résolus silencieusement)**

- Destination unique (décision 2026-10-10) ↔ deux dimensions pour une tâche : le champ de nature enregistré en 001 prévoyait cette évolution ; le schéma reste à décider.
- Garde d'annulation `status = 'todo' AND updated_at = created_at` ↔ création riche directe.
- Dispositif de répétition de migration limité à v1 → v2 ↔ migration v2 → v3 à répéter.
- Texte dupliqué (capture + description) ↔ conservation sans suppression automatique.
- Calendrier externe (réseau, identifiants) ↔ fonctionnement initial sans réseau et CSP stricte.

## Arbitrages d'avant la brique 001 (clos, sauf la sauvegarde de Stable)

1. ~~Pile technologique Windows~~ → actée le 2026-10-08 (voir ci-dessus).
2. **Emplacement technique des données** : appliqué en Dev ; la **stratégie de sauvegarde/restauration** reste ouverte et doit être décidée **avant toute utilisation de Stable avec des données réelles**.
3. ~~Nom technique de la base~~ → `morganiser.db`, acté le 2026-10-10.

## À arbitrer au fil des briques

- Placement exact, taille et ajustement des panneaux dans le grand tableau de bord (cadrage du 2026-10-14 : `docs/10_TABLEAU_DE_BORD.md`).
- Heuristique initiale de tri et paramètres d'urgences.
- Détails des statuts / dépendances / échéances.
- Modèle d'attachement des notes et documents.
- Chiffrement au repos et gestion des clés avant usage de données très sensibles.
- Solution mobile, synchronisation, conflits et chiffrement en transit.
- Licence et stratégie de publication si open source.

## Politique de décision

Chaque arbitrage important : date, question, options, choix, justification, impacts, révision éventuelle. Ne pas transformer une idée discutée en fonctionnalité déjà promise pour une version précise.
