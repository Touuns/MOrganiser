import { Badge } from "../../components/Badge";
import type { Destination, Task, TaskStatus } from "./api";
import { formatShort, isoDate } from "./format";
import "./CaptureCard.css";

export const STATUS_LABELS: Record<TaskStatus, string> = {
  todo: "À faire",
  in_progress: "En cours",
  waiting: "En attente",
  blocked: "Bloquée",
  done: "Terminée",
};

/** Libellés d'une destination, ou son identifiant si elle n'est plus listée (archivée). */
function destinationLabel(destinations: Destination[], id: string | null): string | null {
  if (id === null) return null;
  return destinations.find((d) => d.id === id)?.label ?? id;
}

/**
 * Classement affiché d'une tâche : « Univers · Rubrique » selon ce qui est renseigné, ou `null`.
 * Les deux dimensions sont indépendantes et facultatives.
 */
export function taskClassificationLabel(task: Task, destinations: Destination[]): string | null {
  const parts = [
    destinationLabel(destinations, task.responsibilityId),
    destinationLabel(destinations, task.sectionId),
  ].filter((part): part is string => part !== null);
  return parts.length > 0 ? parts.join(" · ") : null;
}

interface TaskCardProps {
  task: Task;
  /** Libellé de la destination, ou `null` si la tâche n'en a pas. */
  destinationLabel: string | null;
  selected: boolean;
  onOpen: (task: Task) => void;
}

/**
 * Carte d'une tâche dans la liste. Mêmes classes que `CaptureCard` : un seul style de liste.
 * `data-capture-id` est l'attribut lu par l'ancrage de défilement (identifiant de l'élément).
 */
export function TaskCard({ task, destinationLabel, selected, onOpen }: TaskCardProps) {
  return (
    <li
      className="inbox__item"
      data-capture-id={task.id}
      data-task-id={task.id}
      data-selected={selected || undefined}
    >
      <button
        type="button"
        className="inbox__open"
        aria-current={selected ? "true" : undefined}
        onClick={() => onOpen(task)}
      >
        <span className="inbox__content">{task.title}</span>
        <span className="inbox__meta">
          <Badge>{STATUS_LABELS[task.status]}</Badge>
          {destinationLabel && <Badge tone="accent">{destinationLabel}</Badge>}
          <time dateTime={isoDate(task.createdAt)}>{formatShort(task.createdAt)}</time>
        </span>
      </button>
    </li>
  );
}
