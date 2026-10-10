import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
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

beforeEach(() => {
  backend = createFakeBackend();
  tauri.invoke.mockReset();
  tauri.invoke.mockImplementation((command, args) => backend.invoke(command, args));
});

/** Crée une tâche par la commande (comme le ferait le panneau de conversion). */
async function makeTask(content: string, destination: string | null = null, title: string | null = null) {
  const item = backend.seed(content, destination);
  const done = (await backend.invoke("convert_inbox_item_to_task", {
    id: item.id,
    expectedUpdatedAt: item.updatedAt,
    title,
  })) as { task: { id: string } };
  return { item, taskId: done.task.id };
}

const openTasks = async () => {
  fireEvent.click(await screen.findByRole("button", { name: "Tâches" }));
  return screen.findByRole("region", { name: "Tâches" });
};
const taskCards = (view: HTMLElement) => within(view).queryAllByRole("listitem");
const titles = (view: HTMLElement) => taskCards(view).map((li) => li.querySelector(".inbox__content")?.textContent);
const taskDetail = () => screen.getByRole("region", { name: "Fiche de la tâche" });

describe("002-B — accès et liste des tâches", () => {
  it("le lien « Tâches » est dans l'en-tête de la boîte, à côté de « Corbeille »", async () => {
    render(<InboxHome />);
    const box = (await screen.findByRole("heading", { name: "À organiser" })).closest("section") as HTMLElement;
    const tasks = within(box).getByRole("button", { name: "Tâches" });
    const trash = within(box).getByRole("button", { name: "Corbeille" });
    expect(tasks.parentElement).toBe(trash.parentElement);
  });

  it("aucune tâche : message clair, retour possible, la capture rapide reste visible", async () => {
    render(<InboxHome />);
    const view = await openTasks();
    expect(await within(view).findByText(/Aucune tâche pour le moment/)).toBeInTheDocument();
    expect(screen.getByLabelText("Capture rapide", { selector: "textarea" })).toBeInTheDocument();
    fireEvent.click(within(view).getByRole("button", { name: "← Retour" }));
    expect(await screen.findByRole("heading", { name: "À organiser" })).toBeInTheDocument();
  });

  it("la plus ancienne en haut, la plus récente en bas, avec titre, destination, statut et date", async () => {
    await makeTask("Première", "finances", "Payer la facture");
    await makeTask("Deuxième");
    await makeTask("Troisième", "moi");
    render(<InboxHome />);
    const view = await openTasks();
    await waitFor(() => expect(titles(view)).toEqual(["Payer la facture", "Deuxième", "Troisième"]));
    const first = taskCards(view)[0];
    expect(first).toHaveTextContent("À faire");
    expect(first).toHaveTextContent("Finances");
    expect(first.querySelector("time")).not.toBeNull();
    expect(taskCards(view)[1]).not.toHaveTextContent("Finances");
    expect(within(view).getByText("3 affichées sur 3")).toBeInTheDocument();
  });

  it("une tâche annulée n'apparaît plus ; les captures converties ne reviennent pas dans la boîte", async () => {
    const { taskId } = await makeTask("Annulée");
    await makeTask("Gardée");
    await backend.invoke("cancel_task_conversion", { id: taskId });
    render(<InboxHome />);
    const view = await openTasks();
    await waitFor(() => expect(titles(view)).toEqual(["Gardée"]));
    fireEvent.click(within(view).getByRole("button", { name: "← Retour" }));
    // « Annulée » est redevenue une capture ; « Gardée » est traitée, donc absente de la boîte.
    await screen.findByText("Annulée");
    expect(screen.queryByText("Gardée")).not.toBeInTheDocument();
  });

  it("pagination par lots : les plus anciennes se chargent au-dessus, sans doublon", async () => {
    for (let i = 0; i < PAGE_SIZE + 5; i++) await makeTask(`Tâche ${String(i).padStart(3, "0")}`);
    render(<InboxHome />);
    const view = await openTasks();
    await waitFor(() => expect(taskCards(view)).toHaveLength(PAGE_SIZE));
    expect(titles(view).at(-1)).toBe(`Tâche ${String(PAGE_SIZE + 4).padStart(3, "0")}`); // la plus récente en bas
    expect(titles(view)[0]).toBe("Tâche 005");
    fireEvent.click(within(view).getByRole("button", { name: /Charger les 5 plus anciennes/ }));
    await waitFor(() => expect(taskCards(view)).toHaveLength(PAGE_SIZE + 5));
    expect(titles(view)[0]).toBe("Tâche 000");
    expect(new Set(titles(view)).size).toBe(PAGE_SIZE + 5);
  });

  it("les tâches viennent de list_tasks (jamais d'une autre commande), avec le curseur renvoyé", async () => {
    for (let i = 0; i < PAGE_SIZE + 1; i++) await makeTask(`T${i}`);
    render(<InboxHome />);
    const view = await openTasks();
    await waitFor(() => expect(taskCards(view)).toHaveLength(PAGE_SIZE));
    fireEvent.click(within(view).getByRole("button", { name: /Charger/ }));
    await waitFor(() => expect(taskCards(view)).toHaveLength(PAGE_SIZE + 1));
    const lists = tauri.invoke.mock.calls.filter(([name]) => name === "list_tasks");
    expect(lists[0][1]).toEqual({ limit: PAGE_SIZE, before: null });
    expect(lists[1][1]).toMatchObject({ limit: PAGE_SIZE, before: { sortKey: expect.any(Number), id: expect.any(String) } });
  });
});

describe("002-B — fiche d'une tâche (lecture seule)", () => {
  it("affiche titre, détails complets, statut, destination, dates et origine ; rien n'est modifiable", async () => {
    const { item } = await makeTask("Appeler la banque\nDemander le relevé de mars", "finances", "Relevé de mars");
    render(<InboxHome />);
    const view = await openTasks();
    fireEvent.click(await within(view).findByRole("button", { name: /Relevé de mars/ }));

    const panel = taskDetail();
    expect(within(panel).getByLabelText("Titre")).toHaveValue("Relevé de mars");
    const details = within(panel).getByLabelText("Détails") as HTMLTextAreaElement;
    expect(details).toHaveValue("Appeler la banque\nDemander le relevé de mars");
    expect(details).toHaveAttribute("readonly");
    expect(within(panel).getByLabelText("Titre")).toHaveAttribute("readonly");
    expect(within(panel).getByText("À faire")).toBeInTheDocument();
    expect(within(panel).getByText("Finances")).toBeInTheDocument();
    await waitFor(() => expect(within(panel).getByText(/Issue de la capture du/)).toBeInTheDocument());
    expect(within(panel).queryByRole("button", { name: /Enregistrer|corbeille|Transformer/ })).not.toBeInTheDocument();
    expect(within(panel).getByText(/Consultation seule/)).toBeInTheDocument();
    expect(backend.find(item.id)!.convertedAt).not.toBeNull(); // rien n'a bougé
  });

  it("la carte ouverte est repérée, Échap ferme la fiche et rend le focus à la carte", async () => {
    await makeTask("Une tâche");
    render(<InboxHome />);
    const view = await openTasks();
    const open = await within(view).findByRole("button", { name: /Une tâche/ });
    open.focus();
    fireEvent.click(open);
    await screen.findByRole("region", { name: "Fiche de la tâche" });
    expect(open).toHaveAttribute("aria-current", "true");

    fireEvent.keyDown(taskDetail(), { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("region", { name: "Fiche de la tâche" })).not.toBeInTheDocument());
    await waitFor(() => expect(open).toHaveFocus());
    expect(open).not.toHaveAttribute("aria-current");
  });

  it("une origine introuvable n'empêche pas d'afficher la tâche", async () => {
    await makeTask("Origine perdue");
    backend.state.failures.set("get_inbox_item", { code: "not_found", message: "Cette capture n'existe plus." });
    render(<InboxHome />);
    const view = await openTasks();
    fireEvent.click(await within(view).findByRole("button", { name: /Origine perdue/ }));
    await waitFor(() => expect(within(taskDetail()).getByText(/introuvable/)).toBeInTheDocument());
    expect(within(taskDetail()).getByLabelText("Titre")).toHaveValue("Origine perdue");
  });

  it("fenêtre compacte : la fiche ouverte est signalée pour que le CSS remplace la vue", async () => {
    await makeTask("Compacte");
    const { container } = render(<InboxHome />);
    const view = await openTasks();
    expect(container.querySelector(".inbox-home")).toHaveAttribute("data-detail", "closed");
    fireEvent.click(await within(view).findByRole("button", { name: /Compacte/ }));
    await screen.findByRole("region", { name: "Fiche de la tâche" });
    expect(container.querySelector(".inbox-home")).toHaveAttribute("data-detail", "open");
    expect(within(taskDetail()).getByRole("button", { name: "Fermer la fiche" })).toBeInTheDocument();
  });
});

describe("002-B — navigation et protections", () => {
  it("ouvrir une tâche avec un brouillon de capture : avertissement, rien de perdu", async () => {
    await makeTask("Tâche existante");
    backend.seed("Capture en cours");
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: /Capture en cours/ }));
    const text = within(screen.getByRole("region", { name: "Fiche de la capture" })).getByLabelText("Texte");
    fireEvent.change(text, { target: { value: "Brouillon précieux" } });

    const view = await openTasks().catch(() => null);
    // Le changement de vue est protégé : l'avertissement apparaît, la vue n'a pas changé.
    expect(view).toBeNull();
    expect(await screen.findByText(/Vous avez des modifications non enregistrées/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Continuer à modifier" }));
    expect(text).toHaveValue("Brouillon précieux");
  });

  it("depuis une tâche ouverte, ouvrir une capture ferme la fiche de la tâche (une seule fiche à la fois)", async () => {
    await makeTask("Tâche");
    backend.seed("Capture libre");
    render(<InboxHome />);
    const view = await openTasks();
    fireEvent.click(await within(view).findByRole("button", { name: /Tâche/ }));
    await screen.findByRole("region", { name: "Fiche de la tâche" });
    fireEvent.click(within(view).getByRole("button", { name: "← Retour" }));
    fireEvent.click(await screen.findByRole("button", { name: /Capture libre/ }));
    await screen.findByRole("region", { name: "Fiche de la capture" });
    expect(screen.queryByRole("region", { name: "Fiche de la tâche" })).not.toBeInTheDocument();
  });

  it("l'annulation depuis la notification ferme la fiche de la tâche annulée et la retire de la liste", async () => {
    backend.seed("À convertir puis annuler");
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: /À convertir puis annuler/ }));
    const detail = screen.getByRole("region", { name: "Fiche de la capture" });
    fireEvent.click(within(detail).getByRole("button", { name: "Transformer en tâche" }));
    await waitFor(() => expect(within(detail).getByLabelText("Titre de la tâche")).toHaveValue("À convertir puis annuler"));
    fireEvent.click(within(detail).getByRole("button", { name: "Créer la tâche" }));
    await screen.findByText("Transformée en tâche");
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await waitFor(() => expect(backend.state.tasks[0].deletedAt).not.toBeNull());
    const view = await openTasks();
    expect(await within(view).findByText(/Aucune tâche pour le moment/)).toBeInTheDocument();
  });

  it("une tâche créée apparaît dans la vue après conversion depuis la fiche", async () => {
    backend.seed("Nouvelle tâche à créer", "externe");
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: /Nouvelle tâche à créer/ }));
    const detail = screen.getByRole("region", { name: "Fiche de la capture" });
    fireEvent.click(within(detail).getByRole("button", { name: "Transformer en tâche" }));
    await waitFor(() => expect(within(detail).getByLabelText("Titre de la tâche")).toHaveValue("Nouvelle tâche à créer"));
    fireEvent.click(within(detail).getByRole("button", { name: "Créer la tâche" }));
    await screen.findByText("Transformée en tâche");

    const view = await openTasks();
    await waitFor(() => expect(titles(view)).toEqual(["Nouvelle tâche à créer"]));
    expect(taskCards(view)[0]).toHaveTextContent("Externe");
  });
});
