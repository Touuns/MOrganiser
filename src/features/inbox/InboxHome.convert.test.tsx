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
const detail = () =>
  screen.getByRole("region", { name: /Fiche de la capture|Capture transformée en tâche|Capture dans la corbeille/ });
const detailText = () => within(detail()).getByLabelText("Texte") as HTMLTextAreaElement;
const transformButton = () => within(detail()).getByRole("button", { name: "Transformer en tâche" });
const modal = () => screen.getByRole("dialog", { name: "Nouvelle tâche" });
const queryModal = () => screen.queryByRole("dialog", { name: "Nouvelle tâche" });
const titleInput = () => within(modal()).getByLabelText("Titre de la tâche") as HTMLInputElement;
const descriptionInput = () => within(modal()).getByLabelText("Description") as HTMLTextAreaElement;
const destinationSelect = () => within(modal()).getByLabelText("Classement (facultatif)") as HTMLSelectElement;
const createButton = () => within(modal()).getByRole("button", { name: /Créer la tâche|Création…/ });
const abandonButton = () => within(modal()).getByRole("button", { name: "Abandonner" });
const closeButton = () => within(modal()).getByRole("button", { name: "Fermer sans créer la tâche" });
const openCard = async (text: string) =>
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(text) }));
const calls = (command: string) => tauri.invoke.mock.calls.filter(([name]) => name === command);
const convertCall = () => calls("convert_inbox_item_to_task")[0][1];
const LEAVE = /Vous avez un brouillon de tâche non enregistré/;

/** Ouvre la fiche, puis la modale, et attend la proposition de titre. */
async function openModal(text: string, expectedTitle?: string) {
  await openCard(text);
  fireEvent.click(transformButton());
  await waitFor(() => expect(queryModal()).toBeInTheDocument());
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

describe("003-A — ouverture de la grande modale", () => {
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

  it("ouvre une vraie modale préremplie, sans aucune écriture, au-dessus d'une fiche toujours présente", async () => {
    const item = backend.seed("Appeler la banque\nDemander le relevé de mars\n\nAvant vendredi", "finances");
    render(<InboxHome />);
    await openModal("Appeler la banque", "Appeler la banque");

    expect(modal()).toHaveAttribute("open"); // ouverte par showModal : couche supérieure, fond inerte
    expect(titleInput()).toHaveValue("Appeler la banque");
    // Texte intégral, retours à la ligne compris : exactement celui de la capture.
    expect(descriptionInput()).toHaveValue("Appeler la banque\nDemander le relevé de mars\n\nAvant vendredi");
    expect(destinationSelect()).toHaveValue("finances");
    expect(createButton()).toBeEnabled();
    expect(within(modal()).getByText("Brouillon")).toBeInTheDocument();
    expect(within(modal()).getByText(/Rien n'est enregistré tant que vous n'avez pas créé la tâche/)).toBeInTheDocument();
    // Le fond de l'application reste là : la fiche et la liste ne sont pas démontées.
    expect(detail()).toBeInTheDocument();
    expect(detailText()).toHaveValue(item.content);
    expect(titleInput()).toHaveFocus();
    // Aucune écriture : ni commande de conversion, ni tâche, ni modification de la capture.
    expect(calls("convert_inbox_item_to_task")).toHaveLength(0);
    expect(backend.state.tasks).toHaveLength(0);
    expect(backend.find(item.id)).toMatchObject({ convertedAt: null, convertedTaskId: null, updatedAt: item.updatedAt });
  });

  it("le panneau intégré de la brique 002 n'existe plus", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openModal("Capture", "Capture");
    expect(within(detail()).queryByRole("region", { name: "Transformer en tâche" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Annuler la conversion" })).not.toBeInTheDocument();
  });

  it("la capture sans destination ouvre une modale sans classement", async () => {
    backend.seed("Sans classement");
    render(<InboxHome />);
    await openModal("Sans classement", "Sans classement");
    expect(destinationSelect()).toHaveValue("");
  });
});

describe("003-A — création à partir du brouillon", () => {
  it("crée la tâche avec le titre, la description et le classement saisis ; la capture reste intacte", async () => {
    const item = backend.seed("Vendre ma PlayStation 5\nAvec ses manettes", "inventaire");
    backend.seed("Autre capture");
    render(<InboxHome />);
    await openModal("Vendre ma PlayStation 5", "Vendre ma PlayStation 5");
    fireEvent.change(titleInput(), { target: { value: "Vendre la console" } });
    fireEvent.change(descriptionInput(), { target: { value: "Console + 2 manettes.\nPrix plancher : 250 €." } });
    fireEvent.change(destinationSelect(), { target: { value: "finances" } });
    fireEvent.click(createButton());

    await waitFor(() => expect(backend.state.tasks).toHaveLength(1));
    expect(convertCall()).toEqual({
      id: item.id,
      expectedUpdatedAt: item.updatedAt,
      title: "Vendre la console",
      details: "Console + 2 manettes.\nPrix plancher : 250 €.",
      destination: { type: "set", id: "finances" },
    });
    const task = backend.state.tasks[0];
    expect(task).toMatchObject({
      title: "Vendre la console",
      details: "Console + 2 manettes.\nPrix plancher : 250 €.",
      destinationId: "finances",
      status: "todo",
      originInboxItemId: item.id,
    });

    // La fiche et la modale se ferment, la carte quitte la boîte, la notification propose d'annuler.
    await waitFor(() => expect(queryModal()).not.toBeInTheDocument());
    await waitFor(() => expect(screen.queryByRole("region", { name: "Fiche de la capture" })).not.toBeInTheDocument());
    await waitFor(() => expect(texts()).toEqual(["Autre capture"]));
    const toast = screen.getByText("Transformée en tâche").closest(".toast") as HTMLElement;
    expect(within(toast).getByRole("button", { name: "Annuler" })).toBeInTheDocument();
    // La capture d'origine : texte, destination et date de modification strictement conservés.
    const stored = backend.find(item.id)!;
    expect(stored.content).toBe("Vendre ma PlayStation 5\nAvec ses manettes");
    expect(stored.destinationId).toBe("inventaire");
    expect(stored.updatedAt).toBe(item.updatedAt);
    expect(stored.convertedTaskId).toBe(task.id);
  });

  it("sans modification : la description est le texte intégral et le classement est « conservé »", async () => {
    const item = backend.seed("Texte\nsur deux lignes", "moi");
    render(<InboxHome />);
    await openModal("Texte", "Texte");
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(convertCall()).toEqual({
      id: item.id,
      expectedUpdatedAt: item.updatedAt,
      title: "Texte",
      details: "Texte\nsur deux lignes",
      destination: { type: "keep" },
    });
    expect(backend.state.tasks[0]).toMatchObject({ details: "Texte\nsur deux lignes", destinationId: "moi" });
  });

  it("« Aucun » retire le classement de la tâche sans toucher à celui de la capture", async () => {
    const item = backend.seed("Classée", "externe");
    render(<InboxHome />);
    await openModal("Classée", "Classée");
    fireEvent.change(destinationSelect(), { target: { value: "" } });
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(convertCall()).toMatchObject({ destination: { type: "clear" } });
    expect(backend.state.tasks[0].destinationId).toBeNull();
    expect(backend.find(item.id)!.destinationId).toBe("externe");
  });

  it("aucun succès avant la confirmation de Rust : « Création… », champs et boutons figés, puis fermeture", async () => {
    backend.seed("Lent");
    const slow = deferred<unknown>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "convert_inbox_item_to_task" ? slow.promise : backend.invoke(command, args),
    );
    render(<InboxHome />);
    await openModal("Lent", "Lent");
    fireEvent.click(createButton());

    expect(createButton()).toBeDisabled();
    expect(createButton()).toHaveTextContent("Création…");
    expect(titleInput()).toHaveAttribute("readonly");
    expect(descriptionInput()).toHaveAttribute("readonly");
    expect(destinationSelect()).toBeDisabled();
    expect(abandonButton()).toBeDisabled();
    expect(closeButton()).toBeDisabled();
    expect(screen.queryByText("Transformée en tâche")).not.toBeInTheDocument();
    expect(texts()).toEqual(["Lent"]); // la carte est encore là

    // Échap pendant l'envoi n'abandonne rien.
    fireEvent.keyDown(titleInput(), { key: "Escape" });
    expect(modal()).toBeInTheDocument();

    await act(async () =>
      slow.resolve(
        await backend.invoke("convert_inbox_item_to_task", {
          id: backend.state.items[0].id,
          expectedUpdatedAt: backend.state.items[0].updatedAt,
          title: "Lent",
        }),
      ),
    );
    expect(await screen.findByText("Transformée en tâche")).toBeInTheDocument();
  });

  it("une double validation (Entrée, Ctrl+Entrée, clic) ne crée qu'une seule tâche", async () => {
    backend.seed("Une seule fois");
    render(<InboxHome />);
    await openModal("Une seule fois", "Une seule fois");
    fireEvent.keyDown(titleInput(), { key: "Enter" });
    fireEvent.keyDown(titleInput(), { key: "Enter" });
    fireEvent.keyDown(descriptionInput(), { key: "Enter", ctrlKey: true });
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(calls("convert_inbox_item_to_task")).toHaveLength(1);
    expect(backend.state.tasks).toHaveLength(1);
  });

  it("Ctrl+Entrée depuis la description crée la tâche ; Entrée seule y ajoute un retour à la ligne", async () => {
    backend.seed("Raccourci");
    render(<InboxHome />);
    await openModal("Raccourci", "Raccourci");
    fireEvent.keyDown(descriptionInput(), { key: "Enter" }); // sans Ctrl : saisie normale
    expect(calls("convert_inbox_item_to_task")).toHaveLength(0);
    fireEvent.keyDown(descriptionInput(), { key: "Enter", ctrlKey: true });
    await screen.findByText("Transformée en tâche");
    expect(calls("convert_inbox_item_to_task")).toHaveLength(1);
  });
});

describe("003-A — validation et échecs (le brouillon n'est jamais perdu)", () => {
  it("un titre vide ne peut pas être validé", async () => {
    backend.seed("Titre à effacer");
    render(<InboxHome />);
    await openModal("Titre à effacer", "Titre à effacer");
    fireEvent.change(titleInput(), { target: { value: "   " } });
    expect(createButton()).toBeDisabled();
    fireEvent.keyDown(titleInput(), { key: "Enter" });
    expect(await within(modal()).findByText("Le titre de la tâche est vide.")).toBeInTheDocument();
    expect(calls("convert_inbox_item_to_task")).toHaveLength(0);
  });

  it("une description effacée est refusée avec une explication, sans jamais reprendre le texte d'origine", async () => {
    const item = backend.seed("Texte d'origine important");
    render(<InboxHome />);
    await openModal("Texte d'origine", "Texte d'origine important");
    fireEvent.change(descriptionInput(), { target: { value: "" } });

    expect(createButton()).toBeDisabled();
    expect(descriptionInput()).toHaveAttribute("aria-invalid", "true");
    expect(within(modal()).getByText(/La description ne peut pas être vide/)).toBeInTheDocument();
    fireEvent.keyDown(descriptionInput(), { key: "Enter", ctrlKey: true });
    fireEvent.keyDown(titleInput(), { key: "Enter" });
    expect(calls("convert_inbox_item_to_task")).toHaveLength(0);
    expect(descriptionInput()).toHaveValue(""); // jamais remplacée en silence
    expect(backend.state.tasks).toHaveLength(0);

    // Une description blanche (espaces, retours) est aussi refusée.
    fireEvent.change(descriptionInput(), { target: { value: "  \n \t " } });
    expect(createButton()).toBeDisabled();

    // Saisir un texte suffit : la tâche reçoit exactement ce texte, pas celui de la capture.
    fireEvent.change(descriptionInput(), { target: { value: "Nouveau texte" } });
    expect(createButton()).toBeEnabled();
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0].details).toBe("Nouveau texte");
    expect(backend.find(item.id)!.content).toBe("Texte d'origine important");
  });

  it("échec de Rust : modale ouverte, titre, description et classement conservés, nouvel essai possible", async () => {
    backend.seed("Fragile", "moi");
    backend.state.failures.set("convert_inbox_item_to_task", {
      code: "storage",
      message: "L'enregistrement local a échoué.",
    });
    render(<InboxHome />);
    await openModal("Fragile", "Fragile");
    fireEvent.change(titleInput(), { target: { value: "Mon titre précis" } });
    fireEvent.change(descriptionInput(), { target: { value: "Ma description longue et précise" } });
    fireEvent.change(destinationSelect(), { target: { value: "administratif" } });
    fireEvent.click(createButton());

    expect(await within(modal()).findByRole("alert")).toHaveTextContent("Votre brouillon est conservé");
    expect(titleInput()).toHaveValue("Mon titre précis");
    expect(descriptionInput()).toHaveValue("Ma description longue et précise");
    expect(destinationSelect()).toHaveValue("administratif");
    expect(createButton()).toBeEnabled();
    expect(backend.state.tasks).toHaveLength(0);
    expect(texts()).toEqual(["Fragile"]);

    backend.state.failures.clear();
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0]).toMatchObject({
      title: "Mon titre précis",
      details: "Ma description longue et précise",
      destinationId: "administratif",
    });
  });

  it("destination refusée par Rust : message, brouillon conservé, un autre classement peut être choisi", async () => {
    backend.seed("Classement");
    backend.state.failures.set("convert_inbox_item_to_task", {
      code: "unknown_destination",
      message: "Cette destination n'est pas disponible.",
    });
    render(<InboxHome />);
    await openModal("Classement", "Classement");
    fireEvent.change(descriptionInput(), { target: { value: "Description saisie" } });
    fireEvent.change(destinationSelect(), { target: { value: "finances" } });
    fireEvent.click(createButton());
    expect(await within(modal()).findByRole("alert")).toHaveTextContent("Choisissez un autre classement");
    expect(descriptionInput()).toHaveValue("Description saisie");

    backend.state.failures.clear();
    fireEvent.change(destinationSelect(), { target: { value: "moi" } });
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0].destinationId).toBe("moi");
  });

  it("une description refusée par Rust (trop longue) est expliquée et conservée", async () => {
    backend.seed("Longue");
    backend.state.failures.set("convert_inbox_item_to_task", {
      code: "details_too_long",
      message: "La description dépasse 10000 caractères.",
    });
    render(<InboxHome />);
    await openModal("Longue", "Longue");
    fireEvent.change(descriptionInput(), { target: { value: "x".repeat(50) } });
    fireEvent.click(createButton());
    expect(await within(modal()).findByRole("alert")).toHaveTextContent("dépasse 10000 caractères");
    expect(descriptionInput()).toHaveValue("x".repeat(50));
  });
});

describe("003-A — fermeture et protection du brouillon", () => {
  it("brouillon intact : Échap, ✕ et « Abandonner » ferment sans avertissement ; la capture reste intacte", async () => {
    const item = backend.seed("Capture");
    render(<InboxHome />);
    for (const close of [
      () => fireEvent.keyDown(titleInput(), { key: "Escape" }),
      () => fireEvent.click(closeButton()),
      () => fireEvent.click(abandonButton()),
    ]) {
      await openModal("Capture", "Capture");
      close();
      await waitFor(() => expect(queryModal()).not.toBeInTheDocument());
      expect(screen.queryByText(LEAVE)).not.toBeInTheDocument();
      expect(detail()).toBeInTheDocument(); // la fiche reste ouverte
      expect(transformButton()).toHaveFocus(); // le focus revient à la commande d'origine
      expect(backend.state.tasks).toHaveLength(0);
      expect(backend.find(item.id)).toMatchObject({ convertedAt: null, content: "Capture" });
      expect(texts()).toEqual(["Capture"]);
    }
  });

  it("un clic sur l'arrière-plan n'abandonne rien, même avec un brouillon modifié", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openModal("Capture", "Capture");
    fireEvent.change(descriptionInput(), { target: { value: "Brouillon précieux" } });

    // Un clic sur la « zone extérieure » est un clic dont la cible est le <dialog> lui-même.
    fireEvent.mouseDown(modal());
    fireEvent.mouseUp(modal());
    fireEvent.click(modal());

    expect(queryModal()).toBeInTheDocument();
    expect(screen.queryByText(LEAVE)).not.toBeInTheDocument();
    expect(descriptionInput()).toHaveValue("Brouillon précieux");
    expect(backend.state.tasks).toHaveLength(0);
  });

  it("brouillon modifié : Échap affiche l'avertissement DANS la modale ; Continuer le garde", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openModal("Capture", "Capture");
    fireEvent.change(descriptionInput(), { target: { value: "Description modifiée" } });

    fireEvent.keyDown(descriptionInput(), { key: "Escape" });
    const warning = await within(modal()).findByText(LEAVE);
    expect(warning).toBeInTheDocument();
    expect(screen.getAllByRole("dialog")).toHaveLength(1); // jamais une seconde modale
    // Rien à « enregistrer » : seulement abandonner ou continuer.
    expect(within(modal()).queryByRole("button", { name: "Enregistrer" })).not.toBeInTheDocument();

    fireEvent.click(within(modal()).getByRole("button", { name: "Continuer" }));
    expect(screen.queryByText(LEAVE)).not.toBeInTheDocument();
    expect(descriptionInput()).toHaveValue("Description modifiée");
    await waitFor(() => expect(titleInput()).toHaveFocus());
    expect(backend.state.tasks).toHaveLength(0);
  });

  it("Échap sur l'avertissement équivaut à « Continuer »", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openModal("Capture", "Capture");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });
    fireEvent.keyDown(titleInput(), { key: "Escape" });
    expect(await within(modal()).findByText(LEAVE)).toBeInTheDocument();
    fireEvent.keyDown(modal(), { key: "Escape" });
    expect(screen.queryByText(LEAVE)).not.toBeInTheDocument();
    expect(titleInput()).toHaveValue("Titre perso");
    expect(modal()).toBeInTheDocument();
  });

  it("brouillon modifié : « Abandonner le brouillon » ferme la modale sans créer ; un nouveau brouillon repart de la capture", async () => {
    const item = backend.seed("Capture", "moi");
    render(<InboxHome />);
    await openModal("Capture", "Capture");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });
    fireEvent.change(descriptionInput(), { target: { value: "Description perso" } });
    fireEvent.change(destinationSelect(), { target: { value: "finances" } });
    fireEvent.click(abandonButton());
    fireEvent.click(await within(modal()).findByRole("button", { name: "Abandonner le brouillon" }));

    await waitFor(() => expect(queryModal()).not.toBeInTheDocument());
    expect(transformButton()).toHaveFocus();
    expect(backend.state.tasks).toHaveLength(0);
    expect(backend.find(item.id)).toMatchObject({ content: "Capture", destinationId: "moi", convertedAt: null });
    expect(texts()).toEqual(["Capture"]); // toujours dans « À organiser »

    // Un nouveau brouillon repart de la capture, jamais de l'ancien.
    fireEvent.click(transformButton());
    await waitFor(() => expect(titleInput()).toHaveValue("Capture"));
    expect(descriptionInput()).toHaveValue("Capture");
    expect(destinationSelect()).toHaveValue("moi");
  });

  it("chaque champ modifié protège le brouillon ; revenir à la valeur d'origine le rend intact", async () => {
    backend.seed("Capture", "moi");
    render(<InboxHome />);
    await openModal("Capture", "Capture");

    for (const edit of [
      () => fireEvent.change(descriptionInput(), { target: { value: "Autre" } }),
      () => fireEvent.change(destinationSelect(), { target: { value: "finances" } }),
      () => fireEvent.change(titleInput(), { target: { value: "Autre titre" } }),
    ]) {
      edit();
      fireEvent.click(closeButton());
      expect(await within(modal()).findByText(LEAVE)).toBeInTheDocument();
      fireEvent.click(within(modal()).getByRole("button", { name: "Continuer" }));
    }

    // Description et classement remis comme à l'ouverture : seul le titre (modifié) protège.
    fireEvent.change(descriptionInput(), { target: { value: "Capture" } });
    fireEvent.change(destinationSelect(), { target: { value: "moi" } });
    fireEvent.change(titleInput(), { target: { value: "Capture" } });
    fireEvent.click(closeButton());
    // Un titre déjà « édité » reste protégé par prudence (comme en 002-B).
    expect(await within(modal()).findByText(LEAVE)).toBeInTheDocument();
  });

  it("description ou classement remis à l'état d'origine : le brouillon redevient intact", async () => {
    backend.seed("Capture", "moi");
    render(<InboxHome />);
    await openModal("Capture", "Capture");
    fireEvent.change(descriptionInput(), { target: { value: "Autre" } });
    fireEvent.change(destinationSelect(), { target: { value: "finances" } });
    fireEvent.change(descriptionInput(), { target: { value: "Capture" } });
    fireEvent.change(destinationSelect(), { target: { value: "moi" } });
    fireEvent.click(closeButton());
    await waitFor(() => expect(queryModal()).not.toBeInTheDocument());
    expect(screen.queryByText(LEAVE)).not.toBeInTheDocument();
  });

  it("brouillon modifié : ouvrir une autre capture ou une autre vue est protégé", async () => {
    backend.seed("Première");
    backend.seed("Seconde");
    render(<InboxHome />);
    await openModal("Première", "Première");
    fireEvent.change(descriptionInput(), { target: { value: "Ma description" } });

    // Le fond est inerte dans un vrai navigateur ; en test, on déclenche directement l'ouverture.
    fireEvent.click(screen.getByRole("button", { name: /Seconde/ }));
    expect(await within(modal()).findByText(LEAVE)).toBeInTheDocument();
    fireEvent.click(within(modal()).getByRole("button", { name: "Continuer" }));
    expect(descriptionInput()).toHaveValue("Ma description");

    fireEvent.click(screen.getByRole("button", { name: "Corbeille" }));
    expect(await within(modal()).findByText(LEAVE)).toBeInTheDocument();
    fireEvent.click(within(modal()).getByRole("button", { name: "Continuer" }));
    expect(screen.getByRole("heading", { name: "À organiser" })).toBeInTheDocument();
    expect(descriptionInput()).toHaveValue("Ma description");

    // Abandon explicite : l'autre capture s'ouvre, rien n'a été créé.
    fireEvent.click(screen.getByRole("button", { name: /Seconde/ }));
    fireEvent.click(await within(modal()).findByRole("button", { name: "Abandonner le brouillon" }));
    await waitFor(() => expect(queryModal()).not.toBeInTheDocument());
    expect(backend.state.tasks).toHaveLength(0);
    expect(detailText()).toHaveValue("Seconde");
  });

  it("la fermeture de la fenêtre est suspendue seulement si le brouillon est modifié", async () => {
    backend.seed("Capture");
    render(<InboxHome />);
    await openModal("Capture", "Capture");
    expect(await requestClose()).toBe(false); // brouillon intact : rien à perdre

    fireEvent.change(descriptionInput(), { target: { value: "Ma description" } });
    expect(await requestClose()).toBe(true);
    expect(await within(modal()).findByText(LEAVE)).toBeInTheDocument();
    fireEvent.click(within(modal()).getByRole("button", { name: "Continuer" }));
    expect(appWindow.destroy).not.toHaveBeenCalled();
    expect(descriptionInput()).toHaveValue("Ma description");

    // Abandonner explicitement : la fenêtre peut se fermer, la capture reste intacte.
    expect(await requestClose()).toBe(true);
    fireEvent.click(await within(modal()).findByRole("button", { name: "Abandonner le brouillon" }));
    await waitFor(() => expect(appWindow.destroy).toHaveBeenCalled());
    expect(backend.state.tasks).toHaveLength(0);
  });

  it("un brouillon abandonné ne laisse aucune trace : la capture n'est ni modifiée ni convertie", async () => {
    const item = backend.seed("Capture d'origine", "moi");
    render(<InboxHome />);
    await openModal("Capture d'origine", "Capture d'origine");
    fireEvent.change(descriptionInput(), { target: { value: "Réécrit" } });
    fireEvent.change(destinationSelect(), { target: { value: "finances" } });
    fireEvent.click(abandonButton());
    fireEvent.click(await within(modal()).findByRole("button", { name: "Abandonner le brouillon" }));
    await waitFor(() => expect(queryModal()).not.toBeInTheDocument());
    expect(calls("update_inbox_item")).toHaveLength(0);
    expect(calls("convert_inbox_item_to_task")).toHaveLength(0);
    expect(backend.find(item.id)).toEqual(item);
  });
});

describe("003-A — capture modifiée non enregistrée (étape explicite)", () => {
  it("texte modifié : Enregistrer, puis la modale reprend la version enregistrée", async () => {
    const item = backend.seed("Avant");
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Après" } });
    fireEvent.click(transformButton());
    expect(await screen.findByText(/Vous avez des modifications non enregistrées/)).toBeInTheDocument();
    expect(queryModal()).not.toBeInTheDocument(); // aucune modale tant que rien n'est décidé
    expect(backend.find(item.id)!.content).toBe("Avant"); // jamais enregistré en silence

    const banner = screen.getByText(/Vous avez des modifications non enregistrées/).closest(".leave") as HTMLElement;
    fireEvent.click(within(banner).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(queryModal()).toBeInTheDocument());
    expect(backend.find(item.id)!.content).toBe("Après");
    expect(titleInput()).toHaveValue("Après");
    expect(descriptionInput()).toHaveValue("Après");
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0].details).toBe("Après");
    expect(convertCall()).toMatchObject({ expectedUpdatedAt: backend.find(item.id)!.updatedAt });
  });

  it("texte modifié : Abandonner rétablit le texte enregistré avant d'ouvrir la modale", async () => {
    const item = backend.seed("Enregistré");
    render(<InboxHome />);
    await openCard("Enregistré");
    fireEvent.change(detailText(), { target: { value: "Brouillon jetable" } });
    fireEvent.click(transformButton());
    fireEvent.click(await screen.findByRole("button", { name: "Abandonner les modifications" }));
    await waitFor(() => expect(queryModal()).toBeInTheDocument());
    expect(descriptionInput()).toHaveValue("Enregistré");
    expect(detailText()).toHaveValue("Enregistré");
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0].details).toBe("Enregistré");
    expect(backend.find(item.id)!.content).toBe("Enregistré");
  });

  it("texte modifié : Continuer à modifier ne change rien et n'ouvre pas la modale", async () => {
    backend.seed("Texte");
    render(<InboxHome />);
    await openCard("Texte");
    fireEvent.change(detailText(), { target: { value: "Texte plus long" } });
    fireEvent.click(transformButton());
    fireEvent.click(await screen.findByRole("button", { name: "Continuer à modifier" }));
    expect(detailText()).toHaveValue("Texte plus long");
    expect(detailText()).not.toHaveAttribute("readonly");
    expect(queryModal()).not.toBeInTheDocument();
  });

  it("« Continuer à modifier » le texte garde le focus sur le texte (comportement 001-B)", async () => {
    backend.seed("Texte");
    render(<InboxHome />);
    await openCard("Texte");
    fireEvent.change(detailText(), { target: { value: "Texte modifié" } });
    fireEvent.click(within(detail()).getByRole("button", { name: "Fermer la fiche" }));
    fireEvent.click(await screen.findByRole("button", { name: "Continuer à modifier" }));
    expect(detailText()).toHaveFocus();
  });
});

describe("003-A — réponses asynchrones et conflits", () => {
  it("une proposition de titre tardive n'écrase jamais un titre déjà modifié", async () => {
    backend.seed("Capture à titrer");
    const late = deferred<string>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "suggest_task_title" ? late.promise : backend.invoke(command, args),
    );
    render(<InboxHome />);
    await openCard("Capture à titrer");
    fireEvent.click(transformButton());
    await waitFor(() => expect(queryModal()).toBeInTheDocument());
    expect(titleInput()).toHaveAttribute("placeholder", expect.stringMatching(/Titre proposé/));
    // La description est préremplie tout de suite, sans attendre la proposition de titre.
    expect(descriptionInput()).toHaveValue("Capture à titrer");

    fireEvent.change(titleInput(), { target: { value: "Mon titre" } });
    await act(async () => late.resolve("Titre proposé tardif"));
    expect(titleInput()).toHaveValue("Mon titre");
  });

  it("une proposition tardive ne touche jamais à la description ni au classement saisis", async () => {
    backend.seed("Capture", "moi");
    const late = deferred<string>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "suggest_task_title" ? late.promise : backend.invoke(command, args),
    );
    render(<InboxHome />);
    await openCard("Capture");
    fireEvent.click(transformButton());
    await waitFor(() => expect(queryModal()).toBeInTheDocument());
    fireEvent.change(descriptionInput(), { target: { value: "Ma description" } });
    fireEvent.change(destinationSelect(), { target: { value: "finances" } });

    await act(async () => late.resolve("Titre proposé"));
    expect(titleInput()).toHaveValue("Titre proposé"); // titre non modifié : la proposition s'applique
    expect(descriptionInput()).toHaveValue("Ma description");
    expect(destinationSelect()).toHaveValue("finances");
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
    await waitFor(() => expect(queryModal()).toBeInTheDocument());
    await act(async () => late.resolve("Titre proposé"));
    expect(titleInput()).toHaveValue("Titre proposé");
  });

  it("une proposition tardive d'une modale refermée est ignorée", async () => {
    backend.seed("Première");
    const late = deferred<string>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "suggest_task_title" ? late.promise : backend.invoke(command, args),
    );
    render(<InboxHome />);
    await openCard("Première");
    fireEvent.click(transformButton());
    await waitFor(() => expect(queryModal()).toBeInTheDocument());
    fireEvent.click(abandonButton()); // brouillon intact : fermeture directe
    await waitFor(() => expect(queryModal()).not.toBeInTheDocument());
    await act(async () => late.resolve("Tardif"));
    expect(queryModal()).not.toBeInTheDocument();
  });

  it("la réponse tardive d'une création ne fait pas rouvrir ni écraser un autre brouillon", async () => {
    backend.seed("Capture");
    const slow = deferred<unknown>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "convert_inbox_item_to_task" ? slow.promise : backend.invoke(command, args),
    );
    render(<InboxHome />);
    await openModal("Capture", "Capture");
    fireEvent.click(createButton());
    const item = backend.state.items[0];
    await act(async () => {
      slow.reject({ code: "storage", message: "L'enregistrement local a échoué." });
    });
    expect(await within(modal()).findByRole("alert")).toHaveTextContent("brouillon est conservé");
    expect(createButton()).toBeEnabled();
    expect(backend.find(item.id)!.convertedAt).toBeNull();
  });

  it("conflit, description NON modifiée : la modale suit la version actuelle, le titre est conservé", async () => {
    const item = backend.seed("Version 1");
    const openedVersion = item.updatedAt; // `seed` renvoie l'objet stocké : on fige la valeur
    render(<InboxHome />);
    await openModal("Version 1", "Version 1");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });

    // Modification ailleurs (autre fenêtre, autre appareil…) pendant que la modale est ouverte.
    const stored = backend.find(item.id)!;
    stored.content = "Version 2";
    stored.updatedAt += 10;

    fireEvent.click(createButton());
    expect(await within(modal()).findByText(/Cette capture a changé depuis son ouverture/)).toBeInTheDocument();
    expect(titleInput()).toHaveValue("Titre perso");
    expect(descriptionInput()).toHaveValue("Version 2");
    expect(backend.state.tasks).toHaveLength(0);

    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0]).toMatchObject({ title: "Titre perso", details: "Version 2" });
    expect(convertCall()).toMatchObject({ expectedUpdatedAt: openedVersion });
    expect(calls("convert_inbox_item_to_task")[1][1]).toMatchObject({ expectedUpdatedAt: openedVersion + 10 });
  });

  it("conflit, description MODIFIÉE : le brouillon de l'utilisateur est conservé tel quel", async () => {
    const item = backend.seed("Version 1", "moi");
    render(<InboxHome />);
    await openModal("Version 1", "Version 1");
    fireEvent.change(descriptionInput(), { target: { value: "Ma réécriture soignée" } });
    fireEvent.change(destinationSelect(), { target: { value: "finances" } });

    const stored = backend.find(item.id)!;
    stored.content = "Version 2";
    stored.destinationId = "externe";
    stored.updatedAt += 10;

    fireEvent.click(createButton());
    expect(await within(modal()).findByText(/Votre brouillon est conservé/)).toBeInTheDocument();
    expect(descriptionInput()).toHaveValue("Ma réécriture soignée");
    expect(destinationSelect()).toHaveValue("finances");
    expect(backend.state.tasks).toHaveLength(0);
    expect(detailText()).toHaveValue("Version 1"); // le texte de la fiche n'a pas été touché

    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    expect(backend.state.tasks[0]).toMatchObject({
      details: "Ma réécriture soignée",
      destinationId: "finances",
    });
    expect(backend.find(item.id)!.content).toBe("Version 2"); // la capture n'est jamais réécrite
  });

  it("déjà convertie ailleurs, brouillon modifié : la modale reste ouverte avec un message, rien n'est perdu", async () => {
    const item = backend.seed("Déjà fait");
    render(<InboxHome />);
    await openModal("Déjà fait", "Déjà fait");
    fireEvent.change(titleInput(), { target: { value: "Titre perso" } });
    fireEvent.change(descriptionInput(), { target: { value: "Ma longue description" } });

    // Une autre conversion a eu lieu entre-temps.
    const existing = await backend.invoke("convert_inbox_item_to_task", {
      id: item.id,
      expectedUpdatedAt: item.updatedAt,
      title: "Créée ailleurs",
    });
    const taskId = (existing as { task: { id: string } }).task.id;

    fireEvent.click(createButton());
    const alert = await within(modal()).findByRole("alert");
    expect(alert).toHaveTextContent("déjà transformée en tâche");
    expect(alert).toHaveTextContent("Votre brouillon reste ici");
    expect(titleInput()).toHaveValue("Titre perso");
    expect(descriptionInput()).toHaveValue("Ma longue description");
    expect(createButton()).toBeDisabled(); // plus de création possible : aucun doublon
    expect(backend.state.tasks).toHaveLength(1);

    // Fermer demande une décision explicite (le brouillon est modifié).
    fireEvent.click(abandonButton());
    fireEvent.click(await within(modal()).findByRole("button", { name: "Abandonner le brouillon" }));
    await waitFor(() => expect(queryModal()).not.toBeInTheDocument());
    // La fiche derrière est devenue lecture seule, avec « Voir la tâche ».
    expect(detailText()).toHaveAttribute("readonly");
    expect(within(detail()).queryByRole("button", { name: "Transformer en tâche" })).not.toBeInTheDocument();
    fireEvent.click(within(detail()).getByRole("button", { name: "Voir la tâche" }));
    const taskPanel = await screen.findByRole("region", { name: "Fiche de la tâche" });
    expect(within(taskPanel).getByLabelText("Titre")).toHaveValue("Créée ailleurs");
    expect(backend.state.tasks[0].id).toBe(taskId);
  });

  it("déjà convertie ailleurs, brouillon intact : la modale se ferme avec un message dans la fiche", async () => {
    const item = backend.seed("Déjà fait");
    render(<InboxHome />);
    await openModal("Déjà fait", "Déjà fait");
    await backend.invoke("convert_inbox_item_to_task", {
      id: item.id,
      expectedUpdatedAt: item.updatedAt,
      title: "Créée ailleurs",
    });
    fireEvent.click(createButton());
    await waitFor(() => expect(queryModal()).not.toBeInTheDocument());
    expect(await within(detail()).findByRole("alert")).toHaveTextContent("déjà transformée en tâche");
    expect(backend.state.tasks).toHaveLength(1);
  });

  it("mise à la corbeille ailleurs, brouillon modifié : message dans la modale, brouillon conservé", async () => {
    const item = backend.seed("Supprimée ailleurs");
    render(<InboxHome />);
    await openModal("Supprimée ailleurs", "Supprimée ailleurs");
    fireEvent.change(descriptionInput(), { target: { value: "Ma description" } });
    backend.find(item.id)!.deletedAt = 99;

    fireEvent.click(createButton());
    const alert = await within(modal()).findByRole("alert");
    expect(alert).toHaveTextContent("corbeille");
    expect(descriptionInput()).toHaveValue("Ma description");
    expect(createButton()).toBeDisabled();
    expect(backend.state.tasks).toHaveLength(0);
  });

  it("capture supprimée définitivement ailleurs : message, brouillon conservé", async () => {
    const item = backend.seed("Disparue");
    render(<InboxHome />);
    await openModal("Disparue", "Disparue");
    fireEvent.change(descriptionInput(), { target: { value: "Ma description" } });
    backend.state.items = backend.state.items.filter((i) => i.id !== item.id);

    fireEvent.click(createButton());
    expect(await within(modal()).findByRole("alert")).toHaveTextContent("n'existe plus");
    expect(descriptionInput()).toHaveValue("Ma description");
    expect(createButton()).toBeDisabled();
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

describe("003-A — notifications, annulation et retour de la capture (mécanismes de 002 inchangés)", () => {
  async function createFirst(text: string) {
    await openModal(text, text);
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
  }

  it("une nouvelle notification remplace la précédente et relance la minuterie, même à horodatage identique", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1_800_000_000_000); // deux événements « au même instant »
    backend.seed("Première");
    backend.seed("Seconde");
    render(<InboxHome />);
    await openModal("Première", "Première");
    fireEvent.click(createButton());
    const first = (await screen.findByText("Transformée en tâche")).closest(".toast") as HTMLElement;

    await openModal("Seconde", "Seconde");
    fireEvent.click(createButton());
    await waitFor(() => expect(backend.state.tasks).toHaveLength(2));
    await waitFor(() => expect(first.isConnected).toBe(false)); // l'ancienne est démontée
    const toasts = document.querySelectorAll(".toast");
    expect(toasts).toHaveLength(1);
    expect(toasts[0]).not.toBe(first); // nouvelle instance : sa minuterie de 4 s repart
    vi.restoreAllMocks();
  });

  it("« Annuler » remet la capture dans la boîte à l'identique et conserve la tâche annulée", async () => {
    const item = backend.seed("À annuler", "moi");
    render(<InboxHome />);
    await openModal("À annuler", "À annuler");
    fireEvent.change(descriptionInput(), { target: { value: "Description réécrite" } });
    fireEvent.change(destinationSelect(), { target: { value: "finances" } });
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    await waitFor(() => expect(cards()).toHaveLength(0));

    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await waitFor(() => expect(texts()).toEqual(["À annuler"]));
    expect(screen.getByRole("status")).toHaveTextContent("Conversion annulée");
    expect(backend.find(item.id)).toMatchObject({
      content: "À annuler",
      destinationId: "moi",
      convertedAt: null,
      convertedTaskId: null,
      updatedAt: item.updatedAt,
    });
    expect(backend.state.tasks).toHaveLength(1);
    expect(backend.state.tasks[0]).toMatchObject({ details: "Description réécrite" }); // conservée
    expect(backend.state.tasks[0].deletedAt).not.toBeNull();
    expect(screen.queryByText("Transformée en tâche")).not.toBeInTheDocument();
  });

  it("annulation refusée si la tâche a été modifiée : la capture reste convertie, message clair", async () => {
    backend.seed("Déjà avancée");
    render(<InboxHome />);
    await createFirst("Déjà avancée");
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
    await createFirst("Deux essais");
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await waitFor(() => expect(texts()).toEqual(["Deux essais"]));

    await openModal("Deux essais", "Deux essais");
    fireEvent.click(createButton());
    await waitFor(() => expect(backend.state.tasks).toHaveLength(2));
    expect(backend.state.tasks.filter((t) => t.deletedAt === null)).toHaveLength(1);
  });

  it("le focus passe à la carte voisine à la fermeture de la fiche", async () => {
    backend.seed("Avant");
    backend.seed("Celle-ci");
    backend.seed("Après");
    render(<InboxHome />);
    await openModal("Celle-ci", "Celle-ci");
    fireEvent.click(createButton());
    await waitFor(() => expect(texts()).toEqual(["Avant", "Après"]));
    await waitFor(() => expect(document.activeElement?.closest("li")?.textContent).toMatch(/Après|Avant/));
  });

  it("l'accueil reflète la conversion (relecture depuis la base) et la capture rapide est vide", async () => {
    for (let i = 0; i < 3; i++) backend.seed(`Capture ${i}`);
    render(<InboxHome />);
    await openModal("Capture 1", "Capture 1");
    fireEvent.click(createButton());
    await screen.findByText("Transformée en tâche");
    await waitFor(() => expect(texts()).toEqual(["Capture 0", "Capture 2"]));
    expect(field()).toHaveValue("");
  });

  it("non-régression : la capture rapide fonctionne toujours et n'ouvre aucune modale", async () => {
    render(<InboxHome />);
    fireEvent.change(field(), { target: { value: "Nouvelle idée" } });
    fireEvent.click(screen.getByRole("button", { name: /Envoyer/ }));
    await waitFor(() => expect(texts()).toEqual(["Nouvelle idée"]));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(backend.state.tasks).toHaveLength(0);
  });
});
