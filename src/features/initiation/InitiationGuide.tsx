import { useEffect, useId, useLayoutEffect, useRef, type KeyboardEvent } from "react";
import type { InitiationStep } from "./useInitiation";
import "./InitiationGuide.css";

/** Où se trouve la capture créée pendant la visite, du point de vue de l'écran actuel. */
export type BoxState =
  | "visible"
  | "loading"
  /** La liste n'a pas pu être relue. */
  | "unavailable"
  /** On est dans « Voir tout » ou la corbeille. */
  | "other-view"
  /** Un filtre la masque. */
  | "filtered"
  /** Boîte relue sans elle (supprimée ou déplacée depuis). */
  | "gone";

interface InitiationGuideProps {
  step: InitiationStep | null;
  /** Une capture a été créée pendant cette visite. */
  hasCreated: boolean;
  boxState: BoxState;
  onStart: () => void;
  onStop: () => void;
  onBack: () => void;
  onShowBox: () => void;
  onShowAll: () => void;
}

/** Éléments que la carte ne doit jamais recouvrir : en-tête de la vue et ligne « Voir tout ». */
const HEADER = ".inbox__header, .paged__header";
const ANCHORS = `${HEADER}, .inbox__more`;
/** Hauteur minimale sous l'en-tête pour y loger la carte (≈ 78 px) et une capture (≈ 72 px). */
const TIGHT_BELOW_HEADER = 175;

/**
 * Carte d'instruction de la visite : une ligne de message et ses boutons, flottante, hors du
 * flux (elle ne déplace rien) et jamais bloquante. Elle se pose juste sous l'en-tête de la vue,
 * donc sans cacher le filtre, la corbeille ni « Voir tout ». La région vivante reste montée
 * pour que chaque étape soit annoncée.
 */
export function InitiationGuide(props: InitiationGuideProps) {
  const { step, hasCreated, boxState } = props;
  const messageId = useId();
  const regionRef = useRef<HTMLDivElement>(null);
  const cardRef = useRef<HTMLDivElement>(null);
  const primaryRef = useRef<HTMLButtonElement>(null);

  // Position sous l'en-tête (qui peut passer à la ligne en fenêtre étroite). Écrite par
  // l'API du style, donc sans attribut `style` en ligne (la politique CSP l'interdirait).
  useLayoutEffect(() => {
    const region = regionRef.current;
    const stage = region?.parentElement;
    if (!region || !stage) return;
    const place = () => {
      const { top: stageTop, height: stageHeight } = stage.getBoundingClientRect();
      const header = stage.querySelector(HEADER);
      const headerBottom = header ? header.getBoundingClientRect().bottom - stageTop : 0;
      let bottom = 0;
      stage.querySelectorAll(ANCHORS).forEach((element) => {
        bottom = Math.max(bottom, element.getBoundingClientRect().bottom - stageTop);
      });
      // Boîte trop basse pour une carte ET une capture sous l'en-tête : la carte se pose alors
      // sur l'en-tête (filtre et corbeille, secondaires pendant la visite) et la liste n'a plus
      // de place à réserver. Décidé sur l'en-tête seul, donc stable quand « Voir tout » apparaît.
      const tight = headerBottom > 0 && stageHeight - headerBottom < TIGHT_BELOW_HEADER;
      region.style.top = bottom > 0 && !tight ? `${Math.round(bottom) + 4}px` : "";
      stage.style.setProperty("--initiation-reserve", tight ? "0px" : "5rem");
    };
    place();
    window.addEventListener("resize", place);
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(place) : null;
    observer?.observe(stage);
    stage.querySelectorAll(ANCHORS).forEach((element) => observer?.observe(element));
    return () => {
      window.removeEventListener("resize", place);
      observer?.disconnect();
      stage.style.removeProperty("--initiation-reserve");
    };
  }, [step, boxState]);

  // Quand le bouton cliqué disparaît, le focus ne doit pas se perdre : il passe à l'action
  // principale de la carte. Un focus ailleurs (le champ de capture) n'est jamais volé.
  useEffect(() => {
    if (step !== "box") return;
    const active = document.activeElement;
    if (!active || active === document.body || cardRef.current?.contains(active)) {
      primaryRef.current?.focus({ preventScroll: true });
    }
  }, [step]);

  function handleKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      event.stopPropagation();
      props.onStop();
    }
  }

  return (
    <div ref={regionRef} className="initiation-region" aria-live="polite">
      {step && (
        <div
          ref={cardRef}
          className="initiation-card"
          role="group"
          aria-labelledby={messageId}
          onKeyDown={handleKeyDown}
        >
          <p id={messageId} className="initiation-card__message">
            {message(step, hasCreated, boxState)}
          </p>
          <div className="initiation-card__actions">
            {step === "proposal" && (
              <>
                <button type="button" className="initiation-card__primary" onClick={props.onStart}>
                  Découvrir
                </button>
                <button type="button" onClick={props.onStop}>
                  Plus tard
                </button>
              </>
            )}
            {step === "capture" && (
              <button type="button" onClick={props.onStop}>
                Passer
              </button>
            )}
            {step === "box" && (
              <>
                {hasCreated && boxState === "other-view" && (
                  <button type="button" onClick={props.onShowBox}>
                    Voir la boîte
                  </button>
                )}
                {hasCreated && boxState === "filtered" && (
                  <button type="button" onClick={props.onShowAll}>
                    Tout afficher
                  </button>
                )}
                <button type="button" onClick={props.onBack}>
                  Précédent
                </button>
                <button type="button" className="initiation-card__primary" ref={primaryRef} onClick={props.onStop}>
                  Terminer
                </button>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

/** Une seule ligne par étape : assez courte pour rester lisible en fenêtre étroite. */
function message(step: InitiationStep, hasCreated: boolean, state: BoxState): string {
  switch (step) {
    case "proposal":
      return "Découvrir M'Organiser ? Deux étapes, rien n'est créé sans vous.";
    case "capture":
      return "1/2 · Écrivez dans le champ, puis Entrée.";
    case "box":
      if (!hasCreated) return "2/2 · Vos captures arrivent dans À organiser.";
      switch (state) {
        case "visible":
          return "2/2 · Votre capture est dans À organiser.";
        case "loading":
          return "2/2 · Mise à jour de la boîte…";
        case "unavailable":
          return "2/2 · La boîte n'a pas pu être relue.";
        case "other-view":
          return "2/2 · Vous n'êtes pas sur la boîte.";
        case "filtered":
          return "2/2 · Un filtre masque votre capture.";
        case "gone":
          return "2/2 · Capture déplacée ou supprimée.";
      }
  }
}
