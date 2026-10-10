import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Destination, InboxFilter, InboxItem } from "./api";
import { HOME_LIMIT, InboxHome } from "./InboxHome";

// Faux pont Tauri : une « base » en mémoire qui imite les commandes Rust.
const tauri = vi.hoisted(() => ({
  invoke: vi.fn<(command: string, args?: Record<string, unknown>) => Promise<unknown>>(),
  isTauri: vi.fn(() => true),
}));
vi.mock("@tauri-apps/api/core", () => tauri);

const DESTINATIONS: Destination[] = [
  { id: "moi", label: "Moi", kind: "responsibility" },
  { id: "externe", label: "Externe", kind: "responsibility" },
  { id: "administratif", label: "Administratif", kind: "section" },
  { id: "finances", label: "Finances", kind: "section" },
  { id: "inventaire", label: "Inventaire", kind: "section" },
];

let db: InboxItem[];
let failCreate: boolean;
let clock: number;

function fakeBackend(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
  switch (command) {
    case "list_destinations":
      return Promise.resolve(DESTINATIONS);
    case "list_inbox_items": {
      const filter = args.filter as InboxFilter;
      const matching = db.filter((item) =>
        filter.type === "all"
          ? true
          : filter.type === "unclassified"
            ? item.destinationId === null
            : item.destinationId === filter.id,
      );
      const sorted = [...matching].sort((a, b) => b.createdAt - a.createdAt);
      return Promise.resolve({ items: sorted.slice(0, args.limit as number), total: sorted.length });
    }
    case "create_inbox_item": {
      if (failCreate) {
        return Promise.reject({ code: "storage", message: "L'enregistrement local a échoué." });
      }
      const item: InboxItem = {
        id: `id-${db.length}`,
        content: String(args.content).trim(),
        destinationId: (args.destinationId as string | null) ?? null,
        createdAt: ++clock,
        updatedAt: clock,
      };
      db.push(item);
      return Promise.resolve(item);
    }
    default:
      return Promise.reject({ code: "unknown", message: `commande inattendue ${command}` });
  }
}

function seed(content: string, destinationId: string | null = null) {
  db.push({ id: `seed-${db.length}`, content, destinationId, createdAt: ++clock, updatedAt: clock });
}

const field = () => screen.getByLabelText("Capture rapide", { selector: "textarea" });
const list = () => screen.getByRole("list");
const capture = async (text: string, destination?: string) => {
  fireEvent.change(field(), { target: { value: text } });
  if (destination) {
    fireEvent.change(screen.getByLabelText("Destination (facultatif)"), { target: { value: destination } });
  }
  fireEvent.keyDown(field(), { key: "Enter" });
  await waitFor(() => expect(field()).toHaveValue(""));
};

describe("InboxHome (boîte au-dessus de la capture)", () => {
  beforeEach(() => {
    db = [];
    failCreate = false;
    clock = 1_760_000_000_000;
    tauri.invoke.mockReset();
    tauri.invoke.mockImplementation(fakeBackend);
  });

  it("place la boîte « À organiser » avant (au-dessus) du champ de capture", async () => {
    render(<InboxHome />);
    const heading = await screen.findByRole("heading", { name: "À organiser" });
    const position = heading.compareDocumentPosition(field());
    expect(position & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
  });

  it("affiche un état vide accueillant", async () => {
    render(<InboxHome />);
    expect(await screen.findByText(/Rien à organiser pour l'instant/)).toBeInTheDocument();
  });

  it("une capture apparaît en bas (au-dessus du champ), une seule fois, marquée « À classer »", async () => {
    seed("Ancienne capture");
    render(<InboxHome />);
    await screen.findByText("Ancienne capture");
    await capture("Vendre ma PlayStation 5");
    await waitFor(() => expect(within(list()).getAllByRole("listitem")).toHaveLength(2));
    const [first, last] = within(list()).getAllByRole("listitem");
    expect(first).toHaveTextContent("Ancienne capture");
    expect(last).toHaveTextContent("Vendre ma PlayStation 5");
    expect(last).toHaveTextContent("À classer");
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
    expect(within(list()).getAllByRole("listitem")).toHaveLength(2);

    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "destination:inventaire" } });
    await waitFor(() => expect(within(list()).getAllByRole("listitem")).toHaveLength(1));
    expect(list()).toHaveTextContent("Vendre ma PlayStation 5");

    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "unclassified" } });
    await waitFor(() => expect(list()).toHaveTextContent("Note libre"));
    expect(list()).not.toHaveTextContent("PlayStation");
  });

  it("le texte est affiché comme du texte, jamais interprété", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    const dangerous = "'; DROP TABLE inbox_items; -- <b>gras</b> <img src=x onerror=alert(1)> l'été";
    await capture(dangerous);
    expect(await screen.findByText(dangerous)).toBeInTheDocument();
    expect(list().querySelector("b, img")).toBeNull();
  });

  it("en cas d'échec : rien n'apparaît, le texte reste dans le champ", async () => {
    failCreate = true;
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    fireEvent.change(field(), { target: { value: "À ne pas perdre" } });
    fireEvent.keyDown(field(), { key: "Enter" });
    expect(await screen.findByRole("alert")).toHaveTextContent("L'enregistrement local a échoué.");
    expect(field()).toHaveValue("À ne pas perdre");
    expect(screen.queryByRole("list")).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent("");
  });

  it("signale une capture masquée par le filtre actif", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    fireEvent.change(screen.getByLabelText("Afficher"), { target: { value: "destination:moi" } });
    await screen.findByText(/Aucune capture pour ce filtre/);
    await capture("Pour Finances", "finances");
    expect(screen.getByRole("status")).toHaveTextContent("masquée par le filtre");
  });

  it("plusieurs captures successives s'ajoutent en bas, dans l'ordre chronologique", async () => {
    render(<InboxHome />);
    await screen.findByText(/Rien à organiser/);
    await capture("Un");
    await capture("Deux", "moi");
    await capture("Trois");
    await waitFor(() => expect(within(list()).getAllByRole("listitem")).toHaveLength(3));
    const texts = within(list()).getAllByRole("listitem").map((li) => li.querySelector("p")?.textContent);
    expect(texts).toEqual(["Un", "Deux", "Trois"]);
  });

  it(`au-delà de ${HOME_LIMIT} : garde les plus récentes (pas les plus anciennes), en ordre chronologique`, async () => {
    for (let i = 0; i < HOME_LIMIT + 5; i++) seed(`Capture ${i}`);
    render(<InboxHome />);
    await screen.findByText(`Capture ${HOME_LIMIT + 4}`);
    const texts = within(list()).getAllByRole("listitem").map((li) => li.querySelector("p")?.textContent);
    expect(texts).toHaveLength(HOME_LIMIT);
    expect(texts[0]).toBe("Capture 5"); // les 5 plus anciennes (0 à 4) ne sont pas affichées
    expect(texts.at(-1)).toBe(`Capture ${HOME_LIMIT + 4}`); // la plus récente, en bas
    expect(screen.queryByText("Capture 0")).not.toBeInTheDocument();
    expect(screen.getByText(/Les 20 plus récentes sur 25/)).toBeInTheDocument();
    expect(tauri.invoke).toHaveBeenCalledWith("list_inbox_items", { filter: { type: "all" }, limit: HOME_LIMIT });

    // Une nouvelle capture apparaît en bas ; la plus ancienne affichée sort de la sélection.
    await capture("Toute nouvelle");
    await waitFor(() => expect(within(list()).getAllByRole("listitem").at(-1)).toHaveTextContent("Toute nouvelle"));
    expect(within(list()).getAllByRole("listitem")).toHaveLength(HOME_LIMIT);
    expect(screen.queryByText("Capture 5")).not.toBeInTheDocument();
  });

  it("une erreur de lecture propose de réessayer", async () => {
    tauri.invoke.mockImplementation((command, args) =>
      command === "list_inbox_items"
        ? Promise.reject({ code: "unavailable", message: "La base de données est indisponible." })
        : fakeBackend(command, args),
    );
    render(<InboxHome />);
    expect(await screen.findByRole("alert")).toHaveTextContent("La base de données est indisponible.");
    tauri.invoke.mockImplementation(fakeBackend);
    fireEvent.click(screen.getByRole("button", { name: "Réessayer" }));
    expect(await screen.findByText(/Rien à organiser/)).toBeInTheDocument();
  });
});
