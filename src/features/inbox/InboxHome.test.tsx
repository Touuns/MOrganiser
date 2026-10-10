import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeBackend, type FakeBackend } from "../../test/fakeBackend";
import { HOME_LIMIT, InboxHome } from "./InboxHome";
import { PAGE_SIZE } from "./PagedCaptureView";

// Faux pont Tauri : une « base » en mémoire qui imite les commandes Rust.
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

const field = () => screen.getByLabelText("Capture rapide", { selector: "textarea" });
const cards = () => screen.queryAllByRole("listitem");
const texts = () => cards().map((li) => li.querySelector(".inbox__content")?.textContent);
const capture = async (text: string, destination?: string) => {
  fireEvent.change(field(), { target: { value: text } });
  if (destination) {
    fireEvent.change(screen.getByLabelText("Destination (facultatif)"), { target: { value: destination } });
  }
  fireEvent.keyDown(field(), { key: "Enter" });
  await waitFor(() => expect(field()).toHaveValue(""));
};
const openCard = async (text: string) => fireEvent.click(await screen.findByRole("button", { name: new RegExp(text) }));
const detail = () => screen.getByRole("region", { name: /Fiche de la capture|Capture dans la corbeille/ });
const detailText = () => within(detail()).getByLabelText("Texte") as HTMLTextAreaElement;
const detailDestination = () => within(detail()).getByLabelText("Destination (facultatif)") as HTMLSelectElement;

describe("InboxHome : boîte et capture (001-A)", () => {
  it("place la boîte « À organiser » avant (au-dessus) du champ de capture", async () => {
    render(<InboxHome />);
    const heading = await screen.findByRole("heading", { name: "À organiser" });
    expect(heading.compareDocumentPosition(field()) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("affiche un état vide accueillant", async () => {
    render(<InboxHome />);
    expect(await screen.findByText(/Rien à organiser pour l'instant/)).toBeInTheDocument();
  });

  it("une capture apparaît en bas (au-dessus du champ), une seule fois, marquée « À classer »", async () => {
    backend.seed("Ancienne capture");
    render(<InboxHome />);
    await screen.findByText("Ancienne capture");
    await capture("Vendre ma PlayStation 5");
    await waitFor(() => expect(cards()).toHaveLength(2));
    expect(texts()).toEqual(["Ancienne capture", "Vendre ma PlayStation 5"]);
    expect(cards()[1]).toHaveTextContent("À classer");
    expect(screen.getAllByText("Vendre ma PlayStation 5")).toHaveLength(1);
    expect(screen.getByRole("status")).toHaveTextContent("Capture enregistrée.");
  });

  it("avec destination : reste dans la boîte, affiche la destination, filtrable", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    await capture("Vendre ma PlayStation 5", "inventaire");
    await capture("Note libre");
    const item = (await screen.findByText("Vendre ma PlayStation 5")).closest("li")!;
    expect(item).toHaveTextContent("Inventaire");
    expect(cards()).toHaveLength(2);

    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "destination:inventaire" } });
    await waitFor(() => expect(cards()).toHaveLength(1));
    expect(texts()).toEqual(["Vendre ma PlayStation 5"]);

    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "unclassified" } });
    await waitFor(() => expect(texts()).toEqual(["Note libre"]));
  });

  it("plusieurs captures successives s'ajoutent en bas, dans l'ordre chronologique", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    await capture("Un");
    await capture("Deux", "moi");
    await capture("Trois");
    await waitFor(() => expect(cards()).toHaveLength(3));
    expect(texts()).toEqual(["Un", "Deux", "Trois"]);
  });

  it("le texte est affiché comme du texte, jamais interprété", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    const dangerous = "'; DROP TABLE inbox_items; -- <b>gras</b> <img src=x onerror=alert(1)> l'été";
    await capture(dangerous);
    expect(await screen.findByText(dangerous)).toBeInTheDocument();
    expect(screen.getByRole("list").querySelector("b, img")).toBeNull();
  });

  it("en cas d'échec de capture : rien n'apparaît, le texte reste dans le champ", async () => {
    backend.state.failures.set("create_inbox_item", { code: "storage", message: "L'enregistrement local a échoué." });
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    fireEvent.change(field(), { target: { value: "À ne pas perdre" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("L'enregistrement local a échoué.");
    expect(field()).toHaveValue("À ne pas perdre");
    expect(cards()).toHaveLength(0);
  });

  it("signale une capture masquée par le filtre actif", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "destination:moi" } });
    await screen.findByText(/Aucune capture pour ce filtre/);
    await capture("Pour Finances", "finances");
    expect(screen.getByRole("status")).toHaveTextContent("masquée par le filtre");
  });

  it(`au-delà de ${HOME_LIMIT} : garde les plus récentes (pas les plus anciennes), en ordre chronologique`, async () => {
    for (let i = 0; i < HOME_LIMIT + 5; i++) backend.seed(`Capture ${i}`);
    render(<InboxHome />);
    await screen.findByText(`Capture ${HOME_LIMIT + 4}`);
    expect(texts()).toHaveLength(HOME_LIMIT);
    expect(texts()[0]).toBe("Capture 5");
    expect(texts().at(-1)).toBe(`Capture ${HOME_LIMIT + 4}`);
    expect(screen.queryByText("Capture 0")).not.toBeInTheDocument();
    expect(screen.getByText(/Les 20 plus récentes sur 25/)).toBeInTheDocument();
    expect(tauri.invoke).toHaveBeenCalledWith("list_inbox_items", {
      filter: { type: "all" },
      limit: HOME_LIMIT,
      before: null,
    });

    await capture("Toute nouvelle");
    await waitFor(() => expect(texts().at(-1)).toBe("Toute nouvelle"));
    expect(texts()).toHaveLength(HOME_LIMIT);
    expect(screen.queryByText("Capture 5")).not.toBeInTheDocument();
  });

  it("une erreur de lecture propose de réessayer", async () => {
    backend.state.failures.set("list_inbox_items", {
      code: "unavailable",
      message: "La base de données est indisponible.",
    });
    render(<InboxHome />);
    expect(await screen.findByRole("alert")).toHaveTextContent("La base de données est indisponible.");
    backend.state.failures.clear();
    fireEvent.click(screen.getByRole("button", { name: "Réessayer" }));
    expect(await screen.findByText(/Rien à organiser/)).toBeInTheDocument();
  });
});

describe("InboxHome : fiche, modification et corbeille (001-B)", () => {
  it("un clic sur une carte ouvre la fiche : texte, destination, dates", async () => {
    backend.seed("Rembourser Paul", "finances");
    render(<InboxHome />);
    await openCard("Rembourser Paul");
    expect(detailText()).toHaveValue("Rembourser Paul");
    expect(detailDestination()).toHaveValue("finances");
    expect(within(detail()).getByText("Créée")).toBeInTheDocument();
    expect(within(detail()).getByText("Dernière modification")).toBeInTheDocument();
    expect(within(detail()).getByText("Jamais modifiée")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Rembourser Paul/ })).toHaveAttribute("aria-current", "true");
  });

  it("la fiche est ouvrable au clavier (le bouton de la carte reçoit le focus)", async () => {
    backend.seed("Au clavier");
    render(<InboxHome />);
    const card = await screen.findByRole("button", { name: /Au clavier/ });
    card.focus();
    expect(card).toHaveFocus();
    expect(card.tagName).toBe("BUTTON"); // Entrée et Espace activent un bouton natif
  });

  it("modifier texte et destination puis enregistrer met à jour la liste et la date", async () => {
    const created = backend.seed("Avant");
    const { createdAt, updatedAt: openedVersion } = created; // le faux backend modifie l'objet sur place
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Après" } });
    fireEvent.change(detailDestination(), { target: { value: "moi" } });
    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(texts()).toEqual(["Après"]));
    expect(cards()[0]).toHaveTextContent("Moi");
    expect(within(detail()).queryByText("Jamais modifiée")).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("Modifications enregistrées.");
    expect(backend.find(created.id)!.updatedAt).toBeGreaterThan(createdAt);
    expect(backend.find(created.id)!.createdAt).toBe(createdAt);
    expect(backend.state.items).toHaveLength(1);
    expect(tauri.invoke).toHaveBeenCalledWith("update_inbox_item", {
      id: created.id,
      content: "Après",
      destinationId: "moi",
      expectedUpdatedAt: openedVersion,
    });
  });

  it("on peut retirer la destination (« Aucune »)", async () => {
    backend.seed("Classée", "finances");
    render(<InboxHome />);
    await openCard("Classée");
    fireEvent.change(detailDestination(), { target: { value: "" } });
    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(cards()[0]).toHaveTextContent("À classer"));
  });

  it("Enregistrer n'est actif que s'il y a une modification ; Annuler remet l'original", async () => {
    backend.seed("Original");
    render(<InboxHome />);
    await openCard("Original");
    const save = within(detail()).getByRole("button", { name: "Enregistrer" });
    const cancel = within(detail()).getByRole("button", { name: "Annuler les modifications" });
    expect(save).toBeDisabled();
    expect(cancel).toBeDisabled();
    fireEvent.change(detailText(), { target: { value: "Brouillon" } });
    expect(save).toBeEnabled();
    fireEvent.click(cancel);
    expect(detailText()).toHaveValue("Original");
    expect(save).toBeDisabled();
  });

  it("un texte vide ne peut pas être enregistré", async () => {
    backend.seed("Plein");
    render(<InboxHome />);
    await openCard("Plein");
    fireEvent.change(detailText(), { target: { value: "   " } });
    expect(within(detail()).getByRole("button", { name: "Enregistrer" })).toBeDisabled();
  });

  it("fermer avec des modifications non enregistrées demande confirmation", async () => {
    backend.seed("Fragile");
    render(<InboxHome />);
    await openCard("Fragile");
    fireEvent.change(detailText(), { target: { value: "Modifié" } });
    fireEvent.click(within(detail()).getByRole("button", { name: "Fermer la fiche" }));
    expect(await screen.findByText(/modifications non enregistrées/)).toBeInTheDocument();
    expect(detail()).toBeInTheDocument();

    // Continuer à modifier : rien ne bouge, le brouillon est intact.
    fireEvent.click(screen.getByRole("button", { name: "Continuer à modifier" }));
    expect(screen.queryByText(/modifications non enregistrées/)).not.toBeInTheDocument();
    expect(detailText()).toHaveValue("Modifié");
  });

  it("abandonner ferme la fiche sans rien écrire", async () => {
    const item = backend.seed("Stable");
    render(<InboxHome />);
    await openCard("Stable");
    fireEvent.change(detailText(), { target: { value: "Jetable" } });
    fireEvent.keyDown(detailText(), { key: "Escape" });
    fireEvent.click(await screen.findByRole("button", { name: "Abandonner les modifications" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: /Fiche/ })).not.toBeInTheDocument());
    expect(backend.find(item.id)!.content).toBe("Stable");
  });

  it("« Enregistrer » depuis l'avertissement enregistre puis ferme", async () => {
    const item = backend.seed("Avant");
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Après" } });
    fireEvent.click(within(detail()).getByRole("button", { name: "Fermer la fiche" }));
    const banner = (await screen.findByText(/modifications non enregistrées/)).closest("div")!;
    fireEvent.click(within(banner).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: /Fiche/ })).not.toBeInTheDocument());
    expect(backend.find(item.id)!.content).toBe("Après");
  });

  it("ouvrir une autre carte avec un brouillon demande aussi confirmation", async () => {
    backend.seed("Première");
    backend.seed("Seconde");
    render(<InboxHome />);
    await openCard("Première");
    fireEvent.change(detailText(), { target: { value: "Brouillon" } });
    fireEvent.click(screen.getByRole("button", { name: /Seconde/ }));
    expect(await screen.findByText(/modifications non enregistrées/)).toBeInTheDocument();
    expect(detailText()).toHaveValue("Brouillon");
    fireEvent.click(screen.getByRole("button", { name: "Abandonner les modifications" }));
    await waitFor(() => expect(detailText()).toHaveValue("Seconde"));
  });

  it("un échec d'enregistrement conserve intégralement le brouillon et permet de réessayer", async () => {
    backend.seed("Base", "moi");
    render(<InboxHome />);
    await openCard("Base");
    fireEvent.change(detailText(), { target: { value: "Brouillon précieux" } });
    fireEvent.change(detailDestination(), { target: { value: "externe" } });
    backend.state.failures.set("update_inbox_item", { code: "storage", message: "L'enregistrement local a échoué." });
    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    expect(await within(detail()).findByRole("alert")).toHaveTextContent("Votre brouillon est conservé");
    expect(detailText()).toHaveValue("Brouillon précieux");
    expect(detailDestination()).toHaveValue("externe");
    expect(texts()).toEqual(["Base"]);

    backend.state.failures.clear();
    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(texts()).toEqual(["Brouillon précieux"]));
  });

  it("un conflit de version conserve le brouillon et demande un choix explicite", async () => {
    const item = backend.seed("Version A");
    render(<InboxHome />);
    await openCard("Version A");
    fireEvent.change(detailText(), { target: { value: "Mon brouillon" } });
    // Modification faite ailleurs entre-temps.
    backend.find(item.id)!.content = "Version B ailleurs";
    backend.find(item.id)!.updatedAt += 5;

    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    expect(await within(detail()).findByRole("alert")).toHaveTextContent("a changé depuis son ouverture");
    expect(detailText()).toHaveValue("Mon brouillon");
    expect(backend.find(item.id)!.content).toBe("Version B ailleurs"); // rien n'a été écrasé

    // Second « Enregistrer » : choix explicite de remplacer la version actuelle.
    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(backend.find(item.id)!.content).toBe("Mon brouillon"));
  });

  it("mettre à la corbeille : la carte disparaît, la notification propose d'annuler", async () => {
    const item = backend.seed("À jeter", "externe");
    backend.seed("Autre");
    render(<InboxHome />);
    await openCard("À jeter");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await waitFor(() => expect(texts()).toEqual(["Autre"]));
    expect(screen.queryByRole("region", { name: /Fiche/ })).not.toBeInTheDocument();
    expect(screen.getByText("Capture mise à la corbeille.")).toBeInTheDocument();
    expect(backend.find(item.id)!.deletedAt).not.toBeNull();
    expect(backend.state.items).toHaveLength(2); // rien n'est détruit

    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await waitFor(() => expect(texts()).toEqual(["À jeter", "Autre"]));
    expect(backend.find(item.id)!.deletedAt).toBeNull();
    expect(backend.find(item.id)!.destinationId).toBe("externe");
    expect(screen.getByRole("status")).toHaveTextContent("Capture restaurée.");
  });

  it("la corbeille avec brouillon propose d'abord d'enregistrer ou d'abandonner", async () => {
    const item = backend.seed("Avant");
    render(<InboxHome />);
    await openCard("Avant");
    fireEvent.change(detailText(), { target: { value: "Modifié" } });
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    expect(await screen.findByText(/modifications non enregistrées/)).toBeInTheDocument();
    expect(backend.find(item.id)!.deletedAt).toBeNull(); // pas encore supprimée
    fireEvent.click(screen.getByRole("button", { name: "Continuer à modifier" }));
    expect(backend.find(item.id)!.deletedAt).toBeNull();
  });

  it("un échec de mise à la corbeille est signalé sans fausse réussite", async () => {
    const item = backend.seed("Résiste");
    render(<InboxHome />);
    await openCard("Résiste");
    backend.state.failures.set("trash_inbox_item", { code: "storage", message: "L'enregistrement local a échoué." });
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    expect(await within(detail()).findByRole("alert")).toHaveTextContent("n'a pas été déplacée");
    expect(texts()).toEqual(["Résiste"]);
    expect(screen.queryByText("Capture mise à la corbeille.")).not.toBeInTheDocument();
    expect(backend.find(item.id)!.deletedAt).toBeNull();
  });

  it("l'annulation depuis la notification signale son échec clairement", async () => {
    backend.seed("Incertaine");
    render(<InboxHome />);
    await openCard("Incertaine");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await screen.findByText("Capture mise à la corbeille.");
    backend.state.failures.set("restore_inbox_item", { code: "storage", message: "L'enregistrement local a échoué." });
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("toujours dans la corbeille");
    expect(texts()).toEqual([]);
  });

  it("la vue Corbeille liste les captures supprimées et permet de les restaurer", async () => {
    const trashed = backend.seed("Supprimée", "finances");
    backend.find(trashed.id)!.deletedAt = 1_760_000_999_000;
    backend.seed("Active");
    render(<InboxHome />);
    await screen.findByText("Active");
    fireEvent.click(screen.getByRole("button", { name: "Corbeille" }));
    expect(await screen.findByRole("heading", { name: "Corbeille" })).toBeInTheDocument();
    await waitFor(() => expect(texts()).toEqual(["Supprimée"]));
    expect(cards()[0]).toHaveTextContent("Supprimée le");
    expect(cards()[0]).toHaveTextContent("Finances");

    fireEvent.click(screen.getByRole("button", { name: "Restaurer" }));
    await waitFor(() => expect(screen.getByText("La corbeille est vide.")).toBeInTheDocument());
    expect(backend.find(trashed.id)!.deletedAt).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "← Retour" }));
    await waitFor(() => expect(texts()).toEqual(["Supprimée", "Active"]));
  });

  it("une capture de la corbeille s'ouvre en lecture seule, avec Restaurer", async () => {
    const trashed = backend.seed("Lecture seule");
    backend.find(trashed.id)!.deletedAt = 1_760_000_999_000;
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: "Corbeille" }));
    await openCard("Lecture seule");
    expect(detailText()).toHaveAttribute("readonly");
    expect(within(detail()).queryByRole("button", { name: "Enregistrer" })).not.toBeInTheDocument();
    fireEvent.click(within(detail()).getByRole("button", { name: "Restaurer" }));
    await waitFor(() => expect(backend.find(trashed.id)!.deletedAt).toBeNull());
    await waitFor(() => expect(detailText()).not.toHaveAttribute("readonly"));
  });

  it("un échec de restauration est signalé et la capture reste dans la corbeille", async () => {
    const trashed = backend.seed("Coincée");
    backend.find(trashed.id)!.deletedAt = 1_760_000_999_000;
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: "Corbeille" }));
    await screen.findByText("Coincée");
    backend.state.failures.set("restore_inbox_item", { code: "storage", message: "L'enregistrement local a échoué." });
    fireEvent.click(screen.getByRole("button", { name: "Restaurer" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("toujours dans la corbeille");
    expect(texts()).toEqual(["Coincée"]);
  });
});

describe("InboxHome : « Voir tout » (001-B3)", () => {
  async function openAll() {
    fireEvent.click(await screen.findByRole("button", { name: "Voir tout" }));
    return screen.findByRole("heading", { name: "Toutes les captures" });
  }

  it("n'apparaît que s'il y a plus de captures que l'accueil n'en montre", async () => {
    for (let i = 0; i < HOME_LIMIT; i++) backend.seed(`C${i}`);
    render(<InboxHome />);
    await screen.findByText("C0");
    expect(screen.queryByRole("button", { name: "Voir tout" })).not.toBeInTheDocument();
  });

  it(`charge l'historique par lots de ${PAGE_SIZE}, plus anciennes au-dessus, sans doublon`, async () => {
    const total = 130;
    for (let i = 0; i < total; i++) backend.seed(`Capture ${String(i).padStart(3, "0")}`);
    render(<InboxHome />);
    await openAll();
    await waitFor(() => expect(cards()).toHaveLength(PAGE_SIZE));
    expect(texts()[0]).toBe("Capture 080");
    expect(texts().at(-1)).toBe("Capture 129");
    expect(screen.getByText(`${PAGE_SIZE} affichées sur ${total}`)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Charger les 50 plus anciennes \(80 restantes\)/ }));
    await waitFor(() => expect(cards()).toHaveLength(2 * PAGE_SIZE));
    expect(texts()[0]).toBe("Capture 030");
    expect(texts().at(-1)).toBe("Capture 129");

    fireEvent.click(screen.getByRole("button", { name: /Charger les 30 plus anciennes/ }));
    await waitFor(() => expect(cards()).toHaveLength(total));
    expect(texts()[0]).toBe("Capture 000");
    expect(new Set(texts()).size).toBe(total);
    expect(screen.queryByRole("button", { name: /plus anciennes/ })).not.toBeInTheDocument();
    // Jamais toute la base d'un coup : chaque appel est limité à un lot.
    const limits = tauri.invoke.mock.calls
      .filter(([command]) => command === "list_inbox_items")
      .map(([, args]) => (args as { limit: number }).limit);
    expect(Math.max(...limits)).toBeLessThanOrEqual(PAGE_SIZE);
  });

  it("conserve la position de défilement quand un lot plus ancien s'ajoute au-dessus", async () => {
    Object.defineProperty(HTMLElement.prototype, "scrollHeight", {
      configurable: true,
      get() {
        return this.classList.contains("paged__scroll") ? this.querySelectorAll("li").length * 100 : 0;
      },
    });
    try {
      for (let i = 0; i < 80; i++) backend.seed(`S${i}`);
      render(<InboxHome />);
      await openAll();
      await waitFor(() => expect(cards()).toHaveLength(PAGE_SIZE));
      const scroller = document.querySelector(".paged__scroll") as HTMLElement;
      expect(scroller.scrollTop).toBe(PAGE_SIZE * 100); // d'abord tout en bas (la plus récente)
      scroller.scrollTop = 300; // l'utilisateur a remonté
      fireEvent.click(screen.getByRole("button", { name: /plus anciennes/ }));
      await waitFor(() => expect(cards()).toHaveLength(80));
      expect(scroller.scrollTop).toBe(300 + 30 * 100); // les 30 ajoutées au-dessus n'ont rien déplacé
    } finally {
      delete (HTMLElement.prototype as { scrollHeight?: number }).scrollHeight;
    }
  });

  it("conserve le filtre et repart de zéro quand il change", async () => {
    for (let i = 0; i < 60; i++) backend.seed(`F${i}`, i % 2 === 0 ? "finances" : "moi");
    render(<InboxHome />);
    await openAll();
    await waitFor(() => expect(cards()).toHaveLength(PAGE_SIZE));
    fireEvent.click(screen.getByRole("button", { name: /plus anciennes/ })); // charge un lot de plus
    await waitFor(() => expect(cards()).toHaveLength(60));

    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "destination:finances" } });
    await waitFor(() => expect(cards()).toHaveLength(30));
    expect(texts().every((t) => Number(t!.slice(1)) % 2 === 0)).toBe(true);
    expect(screen.queryByRole("button", { name: /plus anciennes/ })).not.toBeInTheDocument();
    expect(texts().at(-1)).toBe("F58");
  });

  it("des captures ajoutées pendant la consultation ne créent ni doublon ni trou", async () => {
    for (let i = 0; i < 70; i++) backend.seed(`Ancienne ${String(i).padStart(2, "0")}`);
    render(<InboxHome />);
    await openAll();
    await waitFor(() => expect(cards()).toHaveLength(PAGE_SIZE));
    // Des captures arrivent (autre fenêtre, mobile futur…) avant le chargement du lot suivant.
    for (let i = 0; i < 5; i++) backend.seed(`Nouvelle ${i}`);
    fireEvent.click(screen.getByRole("button", { name: /plus anciennes/ }));
    await waitFor(() => expect(cards()).toHaveLength(70));
    const names = texts() as string[];
    expect(names).toEqual(Array.from({ length: 70 }, (_, i) => `Ancienne ${String(i).padStart(2, "0")}`));
    expect(names.some((n) => n.startsWith("Nouvelle"))).toBe(false);
  });

  it("une nouvelle capture est visible dans « Voir tout » après envoi", async () => {
    for (let i = 0; i < 25; i++) backend.seed(`V${i}`);
    render(<InboxHome />);
    await openAll();
    await waitFor(() => expect(cards()).toHaveLength(25));
    await capture("Dernière arrivée");
    await waitFor(() => expect(texts().at(-1)).toBe("Dernière arrivée"));
    expect(cards()).toHaveLength(26);
  });

  it("le retour ramène à l'accueil sans perdre le champ de capture", async () => {
    for (let i = 0; i < 25; i++) backend.seed(`R${i}`);
    render(<InboxHome />);
    await openAll();
    fireEvent.change(field(), { target: { value: "Brouillon de capture" } });
    fireEvent.click(screen.getByRole("button", { name: "← Retour" }));
    expect(await screen.findByRole("heading", { name: "À organiser" })).toBeInTheDocument();
    expect(field()).toHaveValue("Brouillon de capture");
  });
});
