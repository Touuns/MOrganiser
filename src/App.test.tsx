import { render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";

// On remplace le pont Tauri : les tests tournent sans fenêtre Windows ni partie Rust.
const tauri = vi.hoisted(() => ({
  isTauri: vi.fn<() => boolean>(),
  invoke: vi.fn<(command: string) => Promise<unknown>>(),
}));
vi.mock("@tauri-apps/api/core", () => tauri);

describe("App (fenêtre de fondation)", () => {
  beforeEach(() => {
    tauri.isTauri.mockReset();
    tauri.invoke.mockReset();
  });

  it("affiche le nom de l'application et l'état des fondations", async () => {
    tauri.isTauri.mockReturnValue(false);
    render(<App />);
    expect(screen.getByText("M'Organiser")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Fondations" })).toBeInTheDocument();
    expect(await screen.findByText("Navigateur")).toBeInTheDocument();
  });

  it("dans un navigateur, n'appelle jamais la partie Rust", async () => {
    tauri.isTauri.mockReturnValue(false);
    render(<App />);
    await screen.findByText("Navigateur");
    expect(tauri.invoke).not.toHaveBeenCalled();
  });

  it("affiche l'environnement Dev et son dossier de données", async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockResolvedValue({
      channel: "dev",
      version: "0.1.0",
      dataDir: "C:\\Exemple\\com.morganiser.desktop.dev\\data",
    });
    render(<App />);
    expect(await screen.findByText("Dev")).toBeInTheDocument();
    expect(screen.getByText("C:\\Exemple\\com.morganiser.desktop.dev\\data")).toBeInTheDocument();
    expect(tauri.invoke).toHaveBeenCalledWith("app_info");
  });

  it("distingue visuellement Stable", async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockResolvedValue({ channel: "stable", version: "0.1.0", dataDir: "X" });
    render(<App />);
    expect(await screen.findByText("Stable")).toHaveClass("badge--stable");
  });

  it("signale clairement une erreur de lecture", async () => {
    tauri.isTauri.mockReturnValue(true);
    tauri.invoke.mockRejectedValue("échec simulé");
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("échec simulé");
  });
});
