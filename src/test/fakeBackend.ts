import type { Cursor, Destination, InboxFilter, InboxItem, InboxPage } from "../features/inbox/api";

/** Erreur telle que Rust la renvoie : `{ code, message }`. */
export interface BackendError {
  code: string;
  message: string;
}

export const DESTINATIONS: Destination[] = [
  { id: "moi", label: "Moi", kind: "responsibility" },
  { id: "externe", label: "Externe", kind: "responsibility" },
  { id: "administratif", label: "Administratif", kind: "section" },
  { id: "finances", label: "Finances", kind: "section" },
  { id: "inventaire", label: "Inventaire", kind: "section" },
];

const MAX_LIMIT = 200;

/**
 * Faux « Rust + SQLite » en mémoire pour les tests de l'interface : mêmes règles que
 * src-tauri/src/inbox.rs (pagination par curseur, verrou optimiste, corbeille logique).
 */
export function createFakeBackend() {
  const state = {
    items: [] as InboxItem[],
    clock: 1_760_000_000_000,
    sequence: 0,
    /** Échecs à injecter : commande -> erreur (rejetée tant que la valeur est présente). */
    failures: new Map<string, BackendError>(),
    calls: [] as { command: string; args: Record<string, unknown> }[],
  };

  const tick = () => ++state.clock;

  function seed(content: string, destinationId: string | null = null): InboxItem {
    const now = tick();
    state.sequence += 1;
    const item: InboxItem = {
      id: `id-${String(state.sequence).padStart(4, "0")}`,
      content,
      destinationId,
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    };
    state.items.push(item);
    return item;
  }

  const find = (id: string) => state.items.find((item) => item.id === id);
  const fail = (code: string, message: string): Promise<never> => Promise.reject({ code, message });

  function matches(item: InboxItem, filter: InboxFilter): boolean {
    if (filter.type === "all") return true;
    if (filter.type === "unclassified") return item.destinationId === null;
    return item.destinationId === filter.id;
  }

  function page(
    trash: boolean,
    filter: InboxFilter,
    limit: number,
    before: Cursor | null,
  ): InboxPage {
    const key = (item: InboxItem) => (trash ? (item.deletedAt as number) : item.createdAt);
    const scoped = state.items
      .filter((item) => (trash ? item.deletedAt !== null : item.deletedAt === null))
      .filter((item) => matches(item, filter))
      .sort((a, b) => key(b) - key(a) || (a.id < b.id ? 1 : -1));
    const older = before
      ? scoped.filter((item) => key(item) < before.sortKey || (key(item) === before.sortKey && item.id < before.id))
      : scoped;
    const size = Math.min(Math.max(limit, 1), MAX_LIMIT);
    const items = older.slice(0, size);
    const last = items.at(-1);
    const nextCursor = older.length > size && last ? { sortKey: key(last), id: last.id } : null;
    return { items: items.map((i) => ({ ...i })), total: scoped.length, nextCursor };
  }

  function invoke(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
    state.calls.push({ command, args });
    const injected = state.failures.get(command);
    if (injected) return Promise.reject(injected);

    switch (command) {
      case "list_destinations":
        return Promise.resolve(DESTINATIONS);
      case "list_inbox_items":
        return Promise.resolve(
          page(false, args.filter as InboxFilter, args.limit as number, (args.before as Cursor) ?? null),
        );
      case "list_trashed_items":
        return Promise.resolve(
          page(true, { type: "all" }, args.limit as number, (args.before as Cursor) ?? null),
        );
      case "get_inbox_item": {
        const item = find(args.id as string);
        return item ? Promise.resolve({ ...item }) : fail("not_found", "Cette capture n'existe plus.");
      }
      case "create_inbox_item": {
        const content = String(args.content).trim();
        if (content === "") return fail("empty_content", "La capture est vide.");
        return Promise.resolve({ ...seed(content, (args.destinationId as string | null) ?? null) });
      }
      case "update_inbox_item": {
        const item = find(args.id as string);
        if (!item) return fail("not_found", "Cette capture n'existe plus.");
        if (item.deletedAt !== null) return fail("trashed", "Cette capture est dans la corbeille.");
        if (item.updatedAt !== args.expectedUpdatedAt) {
          return fail("version_conflict", "Cette capture a été modifiée depuis son ouverture.");
        }
        const content = String(args.content).trim();
        if (content === "") return fail("empty_content", "La capture est vide.");
        const destinationId = (args.destinationId as string | null) ?? null;
        if (content !== item.content || destinationId !== item.destinationId) {
          item.content = content;
          item.destinationId = destinationId;
          item.updatedAt = Math.max(tick(), item.updatedAt + 1);
        }
        return Promise.resolve({ ...item });
      }
      case "trash_inbox_item": {
        const item = find(args.id as string);
        if (!item) return fail("not_found", "Cette capture n'existe plus.");
        if (item.deletedAt !== null) return fail("trashed", "Cette capture est dans la corbeille.");
        item.deletedAt = tick();
        return Promise.resolve({ ...item });
      }
      case "restore_inbox_item": {
        const item = find(args.id as string);
        if (!item) return fail("not_found", "Cette capture n'existe plus.");
        if (item.deletedAt === null) return fail("not_trashed", "Cette capture n'est pas dans la corbeille.");
        item.deletedAt = null;
        return Promise.resolve({ ...item });
      }
      default:
        return Promise.reject({ code: "unknown", message: `commande inattendue ${command}` });
    }
  }

  return { state, seed, invoke, find };
}

export type FakeBackend = ReturnType<typeof createFakeBackend>;
