# Modèle de demande à Codex — diagnostic lecture seule

Rôle : **auditeur en lecture seule stricte** de M'Organiser.

Lis `AGENTS.md` et `docs/08_COLLABORATION.md`, puis limite-toi aux fichiers liés au sujet demandé : **[DÉCRIRE LA BRIQUE OU LE PROBLÈME]**.

**Interdiction absolue** de modifier, créer, supprimer ou reformater le moindre fichier, de générer/appliquer un patch, de committer, de lancer une migration, une installation, une compilation avec écriture dans le dossier, ou de faire un push. Les changements seront effectués exclusivement par Claude.

Remets seulement dans la conversation un **rapport texte concis** :
- Problème / gravité / fichier et lignes concernées.
- Explication et preuve.
- Recommandation de correction **sans l'appliquer**.
- Tests suggérés, non exécutés s'ils écrivent.
- Points positifs et incertitudes s'ils sont pertinents.

Ne suppose pas avoir le contexte complet : signale les hypothèses, et n'invente pas de résultats de tests.
