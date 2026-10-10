import { Badge } from "../../components/Badge";
import type { Task, TaskStatus } from "./api";
import { formatShort, isoDate } from "./format";
import "./CaptureCard.css";

export const STATUS_LABELS: Record<TaskStatus, string> = {
  todo: "À faire",
  in_progress: "En cours",
  waiting: "En attente",
  blocked: "Bloquée",
  done: "Terminée",
};

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
