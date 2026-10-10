import { useId, useImperativeHandle, useRef, useState, type FormEvent, type KeyboardEvent, type Ref } from "react";
import type { Destination } from "./api";
import { DestinationOptions } from "./DestinationOptions";
import "./CaptureForm.css";

export interface CaptureFormHandle {
  /** Du texte saisi n'a pas encore été envoyé. */
  hasUnsent: () => boolean;
  /** Empreinte du texte et de la destination actuels : sert à reconnaître un abandon. */
  draftKey: () => string;
  /** Bord supérieur de la zone de saisie (repère de la fenêtre), ou `null` si elle n'est pas visible. */
  inputTop: () => number | null;
  /** Replace le curseur dans le champ. */
  focus: () => void;
  /** Envoie la saisie ; `true` si elle est enregistrée (ou s'il n'y avait rien à envoyer). */
  submit: () => Promise<boolean>;
}

interface CaptureFormProps {
  ref?: Ref<CaptureFormHandle>;
  destinations: Destination[];
  /** Enregistre la capture ; rejette en cas d'échec (le texte est alors conservé). */
  onSubmit: (content: string, destinationId: string | null) => Promise<void>;
  /** Appelé après un envoi réussi, une fois le champ vidé (s'il n'a pas été modifié entre-temps). */
  onSent?: () => void;
}

/**
 * Capture rapide : Entrée envoie, Maj+Entrée va à la ligne.
 * Le champ n'est vidé qu'après un enregistrement confirmé.
 */
export function CaptureForm(props: CaptureFormProps) {
  const { destinations, onSubmit } = props;
  const [text, setTextState] = useState("");
  const [destinationId, setDestinationState] = useState("");
  // Valeurs immédiates : lisibles par une demande de fermeture ou une réponse tardive avant le
  // prochain rendu (sinon un texte déjà envoyé paraîtrait encore « non envoyé »).
  const draft = useRef({ text: "", destinationId: "" });
  function setText(next: string) {
    draft.current.text = next;
    setTextState(next);
  }
  function setDestinationId(next: string) {
    draft.current.destinationId = next;
    setDestinationState(next);
  }
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Verrou synchrone : bloque un second envoi avant même le prochain rendu.
  const inFlight = useRef(false);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const fieldId = useId();
  const destinationFieldId = useId();
  const errorId = useId();

  // Un envoi en cours : toute nouvelle demande (ex. « Envoyer » à la fermeture) l'attend
  // au lieu d'échouer ou de créer un doublon.
  const running = useRef<Promise<boolean> | null>(null);

  function submit(): Promise<boolean> {
    if (running.current) return running.current;
    const attempt = send().finally(() => {
      running.current = null;
    });
    running.current = attempt;
    return attempt;
  }

  async function send(): Promise<boolean> {
    if (inFlight.current) return false;
    const sentText = draft.current.text;
    const sentDestination = draft.current.destinationId;
    if (sentText.trim() === "") return true;

    inFlight.current = true;
    setPending(true);
    setError(null);
    try {
      await onSubmit(sentText, sentDestination === "" ? null : sentDestination);
      // Ne vider que si rien n'a été modifié pendant l'envoi : une saisie faite
      // entre-temps n'est jamais perdue.
      if (draft.current.text === sentText) setText("");
      if (draft.current.destinationId === sentDestination) setDestinationId("");
      textareaRef.current?.focus({ preventScroll: true });
      props.onSent?.();
      return true;
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : "L'enregistrement a échoué.";
      setError(`${message} Votre texte est conservé : réessayez avec Entrée ou « Envoyer ».`);
      return false;
    } finally {
      inFlight.current = false;
      setPending(false);
    }
  }

  useImperativeHandle(props.ref, () => ({
    hasUnsent: () => draft.current.text.trim() !== "",
    draftKey: () => `${draft.current.text}\u0000${draft.current.destinationId}`,
    inputTop: () => {
      const rect = textareaRef.current?.getBoundingClientRect();
      return rect && rect.height > 0 ? rect.top : null;
    },
    focus: () => textareaRef.current?.focus({ preventScroll: true }),
    submit,
  }));

  function handleKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    // Pendant la composition d'un caractère (accents via IME), Entrée ne valide pas.
    if (event.key === "Enter" && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault();
      void submit();
    }
  }

  function handleSubmit(event: FormEvent) {
    event.preventDefault();
    void submit();
  }

  return (
    <form className="capture" onSubmit={handleSubmit} aria-label="Capture rapide">
      <label className="capture__label" htmlFor={fieldId}>
        Capture rapide
      </label>
      <textarea
        id={fieldId}
        ref={textareaRef}
        className="capture__field"
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="Écrire une idée, une obligation ou une note…"
        rows={2}
        maxLength={10000}
        autoFocus
        aria-describedby={error ? errorId : undefined}
        aria-invalid={error ? true : undefined}
      />
      <div className="capture__row">
        <label className="capture__destination" htmlFor={destinationFieldId}>
          <span>Destination (facultatif)</span>
          <select
            id={destinationFieldId}
            value={destinationId}
            onChange={(event) => setDestinationId(event.target.value)}
          >
            <option value="">Aucune</option>
            <DestinationOptions destinations={destinations} />
          </select>
        </label>
        <span className="capture__hint" aria-hidden="true">
          Entrée pour envoyer · Maj+Entrée pour une nouvelle ligne
        </span>
        <button type="submit" className="capture__send" disabled={pending} aria-busy={pending}>
          {pending ? "Envoi…" : "Envoyer ↑"}
        </button>
      </div>
      {error && (
        <p id={errorId} className="capture__error" role="alert">
          {error}
        </p>
      )}
    </form>
  );
}
