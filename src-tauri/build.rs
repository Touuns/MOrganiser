/// Commandes Rust appelables depuis l'interface. Déclarer ici active le contrôle d'accès
/// de Tauri pour TOUTES les commandes de l'application : une commande absente de cette
/// liste, ou non autorisée dans `capabilities/`, est refusée à l'exécution.
/// Politique complète : docs/04_SECURITE.md, section « Commandes Rust et permissions ».
const APP_COMMANDS: &[&str] = &["app_info"];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS)),
    )
    .expect("échec du script de compilation Tauri");
}
