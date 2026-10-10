/// Commandes Rust appelables depuis l'interface. Déclarer ici active le contrôle d'accès
/// de Tauri pour TOUTES les commandes de l'application : une commande absente de cette
/// liste, ou non autorisée dans `capabilities/`, est refusée à l'exécution.
/// Politique complète : docs/04_SECURITE.md, section « Commandes Rust et permissions ».
const APP_COMMANDS: &[&str] = &[
    "app_info",
    "list_destinations",
    "list_inbox_items",
    "create_inbox_item",
    "list_trashed_items",
    "get_inbox_item",
    "update_inbox_item",
    "trash_inbox_item",
    "restore_inbox_item",
    "convert_inbox_item_to_task",
    "cancel_task_conversion",
    "suggest_task_title",
    "list_tasks",
    "get_task",
    "list_converted_items",
];

fn main() {
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(APP_COMMANDS)),
    )
    .expect("échec du script de compilation Tauri");
}
