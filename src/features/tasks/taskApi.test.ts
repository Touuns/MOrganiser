import { beforeEach, describe, expect, it, vi } from "vitest";
import { createFakeBackend, isValidCalendarDate, type FakeBackend } from "../../test/fakeBackend";
import {
  convertInboxItemToTask,
  createTask,
  updateTask,
  type Destination,
  type Task,
  type TaskFields,
} from "../inbox/api";
import { taskClassificationLabel } from "../inbox/TaskCard";

// Faux pont Tauri : les wrappers TypeScript sont vérifiés contre les mêmes règles que Rust.
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

const fields = (extra: Partial<TaskFields> = {}): TaskFields => ({
  title: "Tâche",
  details: "",
  responsibilityId: null,
  sectionId: null,
  startDate: null,
  dueDate: null,
  priority: null,
  ...extra,
});

describe("contrat des tâches enrichies (003-B1b)", () => {
  it("createTask : sans capture d'origine, description vide acceptée, aucune valeur par défaut inventée", async () => {
    const task = await createTask(fields());
    expect(task).toMatchObject({
      title: "Tâche",
      details: "",
      status: "todo",
      originInboxItemId: null,
      responsibilityId: null,
      sectionId: null,
      startDate: null,
      dueDate: null,
      completedOn: null,
      priority: null, // jamais « normale » par défaut
    });
    expect(tauri.invoke).toHaveBeenCalledWith("create_task", { fields: fields() });
    expect(backend.state.items).toHaveLength(0); // aucune capture créée
  });

  it("univers et rubrique sont indépendants ; une mauvaise nature est refusée", async () => {
    const both = await createTask(fields({ responsibilityId: "externe", sectionId: "administratif" }));
    expect(both).toMatchObject({ responsibilityId: "externe", sectionId: "administratif" });
    await expect(createTask(fields({ responsibilityId: "finances" }))).rejects.toMatchObject({
      code: "destination_kind",
    });
    await expect(createTask(fields({ sectionId: "moi" }))).rejects.toMatchObject({ code: "destination_kind" });
  });

  it("dates : format calendrier strict, début après échéance refusé, priorité limitée à 1–3", async () => {
    expect(isValidCalendarDate("2028-02-29")).toBe(true);
    for (const bad of ["2027-02-29", "2026-2-3", "2026-10-14T10:00", "", "0000-01-01"]) {
      expect(isValidCalendarDate(bad)).toBe(false);
    }
    await expect(createTask(fields({ dueDate: "2026-02-30" }))).rejects.toMatchObject({ code: "invalid_date" });
    await expect(createTask(fields({ startDate: "2026-10-15", dueDate: "2026-10-14" }))).rejects.toMatchObject({
      code: "dates_incoherent",
    });
    await expect(createTask(fields({ priority: 4 }))).rejects.toMatchObject({ code: "invalid_priority" });
    expect(await createTask(fields({ startDate: "2026-10-14", dueDate: "2026-10-14", priority: 3 }))).toMatchObject({
      priority: 3,
    });
  });

  it("updateTask : verrou optimiste, conflit sans écriture, statut et origine intacts", async () => {
    const task = await createTask(fields());
    const edited = await updateTask(task.id, task.updatedAt, fields({ title: "Renommée", priority: 2 }));
    expect(edited).toMatchObject({ title: "Renommée", priority: 2, status: "todo", originInboxItemId: null });
    expect(edited.updatedAt).toBeGreaterThan(task.updatedAt);

    const before = JSON.stringify(backend.state.tasks);
    await expect(updateTask(task.id, task.updatedAt, fields({ title: "Périmée" }))).rejects.toMatchObject({
      code: "task_version_conflict",
    });
    expect(JSON.stringify(backend.state.tasks)).toBe(before);

    // Sans changement réel : aucune écriture.
    const same = await updateTask(task.id, edited.updatedAt, fields({ title: "Renommée", priority: 2 }));
    expect(same.updatedAt).toBe(edited.updatedAt);
  });

  it("updateTask refuse une tâche annulée ou inconnue", async () => {
    const item = backend.seed("Capture");
    const converted = await convertInboxItemToTask(item.id, item.updatedAt, null);
    backend.state.tasks[0].deletedAt = 1;
    await expect(
      updateTask(converted.task.id, converted.task.updatedAt, fields()),
    ).rejects.toMatchObject({ code: "task_not_active" });
    await expect(updateTask("inconnue", 1, fields())).rejects.toMatchObject({ code: "task_not_found" });
  });

  it("la conversion répartit l'ancien classement unique selon sa nature, sans inventer", async () => {
    const section = backend.seed("Rubrique", "finances");
    const responsibility = backend.seed("Univers", "externe");
    const none = backend.seed("Libre");
    const a = (await convertInboxItemToTask(section.id, section.updatedAt, null)).task;
    const b = (await convertInboxItemToTask(responsibility.id, responsibility.updatedAt, null)).task;
    const c = (await convertInboxItemToTask(none.id, none.updatedAt, null)).task;
    expect([a.responsibilityId, a.sectionId]).toEqual([null, "finances"]);
    expect([b.responsibilityId, b.sectionId]).toEqual(["externe", null]);
    expect([c.responsibilityId, c.sectionId]).toEqual([null, null]);
  });

  it("la conversion accepte le classement en deux dimensions, les dates et la priorité", async () => {
    const item = backend.seed("Dossier", "administratif");
    const { task } = await convertInboxItemToTask(item.id, item.updatedAt, "Dossier à compléter", {
      details: "",
      destination: { type: "split", responsibilityId: "externe", sectionId: "administratif" },
      startDate: "2026-10-14",
      dueDate: "2026-11-01",
      priority: 3,
    });
    expect(task).toMatchObject({
      details: "", // description vide acceptée, jamais remplacée par le texte de la capture
      responsibilityId: "externe",
      sectionId: "administratif",
      startDate: "2026-10-14",
      dueDate: "2026-11-01",
      priority: 3,
    });
    expect(backend.find(item.id)!.destinationId).toBe("administratif"); // la capture ne change pas
  });

  it("libellé de classement : les deux dimensions, archivées comprises, jamais inventé", () => {
    const destinations: Destination[] = [
      { id: "externe", label: "Externe", kind: "responsibility" },
      { id: "administratif", label: "Administratif", kind: "section" },
    ];
    const base = { responsibilityId: null, sectionId: null } as Task;
    expect(taskClassificationLabel(base, destinations)).toBeNull();
    expect(taskClassificationLabel({ ...base, responsibilityId: "externe" }, destinations)).toBe("Externe");
    expect(taskClassificationLabel({ ...base, sectionId: "administratif" }, destinations)).toBe("Administratif");
    expect(
      taskClassificationLabel({ ...base, responsibilityId: "externe", sectionId: "administratif" }, destinations),
    ).toBe("Externe · Administratif");
    // Destination archivée (plus listée) : l'identifiant reste affiché plutôt qu'une perte.
    expect(taskClassificationLabel({ ...base, sectionId: "archivee" }, destinations)).toBe("archivee");
  });
});
