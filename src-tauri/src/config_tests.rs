//! Tests de cohérence des fichiers de configuration réels (lus à la compilation).
//! Ils échouent si un identifiant, une commande de lancement ou une permission
//! diverge de ce que le code Rust attend.

use serde_json::Value;

use crate::environment::{self, Channel, DEV_IDENTIFIER, STABLE_IDENTIFIER};

const STABLE_CONF: &str = include_str!("../tauri.conf.json");
const DEV_CONF: &str = include_str!("../tauri.dev.conf.json");
const CAPABILITY: &str = include_str!("../capabilities/default.json");
const PACKAGE_JSON: &str = include_str!("../../package.json");

fn json(text: &str) -> Value {
    serde_json::from_str(text).expect("JSON invalide")
}

fn identifier(conf: &Value) -> &str {
    conf["identifier"].as_str().expect("identifiant manquant")
}

#[test]
fn les_identifiants_des_configurations_correspondent_au_code() {
    assert_eq!(identifier(&json(STABLE_CONF)), STABLE_IDENTIFIER);
    assert_eq!(identifier(&json(DEV_CONF)), DEV_IDENTIFIER);
    assert_eq!(Channel::from_identifier(identifier(&json(DEV_CONF))), Ok(Channel::Dev));
}

#[test]
fn les_configurations_menent_a_des_dossiers_distincts() {
    let root = tempfile::tempdir().unwrap();
    let (_, stable) = environment::prepare(identifier(&json(STABLE_CONF)), false, root.path()).unwrap();
    let (_, dev) = environment::prepare(identifier(&json(DEV_CONF)), true, root.path()).unwrap();
    assert_ne!(stable, dev);
    assert!(!dev.starts_with(&stable) && !stable.starts_with(&dev));
}

#[test]
fn la_configuration_par_defaut_est_refusee_en_debug() {
    // `cargo run` ou `tauri dev` sans la surcharge Dev doit être bloqué.
    let root = tempfile::tempdir().unwrap();
    assert!(environment::prepare(identifier(&json(STABLE_CONF)), true, root.path()).is_err());
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 0);
}

#[test]
fn les_executables_dev_et_stable_ont_des_noms_distincts() {
    assert_ne!(json(STABLE_CONF)["mainBinaryName"], json(DEV_CONF)["mainBinaryName"]);
}

#[test]
fn les_fenetres_sont_creees_apres_les_verifications() {
    // `create: false` : la fenêtre (et le cache WebView2) n'existe qu'après `prepare`.
    for conf in [json(STABLE_CONF), json(DEV_CONF)] {
        let windows = conf["app"]["windows"].as_array().expect("fenêtres manquantes");
        assert!(!windows.is_empty());
        for window in windows {
            assert_eq!(window["create"], Value::Bool(false), "fenêtre {}", window["label"]);
        }
    }
}

#[test]
fn les_scripts_dev_utilisent_la_configuration_dev() {
    let scripts = &json(PACKAGE_JSON)["scripts"];
    for name in ["app:dev", "app:build:dev"] {
        let script = scripts[name].as_str().expect("script manquant");
        assert!(
            script.contains("--config src-tauri/tauri.dev.conf.json"),
            "{name} doit cibler Dev : {script}"
        );
    }
}

#[test]
fn la_capacite_n_accorde_que_les_permissions_revues() {
    // Toute nouvelle permission doit être ajoutée ici consciemment (politique : 04_SECURITE.md).
    let capability = json(CAPABILITY);
    assert_eq!(capability["windows"], serde_json::json!(["main"]));
    assert_eq!(capability["permissions"], serde_json::json!(["allow-app-info"]));
    assert!(capability.get("remote").is_none(), "aucun accès distant autorisé");
}

#[test]
fn la_csp_n_autorise_aucune_origine_externe() {
    let conf = json(STABLE_CONF);
    let csp = conf["app"]["security"]["csp"].as_str().expect("CSP manquante");
    for forbidden in ["http://*", "https:", "'unsafe-eval'", "'unsafe-inline'", "*;"] {
        assert!(!csp.contains(forbidden), "CSP trop permissive : {forbidden}");
    }
    // La surcharge Dev ne doit pas remplacer la politique de sécurité.
    assert!(json(DEV_CONF)["app"].get("security").is_none());
}
