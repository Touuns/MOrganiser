import { invoke } from "@tauri-apps/api/core";

// Types alignés sur src-tauri/src/inbox.rs (sérialisation serde).

export type DestinationKind = "responsibility" | "section";

export interface Destination {
  id: string;
  label: string;
  kind: DestinationKind;
}

export interface InboxItem {
  id: string;
  content: string;
  destinationId: string | null;
  /** Millisecondes UTC. */
  createdAt: number;
  /** Dernière modification du texte ou de la destination (pas la corbeille). */
  updatedAt: number;
  /** Renseigné seulement pour une capture dans la corbeille. */
  deletedAt: number | null;
}

/** Position opaque pour charger les éléments plus anciens : à renvoyer telle quelle. */
export interface Cursor {
  sortKey: number;
  id: string;
}

export interface InboxPage {
  /** Du plus récent au plus ancien. */
  items: InboxItem[];
  total: number;
  /** Présent s'il reste des éléments plus anciens. */
  nextCursor: Cursor | null;
}

export type InboxFilter =
  | { type: "all" }
  | { type: "unclassified" }
  | { type: "destination"; id: string };

/** Erreur renvoyée par Rust : `code` stable, `message` lisible (sans le texte saisi). */
export class InboxApiError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "InboxApiError";
  }
}

function toApiError(error: unknown): InboxApiError {
  if (typeof error === "object" && error !== null && "code" in error && "message" in error) {
    return new InboxApiError(String(error.code), String(error.message));
  }
  return new InboxApiError("unknown", "L'opération a échoué.");
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    throw toApiError(error);
  }
}

export function listDestinations(): Promise<Destination[]> {
  return call("list_destinations");
}

export function listInboxItems(
  filter: InboxFilter,
  limit: number,
  before: Cursor | null = null,
): Promise<InboxPage> {
  return call("list_inbox_items", { filter, limit, before });
}

export function listTrashedItems(limit: number, before: Cursor | null = null): Promise<InboxPage> {
  return call("list_trashed_items", { limit, before });
}

export function createInboxItem(content: string, destinationId: string | null): Promise<InboxItem> {
  return call("create_inbox_item", { content, destinationId });
}

export function getInboxItem(id: string): Promise<InboxItem> {
  return call("get_inbox_item", { id });
}

/** `expectedUpdatedAt` : la version ouverte par l'utilisateur (verrouillage optimiste). */
export function updateInboxItem(
  id: string,
  content: string,
  destinationId: string | null,
  expectedUpdatedAt: number,
): Promise<InboxItem> {
  return call("update_inbox_item", { id, content, destinationId, expectedUpdatedAt });
}

export function trashInboxItem(id: string): Promise<InboxItem> {
  return call("trash_inbox_item", { id });
}

export function restoreInboxItem(id: string): Promise<InboxItem> {
  return call("restore_inbox_item", { id });
}
