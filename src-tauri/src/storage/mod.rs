//! Ouverture de la base SQLite locale et migrations versionnées.
//!
//! Le numéro de version du schéma est stocké dans la base (`PRAGMA user_version`).
//! Chaque migration s'applique dans sa propre transaction : en cas d'échec, la base
//! reste dans la version précédente.
//!
//! Avant de migrer une base **existante**, une sauvegarde cohérente et vérifiée est faite
//! (voir `backup`). Si elle échoue, la migration n'a pas lieu.

mod backup;

use std::fmt;
use std::path::Path;

use rusqlite::{Connection, TransactionBehavior};

/// Nom du fichier de base, dans le dossier `data` de l'environnement (Dev ou Stable).
pub const DB_FILE_NAME: &str = "morganiser.db";

/// Migrations dans l'ordre ; la migration d'indice `i` amène le schéma en version `i + 1`.
const MIGRATIONS: &[&str] = &[
    include_str!("../../migrations/0001_initial.sql"),
    include_str!("../../migrations/0002_tasks.sql"),
    include_str!("../../migrations/0003_task_model.sql"),
];

/// Version du schéma que cette version de l'application sait utiliser.
// Référence de la version courante : utilisée par les tests et la documentation ; `open` en déduit
// la cible de `MIGRATIONS.len()`.
#[cfg_attr(not(test), allow(dead_code))]
pub const SCHEMA_VERSION: u32 = MIGRATIONS.len() as u32;

/// Nom du fichier d'autorisation de migration, dans le dossier de données de l'environnement.
pub const PERMIT_FILE_NAME: &str = "migration-authorization.txt";

/// Autorisation explicite de migrer UNE base existante d'une version précise vers une autre.
/// Contenu du fichier (trois lignes exactes) : `channel=dev`, `from=2`, `to=3`.
/// Elle est inopérante dès que la version de la base a changé (usage unique de fait) et n'est
/// JAMAIS lue pour Stable : une migration de données Stable ne se fait pas silencieusement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationPermit {
    pub from: u32,
    pub to: u32,
}

impl MigrationPermit {
    /// Analyse stricte du contenu : tout écart (ligne en plus, autre canal, valeur illisible) = refus.
    fn parse(text: &str) -> Option<Self> {
        let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let [channel, from, to] = lines.as_slice() else { return None };
        if *channel != "channel=dev" {
            return None;
        }
        Some(MigrationPermit {
            from: from.strip_prefix("from=")?.parse().ok()?,
            to: to.strip_prefix("to=")?.parse().ok()?,
        })
    }

    /// Lit l'autorisation du dossier de données. `None` pour Stable, ou si le fichier est absent
    /// ou invalide : le défaut est toujours le refus.
    pub fn load(data_dir: &Path, channel: crate::environment::Channel) -> Option<Self> {
        if channel != crate::environment::Channel::Dev {
            return None;
        }
        let text = std::fs::read_to_string(data_dir.join(PERMIT_FILE_NAME)).ok()?;
        Self::parse(&text)
    }
}

/// Ouverture de PRODUCTION. Une base neuve (aucune table) est créée et migrée librement. Une base
/// existante à migrer n'est migrée que si `permit` désigne exactement sa version actuelle et la
/// version cible ; sinon elle est refusée AVANT toute écriture (aucun réglage, aucune sauvegarde).
pub fn open_guarded(path: &Path, permit: Option<MigrationPermit>) -> Result<Connection, StorageError> {
    let target = MIGRATIONS.len() as u32;
    // Sonde en LECTURE SEULE : ni réglage, ni checkpoint du -wal, ni création de fichier.
    if path.exists() {
        let probe = Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let current = check_schema_version_against(&probe, target)?;
        let tables: u32 = probe.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%'",
            [],
            |row| row.get(0),
        )?;
        let is_new = current == 0 && tables == 0;
        let needs_migration = current < target;
        if needs_migration && !is_new {
            let authorized = permit.is_some_and(|p| p.from == current && p.to == target);
            if !authorized {
                return Err(StorageError::MigrationNotAuthorized { found: current, target });
            }
        }
    }
    open_with(path, MIGRATIONS)
}

/// Ouverture SANS garde, réservée aux tests : la production passe par [`open_guarded`].
#[cfg(test)]
pub fn open(path: &Path) -> Result<Connection, StorageError> {
    open_with(path, MIGRATIONS)
}

/// `open` avec une liste de migrations explicite : la version cible est `migrations.len()`.
/// Réservée au dispositif de répétition (qui rejoue une étape précise, par exemple v1 → v2) et aux
/// tests ; l'application passe toujours par [`open`], donc par la liste complète.
fn open_with(path: &Path, migrations: &[&str]) -> Result<Connection, StorageError> {
    let target = migrations.len() as u32;
    let mut conn = Connection::open(path)?;
    // D'abord une simple lecture : une base d'une version future est refusée AVANT tout
    // réglage susceptible de la modifier (le passage en WAL est écrit dans le fichier).
    let current = check_schema_version_against(&conn, target)?;
    // Base existante à migrer : sauvegarde cohérente et vérifiée AVANT toute modification.
    // Un échec arrête le démarrage ; la base reste exactement dans son état d'origine.
    if current > 0 && current < target {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        backup::before_migration(&conn, path, current, now_ms).map_err(StorageError::Backup)?;
    }
    configure(&conn)?;
    migrate_with(&mut conn, migrations)?;
    Ok(conn)
}

/// Lit la version du schéma (lecture seule) et refuse une version plus récente que `supported`.
fn check_schema_version_against(conn: &Connection, supported: u32) -> Result<u32, StorageError> {
    let current: u32 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if current > supported {
        return Err(StorageError::NewerSchema { found: current, supported });
    }
    Ok(current)
}

fn configure(conn: &Connection) -> Result<(), StorageError> {
    // Les références (ex. destination d'une capture) sont vérifiées par SQLite.
    conn.pragma_update(None, "foreign_keys", true)?;
    // WAL : chaque écriture validée est d'abord ajoutée à `morganiser.db-wal`, puis
    // reportée plus tard dans `morganiser.db`. Les trois fichiers (`.db`, `-wal`, `-shm`)
    // forment ensemble la base : voir la fiche brique 001, section 7.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    // FULL : chaque validation est forcée sur le disque avant de répondre « enregistré ».
    // Une capture confirmée survit ainsi à un arrêt brutal, y compris une coupure de courant.
    conn.pragma_update(None, "synchronous", "FULL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(())
}

/// Applique les migrations manquantes et renvoie la version finale du schéma.
#[cfg_attr(not(test), allow(dead_code))]
pub fn migrate(conn: &mut Connection) -> Result<u32, StorageError> {
    migrate_with(conn, MIGRATIONS)
}

/// Une transaction par migration : le schéma ET `user_version` changent ensemble, ou pas du
/// tout. (Séparée de `migrate` pour pouvoir tester un échec en cours de route.)
///
/// - La transaction est `IMMEDIATE` : le verrou d'écriture est pris AVANT toute lecture de
///   contrôle, et la version est relue sous ce verrou (un autre processus a pu migrer entre-temps :
///   la migration n'est alors pas rejouée).
/// - Avant de valider, les clés étrangères sont contrôlées : une migration qui laisse une
///   violation est annulée en entier, jamais validée « à moitié ».
fn migrate_with(conn: &mut Connection, migrations: &[&str]) -> Result<u32, StorageError> {
    // Contrôle répété ici pour que `migrate` reste sûre si elle est appelée seule.
    let current = check_schema_version_against(conn, migrations.len() as u32)?;
    for (index, sql) in migrations.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let locked: u32 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if locked > migrations.len() as u32 {
            return Err(StorageError::NewerSchema { found: locked, supported: migrations.len() as u32 });
        }
        if locked as usize > index {
            continue; // déjà appliquée par un autre processus ; `tx` est annulée sans effet
        }
        tx.execute_batch(sql)?;
        let violations: i64 =
            tx.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| row.get(0))?;
        if violations > 0 {
            return Err(StorageError::ForeignKeys { migration: index as u32 + 1, violations });
        }
        tx.pragma_update(None, "user_version", index as u32 + 1)?;
        tx.commit()?;
    }
    Ok(migrations.len() as u32)
}

/// Résultat d'un checkpoint.
#[derive(Debug, PartialEq, Eq)]
pub enum Checkpoint {
    /// Tout le contenu de `-wal` a été reporté dans `.db`, et `-wal` a été vidé.
    Complete,
    /// Checkpoint bloqué (ex. lecture en cours) : les données restent dans `-wal`,
    /// intactes, et seront reportées plus tard. Aucune perte.
    Incomplete,
}

/// Reporte dans `morganiser.db` le contenu de `morganiser.db-wal`, appelé à la fermeture
/// normale. Opération sûre : SQLite ne vide `-wal` qu'après avoir écrit et synchronisé
/// `.db` ; une interruption en cours de route laisse `-wal` intact, relu à l'ouverture
/// suivante. Ce n'est PAS une sauvegarde : rien ne garantit qu'il a eu lieu (arrêt
/// brutal, extinction de Windows), donc `.db` seul ne suffit jamais à copier la base.
pub fn checkpoint(conn: &Connection) -> Result<Checkpoint, StorageError> {
    // SQLite répond (occupé, pages du journal, pages reportées) ; « occupé » n'est pas
    // une erreur, il faut le lire explicitement.
    let busy: i64 = conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))?;
    Ok(if busy == 0 {
        Checkpoint::Complete
    } else {
        Checkpoint::Incomplete
    })
}

#[derive(Debug)]
pub enum StorageError {
    Sqlite(rusqlite::Error),
    NewerSchema { found: u32, supported: u32 },
    /// La sauvegarde préalable à la migration a échoué : la base n'a pas été migrée.
    Backup(String),
    /// La migration laissait des violations de clés étrangères : annulée, base inchangée.
    ForeignKeys { migration: u32, violations: i64 },
    /// Base existante à migrer, sans autorisation explicite : rien n'a été modifié.
    MigrationNotAuthorized { found: u32, target: u32 },
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::Sqlite(error) => write!(f, "erreur de la base de données : {error}"),
            StorageError::Backup(reason) => write!(
                f,
                "la sauvegarde avant migration a échoué ({reason}) ; la base n'a pas été modifiée"
            ),
            StorageError::ForeignKeys { migration, violations } => write!(
                f,
                "la migration {migration} laissait {violations} violation(s) de clés étrangères ; \n                 elle a été annulée et la base n'a pas été modifiée"
            ),
            StorageError::MigrationNotAuthorized { found, target } => write!(
                f,
                "la base existante (schéma v{found}) doit être migrée en v{target}, mais aucune                  autorisation explicite n'a été fournie ; elle n'a pas été modifiée. Procédez à la                  répétition sur copie isolée, puis suivez la procédure d'autorisation (fiche 003 §17)"
            ),
            StorageError::NewerSchema { found, supported } => write!(
                f,
                "la base de données (schéma v{found}) provient d'une version plus récente \
                 de M'Organiser (cette version gère jusqu'à v{supported}) ; elle n'a pas été modifiée"
            ),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<rusqlite::Error> for StorageError {
    fn from(error: rusqlite::Error) -> Self {
        StorageError::Sqlite(error)
    }
}

/// Base en mémoire, migrée, pour les tests des modules de domaine.
#[cfg(test)]
pub fn open_in_memory() -> Connection {
    let mut conn = Connection::open_in_memory().unwrap();
    configure(&conn).unwrap();
    migrate(&mut conn).unwrap();
    conn
}

#[cfg(test)]
mod backup_tests;

#[cfg(test)]
mod rehearsal_tests;

#[cfg(test)]
mod rebuild_proof_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn user_version(conn: &Connection) -> u32 {
        conn.pragma_query_value(None, "user_version", |row| row.get(0)).unwrap()
    }

    // --- Garde-fou contre une migration automatique (003-B1d) : bases FICTIVES uniquement ---

    fn fictitious_v2(path: &Path) {
        let mut conn = Connection::open(path).unwrap();
        configure(&conn).unwrap();
        migrate_with(&mut conn, &MIGRATIONS[..2]).unwrap();
        conn.execute(
            "INSERT INTO inbox_items (id, content, created_at, updated_at) VALUES ('c', 'Fictive', 1, 1)",
            [],
        )
        .unwrap();
    }

    fn permit(from: u32, to: u32) -> Option<MigrationPermit> {
        Some(MigrationPermit { from, to })
    }

    #[test]
    fn une_base_v2_sans_autorisation_est_refusee_et_reste_octet_pour_octet_intacte() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        fictitious_v2(&path);
        let before = std::fs::read(&path).unwrap();
        let wal_before = std::fs::metadata(wal(&path)).ok().map(|m| m.len());

        let error = open_guarded(&path, None).unwrap_err();
        assert!(matches!(error, StorageError::MigrationNotAuthorized { found: 2, target: 3 }), "{error:?}");
        assert_eq!(std::fs::read(&path).unwrap(), before, "base modifiée par le refus");
        // La lecture d'une base WAL peut laisser un `-wal` VIDE (aucune donnée) ; jamais plus.
        let wal_len = std::fs::metadata(wal(&path)).map(|m| m.len()).unwrap_or(0);
        assert_eq!(wal_len, wal_before.unwrap_or(0), "le -wal ne doit contenir aucune écriture");
        assert!(!backup::backup_dir(&path).exists(), "aucune sauvegarde créée par un refus");
        let conn = Connection::open(&path).unwrap();
        assert_eq!(user_version(&conn), 2);
    }

    #[test]
    fn une_autorisation_mal_ciblee_est_refusee() {
        for wrong in [permit(1, 3), permit(2, 2), permit(3, 4), permit(1, 2)] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(DB_FILE_NAME);
            fictitious_v2(&path);
            assert!(matches!(
                open_guarded(&path, wrong),
                Err(StorageError::MigrationNotAuthorized { .. })
            ));
            assert_eq!(user_version(&Connection::open(&path).unwrap()), 2);
        }
    }

    #[test]
    fn une_autorisation_exacte_migre_avec_sauvegarde_et_conserve_les_donnees() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        fictitious_v2(&path);
        let conn = open_guarded(&path, permit(2, 3)).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        assert_eq!(count(&conn), 1);
        let backups: Vec<_> = std::fs::read_dir(backup::backup_dir(&path)).unwrap().flatten().collect();
        assert_eq!(backups.len(), 1, "la sauvegarde préalable reste obligatoire");
    }

    #[test]
    fn une_base_neuve_est_creee_sans_autorisation_et_une_base_a_jour_s_ouvre() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let conn = open_guarded(&path, None).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        drop(conn);
        assert_eq!(user_version(&open_guarded(&path, None).unwrap()), SCHEMA_VERSION);
        assert!(!backup::backup_dir(&path).exists(), "pas de sauvegarde pour une base neuve");
    }

    #[test]
    fn un_fichier_sans_version_mais_avec_des_tables_n_est_pas_une_base_neuve() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch("CREATE TABLE inconnue (x TEXT); INSERT INTO inconnue VALUES ('a');").unwrap();
        }
        assert!(matches!(
            open_guarded(&path, None),
            Err(StorageError::MigrationNotAuthorized { found: 0, .. })
        ));
    }

    #[test]
    fn un_schema_plus_recent_reste_refuse_meme_avec_autorisation() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        Connection::open(&path).unwrap().pragma_update(None, "user_version", 99).unwrap();
        assert!(matches!(open_guarded(&path, permit(2, 3)), Err(StorageError::NewerSchema { found: 99, .. })));
    }

    #[test]
    fn l_autorisation_est_stricte_propre_a_dev_et_jamais_lue_pour_stable() {
        use crate::environment::Channel;
        assert_eq!(
            MigrationPermit::parse("channel=dev
from=2
to=3
"),
            Some(MigrationPermit { from: 2, to: 3 })
        );
        for bad in [
            "", "from=2
to=3", "channel=stable
from=2
to=3", "channel=dev
from=2",
            "channel=dev
from=a
to=3", "channel=dev
from=2
to=3
extra=1", "channel=dev
to=3
from=2",
        ] {
            assert_eq!(MigrationPermit::parse(bad), None, "{bad:?}");
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(PERMIT_FILE_NAME), "channel=dev
from=2
to=3
").unwrap();
        assert!(MigrationPermit::load(dir.path(), Channel::Dev).is_some());
        assert!(MigrationPermit::load(dir.path(), Channel::Stable).is_none());
        assert!(MigrationPermit::load(&dir.path().join("absent"), Channel::Dev).is_none());
    }

    #[test]
    fn une_nouvelle_base_est_migree_a_la_derniere_version() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join(DB_FILE_NAME)).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
        let destinations: u32 = conn
            .query_row("SELECT count(*) FROM destinations", [], |row| row.get(0))
            .unwrap();
        assert_eq!(destinations, 5);
    }

    #[test]
    fn rouvrir_la_base_ne_rejoue_pas_les_migrations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        open(&path).unwrap();
        // Une seconde ouverture rejouerait `CREATE TABLE` et échouerait si la version était ignorée.
        let conn = open(&path).unwrap();
        assert_eq!(user_version(&conn), SCHEMA_VERSION);
    }

    #[test]
    fn une_base_plus_recente_est_refusee_sans_etre_modifiee() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        assert!(matches!(
            open(&path),
            Err(StorageError::NewerSchema { found: 99, .. })
        ));
        let conn = Connection::open(&path).unwrap();
        assert_eq!(user_version(&conn), 99);
        let tables: u32 = conn
            .query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(tables, 0, "aucune table ne doit avoir été créée");
    }

    #[test]
    fn une_base_future_en_journal_classique_reste_strictement_intacte() {
        // Base d'une version future, en journalisation classique (DELETE, pas WAL),
        // avec une donnée : le refus ne doit RIEN modifier, pas même le mode de journal.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "DELETE").unwrap();
            conn.execute_batch("CREATE TABLE future (x TEXT); INSERT INTO future VALUES ('v99');")
                .unwrap();
            conn.pragma_update(None, "user_version", 99).unwrap();
        }
        let before = std::fs::read(&path).unwrap();

        assert!(matches!(
            open(&path),
            Err(StorageError::NewerSchema { found: 99, .. })
        ));

        let after = std::fs::read(&path).unwrap();
        // Octets 18-19 de l'en-tête : 1 = journal classique, 2 = WAL.
        assert_eq!(after[18..20], before[18..20], "le refus a changé le mode de journal");
        assert!(after == before, "le refus a modifié le fichier de base");
        assert!(!wal(&path).exists(), "aucun fichier -wal ne doit être créé");
        let conn = Connection::open(&path).unwrap();
        let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0)).unwrap();
        assert_eq!(journal.to_lowercase(), "delete", "le mode de journal a été changé");
        assert_eq!(user_version(&conn), 99);
    }

    #[test]
    fn apres_checkpoint_complet_le_fichier_db_contient_tout() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let conn = open(&path).unwrap();
        insert(&conn, "a");
        assert_eq!(checkpoint(&conn).unwrap(), Checkpoint::Complete);
        assert_eq!(std::fs::metadata(wal(&path)).unwrap().len(), 0, "-wal vidé");
        // Après un checkpoint COMPLET (et seulement dans ce cas), .db suffit.
        let copy_dir = tempfile::tempdir().unwrap();
        let copy = copy_dir.path().join(DB_FILE_NAME);
        std::fs::copy(&path, &copy).unwrap();
        drop(conn);
        assert_eq!(count(&Connection::open(&copy).unwrap()), 1);
    }

    #[test]
    fn les_reglages_de_securite_sont_actifs() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(&dir.path().join(DB_FILE_NAME)).unwrap();
        let foreign_keys: bool = conn.pragma_query_value(None, "foreign_keys", |row| row.get(0)).unwrap();
        let journal: String = conn.pragma_query_value(None, "journal_mode", |row| row.get(0)).unwrap();
        let synchronous: i64 = conn.pragma_query_value(None, "synchronous", |row| row.get(0)).unwrap();
        assert!(foreign_keys);
        assert_eq!(journal.to_lowercase(), "wal");
        assert_eq!(synchronous, 2, "synchronous = FULL");
    }

    // --- Interruption brutale et fichiers auxiliaires ---

    fn wal(path: &Path) -> std::path::PathBuf {
        path.with_file_name(format!("{DB_FILE_NAME}-wal"))
    }

    fn insert(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO inbox_items (id, content, created_at, updated_at) VALUES (?1, 'Donnée validée', 1, 1)",
            [id],
        )
        .unwrap();
    }

    fn count(conn: &Connection) -> u32 {
        conn.query_row("SELECT count(*) FROM inbox_items", [], |row| row.get(0)).unwrap()
    }

    const CHILD_DB_ENV: &str = "MORGANISER_TEST_CHILD_DB";
    const CHILD_READY: &str = "CAPTURE_VALIDEE";

    /// Exécuté uniquement comme sous-processus par le test d'interruption brutale.
    #[test]
    #[ignore = "sous-processus du test interruption_brutale_apres_validation_aucune_perte"]
    fn sous_processus_ecrit_puis_attend_d_etre_tue() {
        let Ok(path) = std::env::var(CHILD_DB_ENV) else { return };
        let conn = open(Path::new(&path)).unwrap();
        insert(&conn, "validee-avant-arret");
        println!("{CHILD_READY}"); // l'écriture est validée (commit terminé)
        std::thread::sleep(std::time::Duration::from_secs(60)); // tué avant la fin
    }

    #[test]
    fn interruption_brutale_apres_validation_aucune_perte() {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "storage::tests::sous_processus_ecrit_puis_attend_d_etre_tue",
                "--ignored",
                "--nocapture",
            ])
            .env(CHILD_DB_ENV, &path)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        assert!(
            stdout.lines().map_while(Result::ok).any(|line| line.contains(CHILD_READY)),
            "le sous-processus n'a pas confirmé l'écriture"
        );
        // Arrêt brutal (TerminateProcess sous Windows) : ni fermeture, ni checkpoint.
        child.kill().unwrap();
        child.wait().unwrap();

        assert!(std::fs::metadata(wal(&path)).unwrap().len() > 0, "la donnée est dans -wal");
        // Réouverture normale : SQLite relit -wal, la capture validée est là.
        let conn = open(&path).unwrap();
        assert_eq!(count(&conn), 1);
    }

    #[test]
    fn sans_checkpoint_copier_db_seul_perd_les_donnees_mais_db_et_wal_les_gardent() {
        // Documente le piège : état de la base après un arrêt brutal, avant tout checkpoint.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let conn = open(&path).unwrap();
        insert(&conn, "a");

        let naive = tempfile::tempdir().unwrap();
        std::fs::copy(&path, naive.path().join(DB_FILE_NAME)).unwrap();
        let both = tempfile::tempdir().unwrap();
        std::fs::copy(&path, both.path().join(DB_FILE_NAME)).unwrap();
        std::fs::copy(wal(&path), wal(&both.path().join(DB_FILE_NAME))).unwrap();
        drop(conn);

        // `.db` seul : la capture validée manque (le schéma lui-même peut manquer).
        let naive_conn = Connection::open(naive.path().join(DB_FILE_NAME)).unwrap();
        let naive_count: Result<u32, _> =
            naive_conn.query_row("SELECT count(*) FROM inbox_items", [], |row| row.get(0));
        assert!(!matches!(naive_count, Ok(1)), "la copie naïve ne devrait pas contenir la capture");
        // `.db` + `-wal` (sans `-shm`, reconstruit automatiquement) : tout est récupéré.
        assert_eq!(count(&open(&both.path().join(DB_FILE_NAME)).unwrap()), 1);
    }

    #[test]
    fn un_checkpoint_bloque_est_signale_et_ne_perd_rien() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DB_FILE_NAME);
        let conn = open(&path).unwrap();
        conn.busy_timeout(std::time::Duration::from_millis(100)).unwrap();
        insert(&conn, "a");

        // Une lecture en cours sur une autre connexion empêche de vider -wal.
        let reader = Connection::open(&path).unwrap();
        reader.execute_batch("BEGIN; SELECT count(*) FROM inbox_items;").unwrap();
        insert(&conn, "b");
        assert_eq!(checkpoint(&conn).unwrap(), Checkpoint::Incomplete);
        reader.execute_batch("COMMIT;").unwrap();
        drop(reader);
        drop(conn);

        assert_eq!(count(&open(&path).unwrap()), 2, "aucune perte malgré le checkpoint bloqué");
    }
}
