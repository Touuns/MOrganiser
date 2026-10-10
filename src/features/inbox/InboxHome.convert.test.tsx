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
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onCloseRequested: async (handler: (event: CloseEvent) => void | Promise<void>) => {
      appWindow.handler = handler;
      return () => undefined;
    },
    destroy: appWindow.destroy,
  }),
}));

let backend: FakeBackend;

/** Promesse contrôlable pour simuler une réponse lente de Rust. */
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

beforeEach(() => {
  appWindow.handler = null;
  appWindow.destroy.mockReset();
  backend = createFakeBackend();
  tauri.invoke.mockReset();
  tauri.invoke.mockImplementation((command, args) => backend.invoke(command, args));
});

const field = () => screen.getByLabelText("Capture rapide", { selector: "textarea" });
const cards = () => screen.queryAllByRole("listitem");
const texts = () => cards().map((li) => li.querySelector(".inbox__content")?.textContent);
const detail = () => screen.getByRole("region", { name: /Fiche de la capture|Capture transformée en tâche|Capture dans la corbeille/ });
const detailText = () => within(detail()).getByLabelText("Texte") as HTMLTextAreaElement;
const transformButton = () => within(detail()).getByRole("button", { name: "Transformer en tâche" });
const titleInput = () => within(detail()).getByLabelText("Titre de la tâche") as HTMLInputElement;
const createButton = () => within(detail()).getByRole("button", { name: /Créer la tâche|Création…/ });
const openCard = async (text: string) =>
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(text) }));
const calls = (command: string) => tauri.invoke.mock.calls.filter(([name]) => name === command);

/** Ouvre la fiche, puis le panneau, et attend la proposition de titre. */
async function openPanel(text: string, expectedTitle?: string) {
  await openCard(text);
  fireEvent.click(transformButton());
  await waitFor(() => expect(titleInput()).toBeInTheDocument());
  if (expectedTitle !== undefined) await waitFor(() => expect(titleInput()).toHaveValue(expectedTitle));
}

async function requestClose(): Promise<boolean> {
  await waitFor(() => expect(appWindow.handler).not.toBeNull());
  const event = { preventDefault: vi.fn() };
  await act(async () => {
    await appWindow.handler!(event);
  });
  return event.preventDefault.mock.calls.length > 0;
}

describe("002-B — panneau de conversion", () => {
  it("le bouton est dans la fiche d'une capture, pas dans celle d'une capture supprimée", async () => {
    const live = backend.seed("Active");
    const gone = backend.seed("Supprimée");
    backend.find(gone.id)!.deletedAt = 5;
    render(<InboxHome />);
    await openCard("Active");
    expect(transformButton()).toBeEnabled();
    expect(backend.find(live.id)!.convertedAt).toBeNull(); // ouvrir ne convertit rien

    fireEvent.click(screen.getByRole("button", { name: "Corbeille" }));
    fireEvent.click(await screen.findByRole("button", { name: /Supprimée/ }));
    expect(within(detail()).queryByRole("button", { name: "Transformer en tâche" })).not.toBeInTheDocument();
  });

  it("ouvre un panneau intégré : titre proposé, détails et destination repris, texte figé, focus sur le titre", async () => {
    backend.seed("Appeler la banque\nDemander le relevé de mars", "finances");
    render(<InboxHome />);
    await openPanel("Appeler la banque", "Appeler la banque");

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument(); // pas de modale
    const panel = within(detail()).getByRole("region", { name: "Transformer en tâche" });
    expect(within(panel).getByText(/texte complet est conservé tel quel/)).toHaveTextContent("(Finances)");
    expect(within(panel).getByText(/capture\s+d'origine reste conservée/)).toBeInTheDocument();
    expect(detailText()).toHaveAttribute("readonly");
    expect(within(detail()).getByLabelText("Destination (facultatif)")).toBeDisabled();
    expect(titleInput()).toHaveFocus();
    expect(calls("convert_inbox_item_to_task")).toHaveLength(0);
  });

  it("crée la tâche avec la version enregistrée et le titre saisi ; la carte part, la notification propose d'annuler", async () => {
    const item = backend.seed("Vendre ma PlayStation 5\nAvec ses manettes", "inventaire");
    backend.seed("Autre capture");
    render(<InboxHome />);
    await openPanel("Vendre ma PlayStation 5", "Vendre ma PlayStation 5");
    fireEvent.change(titleInput(), { target: { value: "Vendre la console" } });
    fireEvent.click(createButton());

    await waitFor(() => expect(backend.state.tasks).toHaveLength(1));
    expect(calls("convert_inbox_item_to_task")[0][1]).toEqual({
      id: item.id,
      expectedUpdatedAt: item.updatedAt,
      title: "Vendre la console",
    });
    const task = backend.state.tasks[0];
    expect(task.title).toBe("Vendre la console");
    expect(task.details).toBe("Vendre ma PlayStation 5\nAvec ses manettes"); // texte intégral
    expect(task.destinationId).toBe("inventaire");

    // La fiche se ferme, la carte quitte la boîte, la notification apparaît (4 s, « Annuler »).
    await waitFor(() => expect(screen.queryByRole("region", { name: "Fiche de la capture" })).not.toBeInTheDocument());
    await waitFor(() => expect(texts()).toEqual(["Autre capture"]));
    const toast = screen.getByText("Transformée en tâche").closest(".toast") as HTMLElement;
    expect(within(toast).getByRole("button", { name: "Annuler" })).toBeInTheDocument();
    // La capture d'origine est conservée, intacte, avec sa date de modification d'origine.
    const stored = backend.find(item.id)!;
    expect(stored.content).toBe("Vendre ma PlayStation 5\nAvec ses manettes");
    expect(stored.updatedAt).toBe(item.updatedAt);
    expect(stored.convertedTaskId).toBe(task.id);
  });

  it("aucun succès avant la confirmation de Rust : panneau ouvert, « Création… », puis fermeture", async () => {
    backend.seed("Lent");
    const slow = deferred<unknown>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "convert_inbox_item_to_task" ? slow.promise : backend.invoke(command, args),
    );
    render(<InboxHome />);
    await openPanel("Lent", "Lent");
    fireEvent.click(createButton());

    expect(createButton()).toBeDisabled();
    expect(createButton()).toHaveTextContent("Création…");
    expect(titleInput()).toHaveAttribute("readonly");
    expect(screen.queryByText("Transformée en tâche")).not.toBeInTheDocument();
    expect(texts()).toEqual(["Lent"]); // la carte est encore là

    await act(async () => slow.resolve(await backend.invoke("convert_inbox_item_to_task", {
      id: backend.state.items[0].id,
      expectedUpdatedAt: backend.state.items[0].updatedAt,
      title: "Lent",
    })));
    expect(await screen.findByText("Transformée en tâche")).toBeInTheDocument();
  });

  it("une double validation (Entrée répétée, clic) ne crée qu'une seule tâche", async () => {
    backend.seed("Une seule fois");
    render(<InboxHome />);
    await openPanel("Une seule fois", "Une seule fois");
    fireEvent.keyDown(titleInput(), { key: "Enter" });
    fireEvent.keyDown(titleInput(), { key: "Enter" });
    fireEvent.click(createButton());
    fireEvent.keyDown(titleInput(), { key: "Enter" });
    await screen.findByText("Transformée en tâche");
    expect(calls("convert_inbox_item_to_task")).toHaveLength(1);
    expect(backend.state.tasks).toHaveLength(1);
  });

  it("un titre vide ne peut pas être validé", async () => {
    backend.seed("Titre à effacer");
    render(<InboxHome />);
    await openPanel("Titre à effacer", "Titre à effacer");
    fireEvent.change(titleInput(), { target: { value: "   " } });
    expect(createButton()).toBeDisabled();
    fireEvent.keyDown(titleInput(), { key: "Enter" });
    expect(await within(detail()).findByText("Le titre de la tâche est vide.")).toBeInTheDocument();
    expect(calls("convert_inbox_item_to_task")).toHaveLength(0);
  });

  it("échec de Rust : panneau ouvert, titre personnalisé conservé, nouvel essai possible", async () => {
    backend.seed("Fragile");
    backend.state.failures.set("convert_inbox_item_to_task", {
      code: "storage",
      message: "L'enregistrement local a échoué.",
    });
    render(<InboxHome />);
    await openPanel("Fragile", "Fragile");
    fireEvent.change(titleInput(), { target: { value: "Mon titre précis" } });
    fireEvent.click(createButton());

    expect(await within(detail()).findByRole("alert")).toHaveTextContent("Votre titre est conservé");
    expect(titleInput()).toHaveValue("Mon titre précis");
    expect(backend.state.tasks).toHaveLength(0);
    expect(texts()).toEqual(["Fragile"]);

    backend.state.failures.clear();
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0].title).toBe("Mon titre précis");
  });
});

describe("002-B — protection du brouillon et du titre", () => {
  it("texte modifié : Enregistrer puis conversion de la version enregistrée", async () => {
    const item = backend.seed("Avant");
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Après" } });
    fireEvent.click(transformButton());
    expect(await screen.findByText(/Vous avez des modifications non enregistrées/)).toBeInTheDocument();
    expect(titleInput).toThrow(); // pas de panneau tant que rien n'est décidé

    const banner = screen.getByText(/Vous avez des modifications non enregistrées/).closest(".leave") as HTMLElement;
    fireEvent.click(within(banner).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(titleInput()).toHaveValue("Après"));
    expect(backend.find(item.id)!.content).toBe("Après");
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0].details).toBe("Après");
    expect(calls("convert_inbox_item_to_task")[0][1]).toMatchObject({
      expectedUpdatedAt: backend.find(item.id)!.updatedAt,
    });
  });

  it("texte modifié : Abandonner rétablit le texte enregistré avant la conversion", async () => {
    const item = backend.seed("Enregistré");
    render(<InboxHome />);
    await openCard("Enregistré");
    fireEvent.change(detailText(), { target: { value: "Brouillon jetable" } });
    fireEvent.click(transformButton());
    fireEvent.click(await screen.findByRole("button", { name: "Abandonner les modifications" }));
    await waitFor(() => expect(titleInput()).toHaveValue("Enregistré"));
    expect(detailText()).toHaveValue("Enregistré");
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0].details).toBe("Enregistré");
    expect(backend.find(item.id)!.content).toBe("Enregistré");
  });

  it("texte modifié : Continuer à modifier ne change rien", async () => {
    backend.seed("Texte");
    render(<InboxHome />);
    await openCard("Texte");
    fireEvent.change(detailText(), { target: { value: "Texte plus long" } });
    fireEvent.click(transformButton());
    fireEvent.click(await screen.findByRole("button", { name: "Continuer à modifier" }));
    expect(detailText()).toHaveValue("Texte plus long");
    expect(detailText()).not.toHaveAttribute("readonly");
    expect(within(detail()).queryByLabelText("Titre de la tâche")).not.toBeInTheDocument();
  });

  it("titre modifié + Échap : avertissement ; Continuer le garde ; Abandonner ferme le panneau", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openPanel("Capture", "Capture");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });

    fireEvent.keyDown(titleInput(), { key: "Escape" });
    expect(await screen.findByText("Vous avez un titre de tâche non validé.")).toBeInTheDocument();
    // Pas de texte à enregistrer : seulement « Abandonner la conversion » et « Continuer ».
    expect(screen.queryByRole("button", { name: "Enregistrer" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continuer" }));
    expect(titleInput()).toHaveValue("Titre perso");

    fireEvent.keyDown(titleInput(), { key: "Escape" });
    fireEvent.click(await screen.findByRole("button", { name: "Abandonner la conversion" }));
    await waitFor(() => expect(within(detail()).queryByLabelText("Titre de la tâche")).not.toBeInTheDocument());
    expect(transformButton()).toHaveFocus();
    expect(backend.state.tasks).toHaveLength(0);
  });

  it("titre non modifié + Échap : le panneau se ferme sans avertissement", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openPanel("Capture", "Capture");
    fireEvent.keyDown(titleInput(), { key: "Escape" });
    await waitFor(() => expect(within(detail()).queryByLabelText("Titre de la tâche")).not.toBeInTheDocument());
    expect(screen.queryByText(/titre de tâche non validé/)).not.toBeInTheDocument();
    expect(detail()).toBeInTheDocument(); // la fiche reste ouverte
  });

  it("« Annuler la conversion » abandonne volontairement le titre, sans avertissement", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openPanel("Capture", "Capture");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });
    fireEvent.click(within(detail()).getByRole("button", { name: "Annuler la conversion" }));
    await waitFor(() => expect(within(detail()).queryByLabelText("Titre de la tâche")).not.toBeInTheDocument());
    expect(screen.queryByText(/titre de tâche non validé/)).not.toBeInTheDocument();
    // Un nouveau panneau repart de la proposition, pas de l'ancien titre.
    fireEvent.click(transformButton());
    await waitFor(() => expect(titleInput()).toHaveValue("Capture"));
  });

  it("titre modifié : fermer la fiche, ouvrir une autre capture ou une autre vue est protégé", async () => {
    backend.seed("Première");
    backend.seed("Seconde");
    render(<InboxHome />);
    await openPanel("Première", "Première");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });

    // Fermer la fiche
    fireEvent.click(within(detail()).getByRole("button", { name: "Fermer la fiche" }));
    expect(await screen.findByText("Vous avez un titre de tâche non validé.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continuer" }));
    expect(titleInput()).toHaveValue("Titre perso");

    // Ouvrir une autre capture
    fireEvent.click(screen.getByRole("button", { name: /Seconde/ }));
    expect(await screen.findByText("Vous avez un titre de tâche non validé.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continuer" }));
    expect(titleInput()).toHaveValue("Titre perso");

    // Changer de vue
    fireEvent.click(screen.getByRole("button", { name: "Corbeille" }));
    expect(await screen.findByText("Vous avez un titre de tâche non validé.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continuer" }));
    expect(screen.getByRole("heading", { name: "À organiser" })).toBeInTheDocument();
    expect(titleInput()).toHaveValue("Titre perso");
  });

  it("titre modifié : la fermeture de la fenêtre est suspendue", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openPanel("Capture", "Capture");
    expect(await requestClose()).toBe(false); // titre non modifié : rien à perdre

    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });
    expect(await requestClose()).toBe(true);
    expect(await screen.findByText("Vous avez un titre de tâche non validé.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continuer" }));
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(titleInput()).toHaveValue("Titre perso");
  });

  it("pendant le panneau le texte est figé : seul le titre est protégé, et son abandon est explicite", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openPanel("Capture", "Capture");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });
    expect(detailText()).toHaveAttribute("readonly");
    fireEvent.click(within(detail()).getByRole("button", { name: "Fermer la fiche" }));
    expect(await screen.findByText("Vous avez un titre de tâche non validé.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Abandonner la conversion" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Fiche de la capture" })).not.toBeInTheDocument());
    expect(backend.state.tasks).toHaveLength(0);
  });
});

describe("002-B — réponses asynchrones et conflits", () => {
  it("une proposition de titre tardive n'écrase jamais un titre déjà modifié", async () => {
    backend.seed("Capture à titrer");
    const late = deferred<string>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "suggest_task_title" ? late.promise : backend.invoke(command, args),
    );
    render(<InboxHome />);
    await openCard("Capture à titrer");
    fireEvent.click(transformButton());
    await waitFor(() => expect(titleInput()).toBeInTheDocument());
    expect(titleInput()).toHaveAttribute("placeholder", expect.stringMatching(/Titre proposé/));

    fireEvent.change(titleInput(), { target: { value: "Mon titre" } });
    await act(async () => late.resolve("Titre proposé tardif"));
    expect(titleInput()).toHaveValue("Mon titre");
  });

  it("une proposition arrivée à temps (titre non modifié) est appliquée", async () => {
    backend.seed("Capture à titrer");
    const late = deferred<string>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "suggest_task_title" ? late.promise : backend.invoke(command, args),
    );
    render(<InboxHome />);
    await openCard("Capture à titrer");
    fireEvent.click(transformButton());
    await waitFor(() => expect(titleInput()).toBeInTheDocument());
    await act(async () => late.resolve("Titre proposé"));
    expect(titleInput()).toHaveValue("Titre proposé");
  });

  it("une proposition tardive d'une fiche refermée est ignorée", async () => {
    backend.seed("Première");
    const late = deferred<string>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "suggest_task_title" ? late.promise : backend.invoke(command, args),
    );
    render(<InboxHome />);
    await openCard("Première");
    fireEvent.click(transformButton());
    await waitFor(() => expect(titleInput()).toBeInTheDocument());
    fireEvent.click(within(detail()).getByRole("button", { name: "Annuler la conversion" }));
    await act(async () => late.resolve("Tardif"));
    expect(within(detail()).queryByLabelText("Titre de la tâche")).not.toBeInTheDocument();
  });

  it("conflit de version : texte actuel affiché, titre conservé, nouvelle confirmation sur la version actualisée", async () => {
    const item = backend.seed("Version 1");
    render(<InboxHome />);
    await openPanel("Version 1", "Version 1");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });

    // Modification ailleurs (autre fenêtre, autre appareil…) pendant que le panneau est ouvert.
    const stored = backend.find(item.id)!;
    stored.content = "Version 2";
    stored.updatedAt += 10;

    fireEvent.click(createButton());
    expect(await within(detail()).findByText(/Cette capture a changé depuis son ouverture/)).toBeInTheDocument();
    expect(titleInput()).toHaveValue("Titre perso");
    expect(detailText()).toHaveValue("Version 2");
    expect(backend.state.tasks).toHaveLength(0);

    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0]).toMatchObject({ title: "Titre perso", details: "Version 2" });
  });

  it("déjà convertie ailleurs : fiche en lecture seule, titre rappelé, « Voir la tâche » ouvre la tâche existante", async () => {
    const item = backend.seed("Déjà fait");
    render(<InboxHome />);
    await openPanel("Déjà fait", "Déjà fait");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });

    // Une autre conversion a eu lieu entre-temps.
    const existing = await backend.invoke("convert_inbox_item_to_task", {
      id: item.id,
      expectedUpdatedAt: item.updatedAt,
      title: "Créée ailleurs",
    });
    const taskId = (existing as { task: { id: string } }).task.id;

    fireEvent.click(createButton());
    const alert = await within(detail()).findByRole("alert");
    expect(alert).toHaveTextContent("déjà transformée en tâche");
    expect(alert).toHaveTextContent("Titre perso"); // jamais effacé en silence
    expect(backend.state.tasks).toHaveLength(1); // aucun doublon
    expect(detailText()).toHaveAttribute("readonly");
    expect(within(detail()).queryByRole("button", { name: "Transformer en tâche" })).not.toBeInTheDocument();
    expect(within(detail()).queryByRole("button", { name: "Mettre à la corbeille" })).not.toBeInTheDocument();

    fireEvent.click(within(detail()).getByRole("button", { name: "Voir la tâche" }));
    const taskPanel = await screen.findByRole("region", { name: "Fiche de la tâche" });
    expect(within(taskPanel).getByLabelText("Titre")).toHaveValue("Créée ailleurs");
    expect(backend.state.tasks[0].id).toBe(taskId);
  });

  it("mise à la corbeille ailleurs : message avec le titre, rien de converti", async () => {
    const item = backend.seed("Supprimée ailleurs");
    render(<InboxHome />);
    await openPanel("Supprimée ailleurs", "Supprimée ailleurs");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });
    backend.find(item.id)!.deletedAt = 99;

    fireEvent.click(createButton());
    const alert = await within(detail()).findByRole("alert");
    expect(alert).toHaveTextContent("corbeille");
    expect(alert).toHaveTextContent("Titre perso");
    expect(backend.state.tasks).toHaveLength(0);
  });

  it("capture convertie ailleurs : l'enregistrement du texte est refusé proprement", async () => {
    const item = backend.seed("Texte");
    render(<InboxHome />);
    await openCard("Texte");
    fireEvent.change(detailText(), { target: { value: "Texte modifié" } });
    await backend.invoke("convert_inbox_item_to_task", {
      id: item.id,
      expectedUpdatedAt: item.updatedAt,
      title: null,
    });
    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    expect(await within(detail()).findByRole("alert")).toHaveTextContent("transformée en tâche");
    expect(backend.find(item.id)!.content).toBe("Texte"); // rien d'écrasé
    expect(detailText()).toHaveValue("Texte modifié"); // brouillon visible, jamais perdu
  });
});

describe("002-B — annulation et retour de la capture", () => {
  async function convertFirst(text: string) {
    await openPanel(text, text);
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
  }

  it("« Annuler » remet la capture dans la boîte et conserve la tâche annulée", async () => {
    const item = backend.seed("À annuler", "moi");
    render(<InboxHome />);
    await convertFirst("À annuler");
    await waitFor(() => expect(cards()).toHaveLength(0));

    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await waitFor(() => expect(texts()).toEqual(["À annuler"]));
    expect(screen.getByRole("status")).toHaveTextContent("Conversion annulée");
    expect(backend.find(item.id)).toMatchObject({ convertedAt: null, convertedTaskId: null, updatedAt: item.updatedAt });
    expect(backend.state.tasks).toHaveLength(1);
    expect(backend.state.tasks[0].deletedAt).not.toBeNull(); // conservée, annulée
    expect(screen.queryByText("Transformée en tâche")).not.toBeInTheDocument();
  });

  it("annulation refusée si la tâche a été modifiée : la capture reste convertie, message clair", async () => {
    backend.seed("Déjà avancée");
    render(<InboxHome />);
    await convertFirst("Déjà avancée");
    backend.state.tasks[0].updatedAt += 1; // la tâche a été enrichie
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(await screen.findByText(/déjà été modifiée/)).toBeInTheDocument();
    expect(screen.getByText(/reste transformée en tâche/)).toBeInTheDocument();
    expect(cards()).toHaveLength(0);
    expect(backend.state.tasks[0].deletedAt).toBeNull();
  });

  it("reconversion possible après annulation, sans doublon actif", async () => {
    backend.seed("Deux essais");
    render(<InboxHome />);
    await convertFirst("Deux essais");
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await waitFor(() => expect(texts()).toEqual(["Deux essais"]));

    await openPanel("Deux essais", "Deux essais");
    fireEvent.click(createButton());
    await waitFor(() => expect(backend.state.tasks).toHaveLength(2));
    expect(backend.state.tasks.filter((t) => t.deletedAt === null)).toHaveLength(1);
  });

  it("le focus passe à la carte voisine à la fermeture de la fiche", async () => {
    backend.seed("Avant");
    backend.seed("Celle-ci");
    backend.seed("Après");
    render(<InboxHome />);
    await openPanel("Celle-ci", "Celle-ci");
    fireEvent.click(createButton());
    await waitFor(() => expect(texts()).toEqual(["Avant", "Après"]));
    await waitFor(() => expect(document.activeElement?.closest("li")?.textContent).toMatch(/Après|Avant/));
  });

  it("l'accueil et les compteurs reflètent la conversion (relecture depuis la base)", async () => {
    for (let i = 0; i < 3; i++) backend.seed(`Capture ${i}`);
    render(<InboxHome />);
    await openPanel("Capture 1", "Capture 1");
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    await waitFor(() => expect(texts()).toEqual(["Capture 0", "Capture 2"]));
    expect(field()).toHaveValue("");
  });
});
