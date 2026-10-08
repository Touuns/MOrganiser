//! Séparation des environnements Dev et Stable.
//!
//! Chaque environnement a son propre identifiant d'application et donc son propre
//! dossier de données sous `%LOCALAPPDATA%`. Seule [`prepare`] touche au disque, et
//! uniquement après toutes les vérifications ; le reste est de la logique pure.

use std::fmt;
use std::path::{Path, PathBuf};

use serde::Serialize;

/// Identifiant de la version Stable (doit correspondre à `tauri.conf.json`).
pub const STABLE_IDENTIFIER: &str = "com.morganiser.desktop";
/// Identifiant de la version Dev (doit correspondre à `tauri.dev.conf.json`).
pub const DEV_IDENTIFIER: &str = "com.morganiser.desktop.dev";
/// Sous-dossier réservé aux données de l'application (future base SQLite, sauvegardes…).
/// Le reste du dossier de l'identifiant est utilisé par WebView2 pour son cache.
pub const DATA_SUBDIR: &str = "data";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Dev,
    Stable,
}

impl Channel {
    /// Déduit l'environnement de l'identifiant configuré. Tout identifiant inconnu est
    /// refusé plutôt que deviné, pour ne jamais écrire au mauvais endroit.
    pub fn from_identifier(identifier: &str) -> Result<Self, EnvironmentError> {
        match identifier {
            DEV_IDENTIFIER => Ok(Channel::Dev),
            STABLE_IDENTIFIER => Ok(Channel::Stable),
            other => Err(EnvironmentError::UnknownIdentifier(other.to_owned())),
        }
    }

    pub fn identifier(self) -> &'static str {
        match self {
            Channel::Dev => DEV_IDENTIFIER,
            Channel::Stable => STABLE_IDENTIFIER,
        }
    }
}

/// Garde-fou : une compilation de développement (debug) ne doit jamais tourner avec
/// l'identifiant Stable, sinon elle utiliserait les données personnelles réelles.
pub fn ensure_build_matches(channel: Channel, is_debug_build: bool) -> Result<(), EnvironmentError> {
    if is_debug_build && channel == Channel::Stable {
        return Err(EnvironmentError::DebugBuildOnStable);
    }
    Ok(())
}

/// Dossier de données d'un environnement, à partir de la racine locale de l'utilisateur
/// (`%LOCALAPPDATA%` sous Windows).
pub fn data_dir(local_data_root: &Path, channel: Channel) -> PathBuf {
    local_data_root.join(channel.identifier()).join(DATA_SUBDIR)
}

/// Préparation complète au démarrage : valide l'identifiant, vérifie la cohérence avec
/// le type de compilation, puis (seulement alors) crée le dossier de données.
/// En cas de refus, rien n'est créé sous `local_data_root`.
pub fn prepare(
    identifier: &str,
    is_debug_build: bool,
    local_data_root: &Path,
) -> Result<(Channel, PathBuf), EnvironmentError> {
    let channel = Channel::from_identifier(identifier)?;
    ensure_build_matches(channel, is_debug_build)?;

    let dir = data_dir(local_data_root, channel);
    std::fs::create_dir_all(&dir).map_err(|error| EnvironmentError::DataDirCreation {
        path: dir.clone(),
        reason: error.to_string(),
    })?;
    Ok((channel, dir))
}

#[derive(Debug, PartialEq, Eq)]
pub enum EnvironmentError {
    UnknownIdentifier(String),
    DebugBuildOnStable,
    DataDirCreation { path: PathBuf, reason: String },
}

impl fmt::Display for EnvironmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvironmentError::UnknownIdentifier(id) => {
                write!(f, "identifiant d'application inconnu : « {id} »")
            }
            EnvironmentError::DebugBuildOnStable => write!(
                f,
                "une compilation de développement ne peut pas utiliser l'environnement Stable ; \
                 lancez `pnpm app:dev`"
            ),
            EnvironmentError::DataDirCreation { path, reason } => write!(
                f,
                "impossible de créer le dossier de données « {} » : {reason}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for EnvironmentError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from(r"C:\Users\Exemple\AppData\Local")
    }

    #[test]
    fn reconnait_les_deux_identifiants() {
        assert_eq!(Channel::from_identifier(DEV_IDENTIFIER), Ok(Channel::Dev));
        assert_eq!(Channel::from_identifier(STABLE_IDENTIFIER), Ok(Channel::Stable));
    }

    #[test]
    fn refuse_un_identifiant_inconnu_ou_approchant() {
        for id in ["", "com.morganiser", "com.morganiser.desktop.DEV", "com.morganiser.desktop.dev2"] {
            assert!(Channel::from_identifier(id).is_err(), "accepté à tort : {id:?}");
        }
    }

    #[test]
    fn identifiant_et_channel_sont_coherents() {
        for channel in [Channel::Dev, Channel::Stable] {
            assert_eq!(Channel::from_identifier(channel.identifier()), Ok(channel));
        }
    }

    #[test]
    fn dev_et_stable_ont_des_dossiers_distincts_et_non_imbriques() {
        let dev = data_dir(&root(), Channel::Dev);
        let stable = data_dir(&root(), Channel::Stable);
        assert_ne!(dev, stable);
        assert!(!dev.starts_with(&stable), "Dev ne doit pas être dans Stable");
        assert!(!stable.starts_with(&dev), "Stable ne doit pas être dans Dev");
    }

    #[test]
    fn le_dossier_de_donnees_est_sous_la_racine_utilisateur() {
        let dev = data_dir(&root(), Channel::Dev);
        assert_eq!(dev, root().join(DEV_IDENTIFIER).join(DATA_SUBDIR));
    }

    #[test]
    fn une_compilation_debug_refuse_stable() {
        assert_eq!(
            ensure_build_matches(Channel::Stable, true),
            Err(EnvironmentError::DebugBuildOnStable)
        );
        assert_eq!(ensure_build_matches(Channel::Dev, true), Ok(()));
        assert_eq!(ensure_build_matches(Channel::Stable, false), Ok(()));
        assert_eq!(ensure_build_matches(Channel::Dev, false), Ok(()));
    }

    // --- `prepare` : toujours sur un dossier temporaire, jamais sur le vrai %LOCALAPPDATA% ---

    fn entries(dir: &Path) -> Vec<PathBuf> {
        let mut list: Vec<PathBuf> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        list.sort();
        list
    }

    #[test]
    fn prepare_dev_cree_uniquement_le_dossier_dev() {
        let root = tempfile::tempdir().unwrap();
        let (channel, dir) = prepare(DEV_IDENTIFIER, true, root.path()).unwrap();
        assert_eq!(channel, Channel::Dev);
        assert_eq!(dir, data_dir(root.path(), Channel::Dev));
        assert!(dir.is_dir());
        assert_eq!(entries(root.path()), vec![root.path().join(DEV_IDENTIFIER)]);
    }

    #[test]
    fn prepare_refuse_stable_en_debug_sans_rien_creer() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            prepare(STABLE_IDENTIFIER, true, root.path()),
            Err(EnvironmentError::DebugBuildOnStable)
        );
        assert!(entries(root.path()).is_empty());
    }

    #[test]
    fn prepare_refuse_un_identifiant_inconnu_sans_rien_creer() {
        let root = tempfile::tempdir().unwrap();
        assert!(matches!(
            prepare("com.morganiser.autre", false, root.path()),
            Err(EnvironmentError::UnknownIdentifier(_))
        ));
        assert!(entries(root.path()).is_empty());
    }

    #[test]
    fn prepare_dev_ne_touche_pas_un_dossier_stable_existant() {
        let root = tempfile::tempdir().unwrap();
        let stable = data_dir(root.path(), Channel::Stable);
        std::fs::create_dir_all(&stable).unwrap();
        let marker = stable.join("temoin.txt");
        std::fs::write(&marker, "donnée Stable fictive").unwrap();
        let before = std::fs::metadata(&marker).unwrap().modified().unwrap();

        prepare(DEV_IDENTIFIER, true, root.path()).unwrap();
        prepare(DEV_IDENTIFIER, true, root.path()).unwrap(); // relance : idempotent

        assert_eq!(entries(&stable), vec![marker.clone()]);
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "donnée Stable fictive");
        assert_eq!(std::fs::metadata(&marker).unwrap().modified().unwrap(), before);
    }

    #[test]
    fn prepare_signale_un_dossier_impossible_a_creer() {
        let root = tempfile::tempdir().unwrap();
        // Un fichier à la place du dossier de l'identifiant empêche la création.
        std::fs::write(root.path().join(DEV_IDENTIFIER), "").unwrap();
        assert!(matches!(
            prepare(DEV_IDENTIFIER, true, root.path()),
            Err(EnvironmentError::DataDirCreation { .. })
        ));
    }

    #[test]
    fn le_channel_est_serialise_en_minuscules() {
        // Contrat avec l'interface TypeScript (type `Channel` dans src/lib/appInfo.ts).
        assert_eq!(serde_json::to_string(&Channel::Dev).unwrap(), "\"dev\"");
        assert_eq!(serde_json::to_string(&Channel::Stable).unwrap(), "\"stable\"");
    }
}
