import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeBackend, type FakeBackend } from "../../test/fakeBackend";
import { InboxHome } from "./InboxHome";

// Faux pont Tauri (base en mémoire) et fausse fenêtre : capte le gestionnaire de fermeture.
const tauri = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>(),
  isTauri: vi.fn(() => true),
}));
vi.mock("@tauri-apps/api/core", () => tauri);

interface CloseEvent {
  preventDefault: () => void;
}
const appWindow = vi.hoisted(() => ({
  handler: null as null | ((event: CloseEvent) => void | Promise<void>),
  destroy: vi.fn(),
  unlisten: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: async (handler: (event: CloseEvent) => void | Promise<void>) => {
      appWindow.handler = handler;
      return appWindow.unlisten;
    },
    destroy: appWindow.destroy,
  }),
}));

let backend: FakeBackend;

beforeEach(() => {
  appWindow.handler = null;
  appWindow.destroy.mockReset();
  appWindow.unlisten.mockReset();
  backend = createFakeBackend();
  tauri.invoke.mockReset();
  tauri.invoke.mockImplementation((command, args) => backend.invoke(command, args));
});

const field = () => screen.getByLabelText("Capture rapide", { selector: "textarea" });
const detail = () => screen.getByRole("region", { name: /Fiche de la capture/ });
const detailText = () => within(detail()).getByLabelText("Texte") as HTMLTextAreaElement;
const openCard = async (text: string) =>
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(text) }));
const draftBanner = () => screen.findByText(/Vous avez des modifications non enregistrées/);
const captureBanner = () => screen.findByText(/texte non envoyé dans la capture rapide/);

/** Simule la demande de fermeture de Tauri ; renvoie si la fermeture a été suspendue. */
async function requestClose(): Promise<boolean> {
  await waitFor(() => expect(appWindow.handler).not.toBeNull());
  const event = { preventDefault: vi.fn() };
  await act(async () => {
    await appWindow.handler!(event);
  });
  return event.preventDefault.mock.calls.length > 0;
}

describe("Fermeture de la fenêtre protégée", () => {
  it("sans rien à perdre : fermeture normale, sans confirmation", async () => {
    backend.seed("Rien à signaler");
    render(<InboxHome />);
    await screen.findByText("Rien à signaler");
    expect(await requestClose()).toBe(false);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(appWindow.destroy).not.toHaveBeenCalled(); // c'est Tauri qui ferme
  });

  it("une fiche ouverte mais non modifiée ne bloque pas la fermeture", async () => {
    backend.seed("Simple lecture");
    render(<InboxHome />);
    await openCard("Simple lecture");
    expect(await requestClose()).toBe(false);
  });

  it("fiche modifiée → « Continuer à modifier » : la fenêtre reste ouverte", async () => {
    backend.seed("Texte");
    render(<InboxHome />);
    await openCard("Texte");
    fireEvent.change(detailText(), { target: { value: "Brouillon" } });
    expect(await requestClose()).toBe(true);
    await draftBanner();
    fireEvent.click(screen.getByRole("button", { name: "Continuer à modifier" }));
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(detailText()).toHaveValue("Brouillon");
    expect(screen.queryByText(/modifications non enregistrées/)).not.toBeInTheDocument();
    expect(detailText()).toHaveFocus(); // on peut poursuivre au clavier
  });

  it("fiche modifiée → « Abandonner » : fermeture effective, une seule fois, sans nouvelle confirmation", async () => {
    const item = backend.seed("Texte");
    render(<InboxHome />);
    await openCard("Texte");
    fireEvent.change(detailText(), { target: { value: "Jetable" } });
    expect(await requestClose()).toBe(true);
    await draftBanner();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner les modifications" }));
    expect(appWindow.destroy).toHaveBeenCalledTimes(1);
    expect(backend.find(item.id)!.content).toBe("Texte"); // rien n'a été écrit
    expect(screen.queryByText(/non enregistrées/)).not.toBeInTheDocument();
  });

  it("fiche modifiée → « Enregistrer » : enregistre puis ferme", async () => {
    const item = backend.seed("Avant");
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Après" } });
    fireEvent.change(within(detail()).getByLabelText("Destination (facultatif)"), {
      target: { value: "finances" },
    });
    expect(await requestClose()).toBe(true);
    const banner = (await draftBanner()).closest(".leave") as HTMLElement;
    fireEvent.click(within(banner).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1));
    expect(backend.find(item.id)!.content).toBe("Après");
    expect(backend.find(item.id)!.destinationId).toBe("finances");
  });

  it("échec de l'enregistrement : brouillon conservé, la fenêtre reste ouverte, on peut réessayer", async () => {
    const item = backend.seed("Fragile");
    render(<InboxHome />);
    await openCard("Fragile");
    fireEvent.change(detailText(), { target: { value: "Brouillon précieux" } });
    backend.state.failures.set("update_inbox_item", {
      code: "storage",
      message: "L'enregistrement local a échoué.",
    });
    expect(await requestClose()).toBe(true);
    const banner = (await draftBanner()).closest(".leave") as HTMLElement;
    fireEvent.click(within(banner).getByRole("button", { name: "Enregistrer" }));
    expect(await within(detail()).findByText(/Votre brouillon est conservé/)).toBeInTheDocument();
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(detailText()).toHaveValue("Brouillon précieux");
    expect(backend.find(item.id)!.content).toBe("Fragile");

    backend.state.failures.clear();
    fireEvent.click(within(banner).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1));
    expect(backend.find(item.id)!.content).toBe("Brouillon précieux");
  });

  it("texte non envoyé dans la capture rapide → « Continuer à écrire »", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    fireEvent.change(field(), { target: { value: "Idée en cours" } });
    expect(await requestClose()).toBe(true);
    await captureBanner();
    fireEvent.click(screen.getByRole("button", { name: "Continuer à écrire" }));
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(field()).toHaveValue("Idée en cours");
    expect(field()).toHaveFocus();
  });

  it("texte non envoyé → « Envoyer » : enregistre (avec sa destination) puis ferme", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    fireEvent.change(field(), { target: { value: "À envoyer avant de partir" } });
    fireEvent.change(screen.getByLabelText("Destination (facultatif)"), { target: { value: "moi" } });
    expect(await requestClose()).toBe(true);
    await captureBanner();
    fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1));
    expect(backend.state.items.map((i) => [i.content, i.destinationId])).toEqual([
      ["À envoyer avant de partir", "moi"],
    ]);
  });

  it("texte non envoyé → échec de l'envoi : texte conservé, fenêtre ouverte", async () => {
    backend.state.failures.set("create_inbox_item", {
      code: "storage",
      message: "L'enregistrement local a échoué.",
    });
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    fireEvent.change(field(), { target: { value: "Ne pas perdre" } });
    expect(await requestClose()).toBe(true);
    await captureBanner();
    fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
    expect(await screen.findByText(/L'envoi a échoué/)).toBeInTheDocument();
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(field()).toHaveValue("Ne pas perdre");
    expect(backend.state.items).toHaveLength(0);
  });

  it("texte non envoyé → « Abandonner ce texte » : fermeture effective", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    fireEvent.change(field(), { target: { value: "Sans importance" } });
    expect(await requestClose()).toBe(true);
    await captureBanner();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner ce texte" }));
    expect(appWindow.destroy).toHaveBeenCalledTimes(1);
    expect(backend.state.items).toHaveLength(0);
  });

  it("fiche modifiée ET texte non envoyé : deux étapes, « Continuer » à l'une annule la fermeture", async () => {
    backend.seed("Fiche");
    render(<InboxHome />);
    await openCard("Fiche");
    fireEvent.change(detailText(), { target: { value: "Fiche modifiée" } });
    fireEvent.change(field(), { target: { value: "Capture en cours" } });
    expect(await requestClose()).toBe(true);

    // Étape 1 : la fiche. On l'abandonne ; la capture rapide est alors proposée.
    await draftBanner();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner les modifications" }));
    expect(appWindow.destroy).not.toHaveBeenCalled();
    await captureBanner();

    // Étape 2 : continuer à écrire annule la fermeture.
    fireEvent.click(screen.getByRole("button", { name: "Continuer à écrire" }));
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(field()).toHaveValue("Capture en cours");

    // « Abandonner » la fiche n'a rien effacé (la fenêtre est restée ouverte) : son
    // brouillon est toujours là et la fermeture suivante le redemande, par prudence.
    expect(detailText()).toHaveValue("Fiche modifiée");
    expect(await requestClose()).toBe(true);
    await draftBanner();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner les modifications" }));
    await captureBanner();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner ce texte" }));
    expect(appWindow.destroy).toHaveBeenCalledTimes(1);
  });

  it("une capture déjà envoyée ne bloque plus la fermeture", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    fireEvent.change(field(), { target: { value: "Déjà envoyée" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    await waitFor(() => expect(field()).toHaveValue(""));
    expect(await requestClose()).toBe(false);
  });

  /** Retarde la réponse d'une commande tant que `release()` n'est pas appelé. */
  function hold(command: string) {
    let release!: () => void;
    const gate = new Promise<void>((resolve) => (release = resolve));
    const normal = tauri.invoke.getMockImplementation()!;
    tauri.invoke.mockImplementation(async (name, args) => {
      if (name === command) await gate;
      return normal(name, args);
    });
    return release;
  }

  it("fermeture demandée pendant l'enregistrement d'une fiche : rien n'est perdu, fermeture à la fin", async () => {
    const item = backend.seed("Avant");
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Après" } });
    const release = hold("update_inbox_item");
    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(within(detail()).getByRole("button", { name: "Enregistrement…" })).toBeInTheDocument());

    expect(await requestClose()).toBe(true); // le brouillon n'est pas encore sûr
    await draftBanner();
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(backend.find(item.id)!.content).toBe("Avant");

    await act(async () => release());
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1)); // une seule fois
    expect(backend.find(item.id)!.content).toBe("Après");
  });

  it("fermeture demandée pendant l'enregistrement, qui échoue : fenêtre et brouillon conservés", async () => {
    const item = backend.seed("Avant");
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Brouillon" } });
    const release = hold("update_inbox_item");
    backend.state.failures.set("update_inbox_item", { code: "storage", message: "L'enregistrement local a échoué." });
    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    expect(await requestClose()).toBe(true);
    await draftBanner();
    await act(async () => release());
    expect(await within(detail()).findByText(/Votre brouillon est conservé/)).toBeInTheDocument();
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(detailText()).toHaveValue("Brouillon");
    expect(backend.find(item.id)!.content).toBe("Avant");
  });

  it("fermeture demandée pendant l'envoi d'une capture : « Envoyer » attend, pas de doublon ni de faux échec", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    const release = hold("create_inbox_item");
    fireEvent.change(field(), { target: { value: "Envoi en cours" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await requestClose()).toBe(true);
    await captureBanner();
    fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
    expect(screen.queryByText(/L'envoi a échoué/)).not.toBeInTheDocument();

    await act(async () => release());
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1));
    expect(backend.state.items.map((i) => i.content)).toEqual(["Envoi en cours"]);
  });

  it("retire son écouteur quand l'accueil est démonté", async () => {
    const { unmount } = render(<InboxHome />);
    await waitFor(() => expect(appWindow.handler).not.toBeNull());
    await act(async () => unmount());
    expect(appWindow.unlisten).toHaveBeenCalledTimes(1);
  });
});
