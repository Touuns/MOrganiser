//! Point d'entrée de l'application M'Organiser (partie Rust).
//!
//! Au démarrage : on détermine l'environnement (Dev ou Stable) à partir de l'identifiant
//! configuré, on vérifie qu'il est cohérent avec le type de compilation, puis on prépare
//! le dossier de données propre à cet environnement.

mod commands;
#[cfg(test)]
mod config_tests;
mod environment;

use std::path::PathBuf;

use tauri::Manager;

use environment::Channel;

/// État partagé, accessible depuis les commandes.
pub struct AppEnvironment {
    pub channel: Channel,
    pub data_dir: PathBuf,
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let local_data_root = app.path().local_data_dir()?;
            let (channel, data_dir) = environment::prepare(
                &app.config().identifier,
                cfg!(debug_assertions),
                &local_data_root,
            )?;

            app.manage(AppEnvironment { channel, data_dir });

            // La fenêtre (et donc le cache WebView2 de l'environnement) n'est créée qu'après
            // les vérifications ci-dessus : voir `"create": false` dans tauri.conf.json.
            for window in &app.config().app.windows {
                tauri::WebviewWindowBuilder::from_config(app.handle(), window)?.build()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de M'Organiser");
}
