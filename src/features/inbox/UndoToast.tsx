import { useEffect } from "react";
import "./UndoToast.css";

/** Durée d'affichage de la notification avant disparition automatique. */
export const UNDO_DURATION_MS = 8000;

interface UndoToastProps {
  message: string;
  actionLabel: string;
  onAction: () => void;
  onDismiss: () => void;
  durationMs?: number;
}

/** Retour discret après une action réversible, avec une possibilité d'annuler. */
export function UndoToast(props: UndoToastProps) {
  const { message, actionLabel, onAction, onDismiss, durationMs = UNDO_DURATION_MS } = props;

  useEffect(() => {
    const timer = window.setTimeout(onDismiss, durationMs);
    return () => window.clearTimeout(timer);
  }, [durationMs, onDismiss]);

  return (
    <div className="toast" role="status">
      <span>{message}</span>
      <button type="button" className="toast__action" onClick={onAction}>
        {actionLabel}
      </button>
    </div>
  );
}
