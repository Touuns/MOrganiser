import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeBackend, type FakeBackend } from "../../test/fakeBackend";
import { DEPARTURE_DURATION_MS } from "./arrivalAnimation";
import { InboxHome } from "./InboxHome";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>(),
  isTauri: vi.fn(() => true),
}));
vi.mock("@tauri-apps/api/core", () => tauri);

// jsdom n'a ni API d'animation ni mise en page : des simulations en tiennent lieu.
interface FakeAnimation {
  el: Element;
  keyframes: Keyframe[];
  options: KeyframeAnimationOptions;
  cancelled: boolean;
  finished: Promise<void>;
  finish: () => void;
  cancel: () => void;
}
let animations: FakeAnimation[] = [];
let reducedMotion = false;
let narrowWithSheet = false;

const zero = () => ({ left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0, x: 0, y: 0, toJSON: () => ({}) }) as DOMRect;
const box = (left: number, top: number, width: number, height: number) =>
  ({ left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) }) as DOMRect;

const originalRect = HTMLElement.prototype.getBoundingClientRect;
const originalAnimate = (Element.prototype as { animate?: unknown }).animate;
const originalMatchMedia = window.matchMedia;

beforeEach(() => {
  animations = [];
  reducedMotion = false;
  narrowWithSheet = false;
  HTMLElement.prototype.getBoundingClientRect = function (this: HTMLElement) {
    if (this.classList.contains("capture__field")) return box(20, 520, 600, 60);
    if (this.classList.contains("inbox__list") || this.classList.contains("paged__scroll")) {
      return box(20, 100, 600, 380);
    }
    if (this.hasAttribute("data-capture-id")) {
      // Fenêtre étroite : tant que la fiche est ouverte, la liste est masquée (dimensions nulles).
      if (narrowWithSheet && document.querySelector(".detail")) return zero();
      const list = this.closest(".inbox__list, .paged__list");
      if (!list) return zero();
      const cards = [...list.querySelectorAll("[data-capture-id]")];
      const fromBottom = cards.length - 1 - cards.indexOf(this);
      return box(30, 400 - fromBottom * 80, 580, 70); // la plus récente est la plus basse
    }
    return zero();
  };
  (Element.prototype as { animate: unknown }).animate = function (
    this: Element,
    keyframes: Keyframe[],
    options: KeyframeAnimationOptions,
  ) {
    let resolve!: () => void;
    const finished = new Promise<void>((r) => (resolve = r));
    const animation: FakeAnimation = {
      el: this,
      keyframes,
      options,
      cancelled: false,
      finished,
      finish: () => resolve(),
      cancel() {
        this.cancelled = true;
        resolve();
      },
    };
    animations.push(animation);
    return animation as unknown as Animation;
  };
  window.matchMedia = ((query: string) => ({
    matches: reducedMotion && query.includes("reduce"),
    media: query,
    addEventListener() {},
    removeEventListener() {},
  })) as unknown as typeof window.matchMedia;
});

afterEach(() => {
  HTMLElement.prototype.getBoundingClientRect = originalRect;
  (Element.prototype as { animate?: unknown }).animate = originalAnimate;
  window.matchMedia = originalMatchMedia;
});

let backend: FakeBackend;
beforeEach(() => {
  backend = createFakeBackend();
  tauri.invoke.mockReset();
  tauri.invoke.mockImplementation((command, args) => backend.invoke(command, args));
});

const field = () => screen.getByLabelText("Capture rapide", { selector: "textarea" });
const ghosts = () => [...document.querySelectorAll<HTMLElement>(".departure-ghost")];
const detail = () => screen.getByRole("region", { name: /Fiche de la capture/ });
// La boîte vide n'a plus de liste : on lit alors « aucune carte » plutôt que de lever une erreur.
const listCards = () => {
  const list = screen.queryByRole("list");
  return list ? within(list).queryAllByRole("listitem") : [];
};
const cardTexts = () => listCards().map((li) => li.querySelector(".inbox__content")?.textContent);
const openCard = async (text: RegExp | string) =>
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(text) }));
const toastText = () => screen.queryByText("Déplacée dans la corbeille");
const finishAll = () => animations.forEach((a) => a.finish());

async function trashFromSheet(text: string) {
  await openCard(text);
  fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
}

describe("Disparition d'une capture mise à la corbeille (001-C)", () => {
  it("après confirmation de la base : la carte disparaît en fondu, les autres reprennent leur place", async () => {
    const alpha = backend.seed("Alpha");
    const bravo = backend.seed("Bravo");
    backend.seed("Charlie");
    render(<InboxHome />);
    await trashFromSheet("Bravo");
    await waitFor(() => expect(ghosts()).toHaveLength(1));

    // La suppression logique n'a pas attendu l'animation : elle est déjà faite en base.
    expect(backend.find(bravo.id)!.deletedAt).not.toBeNull();
    const ghost = ghosts()[0];
    expect(ghost.parentElement).toBe(document.body);
    expect(ghost.getAttribute("aria-hidden")).toBe("true");
    expect(ghost.hasAttribute("inert")).toBe(true);
    expect(ghost.hasAttribute("data-capture-id")).toBe(false);
    expect(ghost.textContent).toContain("Bravo");

    const fade = animations.find((a) => a.el === ghost)!;
    expect(fade.options.duration).toBe(DEPARTURE_DURATION_MS);
    expect(DEPARTURE_DURATION_MS).toBeGreaterThanOrEqual(200);
    expect(DEPARTURE_DURATION_MS).toBeLessThanOrEqual(250);
    expect(fade.keyframes[0].opacity).toBe(1);
    expect(fade.keyframes[1].opacity).toBe(0);

    // Liste relue : Bravo n'y est plus, et Alpha glisse (240 -> 320, soit 80 px) au lieu de sauter.
    await waitFor(() => expect(cardTexts()).toEqual(["Alpha", "Charlie"]));
    const alphaCard = within(screen.getByRole("list")).getByText("Alpha").closest("li")!;
    const shift = animations.find((a) => a.el === alphaCard);
    expect(shift?.keyframes[0].transform).toBe("translateY(-80px)");
    expect(shift?.keyframes[1].transform).toBe("none");
    expect(backend.find(alpha.id)!.deletedAt).toBeNull(); // les autres captures sont intactes

    finishAll();
    await waitFor(() => expect(ghosts()).toHaveLength(0));
    expect(cardTexts()).toEqual(["Alpha", "Charlie"]); // jamais redevenue active
    expect(backend.find(bravo.id)!.deletedAt).not.toBeNull();
  });

  it("la notification est flottante, visible tout de suite, et annonce l'action", async () => {
    backend.seed("Alpha");
    render(<InboxHome />);
    const region = document.querySelector(".toast-region") as HTMLElement;
    expect(region).not.toBeNull(); // région vivante déjà présente avant l'action
    expect(region.getAttribute("aria-live")).toBe("polite");
    expect(region.closest(".inbox-home__main")).toBeNull(); // hors du flux : rien ne se déplace

    await trashFromSheet("Alpha");
    const toast = await screen.findByText("Déplacée dans la corbeille");
    expect(region.contains(toast)).toBe(true);
    expect(screen.getByRole("button", { name: "Annuler" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: /Fiche/ })).not.toBeInTheDocument(); // fiche fermée
  });

  it("la saisie n'est pas bloquée pendant la notification et la transition", async () => {
    backend.seed("Alpha");
    render(<InboxHome />);
    await trashFromSheet("Alpha");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    fireEvent.change(field(), { target: { value: "Nouvelle idée" } });
    expect(field()).toHaveValue("Nouvelle idée");
  });

  it("échec de la suppression : ni animation ni notification trompeuse", async () => {
    const item = backend.seed("Alpha");
    backend.state.failures.set("trash_inbox_item", { code: "storage", message: "L'enregistrement local a échoué." });
    render(<InboxHome />);
    await trashFromSheet("Alpha");
    expect(await within(detail()).findByText(/n'a pas été déplacée/)).toBeInTheDocument();
    expect(animations).toHaveLength(0);
    expect(ghosts()).toHaveLength(0);
    expect(toastText()).not.toBeInTheDocument();
    expect(cardTexts()).toEqual(["Alpha"]);
    expect(backend.find(item.id)!.deletedAt).toBeNull();
  });

  it("mouvement réduit : disparition immédiate, notification affichée", async () => {
    reducedMotion = true;
    backend.seed("Alpha");
    render(<InboxHome />);
    await trashFromSheet("Alpha");
    expect(await screen.findByText("Déplacée dans la corbeille")).toBeInTheDocument();
    await waitFor(() => expect(cardTexts()).toEqual([]));
    expect(animations).toHaveLength(0);
    expect(ghosts()).toHaveLength(0);
  });

  it("annulation pendant la transition : carte restaurée visible, aucun fantôme persistant", async () => {
    const bravo = backend.seed("Bravo");
    backend.seed("Charlie");
    render(<InboxHome />);
    await trashFromSheet("Bravo");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    await waitFor(() => expect(cardTexts()).toEqual(["Charlie"]));

    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(ghosts()).toHaveLength(0); // retiré tout de suite
    await waitFor(() => expect(cardTexts()).toEqual(["Bravo", "Charlie"]));
    const restored = within(screen.getByRole("list")).getByText("Bravo").closest("li") as HTMLElement;
    expect(restored.style.visibility).toBe("");
    expect(backend.find(bravo.id)!.deletedAt).toBeNull();
    finishAll();
    expect(ghosts()).toHaveLength(0);
    expect(screen.getByRole("status")).toHaveTextContent("Capture restaurée.");
  });

  it("deux suppressions rapprochées : l'annulation vise la dernière capture seulement", async () => {
    const alpha = backend.seed("Alpha");
    const bravo = backend.seed("Bravo");
    render(<InboxHome />);
    await trashFromSheet("Alpha");
    await waitFor(() => expect(cardTexts()).toEqual(["Bravo"]));
    await trashFromSheet("Bravo");
    await waitFor(() => expect(cardTexts()).toEqual([]));

    expect(screen.getAllByText("Déplacée dans la corbeille")).toHaveLength(1);
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await waitFor(() => expect(cardTexts()).toEqual(["Bravo"]));
    expect(backend.find(bravo.id)!.deletedAt).toBeNull();
    expect(backend.find(alpha.id)!.deletedAt).not.toBeNull(); // la première reste à la corbeille
    finishAll();
    expect(ghosts()).toHaveLength(0);
  });

  it("depuis « Voir tout » : la carte disparaît aussi en fondu", async () => {
    for (let i = 0; i < 25; i++) backend.seed(`Capture ${String(i).padStart(2, "0")}`);
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: "Voir tout" }));
    await screen.findByRole("heading", { name: "Toutes les captures" });
    await openCard("Capture 24");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));

    await waitFor(() => expect(ghosts()).toHaveLength(1));
    expect(ghosts()[0].textContent).toContain("Capture 24");
    await waitFor(() =>
      expect(document.querySelector(".paged__list")!.textContent).not.toContain("Capture 24"),
    );
    expect(screen.getByText("Déplacée dans la corbeille")).toBeInTheDocument();
    finishAll();
  });

  it("capture absente de la liste actuelle : confirmation seule, sans trajet inventé", async () => {
    backend.seed("Alpha", "finances");
    backend.seed("Bravo");
    render(<InboxHome />);
    await openCard("Alpha");
    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "unclassified" } }); // exclut Alpha
    await waitFor(() => expect(cardTexts()).toEqual(["Bravo"]));
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));

    expect(await screen.findByText("Déplacée dans la corbeille")).toBeInTheDocument();
    expect(animations).toHaveLength(0);
    expect(ghosts()).toHaveLength(0);
    expect(cardTexts()).toEqual(["Bravo"]);
  });

  it("redimensionnement pendant la transition : annulée, la capture reste à la corbeille", async () => {
    const bravo = backend.seed("Bravo");
    backend.seed("Charlie");
    render(<InboxHome />);
    await trashFromSheet("Bravo");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    window.dispatchEvent(new Event("resize"));
    expect(ghosts()).toHaveLength(0);
    await waitFor(() => expect(cardTexts()).toEqual(["Charlie"]));
    expect(backend.find(bravo.id)!.deletedAt).not.toBeNull();
  });

  it("changement de vue pendant la transition : fantôme retiré", async () => {
    backend.seed("Bravo");
    render(<InboxHome />);
    await trashFromSheet("Bravo");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    fireEvent.click(screen.getByRole("button", { name: "Corbeille" }));
    await waitFor(() => expect(ghosts()).toHaveLength(0));
    expect(await screen.findByText("Bravo")).toBeInTheDocument(); // elle est bien dans la corbeille
  });

  it("le focus passe à la carte voisine, ou au champ de capture s'il n'y en a plus", async () => {
    backend.seed("Alpha");
    backend.seed("Bravo");
    backend.seed("Charlie");
    render(<InboxHome />);
    await trashFromSheet("Bravo");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    expect(document.activeElement?.textContent).toContain("Charlie"); // voisine suivante

    await trashFromSheet("Alpha");
    await waitFor(() => expect(cardTexts()).toEqual(["Charlie"]));
    await trashFromSheet("Charlie");
    await waitFor(() => expect(cardTexts()).toEqual([]));
    expect(field()).toHaveFocus();
  });

  it("fenêtre étroite : l'animation a lieu une fois la fiche refermée et la liste de nouveau visible", async () => {
    narrowWithSheet = true;
    backend.seed("Alpha");
    backend.seed("Bravo");
    render(<InboxHome />);
    await trashFromSheet("Bravo");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    expect(ghosts()[0].textContent).toContain("Bravo");
    await act(async () => finishAll());
    await waitFor(() => expect(ghosts()).toHaveLength(0));
  });
});

describe("Carte masquée par la transition : jamais durablement invisible", () => {
  /** Retarde la réponse d'une commande jusqu'à `release()`. */
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

  const pagedCard = (text: string) =>
    within(document.querySelector(".paged__list") as HTMLElement).getByText(text).closest("li") as HTMLElement;

  async function openAllAndPick(text: string) {
    for (let i = 0; i < 25; i++) backend.seed(`Capture ${String(i).padStart(2, "0")}`);
    render(<InboxHome />);
    fireEvent.click(await screen.findByRole("button", { name: "Voir tout" }));
    await screen.findByRole("heading", { name: "Toutes les captures" });
    await openCard(text);
  }

  it("relecture en échec après la suppression : l'ancienne carte n'est pas laissée invisible", async () => {
    await openAllAndPick("Capture 24");
    // La suppression réussit, mais la relecture de la liste échoue.
    backend.state.failures.set("list_inbox_items", { code: "storage", message: "Lecture impossible." });
    const card = pagedCard("Capture 24");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    expect(card.style.visibility).toBe("hidden"); // masquée pendant le trajet

    finishAll(); // l'animation se termine alors que la liste n'a pas pu être relue
    await waitFor(() => expect(ghosts()).toHaveLength(0));
    expect(await screen.findByText("Lecture impossible.")).toBeInTheDocument();
    expect(card.isConnected).toBe(true); // toujours dans le DOM (liste périmée)
    expect(card.style.visibility).toBe(""); // aucun style laissé par l'animation
    expect(card.hasAttribute("data-departed")).toBe(true); // marquée comme sortie, non interactive
    expect(card.hasAttribute("inert")).toBe(true);
  });

  it("la relecture réussie ensuite retire la carte ; aucune trace ne subsiste", async () => {
    await openAllAndPick("Capture 24");
    backend.state.failures.set("list_inbox_items", { code: "storage", message: "Lecture impossible." });
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    finishAll();
    await screen.findByText("Lecture impossible.");

    backend.state.failures.clear();
    fireEvent.click(screen.getByRole("button", { name: "Réessayer" }));
    await waitFor(() =>
      expect(document.querySelector(".paged__list")!.textContent).not.toContain("Capture 24"),
    );
    expect(ghosts()).toHaveLength(0);
  });

  it("redimensionnement avant la fin de la relecture : carte visible puis retirée à la relecture", async () => {
    await openAllAndPick("Capture 24");
    const releaseList = hold("list_inbox_items"); // la relecture tarde
    const card = pagedCard("Capture 24");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    expect(card.style.visibility).toBe("hidden");

    window.dispatchEvent(new Event("resize")); // annule la transition avant la relecture
    expect(ghosts()).toHaveLength(0);
    expect(card.isConnected).toBe(true);
    expect(card.style.visibility).toBe(""); // pas durablement invisible

    await releaseList();
    await waitFor(() =>
      expect(document.querySelector(".paged__list")!.textContent).not.toContain("Capture 24"),
    );
  });

  it("« Annuler » après une transition interrompue : la carte redevient normale et interactive", async () => {
    await openAllAndPick("Capture 24");
    backend.state.failures.set("list_inbox_items", { code: "storage", message: "Lecture impossible." });
    const card = pagedCard("Capture 24");
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    finishAll();
    await screen.findByText("Lecture impossible.");
    expect(card.hasAttribute("data-departed")).toBe(true);

    backend.state.failures.clear();
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await waitFor(() => expect(screen.getByRole("status")).toHaveTextContent("Capture restaurée."));
    await waitFor(() => expect(pagedCard("Capture 24")).toBeInTheDocument());
    const restored = pagedCard("Capture 24");
    expect(restored.style.visibility).toBe("");
    expect(restored.hasAttribute("data-departed")).toBe(false);
    expect(restored.hasAttribute("inert")).toBe(false);
  });
});
