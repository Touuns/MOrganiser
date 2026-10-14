import { useId, type Ref, type RefObject } from "react";
import { LeaveBanner } from "../../components/LeaveBanner";
import { Modal } from "../../components/Modal";
import type { Destination } from "../inbox/api";
import { TaskEditor } from "./TaskEditor";
import "./TaskDraftModal.css";

/** Avertissement affiché dans la modale quand on tente de la quitter avec un brouillon modifié. */
export interface DraftLeavePrompt {
  onDiscard: () => void;
  onKeep: () => void;
}

interface TaskDraftModalProps {
  destinations: Destination[];
  title: string;
  description: string;
  destinationId: string;
  onTitleChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onDestinationChange: (value: string) => void;
  suggestingTitle: boolean;
  /** Création en cours : champs et boutons figés, aucune double validation possible. */
  busy: boolean;
  /** Message de refus ou d'échec, sous les champs. Le brouillon est toujours conservé. */
  message: string | null;
  /** La création n'est plus possible (capture supprimée ou convertie ailleurs). */
  blocked: boolean;
  leave: DraftLeavePrompt | null;
  /** Créer la tâche. */
  onSubmit: () => void;
  /** ✕, « Abandonner », Échap : l'appelant protège le brouillon. */
  onRequestClose: () => void;
  titleRef: RefObject<HTMLInputElement | null>;
}

/**
 * Grande fiche modale de **création d'une tâche à partir d'une capture** (brouillon).
 *
 * Rien n'est écrit tant que « Créer la tâche » n'a pas abouti. Le composant ne garde aucun état :
 * le brouillon appartient à l'appelant, qui le protège (avertissement, fermeture de la fenêtre).
 */
export function TaskDraftModal(props: TaskDraftModalProps) {
  const { busy, blocked, message, title, description } = props;
  const headingId = useId();
  const leadId = useId();

  const titleBlank = title.trim() === "";
  const descriptionBlank = description.trim() === "";
  // Contrainte actuelle du schéma : une tâche ne peut pas avoir une description vide. On l'explique
  // plutôt que de remettre en silence le texte d'origine.
  const descriptionError = descriptionBlank
    ? "La description ne peut pas être vide pour le moment. Saisissez un texte pour créer la tâche."
    : null;

  return (
    <Modal
      labelledBy={headingId}
      describedBy={leadId}
      onRequestClose={props.onRequestClose}
      initialFocus={props.titleRef}
    >
      <header className="modal__header">
        <div className="task-draft__heading">
          <h2 id={headingId} className="task-draft__title">
            Nouvelle tâche
          </h2>
          <span className="task-draft__badge">Brouillon</span>
        </div>
        <button type="button" onClick={props.onRequestClose} disabled={busy} aria-label="Fermer sans créer la tâche">
          <span aria-hidden="true">✕</span>
        </button>
      </header>

      {props.leave && (
        <div className="task-draft__leave">
          <LeaveBanner
            message="Vous avez un brouillon de tâche non enregistré."
            discardLabel="Abandonner le brouillon"
            keepLabel="Continuer"
            onSave={() => undefined}
            onDiscard={props.leave.onDiscard}
            onKeep={props.leave.onKeep}
          />
        </div>
      )}

      <div className="modal__body">
        <p id={leadId} className="task-draft__lead">
          Rien n'est enregistré tant que vous n'avez pas créé la tâche. La capture d'origine reste
          intacte et retrouvable.
        </p>
        <TaskEditor
          title={title}
          description={description}
          destinationId={props.destinationId}
          destinations={props.destinations}
          onTitleChange={props.onTitleChange}
          onDescriptionChange={props.onDescriptionChange}
          onDestinationChange={props.onDestinationChange}
          onSubmit={props.onSubmit}
          readOnly={busy}
          suggestingTitle={props.suggestingTitle}
          descriptionError={descriptionError}
          titleRef={props.titleRef as Ref<HTMLInputElement>}
        />
        {message && (
          <p className="task-draft__message" role="alert">
            {message}
          </p>
        )}
      </div>

      <footer className="modal__footer">
        <button
          type="button"
          className="modal__primary"
          onClick={props.onSubmit}
          disabled={busy || blocked || titleBlank || descriptionBlank}
          aria-busy={busy}
        >
          {busy ? "Création…" : "Créer la tâche"}
        </button>
        <button type="button" onClick={props.onRequestClose} disabled={busy}>
          Abandonner
        </button>
        <span className="task-draft__hint">Ctrl+Entrée pour créer · Échap pour fermer</span>
      </footer>
    </Modal>
  );
}
