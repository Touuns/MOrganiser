//! Point d'entrée de l'application M'Organiser (partie Rust).
//!
//! Au démarrage : on détermine l'environnement (Dev ou Stable) à partir de l'identifiant
//! configuré, on vérifie qu'il est cohérent avec le type de compilation, puis on prépare
//! le dossier de données propre à cet environnement, et enfin on ouvre la base SQLite
//! qui s'y trouve.

mod commands;
#[cfg(test)]
mod config_tests;
mod environment;
mod inbox;
mod storage;

use std::path::PathBuf;
use std::sync::Mutex;

use rusqlite::Connection;
use tauri::Manager;

use environment::Channel;

/// État partagé, accessible depuis les commandes.
pub struct AppEnvironment {
    pub channel: Channel,
    pub data_dir: PathBuf,
}

/// Connexion unique à la base de l'environnement courant. Le verrou fait passer les
/// accès un par un : deux écritures ne se chevauchent jamais.
pub struct Database(pub Mutex<Connection>);

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let local_data_root = app.path().local_data_dir()?;
            let (channel, data_dir) = environment::prepare(
                &app.config().identifier,
                cfg!(debug_assertions),
                &local_data_root,
            )?;

            // La base est toujours dans le dossier validé ci-dessus (Dev ou Stable).
            let conn = storage::open(&data_dir.join(storage::DB_FILE_NAME))?;
            app.manage(Database(Mutex::new(conn)));
            app.manage(AppEnvironment { channel, data_dir });

            // La fenêtre (et donc le cache WebView2 de l'environnement) n'est créée qu'après
            // les vérifications ci-dessus : voir `"create": false` dans tauri.conf.json.
            for window in &app.config().app.windows {
                tauri::WebviewWindowBuilder::from_config(app.handle(), window)?.build()?;
            }
            Ok(())
        })
        // Toute commande ajoutée ici doit aussi figurer dans `build.rs` (APP_COMMANDS)
        // et être autorisée dans `capabilities/default.json`.
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_destinations,
            commands::list_inbox_items,
            commands::create_inbox_item,
        ])
        .build(tauri::generate_context!())
        .expect("erreur au lancement de M'Organiser")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // Fermeture : intégrer le journal WAL dans morganiser.db (voir storage::checkpoint).
                // En cas d'échec, rien n'est perdu : les données restent dans -wal.
                if let Some(db) = app.try_state::<Database>() {
                    if let Ok(conn) = db.0.lock() {
                        match storage::checkpoint(&conn) {
                            Ok(storage::Checkpoint::Complete) => {}
                            Ok(storage::Checkpoint::Incomplete) => eprintln!(
                                "[M'Organiser] checkpoint incomplet : données conservées dans -wal"
                            ),
                            Err(error) => eprintln!(
                                "[M'Organiser] checkpoint impossible ({error}) : données conservées dans -wal"
                            ),
                        }
                    }
                }
            }
        });
}
