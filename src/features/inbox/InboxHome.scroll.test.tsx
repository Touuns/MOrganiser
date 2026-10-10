import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeBackend, type FakeBackend } from "../../test/fakeBackend";
import { InboxHome } from "./InboxHome";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>(),
  isTauri: vi.fn(() => true),
}));
vi.mock("@tauri-apps/api/core", () => tauri);

// --- Mise en page simulée : liste défilante de cartes de 70 px espacées de 80 px ---
const LIST_TOP = 100;
const LIST_HEIGHT = 380;
const STEP = 80;
const scrollTops = new WeakMap<Element, number>();

const box = (left: number, top: number, width: number, height: number) =>
  ({ left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) }) as DOMRect;
const isList = (el: Element) => el.classList.contains("inbox__list");
const count = (el: Element) => el.querySelectorAll("[data-capture-id]").length;
const maxScroll = (el: Element) => Math.max(0, count(el) * STEP - LIST_HEIGHT);

const originalRect = HTMLElement.prototype.getBoundingClientRect;
const originalScrollTop = Object.getOwnPropertyDescriptor(Element.prototype, "scrollTop")!;
const originalScrollHeight = Object.getOwnPropertyDescriptor(Element.prototype, "scrollHeight")!;
const originalFocus = HTMLElement.prototype.focus;

beforeEach(() => {
  HTMLElement.prototype.getBoundingClientRect = function (this: HTMLElement) {
    if (isList(this)) return box(20, LIST_TOP, 600, LIST_HEIGHT);
    if (this.hasAttribute("data-capture-id")) {
      const list = this.closest(".inbox__list");
      if (!list) return box(0, 0, 0, 0);
      const index = [...list.querySelectorAll("[data-capture-id]")].indexOf(this);
      return box(30, LIST_TOP + index * STEP - (scrollTops.get(list) ?? 0), 580, 70);
    }
    return box(0, 0, 0, 0);
  };
  Object.defineProperty(Element.prototype, "scrollTop", {
    configurable: true,
    get(this: Element) {
      return isList(this) ? (scrollTops.get(this) ?? 0) : originalScrollTop.get!.call(this);
    },
    set(this: Element, value: number) {
      if (isList(this)) scrollTops.set(this, Math.min(Math.max(0, value), maxScroll(this)));
      else originalScrollTop.set!.call(this, value);
    },
  });
  Object.defineProperty(Element.prototype, "scrollHeight", {
    configurable: true,
    get(this: Element) {
      return isList(this) ? count(this) * STEP : originalScrollHeight.get!.call(this);
    },
  });
});

afterEach(() => {
  HTMLElement.prototype.getBoundingClientRect = originalRect;
  Object.defineProperty(Element.prototype, "scrollTop", originalScrollTop);
  Object.defineProperty(Element.prototype, "scrollHeight", originalScrollHeight);
  HTMLElement.prototype.focus = originalFocus;
});

let backend: FakeBackend;
beforeEach(() => {
  backend = createFakeBackend();
  tauri.invoke.mockReset();
  tauri.invoke.mockImplementation((command, args) => backend.invoke(command, args));
});

const name = (i: number) => `Item ${String(i).padStart(2, "0")}`;
const list = () => screen.getByRole("list");
const listEl = () => document.querySelector(".inbox__list") as HTMLElement;
const detail = () => screen.getByRole("region", { name: /Fiche de la capture/ });
const field = () => screen.getByLabelText("Capture rapide", { selector: "textarea" });
const cardTop = (text: string) =>
  (within(list()).getByText(text).closest("li") as HTMLElement).getBoundingClientRect().top;
const openCard = async (text: string) =>
  fireEvent.click(await screen.findByRole("button", { name: new RegExp(text) }));
const scrollTo = (value: number) => {
  listEl().scrollTop = value;
  fireEvent.scroll(listEl());
};

/** 30 captures : la boîte en affiche les 20 plus récentes (Item 10 à Item 29). */
async function setup() {
  for (let i = 0; i < 30; i++) backend.seed(name(i));
  render(<InboxHome />);
  await screen.findByText(name(29));
  await waitFor(() => expect(listEl().scrollTop).toBe(maxScroll(listEl()))); // premier affichage : en bas
}

describe("Conservation de la zone de lecture (001-C)", () => {
  it("mise à la corbeille : la liste reste sur les captures consultées, sans saut", async () => {
    await setup();
    scrollTo(400); // on remonte vers des captures plus anciennes
    expect(cardTop(name(15))).toBe(LIST_TOP); // première carte visible

    await openCard(name(18));
    expect(listEl().scrollTop).toBe(400); // ouvrir une fiche ne défile pas
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));

    // Une capture plus ancienne (Item 09) entre dans l'ensemble des 20 plus récentes.
    await waitFor(() => expect(within(list()).queryByText(name(18))).not.toBeInTheDocument());
    await screen.findByText(name(9));
    expect(listEl().scrollTop).toBe(480); // +80 px : compensation exacte de l'entrée
    expect(listEl().scrollTop).not.toBe(maxScroll(listEl())); // jamais ramenée en bas
    expect(cardTop(name(15))).toBe(LIST_TOP); // la carte de lecture n'a pas bougé
  });

  it("l'ancre supprimée est remplacée par une voisine appropriée", async () => {
    await setup();
    scrollTo(400);
    await openCard(name(15)); // la première carte visible sert d'ancre : on la supprime
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await waitFor(() => expect(within(list()).queryByText(name(15))).not.toBeInTheDocument());
    await screen.findByText(name(9));
    expect(listEl().scrollTop).not.toBe(maxScroll(listEl()));
    expect(cardTop(name(16))).toBe(LIST_TOP + STEP); // la voisine reste là où elle était
  });

  it("ouverture, fermeture et édition d'une fiche : aucun défilement", async () => {
    await setup();
    scrollTo(400);
    await openCard(name(17));
    expect(listEl().scrollTop).toBe(400);
    fireEvent.change(within(detail()).getByLabelText("Texte"), { target: { value: "Item 17 modifiée" } });
    fireEvent.click(within(detail()).getByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(within(list()).getByText("Item 17 modifiée")).toBeInTheDocument());
    expect(listEl().scrollTop).toBe(400);
    expect(cardTop("Item 17 modifiée")).toBe(LIST_TOP + 7 * STEP - 400);
    fireEvent.click(within(detail()).getByRole("button", { name: "Fermer la fiche" }));
    expect(listEl().scrollTop).toBe(400);
  });

  it("restauration (Annuler) : la liste n'est pas ramenée en bas", async () => {
    await setup();
    scrollTo(400);
    await openCard(name(18));
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await screen.findByText(name(9));
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    await waitFor(() => expect(within(list()).getByText(name(18))).toBeInTheDocument());
    expect(listEl().scrollTop).not.toBe(maxScroll(listEl()));
    expect(cardTop(name(15))).toBe(LIST_TOP);
  });

  it("nouvelle capture : le défilement automatique vers le bas est conservé", async () => {
    await setup();
    scrollTo(400);
    fireEvent.change(field(), { target: { value: "Nouvelle arrivée" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    await screen.findByText("Nouvelle arrivée");
    await waitFor(() => expect(listEl().scrollTop).toBe(maxScroll(listEl())));
  });

  it("changement de filtre : la liste repart du bas", async () => {
    backend.seed("Avec destination", "finances");
    for (let i = 0; i < 29; i++) backend.seed(name(i));
    render(<InboxHome />);
    await screen.findByText(name(28));
    scrollTo(400);
    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "unclassified" } });
    await waitFor(() => expect(listEl().scrollTop).toBe(maxScroll(listEl())));
  });

  it("le focus qui suit une suppression ne fait pas défiler la liste", async () => {
    await setup();
    scrollTo(400);
    const focusCalls: (FocusOptions | undefined)[] = [];
    HTMLElement.prototype.focus = function (this: HTMLElement, options?: FocusOptions) {
      focusCalls.push(options);
      return originalFocus.call(this, options);
    };
    await openCard(name(18));
    focusCalls.length = 0;
    fireEvent.click(within(detail()).getByRole("button", { name: "Mettre à la corbeille" }));
    await screen.findByText(name(9));
    expect(focusCalls.length).toBeGreaterThan(0);
    expect(focusCalls.every((options) => options?.preventScroll === true)).toBe(true);
    expect(listEl().scrollTop).toBe(480);
  });
});

describe("Disposition : la fiche est un panneau flottant indépendant", () => {
  it("ouvrir ou fermer la fiche ne remonte ni ne recrée la boîte et la capture rapide", async () => {
    backend.seed("Alpha");
    render(<InboxHome />);
    await screen.findByText("Alpha");
    const box = document.querySelector(".inbox") as HTMLElement;
    const capture = document.querySelector(".capture") as HTMLElement;
    const home = document.querySelector(".inbox-home") as HTMLElement;
    const homeBefore = home.getAttribute("style");

    await openCard("Alpha");
    const panel = document.querySelector(".inbox-home__detail") as HTMLElement;
    // Le panneau est dans la colonne principale (placé en absolu), pas une colonne de la grille :
    // aucun changement de largeur ou de position de la boîte n'est possible.
    const stage = document.querySelector(".inbox-home__stage") as HTMLElement;
    expect(panel.parentElement).toBe(stage);
    expect(stage.parentElement).toBe(document.querySelector(".inbox-home__main"));
    // La scène ne contient pas la capture rapide : le panneau ne peut pas la recouvrir.
    expect(stage.contains(document.querySelector(".capture"))).toBe(false);
    expect(document.querySelector(".inbox")).toBe(box);
    expect(document.querySelector(".capture")).toBe(capture);
    expect(home.getAttribute("style")).toBe(homeBefore);

    fireEvent.click(within(detail()).getByRole("button", { name: "Fermer la fiche" }));
    expect(document.querySelector(".inbox")).toBe(box);
    expect(document.querySelector(".capture")).toBe(capture);
    expect(document.querySelector(".inbox-home__detail")).toBeNull();
  });
});
