import { useEffect, useRef } from "react";
import "./LeaveBanner.css";

interface LeaveBannerProps {
  message: string;
  /** Absent : rien à enregistrer (seul un titre de conversion est en jeu). */
  saveLabel?: string;
  discardLabel: string;
  keepLabel: string;
  onSave: () => void;
  onDiscard: () => void;
  onKeep: () => void;
  /** Enregistrement en cours : le bouton principal est désactivé. */
  saving?: boolean;
  /** Échec de l'enregistrement demandé depuis cette bannière. */
  error?: string | null;
}

/**
 * Avertissement avant de perdre du texte non enregistré : trois choix explicites.
 * Le focus va sur « continuer » (le choix le plus sûr) ; `Échap` est géré par le parent.
 */
export function LeaveBanner(props: LeaveBannerProps) {
  const keepRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    keepRef.current?.focus();
  }, []);

  return (
    <div className="leave" role="alert">
      <p>{props.message}</p>
      <div className="leave__actions">
        {props.saveLabel !== undefined && (
          <button type="button" onClick={props.onSave} disabled={props.saving}>
            {props.saveLabel}
          </button>
        )}
        <button type="button" onClick={props.onDiscard}>
          {props.discardLabel}
        </button>
        <button type="button" ref={keepRef} onClick={props.onKeep}>
          {props.keepLabel}
        </button>
      </div>
      {props.error && <p className="leave__error">{props.error}</p>}
    </div>
  );
}
