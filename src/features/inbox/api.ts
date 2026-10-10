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
  updatedAt: number;
}

export interface InboxPage {
  items: InboxItem[];
  total: number;
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

export function listInboxItems(filter: InboxFilter, limit: number): Promise<InboxPage> {
  return call("list_inbox_items", { filter, limit });
}

export function createInboxItem(content: string, destinationId: string | null): Promise<InboxItem> {
  return call("create_inbox_item", { content, destinationId });
}
