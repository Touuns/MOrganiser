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
  /** Renseigné seulement pour une capture transformée en tâche (lecture seule). */
  convertedAt: number | null;
  /** Tâche active issue de cette capture (présente si et seulement si `convertedAt` l'est). */
  convertedTaskId: string | null;
}

// Types alignés sur src-tauri/src/tasks.rs (sérialisation serde).

export type TaskStatus = "todo" | "in_progress" | "waiting" | "blocked" | "done";

export interface Task {
  id: string;
  title: string;
  /** Copie intégrale du texte de la capture au moment de la conversion. */
  details: string;
  status: TaskStatus;
  destinationId: string | null;
  /** Capture d'origine ; `null` pour une future tâche créée directement. */
  originInboxItemId: string | null;
  createdAt: number;
  updatedAt: number;
  deletedAt: number | null;
}

export interface TaskPage {
  /** De la plus récente à la plus ancienne. */
  items: Task[];
  total: number;
  nextCursor: Cursor | null;
}

/** Résultat d'une conversion ou de son annulation : la tâche et la capture dans leur nouvel état. */
export interface Conversion {
  task: Task;
  item: InboxItem;
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
    /** Présent pour `already_converted` : la tâche existante. */
    readonly taskId?: string,
  ) {
    super(message);
    this.name = "InboxApiError";
  }
}

function toApiError(error: unknown): InboxApiError {
  if (typeof error === "object" && error !== null && "code" in error && "message" in error) {
    const taskId = "taskId" in error && typeof error.taskId === "string" ? error.taskId : undefined;
    return new InboxApiError(String(error.code), String(error.message), taskId);
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

// --- Conversion des captures en tâches (brique 002) ---

/**
 * Transforme une capture en tâche. `expectedUpdatedAt` : la version enregistrée vue par
 * l'utilisateur. `title` : `null` utilise le titre proposé par Rust.
 */
export function convertInboxItemToTask(
  id: string,
  expectedUpdatedAt: number,
  title: string | null,
): Promise<Conversion> {
  return call("convert_inbox_item_to_task", { id, expectedUpdatedAt, title });
}

/** Annule la conversion : la capture revient dans la boîte, la tâche est conservée. */
export function cancelTaskConversion(id: string): Promise<Conversion> {
  return call("cancel_task_conversion", { id });
}

/** Titre proposé pour la conversion d'une capture (première ligne, 120 caractères au plus). */
export function suggestTaskTitle(id: string): Promise<string> {
  return call("suggest_task_title", { id });
}

export function listTasks(limit: number, before: Cursor | null = null): Promise<TaskPage> {
  return call("list_tasks", { limit, before });
}

export function getTask(id: string): Promise<Task> {
  return call("get_task", { id });
}
