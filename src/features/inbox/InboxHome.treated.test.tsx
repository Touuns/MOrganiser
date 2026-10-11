import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeBackend, type FakeBackend } from "../../test/fakeBackend";
import { InboxHome } from "./InboxHome";
import { PAGE_SIZE } from "./PagedCaptureView";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>(),
  isTauri: vi.fn(() => true),
}));
vi.mock("@tauri-apps/api/core", () => tauri);

let backend: FakeBackend;

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((res) => {
    resolve = res;
  });
  return { promise, resolve };
}

beforeEach(() => {
  backend = createFakeBackend();
  tauri.invoke.mockReset();
  tauri.invoke.mockImplementation((command, args) => backend.invoke(command, args));
});

/** Capture déjà convertie en tâche (état de départ des scénarios). */
async function treated(content: string, destination: string | null = null, title: string | null = null) {
  const item = backend.seed(content, destination);
  const done = (await backend.invoke("convert_inbox_item_to_task", {
    id: item.id,
    expectedUpdatedAt: item.updatedAt,
    title,
  })) as { task: { id: string } };
  return { item, taskId: done.task.id };
}

const calls = (command: string) => tauri.invoke.mock.calls.filter(([name]) => name === command);
const openTreated = async () => {
  fireEvent.click(await screen.findByRole("button", { name: "Traitées" }));
  return screen.findByRole("region", { name: "Traitées" });
};
const cards = (view: HTMLElement) => within(view).queryAllByRole("listitem");
const texts = (view: HTMLElement) => cards(view).map((li) => li.querySelector(".inbox__content")?.textContent);
const captureSheet = () => screen.getByRole("region", { name: /Capture transformée en tâche|Fiche de la capture/ });
const taskSheet = () => screen.getByRole("region", { name: "Fiche de la tâche" });
const queryCaptureSheet = () =>
  screen.queryByRole("region", { name: /Capture transformée en tâche|Fiche de la capture/ });
const returnButton = () => within(captureSheet()).getByRole("button", { name: "Remettre dans la boîte" });
const keepButton = () => within(captureSheet()).getByRole("button", { name: "Garder en Traitées" });
const confirmButton = () => within(captureSheet()).getByRole("button", { name: /Confirmer la remise|Remise en cours…/ });

describe("002-C — accès et liste « Traitées »", () => {
  it("le lien est dans l'en-tête de la boîte, à côté de « Tâches » et « Corbeille »", async () => {
    render(<InboxHome />);
    const box = (await screen.findByRole("heading", { name: "À organiser" })).closest("section") as HTMLElement;
    const group = within(box).getByRole("button", { name: "Traitées" }).parentElement;
    expect(within(box).getByRole("button", { name: "Tâches" }).parentElement).toBe(group);
    expect(within(box).getByRole("button", { name: "Corbeille" }).parentElement).toBe(group);
  });

  it("état vide clair, capture rapide visible, retour à la boîte", async () => {
    render(<InboxHome />);
    const view = await openTreated();
    expect(await within(view).findByText(/Aucune capture traitée pour le moment/)).toBeInTheDocument();
    expect(screen.getByLabelText("Capture rapide", { selector: "textarea" })).toBeInTheDocument();
    fireEvent.click(within(view).getByRole("button", { name: "← Retour" }));
    expect(await screen.findByRole("heading", { name: "À organiser" })).toBeInTheDocument();
  });

  it("classées par date de conversion (plus ancienne en haut), avec « Traitée le » et la destination", async () => {
    // Créées A, B, C mais converties dans l'ordre C, A, B : l'ordre suit la CONVERSION.
    const a = backend.seed("Capture A", "finances");
    const b = backend.seed("Capture B");
    const c = backend.seed("Capture C", "moi");
    for (const item of [c, a, b]) {
      await backend.invoke("convert_inbox_item_to_task", { id: item.id, expectedUpdatedAt: item.updatedAt, title: null });
    }
    render(<InboxHome />);
    const view = await openTreated();
    await waitFor(() => expect(texts(view)).toEqual(["Capture C", "Capture A", "Capture B"]));
    expect(cards(view)[0]).toHaveTextContent(/Traitée le/);
    expect(cards(view)[0]).toHaveTextContent("Moi");
    expect(cards(view)[1]).toHaveTextContent("Finances");
    expect(cards(view)[2]).toHaveTextContent("À classer");
    expect(within(view).getByText("3 affichées sur 3")).toBeInTheDocument();
    // Pas de badge « Tâche » redondant.
    expect(within(view).queryByText("Tâche")).not.toBeInTheDocument();
  });

  it("seules les captures ACTUELLEMENT converties y figurent (pas un historique)", async () => {
    const { taskId } = await treated("Conversion annulée");
    await treated("Toujours traitée");
    await backend.invoke("cancel_task_conversion", { id: taskId });
    render(<InboxHome />);
    const view = await openTreated();
    await waitFor(() => expect(texts(view)).toEqual(["Toujours traitée"]));
    // Reconvertie : elle revient, une seule fois.
    const item = backend.find(backend.state.tasks[0].originInboxItemId!)!;
    await backend.invoke("convert_inbox_item_to_task", { id: item.id, expectedUpdatedAt: item.updatedAt, title: null });
    fireEvent.click(within(view).getByRole("button", { name: "← Retour" }));
    const again = await openTreated();
    await waitFor(() => expect(texts(again)).toEqual(["Toujours traitée", "Conversion annulée"]));
  });

  it("pagination par lots de 50 : anciennes au-dessus, sans doublon, compteur exact", async () => {
    for (let i = 0; i < PAGE_SIZE + 5; i++) await treated(`Traitée ${String(i).padStart(3, "0")}`);
    render(<InboxHome />);
    const view = await openTreated();
    await waitFor(() => expect(cards(view)).toHaveLength(PAGE_SIZE));
    expect(texts(view)[0]).toBe("Traitée 005");
    expect(texts(view).at(-1)).toBe(`Traitée ${String(PAGE_SIZE + 4).padStart(3, "0")}`);
    expect(within(view).getByText(`${PAGE_SIZE} affichées sur ${PAGE_SIZE + 5}`)).toBeInTheDocument();
    fireEvent.click(within(view).getByRole("button", { name: /Charger les 5 plus anciennes/ }));
    await waitFor(() => expect(cards(view)).toHaveLength(PAGE_SIZE + 5));
    expect(texts(view)[0]).toBe("Traitée 000");
    expect(new Set(texts(view)).size).toBe(PAGE_SIZE + 5);
    const lists = calls("list_converted_items");
    expect(lists[0][1]).toEqual({ limit: PAGE_SIZE, before: null });
    expect(lists[1][1]).toMatchObject({ before: { sortKey: expect.any(Number), id: expect.any(String) } });
  });

  it("le tri et la pagination suivent la date de CONVERSION (pas la création), dates d'origine inchangées", async () => {
    // 55 captures créées dans l'ordre 0..54, mais converties dans l'ordre inverse 54..0.
    const created = Array.from({ length: PAGE_SIZE + 5 }, (_, i) => backend.seed(`Capture ${String(i).padStart(3, "0")}`));
    const origin = created.map((item) => ({ id: item.id, createdAt: item.createdAt, updatedAt: item.updatedAt }));
    for (const item of [...created].reverse()) {
      await backend.invoke("convert_inbox_item_to_task", { id: item.id, expectedUpdatedAt: item.updatedAt, title: null });
    }
    render(<InboxHome />);
    const view = await openTreated();
    await waitFor(() => expect(cards(view)).toHaveLength(PAGE_SIZE));
    // Les 50 conversions les plus récentes = captures 049..000 ; la plus ancienne conversion en haut.
    expect(texts(view)[0]).toBe("Capture 049");
    expect(texts(view).at(-1)).toBe("Capture 000");

    fireEvent.click(within(view).getByRole("button", { name: /Charger les 5 plus anciennes/ }));
    await waitFor(() => expect(cards(view)).toHaveLength(PAGE_SIZE + 5));
    expect(texts(view).slice(0, 5)).toEqual(["Capture 054", "Capture 053", "Capture 052", "Capture 051", "Capture 050"]);
    expect(new Set(texts(view)).size).toBe(PAGE_SIZE + 5);

    // Le curseur de la page suivante porte la date de conversion de la dernière carte reçue.
    const cursor = (calls("list_converted_items")[1][1] as { before: { sortKey: number; id: string } }).before;
    const boundary = backend.find(created[PAGE_SIZE - 1].id)!; // Capture 049, la plus ancienne de la page 1
    expect(cursor).toEqual({ sortKey: boundary.convertedAt, id: boundary.id });
    expect(cursor.sortKey).not.toBe(boundary.createdAt);

    // Les dates d'origine n'ont pas bougé.
    for (const o of origin) {
      expect(backend.find(o.id)).toMatchObject({ createdAt: o.createdAt, updatedAt: o.updatedAt });
    }
  });

  it("non-régression : la boîte, « Voir tout », « Tâches » et « Corbeille » ne montrent pas les captures traitées", async () => {
    await treated("Déjà traitée");
    backend.seed("Encore dans la boîte");
    const trashed = backend.seed("À la corbeille");
    backend.find(trashed.id)!.deletedAt = 5;
    render(<InboxHome />);
    await screen.findByText("Encore dans la boîte");
    expect(screen.queryByText("Déjà traitée")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Corbeille" }));
    const trash = await screen.findByRole("region", { name: "Corbeille" });
    await waitFor(() => expect(texts(trash)).toEqual(["À la corbeille"]));
    fireEvent.click(within(trash).getByRole("button", { name: "← Retour" }));

    fireEvent.click(await screen.findByRole("button", { name: "Tâches" }));
    const tasks = await screen.findByRole("region", { name: "Tâches" });
    await waitFor(() => expect(texts(tasks)).toEqual(["Déjà traitée"]));
  });
});

describe("002-C — consultation de l'origine (lecture seule)", () => {
  it("la fiche conserve texte, destination et dates d'origine ; rien n'est modifiable", async () => {
    const { item } = await treated("Appeler la banque\nDemander le relevé de mars", "finances");
    render(<InboxHome />);
    const view = await openTreated();
    fireEvent.click(await within(view).findByRole("button", { name: /Appeler la banque/ }));

    const sheet = captureSheet();
    const text = within(sheet).getByLabelText("Texte") as HTMLTextAreaElement;
    expect(text).toHaveValue("Appeler la banque\nDemander le relevé de mars");
    expect(text).toHaveAttribute("readonly");
    expect(within(sheet).getByLabelText("Destination (facultatif)")).toBeDisabled();
    expect(within(sheet).getByLabelText("Destination (facultatif)")).toHaveValue("finances");
    expect(within(sheet).getByText(/Transformée en tâche le/)).toBeInTheDocument();
    expect(within(sheet).getByText("Jamais modifiée")).toBeInTheDocument();
    for (const name of ["Enregistrer", "Mettre à la corbeille", "Transformer en tâche"]) {
      expect(within(sheet).queryByRole("button", { name })).not.toBeInTheDocument();
    }
    expect(backend.find(item.id)!.content).toBe(item.content);
  });
});

describe("002-C — navigation tâche ↔ capture", () => {
  it("capture → tâche → capture, une seule fiche à la fois", async () => {
    const { item } = await treated("Aller-retour", null, "Titre de la tâche");
    render(<InboxHome />);
    const view = await openTreated();
    fireEvent.click(await within(view).findByRole("button", { name: /Aller-retour/ }));
    fireEvent.click(within(captureSheet()).getByRole("button", { name: "Voir la tâche" }));

    expect(await screen.findByRole("region", { name: "Fiche de la tâche" })).toBeInTheDocument();
    expect(queryCaptureSheet()).not.toBeInTheDocument();
    expect(within(taskSheet()).getByLabelText("Titre")).toHaveValue("Titre de la tâche");

    fireEvent.click(await within(taskSheet()).findByRole("button", { name: "Voir la capture" }));
    await waitFor(() => expect(queryCaptureSheet()).toBeInTheDocument());
    expect(screen.queryByRole("region", { name: "Fiche de la tâche" })).not.toBeInTheDocument();
    expect(within(captureSheet()).getByLabelText("Texte")).toHaveValue(item.content);
    expect(calls("get_inbox_item").some(([, args]) => (args as { id: string }).id === item.id)).toBe(true);
  });

  it("le brouillon d'une capture est protégé avant d'ouvrir sa tâche, rien n'est perdu", async () => {
    // Capture active avec un brouillon : « Voir la tâche » d'une AUTRE capture traitée passe par la garde.
    await treated("Traitée A");
    render(<InboxHome />);
    const view = await openTreated();
    fireEvent.click(await within(view).findByRole("button", { name: /Traitée A/ }));
    fireEvent.click(within(captureSheet()).getByRole("button", { name: "Voir la tâche" }));
    await screen.findByRole("region", { name: "Fiche de la tâche" });
    // Retour à la capture, puis fermeture : aucune garde parasite sans brouillon.
    fireEvent.click(within(taskSheet()).getByRole("button", { name: "Voir la capture" }));
    await waitFor(() => expect(queryCaptureSheet()).toBeInTheDocument());
    fireEvent.click(within(captureSheet()).getByRole("button", { name: "Fermer la fiche" }));
    await waitFor(() => expect(queryCaptureSheet()).not.toBeInTheDocument());
    expect(screen.queryByText(/modifications non enregistrées/)).not.toBeInTheDocument();
  });

  it("réponse tardive de « Voir la tâche » : n'ouvre jamais une fiche périmée", async () => {
    await treated("Première");
    await treated("Seconde");
    const late = deferred<unknown>();
    let first = true;
    tauri.invoke.mockImplementation((command, args) => {
      if (command === "get_task" && first) {
        first = false;
        return late.promise;
      }
      return backend.invoke(command, args);
    });
    render(<InboxHome />);
    const view = await openTreated();
    fireEvent.click(await within(view).findByRole("button", { name: /Première/ }));
    fireEvent.click(within(captureSheet()).getByRole("button", { name: "Voir la tâche" })); // en attente

    // L'utilisateur ouvre une autre capture avant la réponse.
    fireEvent.click(within(view).getByRole("button", { name: /Seconde/ }));
    await waitFor(() => expect(within(captureSheet()).getByLabelText("Texte")).toHaveValue("Seconde"));

    const task = backend.state.tasks[0];
    await act(async () => late.resolve({ ...task }));
    expect(screen.queryByRole("region", { name: "Fiche de la tâche" })).not.toBeInTheDocument();
    expect(within(captureSheet()).getByLabelText("Texte")).toHaveValue("Seconde");
  });

  it("réponse tardive de « Voir la capture » après fermeture de la tâche : rien ne s'ouvre", async () => {
    await treated("Origine tardive");
    const late = deferred<unknown>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "get_inbox_item" && (args as { id: string }).id === backend.state.items[0].id && lateOnce.value
        ? ((lateOnce.value = false), late.promise)
        : backend.invoke(command, args),
    );
    const lateOnce = { value: false };
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: "Tâches" }));
    const tasks = await screen.findByRole("region", { name: "Tâches" });
    fireEvent.click(await within(tasks).findByRole("button", { name: /Origine tardive/ }));
    await within(taskSheet()).findByText(/Issue de la capture du/); // lecture de l'origine déjà faite
    lateOnce.value = true;
    fireEvent.click(within(taskSheet()).getByRole("button", { name: "Voir la capture" })); // en attente
    fireEvent.click(within(taskSheet()).getByRole("button", { name: "Fermer la fiche" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Fiche de la tâche" })).not.toBeInTheDocument());

    await act(async () => late.resolve({ ...backend.state.items[0] }));
    expect(queryCaptureSheet()).not.toBeInTheDocument();
  });

  it("tâche devenue inactive (annulée ailleurs) : message, aucune fiche de tâche, listes relues", async () => {
    const { item, taskId } = await treated("Annulée ailleurs");
    render(<InboxHome />);
    const view = await openTreated();
    fireEvent.click(await within(view).findByRole("button", { name: /Annulée ailleurs/ }));
    await backend.invoke("cancel_task_conversion", { id: taskId }); // autre fenêtre

    fireEvent.click(within(captureSheet()).getByRole("button", { name: "Voir la tâche" }));
    expect(await screen.findByText(/Cette tâche a été annulée/)).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Fiche de la tâche" })).not.toBeInTheDocument();
    await waitFor(() => expect(texts(view)).toEqual([]));
    expect(backend.find(item.id)!.convertedAt).toBeNull();
  });

  it("tâche introuvable : message d'erreur clair, pas de fiche", async () => {
    await treated("Tâche perdue");
    backend.state.failures.set("get_task", { code: "task_not_found", message: "Cette tâche n'existe plus." });
    render(<InboxHome />);
    const view = await openTreated();
    fireEvent.click(await within(view).findByRole("button", { name: /Tâche perdue/ }));
    fireEvent.click(within(captureSheet()).getByRole("button", { name: "Voir la tâche" }));
    expect(await screen.findByText("Cette tâche n'existe plus.")).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Fiche de la tâche" })).not.toBeInTheDocument();
  });

  it("capture d'origine introuvable : message d'erreur clair, la fiche de la tâche reste ouverte", async () => {
    await treated("Origine perdue");
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: "Tâches" }));
    const tasks = await screen.findByRole("region", { name: "Tâches" });
    fireEvent.click(await within(tasks).findByRole("button", { name: /Origine perdue/ }));
    await within(taskSheet()).findByText(/Issue de la capture du/);
    backend.state.failures.set("get_inbox_item", { code: "not_found", message: "Cette capture n'existe plus." });
    fireEvent.click(within(taskSheet()).getByRole("button", { name: "Voir la capture" }));
    expect(await screen.findByText("Cette capture n'existe plus.")).toBeInTheDocument();
    expect(taskSheet()).toBeInTheDocument();
    expect(queryCaptureSheet()).not.toBeInTheDocument();
  });

  it("fenêtre compacte : la fiche ouverte depuis « Traitées » est signalée pour le remplacement de vue", async () => {
    await treated("Compacte");
    const { container } = render(<InboxHome />);
    const view = await openTreated();
    expect(container.querySelector(".inbox-home")).toHaveAttribute("data-detail", "closed");
    fireEvent.click(await within(view).findByRole("button", { name: /Compacte/ }));
    await screen.findByRole("region", { name: /Capture transformée en tâche/ });
    expect(container.querySelector(".inbox-home")).toHaveAttribute("data-detail", "open");
    expect(within(captureSheet()).getByRole("button", { name: "Fermer la fiche" })).toBeInTheDocument();
  });
});

describe("002-C — « Remettre dans la boîte »", () => {
  async function openTreatedSheet(label: RegExp) {
    render(<InboxHome />);
    const view = await openTreated();
    fireEvent.click(await within(view).findByRole("button", { name: label }));
    await screen.findByRole("region", { name: /Capture transformée en tâche/ });
    return view;
  }

  it("aucun bouton de remise sur les cartes de la liste ni dans la fiche de tâche", async () => {
    await treated("Sans raccourci");
    const view = await openTreatedSheet(/Sans raccourci/);
    expect(within(view).queryByRole("button", { name: /Remettre dans la boîte/ })).not.toBeInTheDocument();
    fireEvent.click(within(captureSheet()).getByRole("button", { name: "Voir la tâche" }));
    await screen.findByRole("region", { name: "Fiche de la tâche" });
    expect(within(taskSheet()).queryByRole("button", { name: /Remettre dans la boîte/ })).not.toBeInTheDocument();
  });

  it("confirmation intégrée : non modale, focus sur le choix sûr, rien n'est appelé avant confirmation", async () => {
    await treated("À confirmer");
    await openTreatedSheet(/À confirmer/);
    fireEvent.click(returnButton());

    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    const panel = within(captureSheet()).getByRole("region", { name: "Remettre dans la boîte" });
    expect(panel).toHaveTextContent(/tâche liée sera annulée mais conservée/);
    expect(panel).toHaveTextContent(/« Voir tout » la retrouve/); // pas de promesse de visibilité immédiate
    await waitFor(() => expect(keepButton()).toHaveFocus());
    expect(calls("cancel_task_conversion")).toHaveLength(0);

    // « Garder en Traitées » ferme la confirmation sans rien faire, focus rendu à la commande.
    fireEvent.click(keepButton());
    await waitFor(() => expect(returnButton()).toHaveFocus());
    expect(calls("cancel_task_conversion")).toHaveLength(0);
    expect(backend.state.tasks[0].deletedAt).toBeNull();
  });

  it("Échap ferme la confirmation sans fermer la fiche", async () => {
    await treated("Échap");
    await openTreatedSheet(/Échap/);
    fireEvent.click(returnButton());
    await waitFor(() => expect(keepButton()).toHaveFocus());
    fireEvent.keyDown(keepButton(), { key: "Escape" });
    await waitFor(() => expect(returnButton()).toHaveFocus());
    expect(captureSheet()).toBeInTheDocument();
    expect(calls("cancel_task_conversion")).toHaveLength(0);
  });

  it("succès : capture restaurée à l'identique, tâche conservée annulée, sortie de « Traitées », focus voisin, sans toast", async () => {
    const { item, taskId } = await treated("Première", "inventaire");
    await treated("Voisine");
    const before = { ...backend.find(item.id)! };
    const view = await openTreatedSheet(/Première/);
    fireEvent.click(returnButton());
    await waitFor(() => expect(keepButton()).toHaveFocus());
    fireEvent.click(confirmButton());

    await waitFor(() => expect(backend.find(item.id)!.convertedAt).toBeNull());
    // Identité, texte, destination et dates d'origine (updatedAt compris).
    const after = backend.find(item.id)!;
    expect(after).toMatchObject({
      id: before.id,
      content: before.content,
      destinationId: "inventaire",
      createdAt: before.createdAt,
      updatedAt: before.updatedAt,
      deletedAt: null,
      convertedAt: null,
      convertedTaskId: null,
    });
    expect(backend.findTask(taskId)!.deletedAt).not.toBeNull(); // conservée
    expect(calls("cancel_task_conversion")).toHaveLength(1);
    expect(calls("cancel_task_conversion")[0][1]).toEqual({ id: taskId });

    // La fiche se ferme, la capture sort de « Traitées », message sans notification « Annuler ».
    await waitFor(() => expect(queryCaptureSheet()).not.toBeInTheDocument());
    await waitFor(() => expect(texts(view)).toEqual(["Voisine"]));
    expect(screen.getByRole("status")).toHaveTextContent("Capture remise dans « À organiser ».");
    expect(document.querySelector(".toast")).toBeNull();
    // Focus utile : la carte voisine.
    await waitFor(() => expect(document.activeElement?.closest("li")?.textContent).toContain("Voisine"));
  });

  it("la capture remise réapparaît dans la boîte et dans « Voir tout », texte inchangé, tâche sortie de la liste", async () => {
    const { item } = await treated("Retour en boîte");
    await openTreatedSheet(/Retour en boîte/);
    fireEvent.click(returnButton());
    fireEvent.click(await screen.findByRole("button", { name: "Confirmer la remise" }));
    await waitFor(() => expect(queryCaptureSheet()).not.toBeInTheDocument());

    fireEvent.click(await screen.findByRole("button", { name: "← Retour" }));
    const card = await screen.findByText("Retour en boîte");
    expect(card.closest("li")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Tâches" }));
    const tasks = await screen.findByRole("region", { name: "Tâches" });
    expect(await within(tasks).findByText(/Aucune tâche pour le moment/)).toBeInTheDocument();
    expect(backend.find(item.id)!.content).toBe("Retour en boîte");
  });

  it("une capture ancienne reste retrouvable par « Voir tout » même hors des 20 de l'accueil", async () => {
    const { item } = await treated("Très ancienne");
    for (let i = 0; i < 25; i++) backend.seed(`Récente ${i}`);
    await openTreatedSheet(/Très ancienne/);
    fireEvent.click(returnButton());
    fireEvent.click(await screen.findByRole("button", { name: "Confirmer la remise" }));
    await waitFor(() => expect(backend.find(item.id)!.convertedAt).toBeNull());
    await waitFor(() => expect(queryCaptureSheet()).not.toBeInTheDocument());

    fireEvent.click(await screen.findByRole("button", { name: "← Retour" }));
    await screen.findByText(/Les 20 plus récentes sur 26/);
    expect(screen.queryByText("Très ancienne")).not.toBeInTheDocument(); // pas sur l'accueil
    fireEvent.click(screen.getByRole("button", { name: "Voir tout" }));
    const all = await screen.findByRole("region", { name: "Toutes les captures" });
    await waitFor(() => expect(within(all).getByText("Très ancienne")).toBeInTheDocument());
  });

  it("double validation : une seule annulation envoyée", async () => {
    await treated("Double clic");
    await openTreatedSheet(/Double clic/);
    fireEvent.click(returnButton());
    const confirm = await screen.findByRole("button", { name: "Confirmer la remise" });
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    fireEvent.click(confirm);
    await waitFor(() => expect(queryCaptureSheet()).not.toBeInTheDocument());
    expect(calls("cancel_task_conversion")).toHaveLength(1);
  });

  it("la remise d'une capture ne retire pas la notification « Annuler » d'une AUTRE capture", async () => {
    await treated("Déjà traitée");
    backend.seed("À convertir maintenant");
    render(<InboxHome />);
    // Conversion d'une autre capture : sa notification est affichée.
    fireEvent.click(await screen.findByRole("button", { name: /À convertir maintenant/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Transformer en tâche" }));
    await screen.findByLabelText("Titre de la tâche");
    fireEvent.click(await screen.findByRole("button", { name: "Créer la tâche" }));
    await screen.findByText("Transformée en tâche");

    // Remise d'une capture traitée différente, pendant que la notification est visible.
    const view = await openTreated();
    fireEvent.click(await within(view).findByRole("button", { name: /Déjà traitée/ }));
    await screen.findByRole("region", { name: /Capture transformée en tâche/ });
    fireEvent.click(returnButton());
    fireEvent.click(await screen.findByRole("button", { name: "Confirmer la remise" }));
    await waitFor(() => expect(queryCaptureSheet()).not.toBeInTheDocument());
    expect(screen.getByText("Transformée en tâche")).toBeInTheDocument();
  });

  it("pas de succès avant Rust : « Remise en cours… », capture toujours dans « Traitées »", async () => {
    const { item } = await treated("Lente");
    const slow = deferred<unknown>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "cancel_task_conversion" ? slow.promise : backend.invoke(command, args),
    );
    const view = await openTreatedSheet(/Lente/);
    fireEvent.click(returnButton());
    fireEvent.click(await screen.findByRole("button", { name: "Confirmer la remise" }));

    expect(confirmButton()).toBeDisabled();
    expect(confirmButton()).toHaveTextContent("Remise en cours…");
    expect(keepButton()).toBeDisabled();
    expect(screen.getByRole("status")).not.toHaveTextContent(/remise dans/);
    expect(texts(view)).toEqual(["Lente"]);

    const result = await backend.invoke("cancel_task_conversion", { id: backend.state.tasks[0].id });
    await act(async () => slow.resolve(result));
    await waitFor(() => expect(queryCaptureSheet()).not.toBeInTheDocument());
    expect(backend.find(item.id)!.convertedAt).toBeNull();
  });

  it("réponse tardive : ne ferme jamais la fiche d'une AUTRE capture ouverte entre-temps", async () => {
    await treated("Remise lente");
    await treated("Autre fiche");
    const slow = deferred<unknown>();
    tauri.invoke.mockImplementation((command, args) =>
      command === "cancel_task_conversion" ? slow.promise : backend.invoke(command, args),
    );
    const view = await openTreatedSheet(/Remise lente/);
    fireEvent.click(returnButton());
    fireEvent.click(await screen.findByRole("button", { name: "Confirmer la remise" }));

    // Pendant l'attente, l'utilisateur ouvre une autre capture.
    fireEvent.click(within(view).getByRole("button", { name: /Autre fiche/ }));
    await waitFor(() => expect(within(captureSheet()).getByLabelText("Texte")).toHaveValue("Autre fiche"));

    const task = backend.state.tasks.find((t) => t.details === "Remise lente")!;
    const result = await backend.invoke("cancel_task_conversion", { id: task.id });
    await act(async () => slow.resolve(result));
    await waitFor(() => expect(texts(view)).toEqual(["Autre fiche"])); // la première est sortie
    expect(within(captureSheet()).getByLabelText("Texte")).toHaveValue("Autre fiche"); // fiche intacte
  });
});

describe("002-C — refus, conflits et erreurs", () => {
  async function openReturnPanel(label: RegExp) {
    render(<InboxHome />);
    const view = await openTreated();
    fireEvent.click(await within(view).findByRole("button", { name: label }));
    await screen.findByRole("region", { name: /Capture transformée en tâche/ });
    fireEvent.click(returnButton());
    await screen.findByRole("button", { name: "Confirmer la remise" });
    return view;
  }

  it("tâche modifiée ou avancée : rien n'est annulé, la capture reste, « Voir la tâche » proposé", async () => {
    const { item, taskId } = await treated("Travail en cours", null, "Mon titre");
    backend.findTask(taskId)!.status = "in_progress";
    backend.findTask(taskId)!.updatedAt += 1;
    const view = await openReturnPanel(/Travail en cours/);
    fireEvent.click(confirmButton());

    const alert = await within(captureSheet()).findByRole("alert");
    expect(alert).toHaveTextContent(/déjà été modifiée/);
    expect(alert).toHaveTextContent(/reste dans « Traitées »/);
    expect(backend.find(item.id)!.convertedTaskId).toBe(taskId); // rien n'a bougé
    expect(backend.findTask(taskId)!.deletedAt).toBeNull();
    expect(texts(view)).toEqual(["Travail en cours"]);
    expect(captureSheet()).toBeInTheDocument();

    fireEvent.click(within(captureSheet()).getByRole("button", { name: "Voir la tâche" }));
    expect(await screen.findByRole("region", { name: "Fiche de la tâche" })).toBeInTheDocument();
    expect(within(taskSheet()).getByLabelText("Titre")).toHaveValue("Mon titre");
  });

  it("conversion déjà annulée ailleurs : message, listes relues, capture de retour dans la boîte", async () => {
    const { item, taskId } = await treated("Déjà annulée");
    const view = await openReturnPanel(/Déjà annulée/);
    await backend.invoke("cancel_task_conversion", { id: taskId }); // autre fenêtre
    fireEvent.click(confirmButton());

    expect(await within(captureSheet()).findByRole("alert")).toHaveTextContent(/déjà été annulée/);
    await waitFor(() => expect(texts(view)).toEqual([]));
    expect(backend.find(item.id)!.convertedAt).toBeNull();
    // La fiche reflète l'état réel : plus de remise proposée, la capture est modifiable.
    expect(within(captureSheet()).queryByRole("button", { name: "Remettre dans la boîte" })).not.toBeInTheDocument();
    expect(within(captureSheet()).getByLabelText("Texte")).not.toHaveAttribute("readonly");
  });

  it("tâche introuvable : erreur claire, état affiché conservé jusqu'à confirmation", async () => {
    const { item } = await treated("Tâche disparue");
    backend.state.failures.set("cancel_task_conversion", {
      code: "task_not_found",
      message: "Cette tâche n'existe plus.",
    });
    await openReturnPanel(/Tâche disparue/);
    fireEvent.click(confirmButton());
    expect(await within(captureSheet()).findByRole("alert")).toHaveTextContent("Cette tâche n'existe plus.");
    expect(backend.find(item.id)!.convertedAt).not.toBeNull();
    expect(screen.queryByText(/Capture remise dans/)).not.toBeInTheDocument();
  });

  it("capture introuvable : message clair, fiche en lecture seule, aucune remise annoncée", async () => {
    await treated("Capture disparue");
    backend.state.failures.set("cancel_task_conversion", {
      code: "not_found",
      message: "Cette capture n'existe plus.",
    });
    await openReturnPanel(/Capture disparue/);
    fireEvent.click(confirmButton());
    expect(await within(captureSheet()).findByRole("alert")).toHaveTextContent("Cette capture n'existe plus.");
    expect(screen.queryByText(/Capture remise dans/)).not.toBeInTheDocument();
    expect(within(captureSheet()).queryByRole("button", { name: "Remettre dans la boîte" })).toBeDisabled();
  });

  it("erreur de stockage ou état incohérent : rien ne change, nouvel essai possible puis succès", async () => {
    const { item } = await treated("Réessai");
    backend.state.failures.set("cancel_task_conversion", {
      code: "storage",
      message: "L'enregistrement local a échoué.",
    });
    const view = await openReturnPanel(/Réessai/);
    fireEvent.click(confirmButton());

    expect(await within(captureSheet()).findByRole("alert")).toHaveTextContent(/vous pouvez réessayer/);
    expect(texts(view)).toEqual(["Réessai"]);
    expect(backend.find(item.id)!.convertedAt).not.toBeNull();
    expect(confirmButton()).toBeEnabled(); // la confirmation reste ouverte

    backend.state.failures.set("cancel_task_conversion", {
      code: "inconsistent_state",
      message: "Un état inattendu a été détecté ; rien n'a été modifié.",
    });
    fireEvent.click(confirmButton());
    expect(await within(captureSheet()).findByRole("alert")).toHaveTextContent(/état inattendu/);
    expect(backend.find(item.id)!.convertedAt).not.toBeNull();

    backend.state.failures.clear();
    fireEvent.click(confirmButton());
    await waitFor(() => expect(backend.find(item.id)!.convertedAt).toBeNull());
    await waitFor(() => expect(queryCaptureSheet()).not.toBeInTheDocument());
  });
});
