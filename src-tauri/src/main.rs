// En version compilée « release », pas de console noire derrière la fenêtre Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    morganiser_lib::run()
}
