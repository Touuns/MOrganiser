import { invoke, isTauri } from "@tauri-apps/api/core";

/** Doit rester aligné avec `Channel` dans src-tauri/src/environment.rs. */
export type Channel = "dev" | "stable";

/** Doit rester aligné avec `AppInfo` dans src-tauri/src/commands.rs. */
export interface AppInfo {
  channel: Channel;
  version: string;
  dataDir: string;
}

/**
 * Demande à la partie Rust l'environnement courant.
 * Renvoie `null` si l'interface tourne dans un simple navigateur (`pnpm dev`).
 */
export async function fetchAppInfo(): Promise<AppInfo | null> {
  if (!isTauri()) return null;
  return invoke<AppInfo>("app_info");
}
