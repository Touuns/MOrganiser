import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../../App";
import { createFakeBackend, type FakeBackend } from "../../test/fakeBackend";
import { INITIATION_KEY, readProposalMemory } from "./initiationStore";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>(),
  isTauri: vi.fn(() => true),
}));
vi.mock("@tauri-apps/api/core", () => tauri);

let backend: FakeBackend;

beforeEach(() => {
  window.localStorage.clear();
  backend = createFakeBackend();
  tauri.invoke.mockReset();
  tauri.invoke.mockImplementation((command, args) =>
    command === "app_info"
      ? Promise.resolve({ channel: "dev", version: "0.1.0", dataDir: "X" })
      : backend.invoke(command, args),
  );
});
afterEach(() => vi.restoreAllMocks());

const field = () => screen.getByLabelText("Capture rapide", { selector: "textarea" }) as HTMLTextAreaElement;
const guide = () => screen.queryByRole("group", { name: /^Découvrir M'Organiser|^1\/2|^2\/2/ });
// Le bouton de l'en-tête (la proposition affiche aussi un bouton « Découvrir »).
const discover = () => document.querySelector(".app__header .app__discover") as HTMLButtonElement;
const send = async (text: string) => {
  fireEvent.change(field(), { target: { value: text } });
  fireEvent.keyDown(field(), { key: "Enter" });
  await waitFor(() => expect(field()).toHaveValue(""));
};
const home = () => document.querySelector(".inbox-home") as HTMLElement;
/** Laisse la détection asynchrone se terminer. */
const settle = () => act(async () => void (await new Promise((r) => setTimeout(r, 0))));

async function renderApp() {
  render(<App />);
  await screen.findByRole("heading", { name: "À organiser" });
  await settle();
}

describe("proposition automatique", () => {
  it("vraie première utilisation : proposée, et mémorisée dès l'affichage", async () => {
    await renderApp();
    expect(await screen.findByRole("group", { name: /^Découvrir M'Organiser/ })).toBeInTheDocument();
    expect(readProposalMemory()).toBe("seen");
    expect(backend.state.items).toHaveLength(0); // rien n'est créé
  });

  it("fermeture sans réponse : jamais reproposée au lancement suivant", async () => {
    await renderApp();
    await screen.findByRole("group", { name: /^Découvrir M'Organiser/ });
    cleanup(); // l'application est fermée : la mémoire locale, elle, subsiste
    await renderApp();
    expect(guide()).not.toBeInTheDocument();
  });

  it("installation existante (captures actives) : rien, mais le réglage est inscrit en silence", async () => {
    backend.seed("Déjà là");
    await renderApp();
    expect(guide()).not.toBeInTheDocument();
    expect(readProposalMemory()).toBe("seen");
  });

  it("une seule capture dans la corbeille suffit à ne pas proposer", async () => {
    const item = backend.seed("Supprimée");
    item.deletedAt = 5;
    await renderApp();
    expect(guide()).not.toBeInTheDocument();
    expect(readProposalMemory()).toBe("seen");
  });

  it("erreur SQLite : aucune proposition et aucune inscription (jamais « base vide »)", async () => {
    backend.state.failures.set("list_trashed_items", { code: "storage", message: "Lecture impossible." });
    await renderApp();
    expect(guide()).not.toBeInTheDocument();
    expect(window.localStorage.getItem(INITIATION_KEY)).toBeNull();
  });

  it("réponse incomplète : aucune proposition", async () => {
    tauri.invoke.mockImplementation((command, args) => {
      if (command === "app_info") return Promise.resolve({ channel: "dev", version: "0.1.0", dataDir: "X" });
      if (command === "list_trashed_items") return Promise.resolve({});
      return backend.invoke(command, args);
    });
    await renderApp();
    expect(guide()).not.toBeInTheDocument();
    expect(window.localStorage.getItem(INITIATION_KEY)).toBeNull();
  });

  it("stockage indisponible : aucune proposition, l'application fonctionne", async () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("bloqué");
    });
    await renderApp();
    expect(guide()).not.toBeInTheDocument();
    await send("Ça marche quand même");
    expect(backend.state.items).toHaveLength(1);
  });

  it("écriture non confirmée : pas de proposition (elle reviendrait à chaque lancement)", async () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => undefined);
    await renderApp();
    expect(guide()).not.toBeInTheDocument();
  });

  it("« Plus tard » ferme la proposition, sans rien créer", async () => {
    await renderApp();
    const card = await screen.findByRole("group", { name: /^Découvrir M'Organiser/ });
    fireEvent.click(within(card).getByRole("button", { name: "Plus tard" }));
    expect(guide()).not.toBeInTheDocument();
    expect(home()).not.toHaveAttribute("data-initiation");
    expect(field()).toHaveFocus();
    expect(backend.state.items).toHaveLength(0);
  });
});

describe("parcours", () => {
  it("le bouton « Découvrir » est permanent et lance la visite sur le vrai champ", async () => {
    backend.seed("Existante");
    await renderApp();
    expect(guide()).not.toBeInTheDocument();
    fireEvent.click(discover());
    expect(await screen.findByRole("group", { name: /^1\/2/ })).toBeInTheDocument();
    expect(home()).toHaveAttribute("data-initiation", "capture");
    expect(field()).toHaveFocus();
    expect(field()).toHaveValue("");
  });

  it("l'étape ne progresse pas sur un envoi vide", async () => {
    await renderApp();
    fireEvent.click(discover());
    fireEvent.change(field(), { target: { value: "   " } });
    fireEvent.keyDown(field(), { key: "Enter" });
    await settle();
    expect(home()).toHaveAttribute("data-initiation", "capture");
    expect(backend.state.items).toHaveLength(0);
  });

  it("l'étape ne progresse pas sur un envoi échoué, puis avance au succès", async () => {
    await renderApp();
    fireEvent.click(discover());
    backend.state.failures.set("create_inbox_item", { code: "storage", message: "Échec simulé." });
    fireEvent.change(field(), { target: { value: "Mon idée" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toBeInTheDocument();
    expect(home()).toHaveAttribute("data-initiation", "capture");
    backend.state.failures.delete("create_inbox_item");
    fireEvent.keyDown(field(), { key: "Enter" });
    await waitFor(() => expect(home()).toHaveAttribute("data-initiation", "box"));
    expect(backend.state.items).toHaveLength(1);
  });

  it("met en évidence la capture réellement créée, et elle seule", async () => {
    backend.seed("Ancienne");
    await renderApp();
    fireEvent.click(discover());
    await send("Vendre ma PlayStation 5");
    const created = backend.state.items.at(-1)!;
    await waitFor(() => expect(document.querySelectorAll("[data-initiation-target]")).toHaveLength(1));
    const target = document.querySelector("[data-initiation-target]") as HTMLElement;
    expect(target.dataset.captureId).toBe(created.id);
    expect(screen.getByText(/Votre capture est dans À organiser/)).toBeInTheDocument();
  });

  it("captures multiples : la plus récente est mise en évidence", async () => {
    await renderApp();
    fireEvent.click(discover());
    await send("Première");
    await send("Seconde");
    const latest = backend.state.items.at(-1)!;
    await waitFor(() =>
      expect((document.querySelector("[data-initiation-target]") as HTMLElement).dataset.captureId).toBe(latest.id),
    );
    expect(document.querySelectorAll("[data-initiation-target]")).toHaveLength(1);
  });

  it("filtre qui masque la capture : message et action explicite, sans rien perdre", async () => {
    await renderApp();
    fireEvent.click(discover());
    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "destination:finances" } });
    fireEvent.change(screen.getByLabelText("Destination (facultatif)", { selector: "select" }), {
      target: { value: "moi" },
    });
    fireEvent.change(field(), { target: { value: "Pour moi" } });
    // un brouillon est en cours : il doit survivre à l'action du parcours
    fireEvent.keyDown(field(), { key: "Enter" });
    await waitFor(() => expect(home()).toHaveAttribute("data-initiation", "box"));
    expect(await screen.findByText(/Un filtre masque votre capture/)).toBeInTheDocument();
    fireEvent.change(field(), { target: { value: "Brouillon à garder" } });
    fireEvent.click(screen.getByRole("button", { name: "Tout afficher" }));
    await waitFor(() => expect(document.querySelectorAll("[data-initiation-target]")).toHaveLength(1));
    expect(field()).toHaveValue("Brouillon à garder");
    expect(backend.state.items).toHaveLength(1);
  });

  it("hors de la boîte (« Voir tout ») : propose d'y revenir", async () => {
    for (let i = 0; i < 22; i += 1) backend.seed(`Capture ${i}`);
    await renderApp();
    fireEvent.click(discover());
    await send("Nouvelle");
    fireEvent.click(await screen.findByRole("button", { name: "Voir tout" }));
    expect(await screen.findByText(/Vous n'êtes pas sur la boîte/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Voir la boîte" }));
    await waitFor(() => expect(document.querySelectorAll("[data-initiation-target]")).toHaveLength(1));
  });

  it("Précédent, Terminer ; « Découvrir » relance autant de fois que voulu", async () => {
    await renderApp();
    for (let round = 0; round < 2; round += 1) {
      fireEvent.click(discover());
      await send(`Passage ${round}`);
      await screen.findByRole("group", { name: /^2\/2/ });
      fireEvent.click(screen.getByRole("button", { name: "Précédent" }));
      expect(home()).toHaveAttribute("data-initiation", "capture");
      await send(`Encore ${round}`);
      fireEvent.click(await screen.findByRole("button", { name: "Terminer" }));
      expect(guide()).not.toBeInTheDocument();
    }
    expect(backend.state.items).toHaveLength(4);
  });

  it("la dernière étape est visible en une ligne, avec « Terminer » et sans texte caché", async () => {
    await renderApp();
    fireEvent.click(discover());
    await send("Ma capture");
    const card = await screen.findByRole("group", { name: "2/2 · Votre capture est dans À organiser." });
    expect(card).toHaveTextContent("Votre capture est dans À organiser.");
    expect(within(card).getByRole("button", { name: "Terminer" })).toBeInTheDocument();
    expect(card.querySelectorAll("p")).toHaveLength(1); // aucun second texte réservé aux lecteurs d'écran
  });

  it("clavier : Terminer est atteignable et le brouillon en cours n'est jamais perdu", async () => {
    await renderApp();
    fireEvent.click(discover());
    await send("Première");
    fireEvent.change(field(), { target: { value: "Brouillon en cours" } });
    const terminer = await screen.findByRole("button", { name: "Terminer" });
    terminer.focus();
    fireEvent.click(terminer);
    expect(guide()).not.toBeInTheDocument();
    expect(field()).toHaveValue("Brouillon en cours");
    expect(backend.state.items).toHaveLength(1); // aucun envoi automatique
  });

  it("« Passer » et Échap arrêtent la visite sans rien modifier", async () => {
    await renderApp();
    fireEvent.click(discover());
    fireEvent.click(screen.getByRole("button", { name: "Passer" }));
    expect(guide()).not.toBeInTheDocument();
    expect(field()).toHaveFocus();
    fireEvent.click(discover());
    const card = screen.getByRole("group", { name: /^1\/2/ });
    fireEvent.keyDown(within(card).getByRole("button", { name: "Passer" }), { key: "Escape" });
    expect(guide()).not.toBeInTheDocument();
    expect(backend.state.items).toHaveLength(0);
  });

  it("Entrée et Maj+Entrée gardent leur rôle pendant la visite", async () => {
    await renderApp();
    fireEvent.click(discover());
    fireEvent.change(field(), { target: { value: "Ligne 1" } });
    fireEvent.keyDown(field(), { key: "Enter", shiftKey: true });
    await settle();
    expect(backend.state.items).toHaveLength(0);
    expect(home()).toHaveAttribute("data-initiation", "capture");
  });

  it("la visite ne bloque rien : la carte est hors du flux et le champ reste utilisable", async () => {
    await renderApp();
    fireEvent.click(discover());
    const card = screen.getByRole("group", { name: /^1\/2/ });
    expect(card.closest(".initiation-region")).not.toBeNull();
    expect(document.querySelector(".inbox-home__stage")?.contains(card)).toBe(true);
    expect(field()).not.toBeDisabled();
  });

  it("une visite lancée pendant la détection n'est pas écrasée par la proposition", async () => {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    tauri.invoke.mockImplementation(async (command, args) => {
      if (command === "app_info") return { channel: "dev", version: "0.1.0", dataDir: "X" };
      if (command === "list_trashed_items") await gate;
      return backend.invoke(command, args);
    });
    render(<App />);
    await screen.findByRole("heading", { name: "À organiser" });
    fireEvent.click(discover());
    release();
    await settle();
    expect(screen.queryByRole("group", { name: /^Découvrir M'Organiser/ })).not.toBeInTheDocument();
    expect(home()).toHaveAttribute("data-initiation", "capture");
  });
});

describe("aperçu navigateur", () => {
  it("hors de l'application : ni bouton « Découvrir » ni lecture de la base", async () => {
    tauri.isTauri.mockReturnValue(false);
    render(<App />);
    expect(screen.queryByRole("button", { name: "Découvrir" })).not.toBeInTheDocument();
    await settle();
    expect(backend.state.calls).toHaveLength(0);
    expect(window.localStorage.getItem(INITIATION_KEY)).toBeNull();
    tauri.isTauri.mockReturnValue(true);
  });
});
