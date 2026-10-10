import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { detectFirstUse } from "./firstUse";
import { markProposalShown, readProposalMemory } from "./initiationStore";

/** proposition d'accueil → champ de capture → boîte et capture créée (fin par « Terminer »). */
export type InitiationStep = "proposal" | "capture" | "box";

export interface InitiationController {
  /** Étape affichée, `null` si aucune visite n'est en cours. */
  step: InitiationStep | null;
  /** Identifiant de la capture réellement créée pendant la visite. */
  createdId: string | null;
  /** Lance (ou relance) la visite, à tout moment. */
  start: () => void;
  /** « Plus tard », « Passer » ou « Terminer » : ferme la visite sans autre effet. */
  stop: () => void;
  back: () => void;
  /** Une capture vient d'être **enregistrée avec succès** (jamais un envoi vide ou échoué). */
  captured: (id: string) => void;
}

interface State {
  step: InitiationStep | null;
  createdId: string | null;
}

/**
 * Visite d'initiation minimale (001-E). Un seul parcours, sans moteur : la reprise et les
 * parcours thématiques relèvent de la brique 007.
 *
 * `enabled` : faux hors de l'application Windows (aperçu navigateur), où il n'y a rien à lire.
 */
export function useInitiation(enabled: boolean): InitiationController {
  const [state, setState] = useState<State>({ step: null, createdId: null });
  // Vrai dès que l'utilisateur a lancé une visite ou capturé : la proposition automatique,
  // encore en attente de la lecture de la base, ne doit alors plus s'afficher.
  const touched = useRef(false);

  useEffect(() => {
    if (!enabled) return;
    let active = true;
    void (async () => {
      if (readProposalMemory() !== "unseen") return;
      const use = await detectFirstUse();
      if (!active || use === "unknown") return; // erreur de lecture : jamais « base vide »
      // Mémorisé AVANT l'affichage : une fermeture sans réponse ne la reproposera pas.
      const remembered = markProposalShown();
      if (use === "first" && remembered && !touched.current) {
        setState((current) => (current.step === null ? { step: "proposal", createdId: null } : current));
      }
    })();
    return () => {
      active = false;
    };
  }, [enabled]);

  const start = useCallback(() => {
    touched.current = true;
    setState({ step: "capture", createdId: null });
  }, []);
  const stop = useCallback(() => setState({ step: null, createdId: null }), []);
  const back = useCallback(
    () => setState((s) => (s.step === "box" ? { ...s, step: "capture" } : s)),
    [],
  );
  const captured = useCallback((id: string) => {
    touched.current = true;
    setState((s) => (s.step === "capture" || s.step === "box" ? { step: "box", createdId: id } : s));
  }, []);

  return useMemo(
    () => ({ ...state, start, stop, back, captured }),
    [state, start, stop, back, captured],
  );
}
