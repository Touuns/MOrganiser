import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeBackend, type FakeBackend } from "../../test/fakeBackend";
import { InboxHome } from "./InboxHome";

// Coordination des opérations asynchrones : réponses différées, changements de fiche,
// décisions pendant l'attente. Tout est déterministe (promesses libérées à la main).
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

/** Retarde la réponse d'une commande jusqu'à `release()` (l'exécution a lieu à ce moment). */
function hold(command: string) {
  let release!: () => void;
  const gate = new Promise<void>((resolve) => (release = resolve));
  const previous = tauri.invoke.getMockImplementation()!;
  tauri.invoke.mockImplementation(async (name, args) => {
    if (name === command) await gate;
    return previous(name, args);
  });
  return async () => {
    await act(async () => release());
  };
}

const field = () => screen.getByLabelText("Capture rapide", { selector: "textarea" });
const detail = () => screen.getByRole("region", { name: /Fiche de la capture|Capture dans la corbeille/ });
const detailText = () => within(detail()).getByLabelText("Texte") as HTMLTextAreaElement;
const openCard = async (text: string) =>
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(text) }));
const saveButton = () => within(detail()).getByRole("button", { name: /Enregistre/ });
const leaveBanner = () => screen.findByText(/Vous avez des modifications non enregistrées/);
const captureBanner = () => screen.findByText(/texte non envoyé dans la capture rapide/);

async function requestClose(): Promise<boolean> {
  await waitFor(() => expect(appWindow.handler).not.toBeNull());
  const event = { preventDefault: vi.fn() };
  await act(async () => {
    await appWindow.handler!(event);
  });
  return event.preventDefault.mock.calls.length > 0;
}

describe("A. sauvegarde différée et nouvelle saisie", () => {
  it("la réponse tardive de A ne remplace pas la saisie B ; la référence est actualisée", async () => {
    const item = backend.seed("Base");
    render(<InboxHome />);
    await openCard("Base");
    fireEvent.change(detailText(), { target: { value: "A" } });
    const release = hold("update_inbox_item");
    fireEvent.click(saveButton());
    await waitFor(() => expect(saveButton()).toHaveTextContent("Enregistrement…"));
    fireEvent.change(detailText(), { target: { value: "B" } }); // nouvelle saisie pendant l'attente

    await release();
    await waitFor(() => expect(backend.find(item.id)!.content).toBe("A"));
    expect(detailText()).toHaveValue("B"); // jamais remplacée par A
    await waitFor(() => expect(saveButton()).toBeEnabled()); // B est un vrai brouillon
    expect(within(detail()).queryByText("Jamais modifiée")).not.toBeInTheDocument(); // référence à jour

    // B s'enregistre normalement à partir de la nouvelle référence (aucun faux conflit).
    fireEvent.click(saveButton());
    await waitFor(() => expect(backend.find(item.id)!.content).toBe("B"));
    expect(within(detail()).queryByRole("alert")).not.toBeInTheDocument();
  });

  it("une fermeture différée ne se poursuit pas s'il reste un nouveau brouillon", async () => {
    const item = backend.seed("Base");
    render(<InboxHome />);
    await openCard("Base");
    fireEvent.change(detailText(), { target: { value: "A" } });
    const release = hold("update_inbox_item");
    fireEvent.click(saveButton());
    expect(await requestClose()).toBe(true);
    await leaveBanner();
    fireEvent.change(detailText(), { target: { value: "B" } });

    await release();
    await waitFor(() => expect(backend.find(item.id)!.content).toBe("A"));
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(detailText()).toHaveValue("B");
    expect(await leaveBanner()).toBeInTheDocument(); // B reste protégé
  });

  it("sans nouvelle saisie, la fermeture différée se poursuit une seule fois", async () => {
    const item = backend.seed("Base");
    render(<InboxHome />);
    await openCard("Base");
    fireEvent.change(detailText(), { target: { value: "A" } });
    const release = hold("update_inbox_item");
    fireEvent.click(saveButton());
    expect(await requestClose()).toBe(true);
    await release();
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1));
    expect(backend.find(item.id)!.content).toBe("A");
  });
});

describe("B. fermeture différée annulable (capture rapide)", () => {
  async function startSend(text: string) {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    const release = hold("create_inbox_item");
    fireEvent.change(field(), { target: { value: text } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await requestClose()).toBe(true);
    await captureBanner();
    return release;
  }

  it("« Continuer à écrire » pendant l'envoi annule réellement la fermeture", async () => {
    const release = await startSend("Idée");
    fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
    fireEvent.click(screen.getByRole("button", { name: "Continuer à écrire" }));
    await release();
    await waitFor(() => expect(backend.state.items).toHaveLength(1)); // l'envoi, lui, aboutit
    expect(appWindow.destroy).not.toHaveBeenCalled();
  });

  it("un nouveau texte saisi pendant l'envoi est réexaminé : pas de destruction", async () => {
    const release = await startSend("Première idée");
    fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
    fireEvent.change(field(), { target: { value: "Seconde idée, pas encore envoyée" } });
    await release();
    await waitFor(() => expect(backend.state.items).toHaveLength(1));
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(field()).toHaveValue("Seconde idée, pas encore envoyée");
    await captureBanner(); // le nouveau texte est proposé à son tour
  });

  it("plusieurs décisions pendant l'envoi : la dernière l'emporte, une seule destruction au plus", async () => {
    const release = await startSend("Idée");
    fireEvent.click(screen.getByRole("button", { name: "Envoyer" }));
    fireEvent.click(screen.getByRole("button", { name: "Continuer à écrire" })); // annule
    expect(await requestClose()).toBe(true); // nouvelle demande de fermeture
    await captureBanner();
    await release();
    // Plus rien à envoyer : la seconde demande (non annulée) peut maintenant aboutir.
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1));
    expect(backend.state.items).toHaveLength(1);
  });
});

describe("C. réponse tardive de corbeille", () => {
  it("la mise à la corbeille de A ne ferme pas la fiche B ni son brouillon", async () => {
    const a = backend.seed("Alpha");
    backend.seed("Bravo");
    render(<InboxHome />);
    await openCard("Alpha");
    const release = hold("trash_inbox_item");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));

    await openCard("Bravo"); // A n'a aucun brouillon : on peut changer de fiche
    await waitFor(() => expect(detailText()).toHaveValue("Bravo"));
    fireEvent.change(detailText(), { target: { value: "Brouillon de Bravo" } });

    await release();
    await waitFor(() => expect(backend.find(a.id)!.deletedAt).not.toBeNull());
    await screen.findByText("Déplacée dans la corbeille");
    expect(detailText()).toHaveValue("Brouillon de Bravo"); // la fiche B est toujours là
    expect(screen.queryByText(/modifications non enregistrées/)).not.toBeInTheDocument();
    expect(await requestClose()).toBe(true); // et son brouillon reste protégé
  });

  it("un brouillon saisi dans A pendant sa propre mise à la corbeille est conservé", async () => {
    const a = backend.seed("Alpha");
    render(<InboxHome />);
    await openCard("Alpha");
    const release = hold("trash_inbox_item");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    fireEvent.change(detailText(), { target: { value: "Ajout pendant l'attente" } });
    await release();
    await waitFor(() => expect(backend.find(a.id)!.deletedAt).not.toBeNull());
    expect(detailText()).toHaveValue("Ajout pendant l'attente"); // fiche non fermée
    expect(await requestClose()).toBe(true);
  });
});

describe("D. brouillon divergent d'une capture supprimée ailleurs", () => {
  it("reste protégé à la fermeture, jamais abandonné implicitement", async () => {
    const item = backend.seed("Original");
    render(<InboxHome />);
    await openCard("Original");
    fireEvent.change(detailText(), { target: { value: "Mon brouillon" } });
    backend.find(item.id)!.deletedAt = backend.state.clock + 10; // supprimée ailleurs

    fireEvent.click(saveButton());
    expect(await within(detail()).findByText(/mise à la corbeille/)).toBeInTheDocument();
    expect(detailText()).toHaveValue("Mon brouillon");
    expect(backend.find(item.id)!.content).toBe("Original");

    expect(await requestClose()).toBe(true); // protégé malgré la fiche non modifiable
    await leaveBanner();
    expect(appWindow.destroy).not.toHaveBeenCalled();

    // « Enregistrer » ne peut pas aboutir et l'explique, sans fermer.
    const banner = (await leaveBanner()).closest(".leave") as HTMLElement;
    fireEvent.click(within(banner).getByRole("button", { name: "Enregistrer" }));
    expect(await within(detail()).findByText(/restaurez-la pour enregistrer/)).toBeInTheDocument();
    expect(appWindow.destroy).not.toHaveBeenCalled();

    fireEvent.click(within(banner).getByRole("button", { name: "Continuer à modifier" }));
    expect(detailText()).toHaveValue("Mon brouillon");
  });

  it("la restauration conserve le brouillon et rend la fiche modifiable", async () => {
    const item = backend.seed("Original");
    render(<InboxHome />);
    await openCard("Original");
    fireEvent.change(detailText(), { target: { value: "Mon brouillon" } });
    backend.find(item.id)!.deletedAt = backend.state.clock + 10;
    fireEvent.click(saveButton());
    await within(detail()).findByText(/mise à la corbeille/);

    fireEvent.click(within(detail()).getByRole("button", { name: "Restaurer" }));
    await waitFor(() => expect(backend.find(item.id)!.deletedAt).toBeNull());
    expect(detailText()).toHaveValue("Mon brouillon");
    await waitFor(() => expect(saveButton()).toBeEnabled());
    fireEvent.click(saveButton());
    await waitFor(() => expect(backend.find(item.id)!.content).toBe("Mon brouillon"));
  });
});

describe("E. enregistrer puis mettre à la corbeille", () => {
  it("les deux opérations ont réellement lieu, dans l'ordre", async () => {
    const item = backend.seed("Avant");
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Modifié avant corbeille" } });
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    const banner = (await leaveBanner()).closest(".leave") as HTMLElement;
    fireEvent.click(within(banner).getByRole("button", { name: "Enregistrer" }));

    await waitFor(() => expect(backend.find(item.id)!.deletedAt).not.toBeNull());
    expect(backend.find(item.id)!.content).toBe("Modifié avant corbeille");
    const order = tauri.invoke.mock.calls
      .map(([command]) => command)
      .filter((c) => c === "update_inbox_item" || c === "trash_inbox_item");
    expect(order).toEqual(["update_inbox_item", "trash_inbox_item"]);
    await waitFor(() => expect(screen.queryByRole("region", { name: /Fiche/ })).not.toBeInTheDocument());
    expect(screen.getByText("Déplacée dans la corbeille")).toBeInTheDocument();
  });

  it("si l'enregistrement échoue, la corbeille n'est pas exécutée", async () => {
    const item = backend.seed("Avant");
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Brouillon" } });
    backend.state.failures.set("update_inbox_item", { code: "storage", message: "L'enregistrement local a échoué." });
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    const banner = (await leaveBanner()).closest(".leave") as HTMLElement;
    fireEvent.click(within(banner).getByRole("button", { name: "Enregistrer" }));
    expect(await within(detail()).findByText(/Votre brouillon est conservé/)).toBeInTheDocument();

    expect(tauri.invoke.mock.calls.some(([c]) => c === "trash_inbox_item")).toBe(false);
    expect(backend.find(item.id)!.deletedAt).toBeNull();
    expect(detailText()).toHaveValue("Brouillon");
  });
});

describe("F. fiche cohérente après restauration depuis la liste", () => {
  it("la fiche ouverte d'une capture restaurée redevient modifiable", async () => {
    const item = backend.seed("Dans la corbeille");
    backend.find(item.id)!.deletedAt = backend.state.clock + 5;
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: "Corbeille" }));
    await openCard("Dans la corbeille");
    expect(detailText()).toHaveAttribute("readonly");

    const list = screen.getByRole("list");
    fireEvent.click(within(list).getByRole("button", { name: "Restaurer" }));
    await waitFor(() => expect(backend.find(item.id)!.deletedAt).toBeNull());

    await waitFor(() => expect(detailText()).not.toHaveAttribute("readonly"));
    expect(within(detail()).getByRole("heading", { name: "Fiche de la capture" })).toBeInTheDocument();
    expect(within(detail()).queryByRole("button", { name: "Restaurer" })).not.toBeInTheDocument();
    expect(within(detail()).getByRole("button", { name: "Mettre à la corbeille" })).toBeInTheDocument();
    expect(within(detail()).queryByText(/Dans la corbeille depuis/)).not.toBeInTheDocument();
  });
});

describe("Point complémentaire : fermeture pendant une corbeille ou une restauration", () => {
  it("la fermeture attend la fin de la mise à la corbeille puis se poursuit", async () => {
    const item = backend.seed("En cours");
    render(<InboxHome />);
    await openCard("En cours");
    const release = hold("trash_inbox_item");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    expect(await requestClose()).toBe(true); // suspendue : une opération est en cours
    expect(appWindow.destroy).not.toHaveBeenCalled();

    await release();
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1));
    expect(backend.find(item.id)!.deletedAt).not.toBeNull();
  });

  it("la fermeture attend aussi la restauration lancée depuis la liste", async () => {
    const item = backend.seed("À restaurer");
    backend.find(item.id)!.deletedAt = backend.state.clock + 5;
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: "Corbeille" }));
    await screen.findByText("À restaurer");
    const release = hold("restore_inbox_item");
    fireEvent.click(screen.getByRole("button", { name: "Restaurer" }));
    expect(await requestClose()).toBe(true);
    expect(appWindow.destroy).not.toHaveBeenCalled();
    await release();
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1));
    expect(backend.find(item.id)!.deletedAt).toBeNull();
  });
});

describe("Abandon limité au brouillon concerné (fermeture avec opération en cours)", () => {
  /** Une restauration reste en vol (via « Annuler »), la fiche ouverte est celle d'une autre capture. */
  async function withRestoreInFlight() {
    backend.seed("Xray");
    backend.seed("Yankee");
    render(<InboxHome />);
    await openCard("Xray");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await screen.findByText("Déplacée dans la corbeille");
    const release = hold("restore_inbox_item");
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    return release;
  }

  it("fiche : brouillon A abandonné, puis B saisi pendant l'attente → B est proposé, jamais perdu", async () => {
    const release = await withRestoreInFlight();
    await openCard("Yankee");
    fireEvent.change(detailText(), { target: { value: "Brouillon A" } });
    expect(await requestClose()).toBe(true);
    await leaveBanner();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner les modifications" }));
    expect(appWindow.destroy).not.toHaveBeenCalled(); // l'opération en cours est attendue

    fireEvent.change(detailText(), { target: { value: "Brouillon B" } });
    await release();
    await leaveBanner(); // B n'a jamais été abandonné : il est proposé à son tour
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(detailText()).toHaveValue("Brouillon B");
  });

  it("capture rapide : texte A abandonné, puis B saisi pendant l'attente → B est proposé, jamais perdu", async () => {
    const release = await withRestoreInFlight();
    fireEvent.change(field(), { target: { value: "Texte A" } });
    expect(await requestClose()).toBe(true);
    await captureBanner();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner ce texte" }));
    expect(appWindow.destroy).not.toHaveBeenCalled();

    fireEvent.change(field(), { target: { value: "Texte B" } });
    await release();
    await captureBanner();
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(field()).toHaveValue("Texte B");
  });

  it("un brouillon inchangé et explicitement abandonné n'est pas redemandé : la fermeture aboutit", async () => {
    const release = await withRestoreInFlight();
    await openCard("Yankee");
    fireEvent.change(detailText(), { target: { value: "Brouillon A" } });
    fireEvent.change(field(), { target: { value: "Texte A" } });
    expect(await requestClose()).toBe(true);
    await leaveBanner();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner les modifications" }));
    await captureBanner();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner ce texte" }));
    await release();
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalledTimes(1));
    expect(screen.queryByText(/non enregistrées|non envoyé/)).not.toBeInTheDocument();
  });
});
