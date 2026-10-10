import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

// On remplace le pont Tauri : les tests tournent sans fenêtre Windows ni partie Rust.
const tauri = vi.hoisted(() => ({
  isTauri: vi.fn<() => boolean>(),
  invoke: vi.fn<(command: string) => Promise<unknown>>(),
}));
vi.mock("@tauri-apps/api/core", () => tauri);

function backend(appInfo: unknown) {
  return (command: string) => {
    switch (command) {
      case "app_info":
        return Promise.resolve(appInfo);
      case "list_destinations":
        return Promise.resolve([]);
      case "list_inbox_items":
        return Promise.resolve({ items: [], total: 0 });
      default:
        return Promise.reject({ code: "unknown", message: command });
    }
  };
}

describe("App", () => {
  beforeEach(() => {
    tauri.isTauri.mockReset();
    tauri.invoke.mockReset();
  });

  it("dans un navigateur : aucun appel à Rust, la capture est indiquée indisponible", async () => {
    tauri.isTauri.mockReturnValue(false);
    render(<App />);
    expect(screen.getByText("M'Organiser")).toBeInTheDocument();
    expect(await screen.findByText("Navigateur")).toBeInTheDocument();
    expect(screen.getByText(/La capture fonctionne uniquement dans l'application Windows/)).toBeInTheDocument();
    expect(screen.queryByLabelText("Capture rapide", { selector: "textarea" })).not.toBeInTheDocument();
    expect(tauri.invoke).not.toHaveBeenCalled();
  });

  it("dans l'application Dev : badge Dev, boîte et capture visibles, dossier de données affiché", async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockImplementation(
      backend({ channel: "dev", version: "0.1.0", dataDir: "C:\\Exemple\\com.morganiser.desktop.dev\\data" }),
    );
    render(<App />);
    expect(await screen.findByText("Dev")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "À organiser" })).toBeInTheDocument();
    expect(screen.getByLabelText("Capture rapide", { selector: "textarea" })).toBeInTheDocument();
    expect(screen.getByText("C:\\Exemple\\com.morganiser.desktop.dev\\data")).toBeInTheDocument();
  });

  it("distingue visuellement Stable", async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockImplementation(backend({ channel: "stable", version: "0.1.0", dataDir: "X" }));
    render(<App />);
    expect(await screen.findByText("Stable")).toHaveClass("badge--stable");
  });

  it("signale clairement une erreur de lecture de l'environnement", async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockImplementation((command: string) =>
      command === "app_info" ? Promise.reject("échec simulé") : backend(null)(command),
    );
    render(<App />);
    expect(await screen.findByText(/Impossible de lire l'environnement/)).toHaveTextContent("échec simulé");
  });
});
