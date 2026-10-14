import { useId, type KeyboardEvent, type Ref } from "react";
import type { Destination } from "../inbox/api";
import { DestinationOptions } from "../inbox/DestinationOptions";
import "./TaskEditor.css";

export const TITLE_MAX_CHARS = 120;
export const DESCRIPTION_MAX_CHARS = 10_000;

interface TaskEditorProps {
  title: string;
  description: string;
  /** Identifiant de la destination, ou `""` pour « Aucun classement ». */
  destinationId: string;
  destinations: Destination[];
  onTitleChange: (value: string) => void;
  onDescriptionChange: (value: string) => void;
  onDestinationChange: (value: string) => void;
  /** Valider (Entrée dans le titre, Ctrl+Entrée partout). L'appelant revalide. */
  onSubmit: () => void;
  /** Les champs ne sont plus modifiables (opération en cours). */
  readOnly?: boolean;
  /** Le titre proposé n'est pas encore arrivé. */
  suggestingTitle?: boolean;
  /** Message sous le titre / sous la description (association accessible aux champs). */
  titleError?: string | null;
  descriptionError?: string | null;
  titleRef?: Ref<HTMLInputElement>;
}

/**
 * Champs d'une fiche de tâche : titre, description, classement. Composant **contrôlé** : il ne
 * possède ni brouillon ni appel de données, pour servir aussi bien à la création (brouillon issu
 * d'une capture) qu'à l'édition d'une tâche existante.
 *
 * Seuls les champs qui existent réellement dans le modèle actuel sont présentés : les dates, le
 * statut et les champs complémentaires viendront avec les sous-briques suivantes.
 */
export function TaskEditor(props: TaskEditorProps) {
  const {
    title,
    description,
    destinationId,
    destinations,
    readOnly = false,
    suggestingTitle = false,
    titleError = null,
    descriptionError = null,
  } = props;
  const titleId = useId();
  const titleErrorId = useId();
  const descriptionId = useId();
  const descriptionErrorId = useId();
  const destinationFieldId = useId();

  /** Ctrl+Entrée (ou Cmd+Entrée) valide depuis n'importe quel champ, comme dans la fiche capture. */
  function handleKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && !event.nativeEvent.isComposing) {
      event.preventDefault();
      props.onSubmit();
    }
  }

  function handleTitleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "Enter" && !event.ctrlKey && !event.metaKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      props.onSubmit();
    }
  }

  const unavailable =
    destinationId !== "" && !destinations.some((destination) => destination.id === destinationId);

  return (
    <div className="task-editor" onKeyDown={handleKeyDown}>
      <div className="task-editor__field">
        <label htmlFor={titleId}>Titre de la tâche</label>
        <input
          id={titleId}
          ref={props.titleRef}
          type="text"
          value={title}
          onChange={(event) => props.onTitleChange(event.target.value)}
          onKeyDown={handleTitleKeyDown}
          readOnly={readOnly}
          maxLength={TITLE_MAX_CHARS}
          placeholder={suggestingTitle ? "Titre proposé en cours de calcul…" : ""}
          aria-describedby={titleError ? titleErrorId : undefined}
          aria-invalid={titleError ? true : undefined}
        />
        {titleError && (
          <p id={titleErrorId} className="task-editor__error">
            {titleError}
          </p>
        )}
      </div>

      <div className="task-editor__field">
        <label htmlFor={descriptionId}>Description</label>
        <textarea
          id={descriptionId}
          value={description}
          onChange={(event) => props.onDescriptionChange(event.target.value)}
          readOnly={readOnly}
          maxLength={DESCRIPTION_MAX_CHARS}
          rows={6}
          aria-describedby={descriptionError ? descriptionErrorId : undefined}
          aria-invalid={descriptionError ? true : undefined}
        />
        {descriptionError && (
          <p id={descriptionErrorId} className="task-editor__error">
            {descriptionError}
          </p>
        )}
      </div>

      <div className="task-editor__field">
        <label htmlFor={destinationFieldId}>Classement (facultatif)</label>
        <select
          id={destinationFieldId}
          value={destinationId}
          onChange={(event) => props.onDestinationChange(event.target.value)}
          disabled={readOnly}
        >
          <option value="">Aucun</option>
          <DestinationOptions destinations={destinations} />
          {unavailable && <option value={destinationId}>{destinationId} (indisponible)</option>}
        </select>
      </div>
    </div>
  );
}
