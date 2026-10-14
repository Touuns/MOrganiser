import { useEffect, useRef, type KeyboardEvent, type ReactNode, type RefObject } from "react";
import "./Modal.css";

interface ModalProps {
  /** Identifiant de l'élément qui titre la fenêtre (nom accessible). */
  labelledBy: string;
  /** Identifiant d'un texte qui décrit la fenêtre (facultatif). */
  describedBy?: string;
  /**
   * Demande de fermeture : Échap, ou demande du système. Jamais appelée par un clic sur l'arrière-plan
   * (un clic accidentel ne doit rien abandonner). C'est à l'appelant de décider s'il faut protéger
   * un brouillon avant de démonter la modale.
   */
  onRequestClose: () => void;
  /** Élément à focaliser à l'ouverture (sinon le premier élément focalisable). */
  initialFocus?: RefObject<HTMLElement | null>;
  children: ReactNode;
}

/**
 * Grande fenêtre modale générique, indépendante du domaine (captures, tâches, fiches futures).
 *
 * S'appuie sur l'élément natif `<dialog>` ouvert par `showModal()` : couche supérieure (donc ni
 * confinée par un `container-type` ni rognée par un `overflow` parent), reste de la page inerte,
 * piège à focus natif, `aria-modal` implicite. Le focus revient à l'élément qui l'a ouverte quand
 * elle disparaît.
 *
 * Structure fournie par les classes `modal__header`, `modal__body`, `modal__footer` (voir le CSS).
 */
export function Modal({ labelledBy, describedBy, onRequestClose, initialFocus, children }: ModalProps) {
  const ref = useRef<HTMLDialogElement>(null);
  // Toujours la dernière fonction, sans rouvrir la fenêtre à chaque rendu.
  const latest = useRef({ onRequestClose, initialFocus });
  latest.current = { onRequestClose, initialFocus };
  // Échap est traité à la touche ET à l'événement `cancel` (autres demandes de fermeture du
  // système) : ne jamais déclencher deux fois pour la même demande.
  const escapedAt = useRef(0);

  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    const opener = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    // Double montage (StrictMode) : ne jamais rouvrir une fenêtre déjà modale.
    if (!dialog.open) dialog.showModal();
    latest.current.initialFocus?.current?.focus({ preventScroll: true });
    return () => {
      if (dialog.open) dialog.close();
      // Focus perdu avec la modale : retour à l'élément d'origine, s'il existe encore.
      const lost = document.activeElement === document.body || dialog.contains(document.activeElement);
      if (opener?.isConnected && lost) opener.focus({ preventScroll: true });
    };
  }, []);

  function handleKeyDown(event: KeyboardEvent<HTMLDialogElement>) {
    if (event.key === "Escape" && !event.nativeEvent.isComposing) {
      // Ni fermeture native, ni la fiche située dessous (les événements React suivent l'arbre).
      event.preventDefault();
      event.stopPropagation();
      escapedAt.current = Date.now();
      latest.current.onRequestClose();
    }
  }

  return (
    <dialog
      ref={ref}
      className="modal"
      aria-labelledby={labelledBy}
      aria-describedby={describedBy}
      onKeyDown={handleKeyDown}
      onCancel={(event) => {
        // Le navigateur ne ferme jamais seul : la décision revient à l'appelant.
        event.preventDefault();
        if (Date.now() - escapedAt.current < 100) return; // déjà traité par la touche
        latest.current.onRequestClose();
      }}
    >
      {children}
    </dialog>
  );
}
