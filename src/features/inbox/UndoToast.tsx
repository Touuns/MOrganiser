import { useEffect, useRef } from "react";
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

/** Retour après une action réversible, avec une possibilité d'annuler. */
export function UndoToast(props: UndoToastProps) {
  const { message, actionLabel, onAction, onDismiss, durationMs = UNDO_DURATION_MS } = props;

  // La minuterie ne repart pas à chaque rendu du parent : seule la dernière fonction est appelée.
  const dismiss = useRef(onDismiss);
  dismiss.current = onDismiss;
  useEffect(() => {
    const timer = window.setTimeout(() => dismiss.current(), durationMs);
    return () => window.clearTimeout(timer);
  }, [durationMs]);

  // Pas de rôle propre : la région vivante persistante qui l'accueille l'annonce (voir InboxHome).
  return (
    <div className="toast">
      <span>{message}</span>
      <button type="button" className="toast__action" onClick={onAction}>
        {actionLabel}
      </button>
    </div>
  );
}
