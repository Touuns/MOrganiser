import type {
  Cursor,
  Destination,
  DestinationChoice,
  InboxFilter,
  InboxItem,
  InboxPage,
  Task,
  TaskFields,
  TaskPage,
} from "../features/inbox/api";

/** Erreur telle que Rust la renvoie : `{ code, message }` (+ `taskId` pour `already_converted`). */
export interface BackendError {
  code: string;
  message: string;
  taskId?: string;
}

export const DESTINATIONS: Destination[] = [
  { id: "moi", label: "Moi", kind: "responsibility" },
  { id: "externe", label: "Externe", kind: "responsibility" },
  { id: "administratif", label: "Administratif", kind: "section" },
  { id: "finances", label: "Finances", kind: "section" },
  { id: "inventaire", label: "Inventaire", kind: "section" },
];

const MAX_LIMIT = 200;
const MAX_TITLE_CHARS = 120;
const MAX_CONTENT_CHARS = 10_000;

const collapse = (text: string) => text.split(/\s+/).filter(Boolean).join(" ");

/** `AAAA-MM-JJ` strict et jour réel du calendrier (mêmes règles que Rust). */
export function isValidCalendarDate(value: string): boolean {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return false;
  const [year, month, day] = [Number(match[1]), Number(match[2]), Number(match[3])];
  if (year === 0 || month < 1 || month > 12 || day < 1) return false;
  const leap = (year % 4 === 0 && year % 100 !== 0) || year % 400 === 0;
  const days = [31, leap ? 29 : 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31][month - 1];
  return day <= days;
}

/** Titre proposé : première ligne non vide, espaces normalisés, 120 caractères au plus (« … » si coupé). */
export function suggestTitle(content: string): string {
  const line = collapse(content.split(/\r?\n/).map((l) => l.trim()).find((l) => l !== "") ?? "");
  const chars = [...line];
  if (chars.length <= MAX_TITLE_CHARS) return line;
  return chars.slice(0, MAX_TITLE_CHARS - 1).join("").trimEnd() + "…";
}

/**
 * Faux « Rust + SQLite » en mémoire pour les tests de l'interface : mêmes règles que
 * src-tauri/src/inbox.rs et src-tauri/src/tasks.rs (pagination par curseur, verrou
 * optimiste, corbeille logique, conversion en tâche atomique, une tâche active par capture).
 */
export function createFakeBackend() {
  const state = {
    items: [] as InboxItem[],
    tasks: [] as Task[],
    clock: 1_760_000_000_000,
    sequence: 0,
    taskSequence: 0,
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
      convertedAt: null,
      convertedTaskId: null,
    };
    state.items.push(item);
    return item;
  }

  const find = (id: string) => state.items.find((item) => item.id === id);
  const findTask = (id: string) => state.tasks.find((task) => task.id === id);
  const fail = (code: string, message: string, extra: Partial<BackendError> = {}): Promise<never> =>
    Promise.reject({ code, message, ...extra });

  function matches(item: InboxItem, filter: InboxFilter): boolean {
    if (filter.type === "all") return true;
    if (filter.type === "unclassified") return item.destinationId === null;
    return item.destinationId === filter.id;
  }

  function page(
    scope: "active" | "trash" | "converted",
    filter: InboxFilter,
    limit: number,
    before: Cursor | null,
  ): InboxPage {
    const key = (item: InboxItem) =>
      scope === "trash"
        ? (item.deletedAt as number)
        : scope === "converted"
          ? (item.convertedAt as number)
          : item.createdAt;
    const scoped = state.items
      // Boîte : ni supprimées ni converties ; corbeille : supprimées ; Traitées : converties.
      .filter((item) =>
        scope === "trash"
          ? item.deletedAt !== null
          : scope === "converted"
            ? item.deletedAt === null && item.convertedAt !== null
            : item.deletedAt === null && item.convertedAt === null,
      )
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

  function taskPage(limit: number, before: Cursor | null): TaskPage {
    const scoped = state.tasks
      .filter((task) => task.deletedAt === null)
      .sort((a, b) => b.createdAt - a.createdAt || (a.id < b.id ? 1 : -1));
    const older = before
      ? scoped.filter(
          (task) =>
            task.createdAt < before.sortKey || (task.createdAt === before.sortKey && task.id < before.id),
        )
      : scoped;
    const size = Math.min(Math.max(limit, 1), MAX_LIMIT);
    const items = older.slice(0, size);
    const last = items.at(-1);
    const nextCursor = older.length > size && last ? { sortKey: last.createdAt, id: last.id } : null;
    return { items: items.map((t) => ({ ...t })), total: scoped.length, nextCursor };
  }

  const kindOf = (id: string) => DESTINATIONS.find((d) => d.id === id)?.kind;

  /** Dates calendaires valides, début <= échéance, priorité 1 à 3 (mêmes règles que Rust). */
  function checkSchedule(
    start: string | null,
    due: string | null,
    priority: number | null,
  ): Promise<never> | null {
    for (const date of [start, due]) {
      if (date !== null && !isValidCalendarDate(date)) {
        return fail("invalid_date", "Une date n'est pas valide : utilisez une vraie date du calendrier.");
      }
    }
    if (start !== null && due !== null && start > due) {
      return fail("dates_incoherent", "La date de début ne peut pas être postérieure à l'échéance.");
    }
    if (priority !== null && ![1, 2, 3].includes(priority)) {
      return fail("invalid_priority", "Cette priorité n'est pas disponible.");
    }
    return null;
  }

  type Classification =
    | { responsibilityId: string | null; sectionId: string | null }
    | { error: Promise<never> };

  /** Une destination pour une dimension : existante, de la bonne nature (les archives ne sont pas simulées). */
  function checkDimension(id: string, expected: "responsibility" | "section"): Promise<never> | null {
    const kind = kindOf(id);
    if (kind === undefined) return fail("unknown_destination", "Cette destination n'est pas disponible.");
    if (kind !== expected) {
      return fail("destination_kind", "Cette destination ne peut pas être utilisée à cet endroit.");
    }
    return null;
  }

  function resolveClassification(captureDestination: string | null, choice: DestinationChoice): Classification {
    const place = (id: string): Classification => {
      const kind = kindOf(id);
      if (kind === undefined) {
        return { error: fail("unknown_destination", "Cette destination n'est pas disponible.") };
      }
      return kind === "responsibility"
        ? { responsibilityId: id, sectionId: null }
        : { responsibilityId: null, sectionId: id };
    };
    switch (choice.type) {
      case "clear":
        return { responsibilityId: null, sectionId: null };
      case "set":
        return place(choice.id);
      case "split": {
        const responsibilityId = choice.responsibilityId || null;
        const sectionId = choice.sectionId || null;
        const error =
          (responsibilityId && checkDimension(responsibilityId, "responsibility")) ||
          (sectionId && checkDimension(sectionId, "section"));
        return error ? { error } : { responsibilityId, sectionId };
      }
      default:
        return captureDestination === null
          ? { responsibilityId: null, sectionId: null }
          : place(captureDestination);
    }
  }

  /** Titre normalisé, description (vide acceptée), calendrier : validation commune des champs. */
  function validateFields(
    fields: TaskFields,
  ): { error: Promise<never> } | { title: string; details: string; fields: TaskFields } {
    const title = collapse(fields.title ?? "");
    if (title === "") return { error: fail("empty_title", "Le titre de la tâche est vide.") };
    if ([...title].length > MAX_TITLE_CHARS) {
      return { error: fail("title_too_long", `Le titre dépasse ${MAX_TITLE_CHARS} caractères.`) };
    }
    const details = String(fields.details ?? "").trim();
    if ([...details].length > MAX_CONTENT_CHARS) {
      return { error: fail("details_too_long", `La description dépasse ${MAX_CONTENT_CHARS} caractères.`) };
    }
    const schedule = checkSchedule(fields.startDate ?? null, fields.dueDate ?? null, fields.priority ?? null);
    if (schedule) return { error: schedule };
    const responsibilityId = fields.responsibilityId || null;
    const sectionId = fields.sectionId || null;
    const dimension =
      (responsibilityId && checkDimension(responsibilityId, "responsibility")) ||
      (sectionId && checkDimension(sectionId, "section"));
    if (dimension) return { error: dimension };
    return { title, details, fields: { ...fields, responsibilityId, sectionId } };
  }

  function createTaskDirect(args: Record<string, unknown>): Promise<unknown> {
    const valid = validateFields(args.fields as TaskFields);
    if ("error" in valid) return valid.error;
    const now = tick();
    state.taskSequence += 1;
    const task: Task = {
      id: `task-${String(state.taskSequence).padStart(4, "0")}`,
      title: valid.title,
      details: valid.details,
      status: "todo",
      responsibilityId: valid.fields.responsibilityId,
      sectionId: valid.fields.sectionId,
      startDate: valid.fields.startDate ?? null,
      dueDate: valid.fields.dueDate ?? null,
      completedOn: null,
      priority: valid.fields.priority ?? null,
      originInboxItemId: null, // aucune capture inventée
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    };
    state.tasks.push(task);
    return Promise.resolve({ ...task });
  }

  function updateTaskFields(args: Record<string, unknown>): Promise<unknown> {
    const valid = validateFields(args.fields as TaskFields);
    if ("error" in valid) return valid.error;
    const task = findTask(args.id as string);
    if (!task) return fail("task_not_found", "Cette tâche n'existe plus.");
    if (task.deletedAt !== null) return fail("task_not_active", "Cette tâche est déjà annulée.");
    if (task.updatedAt !== args.expectedUpdatedAt) {
      return fail("task_version_conflict", "Cette tâche a été modifiée depuis son ouverture.");
    }
    const next = {
      title: valid.title,
      details: valid.details,
      responsibilityId: valid.fields.responsibilityId,
      sectionId: valid.fields.sectionId,
      startDate: valid.fields.startDate ?? null,
      dueDate: valid.fields.dueDate ?? null,
      priority: valid.fields.priority ?? null,
    };
    const changed = (Object.keys(next) as (keyof typeof next)[]).some((key) => task[key] !== next[key]);
    if (changed) {
      Object.assign(task, next);
      task.updatedAt = Math.max(tick(), task.updatedAt + 1); // statut, origine, réalisation : intacts
    }
    return Promise.resolve({ ...task });
  }

  function convert(args: Record<string, unknown>): Promise<unknown> {
    const given = args.title as string | null | undefined;
    let title: string | null = null;
    if (given !== null && given !== undefined) {
      title = collapse(given);
      if (title === "") return fail("empty_title", "Le titre de la tâche est vide.");
      if ([...title].length > MAX_TITLE_CHARS) {
        return fail("title_too_long", `Le titre dépasse ${MAX_TITLE_CHARS} caractères.`);
      }
    }
    // Description : extrémités nettoyées, VIDE ACCEPTÉE (facultative), pas trop longue (comme Rust).
    let details: string | null = null;
    if (args.details !== null && args.details !== undefined) {
      details = String(args.details).trim();
      if ([...details].length > MAX_CONTENT_CHARS) {
        return fail("details_too_long", `La description dépasse ${MAX_CONTENT_CHARS} caractères.`);
      }
    }
    const scheduleError = checkSchedule(
      (args.startDate as string | null | undefined) ?? null,
      (args.dueDate as string | null | undefined) ?? null,
      (args.priority as number | null | undefined) ?? null,
    );
    if (scheduleError) return scheduleError;
    const item = find(args.id as string);
    if (!item) return fail("not_found", "Cette capture n'existe plus.");
    if (item.deletedAt !== null) {
      return fail("trashed", "Cette capture est dans la corbeille : restaurez-la d'abord.");
    }
    if (item.convertedTaskId !== null) {
      return fail("already_converted", "Cette capture est déjà transformée en tâche.", {
        taskId: item.convertedTaskId,
      });
    }
    if (item.updatedAt !== args.expectedUpdatedAt) {
      return fail("version_conflict", "Cette capture a été modifiée depuis son ouverture.");
    }
    // Classement de la tâche : indépendant de celui de la capture, qui ne change jamais.
    const choice = (args.destination as DestinationChoice | null | undefined) ?? { type: "keep" };
    const classification = resolveClassification(item.destinationId, choice);
    if ("error" in classification) return classification.error;
    const now = tick();
    state.taskSequence += 1;
    const task: Task = {
      id: `task-${String(state.taskSequence).padStart(4, "0")}`,
      title: title ?? suggestTitle(item.content),
      details: details ?? item.content,
      status: "todo",
      responsibilityId: classification.responsibilityId,
      sectionId: classification.sectionId,
      startDate: (args.startDate as string | null | undefined) ?? null,
      dueDate: (args.dueDate as string | null | undefined) ?? null,
      completedOn: null,
      priority: (args.priority as number | null | undefined) ?? null,
      originInboxItemId: item.id,
      createdAt: now,
      updatedAt: now,
      deletedAt: null,
    };
    state.tasks.push(task);
    item.convertedAt = now;
    item.convertedTaskId = task.id;
    return Promise.resolve({ task: { ...task }, item: { ...item } });
  }

  function cancel(args: Record<string, unknown>): Promise<unknown> {
    const task = findTask(args.id as string);
    if (!task) return fail("task_not_found", "Cette tâche n'existe plus.");
    if (task.deletedAt !== null) return fail("task_not_active", "Cette tâche est déjà annulée.");
    const origin = task.originInboxItemId ? find(task.originInboxItemId) : undefined;
    if (!origin) return fail("no_origin", "Cette tâche ne vient pas d'une capture.");
    if (task.updatedAt !== task.createdAt || task.status !== "todo") {
      return fail(
        "task_modified",
        "Cette tâche a déjà été modifiée : ouvrez-la plutôt que d'annuler sa création.",
      );
    }
    task.deletedAt = tick();
    origin.convertedAt = null;
    origin.convertedTaskId = null;
    return Promise.resolve({ task: { ...task }, item: { ...origin } });
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
          page("active", args.filter as InboxFilter, args.limit as number, (args.before as Cursor) ?? null),
        );
      case "list_trashed_items":
        return Promise.resolve(
          page("trash", { type: "all" }, args.limit as number, (args.before as Cursor) ?? null),
        );
      case "list_converted_items":
        return Promise.resolve(
          page("converted", { type: "all" }, args.limit as number, (args.before as Cursor) ?? null),
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
        if (item.convertedAt !== null) {
          return fail("converted", "Cette capture a été transformée en tâche : elle n'est plus modifiable ici.");
        }
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
        if (item.convertedAt !== null) {
          return fail("converted", "Cette capture a été transformée en tâche : elle n'est plus modifiable ici.");
        }
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
      case "suggest_task_title": {
        const item = find(args.id as string);
        return item ? Promise.resolve(suggestTitle(item.content)) : fail("not_found", "Cette capture n'existe plus.");
      }
      case "convert_inbox_item_to_task":
        return convert(args);
      case "create_task":
        return createTaskDirect(args);
      case "update_task":
        return updateTaskFields(args);
      case "cancel_task_conversion":
        return cancel(args);
      case "list_tasks":
        return Promise.resolve(taskPage(args.limit as number, (args.before as Cursor) ?? null));
      case "get_task": {
        const task = findTask(args.id as string);
        return task ? Promise.resolve({ ...task }) : fail("task_not_found", "Cette tâche n'existe plus.");
      }
      default:
        return Promise.reject({ code: "unknown", message: `commande inattendue ${command}` });
    }
  }

  return { state, seed, invoke, find, findTask };
}

export type FakeBackend = ReturnType<typeof createFakeBackend>;
