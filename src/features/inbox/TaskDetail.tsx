import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { getInboxItem, type Destination, type Task } from "./api";
import { formatLong } from "./format";
import { STATUS_LABELS } from "./TaskCard";
import "./CaptureDetail.css";

interface TaskDetailProps {
  task: Task;
  destinations: Destination[];
  onClose: () => void;
}

/**
 * Fiche d'une tâche, en **lecture seule** (002-B) : l'édition et les statuts avancés viennent
 * avec la brique 003. Remonter avec `key={task.id}` pour changer de tâche.
 */
export function TaskDetail({ task, destinations, onClose }: TaskDetailProps) {
  const headingId = useId();
  const headingRef = useRef<HTMLHeadingElement>(null);
  // Date de la capture d'origine (lecture facultative : son absence ne gêne pas la fiche).
  const [originDate, setOriginDate] = useState<number | null | "unknown">(null);

  useEffect(() => {
    headingRef.current?.focus({ preventScroll: true });
  }, []);

  useEffect(() => {
    let active = true;
    if (task.originInboxItemId === null) return;
    getInboxItem(task.originInboxItemId)
      .then((origin) => active && setOriginDate(origin.createdAt))
      .catch(() => active && setOriginDate("unknown"));
    return () => {
      active = false;
    };
  }, [task.originInboxItemId]);

  function handleKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      event.stopPropagation();
      onClose();
    }
  }

  const destination = task.destinationId
    ? (destinations.find((d) => d.id === task.destinationId)?.label ?? task.destinationId)
    : "Aucune";

  return (
    <section className="detail" aria-labelledby={headingId} onKeyDown={handleKeyDown}>
      <header className="detail__header">
        <h2 id={headingId} className="detail__title" tabIndex={-1} ref={headingRef}>
          Fiche de la tâche
        </h2>
        <button type="button" className="detail__close" onClick={onClose}>
          <span className="detail__close-wide" aria-hidden="true">
            ✕
          </span>
          <span className="detail__close-narrow" aria-hidden="true">
            ← Retour
          </span>
          <span className="detail__sr">Fermer la fiche</span>
        </button>
      </header>

      <p className="detail__banner">Consultation seule pour le moment.</p>

      <div className="detail__field">
        <label htmlFor={`${headingId}-title`}>Titre</label>
        <input id={`${headingId}-title`} type="text" value={task.title} readOnly />
      </div>

      <div className="detail__field">
        <label htmlFor={`${headingId}-details`}>Détails</label>
        <textarea id={`${headingId}-details`} value={task.details} readOnly rows={6} />
      </div>

      <dl className="detail__dates">
        <dt>Statut</dt>
        <dd>{STATUS_LABELS[task.status]}</dd>
        <dt>Destination</dt>
        <dd>{destination}</dd>
        <dt>Créée</dt>
        <dd>
          <time dateTime={new Date(task.createdAt).toISOString()}>{formatLong(task.createdAt)}</time>
        </dd>
        <dt>Origine</dt>
        <dd>
          {task.originInboxItemId === null
            ? "Créée directement"
            : originDate === null
              ? "Issue d'une capture"
              : originDate === "unknown"
                ? "Issue d'une capture (introuvable)"
                : `Issue de la capture du ${formatLong(originDate)}`}
        </dd>
      </dl>
    </section>
  );
}
