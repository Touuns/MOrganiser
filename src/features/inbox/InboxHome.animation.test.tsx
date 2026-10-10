import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeBackend, type FakeBackend } from "../../test/fakeBackend";
import { ARRIVAL_DURATION_MS } from "./arrivalAnimation";
import { InboxHome } from "./InboxHome";

const tauri = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>(),
  isTauri: vi.fn(() => true),
}));
vi.mock("@tauri-apps/api/core", () => tauri);

// --- Fausse API d'animation et fausse mise en page (jsdom n'a ni l'une ni l'autre) ---
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
let layout = { narrow: false };
let reducedMotion = false;
let animateMode: "normal" | "throws" | "never" = "normal";

const ZERO = { left: 0, top: 0, width: 0, height: 0, right: 0, bottom: 0, x: 0, y: 0 };
const box = (left: number, top: number, width: number, height: number) =>
  ({ left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) }) as DOMRect;

const FIELD_TOP = 520;
const LAST_CARD_TOP = 400;
const CARD_STEP = 80;

const originalRect = HTMLElement.prototype.getBoundingClientRect;
const originalAnimate = (Element.prototype as { animate?: unknown }).animate;
const originalMatchMedia = window.matchMedia;

beforeEach(() => {
  animations = [];
  layout = { narrow: false };
  reducedMotion = false;
  animateMode = "normal";

  HTMLElement.prototype.getBoundingClientRect = function (this: HTMLElement) {
    if (layout.narrow) return box(0, 0, 0, 0);
    if (this.classList.contains("capture__field")) return box(20, FIELD_TOP, 600, 60);
    if (this.classList.contains("inbox__list")) return box(20, 100, 600, 380);
    if (this.hasAttribute("data-capture-id")) {
      const list = this.closest(".inbox__list");
      if (!list) return box(0, 0, 0, 0);
      const cards = [...list.querySelectorAll("[data-capture-id]")];
      const fromBottom = cards.length - 1 - cards.indexOf(this);
      return box(30, LAST_CARD_TOP - fromBottom * CARD_STEP, 580, 70);
    }
    return { ...ZERO, toJSON: () => ({}) } as DOMRect;
  };
  (Element.prototype as { animate: unknown }).animate = function (
    this: Element,
    keyframes: Keyframe[],
    options: KeyframeAnimationOptions,
  ) {
    if (animateMode === "throws" && this.classList.contains("arrival-ghost")) {
      throw new Error("animation indisponible");
    }
    let resolve!: () => void;
    const finished = new Promise<void>((r) => (resolve = r));
    const animation: FakeAnimation = {
      el: this,
      keyframes,
      options,
      cancelled: false,
      finished: animateMode === "never" ? new Promise<void>(() => {}) : finished,
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
const ghosts = () => [...document.querySelectorAll<HTMLElement>(".arrival-ghost")];
const realCard = (text: string) =>
  within(screen.getByRole("list"))
    .getByText(text)
    .closest("li") as HTMLElement;
const ghostAnimations = () => animations.filter((a) => a.el.classList.contains("arrival-ghost"));
const finishAll = () => animations.forEach((a) => a.finish());

async function send(text: string, destination?: string) {
  fireEvent.change(field(), { target: { value: text } });
  if (destination) {
    fireEvent.change(screen.getByLabelText("Destination (facultatif)"), { target: { value: destination } });
  }
  fireEvent.keyDown(field(), { key: "Enter" });
  await waitFor(() => expect(field()).toHaveValue(""));
}

async function ready() {
  render(<InboxHome />);
  await screen.findByRole("heading", { name: "À organiser" });
}

describe("Animation d'arrivée d'une capture (001-C)", () => {
  it("après enregistrement : une carte fantôme monte de la zone de saisie vers la vraie carte", async () => {
    await ready();
    await send("Vendre ma PlayStation 5");
    await waitFor(() => expect(ghosts()).toHaveLength(1));

    // L'enregistrement n'a pas attendu l'animation : la capture est en base, la vraie carte est dans la liste.
    expect(backend.state.items.map((i) => i.content)).toEqual(["Vendre ma PlayStation 5"]);
    const card = realCard("Vendre ma PlayStation 5");
    expect(card.style.visibility).toBe("hidden"); // masquée seulement pendant le trajet

    const ghost = ghosts()[0];
    expect(ghost.parentElement).toBe(document.body);
    expect(ghost.getAttribute("aria-hidden")).toBe("true");
    expect(ghost.hasAttribute("inert")).toBe(true);
    expect(ghost.hasAttribute("data-capture-id")).toBe(false);
    expect(ghost.textContent).toContain("Vendre ma PlayStation 5");

    const [anim] = ghostAnimations();
    expect(anim.options.duration).toBe(ARRIVAL_DURATION_MS);
    expect(ARRIVAL_DURATION_MS).toBe(350);
    // Départ : bord supérieur de la zone de saisie ; arrivée : la place de la vraie carte (en bas).
    expect(anim.keyframes[0].transform).toBe(`translateY(${FIELD_TOP - LAST_CARD_TOP}px)`);
    expect(anim.keyframes[1].transform).toBe("translateY(0)");
    expect(FIELD_TOP - LAST_CARD_TOP).toBeGreaterThan(0); // le trajet monte

    finishAll();
    await waitFor(() => expect(ghosts()).toHaveLength(0));
    expect(card.style.visibility).toBe("");
    expect(screen.getByRole("status")).toHaveTextContent("Capture enregistrée.");
  });

  it("le champ est utilisable immédiatement, pendant le trajet", async () => {
    await ready();
    await send("Première");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    expect(field()).toHaveFocus();
    fireEvent.change(field(), { target: { value: "Seconde, pendant le mouvement" } });
    expect(field()).toHaveValue("Seconde, pendant le mouvement");
  });

  it("échec d'enregistrement : aucune animation, texte conservé", async () => {
    backend.state.failures.set("create_inbox_item", { code: "storage", message: "L'enregistrement local a échoué." });
    await ready();
    fireEvent.change(field(), { target: { value: "À ne pas perdre" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("L'enregistrement local a échoué.");
    expect(animations).toHaveLength(0);
    expect(ghosts()).toHaveLength(0);
    expect(field()).toHaveValue("À ne pas perdre");
  });

  it("deux envois successifs : aucune perte, aucun doublon, aucune carte restée masquée", async () => {
    await ready();
    await send("Première");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    await send("Seconde");
    await waitFor(() => expect(ghostAnimations().length).toBeGreaterThanOrEqual(2));

    // La première arrivée est terminée net : un seul fantôme reste, et sa carte est visible.
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    expect(ghosts()[0].textContent).toContain("Seconde");
    expect(realCard("Première").style.visibility).toBe("");
    expect(realCard("Seconde").style.visibility).toBe("hidden");

    finishAll();
    await waitFor(() => expect(ghosts()).toHaveLength(0));
    expect(realCard("Seconde").style.visibility).toBe("");
    expect(backend.state.items.map((i) => i.content)).toEqual(["Première", "Seconde"]);
    expect(within(screen.getByRole("list")).getAllByRole("listitem")).toHaveLength(2);
  });

  it("les cartes déjà présentes glissent vers leur nouvelle place au lieu de sauter", async () => {
    backend.seed("Ancienne 1");
    backend.seed("Ancienne 2");
    backend.seed("Ancienne 3");
    await ready();
    await screen.findByText("Ancienne 3");
    await send("Nouvelle");
    await waitFor(() => expect(ghosts()).toHaveLength(1));

    const shifted = animations.filter((a) => !a.el.classList.contains("arrival-ghost"));
    expect(shifted).toHaveLength(3);
    for (const animation of shifted) {
      expect(animation.keyframes[0].transform).toBe(`translateY(${CARD_STEP}px)`); // 80 px plus bas avant
      expect(animation.keyframes[1].transform).toBe("none");
    }
    finishAll();
  });

  it("filtre excluant la capture : aucune animation, notification conservée", async () => {
    await ready();
    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "destination:finances" } });
    await screen.findByText(/Aucune capture pour ce filtre/);
    await send("Sans destination");
    expect(screen.getByRole("status")).toHaveTextContent("masquée par le filtre");
    expect(animations).toHaveLength(0);
    expect(ghosts()).toHaveLength(0);
    expect(backend.state.items).toHaveLength(1);
  });

  it("mouvement réduit : aucune trajectoire, insertion immédiate", async () => {
    reducedMotion = true;
    await ready();
    await send("Sans mouvement");
    await screen.findByText("Sans mouvement");
    expect(animations).toHaveLength(0);
    expect(ghosts()).toHaveLength(0);
    expect(realCard("Sans mouvement").style.visibility).toBe("");
  });

  it("liste non visible (fenêtre étroite) : insertion directe sans mouvement", async () => {
    layout.narrow = true;
    await ready();
    await send("Fenêtre étroite");
    await screen.findByText("Fenêtre étroite");
    expect(animations).toHaveLength(0);
    expect(ghosts()).toHaveLength(0);
    expect(realCard("Fenêtre étroite").style.visibility).toBe("");
  });

  it("changement de vue pendant le trajet : fantôme retiré, vraie carte visible", async () => {
    await ready();
    await send("En vol");
    await waitFor(() => expect(ghosts()).toHaveLength(1));

    fireEvent.click(screen.getByRole("button", { name: "Corbeille" })); // la boîte est démontée
    await waitFor(() => expect(ghosts()).toHaveLength(0));
    fireEvent.click(screen.getByRole("button", { name: "← Retour" }));
    await screen.findByText("En vol");
    expect(realCard("En vol").style.visibility).toBe("");
    expect(backend.state.items).toHaveLength(1);
  });

  it("redimensionnement pendant le trajet : animation annulée, vraie carte visible", async () => {
    await ready();
    await send("Redimensionnée");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    const card = realCard("Redimensionnée");
    expect(card.style.visibility).toBe("hidden");

    window.dispatchEvent(new Event("resize"));
    expect(ghosts()).toHaveLength(0);
    expect(card.style.visibility).toBe("");
    expect(ghostAnimations()[0].cancelled).toBe(true);
  });

  it("changement de filtre pendant le trajet : animation annulée, rien ne reste masqué", async () => {
    await ready();
    await send("Filtrée");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "unclassified" } });
    await waitFor(() => expect(ghosts()).toHaveLength(0));
    await screen.findByText("Filtrée");
    expect(realCard("Filtrée").style.visibility).toBe("");
  });

  it("l'interface n'est jamais bloquée : on peut ouvrir une autre capture pendant le trajet", async () => {
    backend.seed("Déjà là");
    await ready();
    await screen.findByText("Déjà là");
    await send("Nouvelle");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    fireEvent.click(screen.getByRole("button", { name: /Déjà là/ }));
    expect(await screen.findByRole("region", { name: "Fiche de la capture" })).toBeInTheDocument();
  });

  it("si l'animation échoue, la capture reste visible et enregistrée", async () => {
    animateMode = "throws";
    await ready();
    await send("Robuste");
    await screen.findByText("Robuste");
    expect(ghosts()).toHaveLength(0);
    expect(realCard("Robuste").style.visibility).toBe("");
    expect(backend.state.items).toHaveLength(1);
  });

  it("si la fin du mouvement n'est jamais signalée, un délai de sécurité rétablit la carte", async () => {
    animateMode = "never";
    await ready();
    await send("Sans fin");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    await waitFor(() => expect(ghosts()).toHaveLength(0), { timeout: 2000 });
    expect(realCard("Sans fin").style.visibility).toBe("");
  });

  it("le fantôme est retiré au démontage de l'accueil", async () => {
    const { unmount } = render(<InboxHome />);
    await screen.findByRole("heading", { name: "À organiser" });
    await send("Démontée");
    await waitFor(() => expect(ghosts()).toHaveLength(1));
    unmount();
    expect(ghosts()).toHaveLength(0);
  });
});
