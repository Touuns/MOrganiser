import { useEffect, useRef } from "react";
import "./UndoToast.css";

/** Durée d'affichage de la notification avant disparition automatique. */
export const UNDO_DURATION_MS = 4000;

interface UndoToastProps {
  message: string;
  actionLabel: string;
  onAction: () => void;
  onDismiss: () => void;
  durationMs?: number;
}

/**
 * Notification compacte après une action réversible, avec une possibilité d'annuler.
 *
 * Le compte à rebours est suspendu tant que la souris la survole ou que son bouton a le focus,
 * puis reprend avec le temps restant : le bouton ne disparaît jamais sous le curseur.
 */
export function UndoToast(props: UndoToastProps) {
  const { message, actionLabel, onAction, onDismiss, durationMs = UNDO_DURATION_MS } = props;
  // La minuterie ne repart pas à chaque rendu du parent : seule la dernière fonction est appelée.
  const dismiss = useRef(onDismiss);
  dismiss.current = onDismiss;

  const remaining = useRef(durationMs);
  const startedAt = useRef(0);
  const timer = useRef<number | undefined>(undefined);
  const hovered = useRef(false);
  const focused = useRef(false);

  const start = () => {
    if (timer.current !== undefined) return;
    startedAt.current = performance.now();
    timer.current = window.setTimeout(() => dismiss.current(), remaining.current);
  };
  const pause = () => {
    if (timer.current === undefined) return;
    window.clearTimeout(timer.current);
    timer.current = undefined;
    remaining.current = Math.max(0, remaining.current - (performance.now() - startedAt.current));
  };
  const refresh = () => (hovered.current || focused.current ? pause() : start());

  useEffect(() => {
    start();
    return () => {
      window.clearTimeout(timer.current);
      timer.current = undefined;
    };
    // Montage uniquement : les changements de durée ne concernent pas une notification déjà affichée.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Pas de rôle propre : la région vivante persistante qui l'accueille l'annonce (voir InboxHome).
  return (
    <div
      className="toast"
      onMouseEnter={() => {
        hovered.current = true;
        refresh();
      }}
      onMouseLeave={() => {
        hovered.current = false;
        refresh();
      }}
      onFocus={() => {
        focused.current = true;
        refresh();
      }}
      onBlur={() => {
        focused.current = false;
        refresh();
      }}
    >
      <span>{message}</span>
      <button type="button" className="toast__action" onClick={onAction}>
        {actionLabel}
      </button>
    </div>
  );
}
