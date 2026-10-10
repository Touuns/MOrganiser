import {
  useEffect,
  useId,
  useImperativeHandle,
  useRef,
  useState,
  type KeyboardEvent,
  type Ref,
} from "react";
import {
  getInboxItem,
  InboxApiError,
  restoreInboxItem,
  trashInboxItem,
  updateInboxItem,
  type Destination,
  type InboxItem,
} from "./api";
import { LeaveBanner } from "../../components/LeaveBanner";
import { DestinationOptions } from "./DestinationOptions";
import { formatLong } from "./format";
import "./CaptureDetail.css";

export interface CaptureDetailHandle {
  /** Des modifications non enregistrées existent (et la capture est modifiable). */
  hasUnsaved: () => boolean;
  /**
   * Demande à quitter la fiche (ou à changer de capture) : exécute `proceed` tout de suite
   * s'il n'y a rien à perdre, sinon propose d'enregistrer, d'abandonner ou de continuer.
   */
  requestLeave: (proceed: () => void) => void;
}

interface CaptureDetailProps {
  item: InboxItem;
  destinations: Destination[];
  onClose: () => void;
  onSaved: (item: InboxItem) => void;
  onTrashed: (item: InboxItem) => void;
  onRestored: (item: InboxItem) => void;
  ref?: Ref<CaptureDetailHandle>;
}

type Busy = "save" | "trash" | "restore" | null;

/**
 * Fiche d'une capture : texte et destination directement modifiables. Le brouillon n'est
 * jamais perdu silencieusement : échec d'écriture, conflit ou fermeture le conservent.
 * À remonter avec `key={item.id}` pour changer de capture.
 */
export function CaptureDetail(props: CaptureDetailProps) {
  const { item, destinations, onClose, onSaved, onTrashed, onRestored } = props;
  const [baseline, setBaseline] = useState(item);
  const [text, setText] = useState(item.content);
  const [destinationId, setDestinationId] = useState(item.destinationId ?? "");
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [gone, setGone] = useState(false);
  const [leave, setLeave] = useState<{ proceed: () => void } | null>(null);
  const inFlight = useRef(false);
  // Départ en attente (fermeture, autre capture) : exécuté une seule fois, dès que le brouillon est sûr.
  const leaveRef = useRef<{ proceed: () => void } | null>(null);
  leaveRef.current = leave;
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const headingId = useId();
  const textId = useId();
  const destinationFieldId = useId();
  const errorId = useId();

  const trashed = baseline.deletedAt !== null;
  const dirty = text !== baseline.content || destinationId !== (baseline.destinationId ?? "");
  const editable = !trashed && !gone;

  useImperativeHandle(props.ref, () => ({
    hasUnsaved: () => dirty && editable,
    requestLeave(proceed) {
      if (dirty) setLeave({ proceed });
      else proceed();
    },
  }));

  // À l'ouverture : le texte (modifiable) ou le titre (corbeille, lecture seule).
  useEffect(() => {
    (trashed ? headingRef.current : textareaRef.current)?.focus();
    // Une seule fois par capture ouverte (la fiche est remontée avec `key={item.id}`).
  }, []);

  async function refreshBaseline() {
    try {
      const latest = await getInboxItem(baseline.id);
      setBaseline(latest);
      return latest;
    } catch {
      return null;
    }
  }

  async function save(): Promise<boolean> {
    if (inFlight.current || !editable) return false;
    if (!dirty) return true;
    if (text.trim() === "") {
      setError("Le texte ne peut pas être vide.");
      return false;
    }
    inFlight.current = true;
    setBusy("save");
    setError(null);
    setNotice(null);
    try {
      const saved = await updateInboxItem(
        baseline.id,
        text,
        destinationId === "" ? null : destinationId,
        baseline.updatedAt,
      );
      setBaseline(saved);
      setText(saved.content);
      setDestinationId(saved.destinationId ?? "");
      onSaved(saved);
      finishLeave();
      return true;
    } catch (cause) {
      await explainSaveFailure(cause);
      return false;
    } finally {
      inFlight.current = false;
      setBusy(null);
    }
  }

  /** Le brouillon est enregistré : le départ demandé peut avoir lieu (une seule fois). */
  function finishLeave() {
    const pending = leaveRef.current;
    if (!pending) return;
    leaveRef.current = null;
    setLeave(null);
    pending.proceed();
  }

  async function explainSaveFailure(cause: unknown) {
    const code = cause instanceof InboxApiError ? cause.code : "unknown";
    if (code === "version_conflict") {
      // On prend la version actuelle comme nouvelle référence, brouillon conservé :
      // un nouvel « Enregistrer » est un choix explicite de remplacer cette version.
      await refreshBaseline();
      setNotice(
        "Cette capture a changé depuis son ouverture. Votre brouillon est conservé : " +
          "enregistrer de nouveau remplacera la version actuelle.",
      );
    } else if (code === "trashed") {
      await refreshBaseline();
      setNotice("Cette capture a été mise à la corbeille. Votre brouillon est conservé.");
    } else if (code === "not_found") {
      setGone(true);
      setError("Cette capture n'existe plus. Votre texte est conservé ci-dessous.");
    } else {
      const message = cause instanceof Error ? cause.message : "L'enregistrement a échoué.";
      setError(`${message} Votre brouillon est conservé : réessayez.`);
    }
  }

  async function trash() {
    if (inFlight.current || !editable) return;
    inFlight.current = true;
    setBusy("trash");
    setError(null);
    try {
      onTrashed(await trashInboxItem(baseline.id));
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : "La mise à la corbeille a échoué.";
      setError(`${message} La capture n'a pas été déplacée.`);
    } finally {
      inFlight.current = false;
      setBusy(null);
    }
  }

  async function restore() {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy("restore");
    setError(null);
    try {
      const restored = await restoreInboxItem(baseline.id);
      setBaseline(restored);
      onRestored(restored);
    } catch (cause) {
      const message = cause instanceof Error ? cause.message : "La restauration a échoué.";
      setError(`${message} La capture est toujours dans la corbeille.`);
    } finally {
      inFlight.current = false;
      setBusy(null);
    }
  }

  function revert() {
    setText(baseline.content);
    setDestinationId(baseline.destinationId ?? "");
    setError(null);
    setNotice(null);
  }

  function decide(choice: "save" | "discard" | "keep") {
    const pending = leave;
    if (!pending) return;
    if (choice === "keep") {
      setLeave(null);
      // Le bouton disparaît : on rend le focus au texte, pour poursuivre au clavier.
      textareaRef.current?.focus();
    } else if (choice === "discard") {
      setLeave(null);
      pending.proceed();
    } else {
      // Si un enregistrement est déjà en cours, `save()` répond non : `finishLeave`
      // s'exécutera à sa fin. Sinon on enchaîne ici (idempotent).
      void save().then((ok) => {
        if (ok) finishLeave();
      });
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      event.stopPropagation();
      if (leave) setLeave(null);
      else if (dirty) setLeave({ proceed: onClose });
      else onClose();
    }
  }

  function handleTextKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && !event.nativeEvent.isComposing) {
      event.preventDefault();
      void save();
    }
  }

  const message = error ?? notice;

  return (
    <section
      className="detail"
      aria-labelledby={headingId}
      onKeyDown={handleKeyDown}
    >
      <header className="detail__header">
        <h2 id={headingId} className="detail__title" tabIndex={-1} ref={headingRef}>
          {trashed ? "Capture dans la corbeille" : "Fiche de la capture"}
        </h2>
        <button
          type="button"
          className="detail__close"
          onClick={() => (dirty ? setLeave({ proceed: onClose }) : onClose())}
        >
          <span className="detail__close-wide" aria-hidden="true">
            ✕
          </span>
          <span className="detail__close-narrow" aria-hidden="true">
            ← Retour
          </span>
          <span className="detail__sr">Fermer la fiche</span>
        </button>
      </header>

      {leave && (
        <LeaveBanner
          message="Vous avez des modifications non enregistrées."
          saveLabel="Enregistrer"
          discardLabel="Abandonner les modifications"
          keepLabel="Continuer à modifier"
          saving={busy === "save"}
          onSave={() => decide("save")}
          onDiscard={() => decide("discard")}
          onKeep={() => decide("keep")}
        />
      )}

      {trashed && (
        <p className="detail__banner">
          Dans la corbeille depuis le {formatLong(baseline.deletedAt!)}. Elle ne s'affiche plus
          dans « À organiser ».
        </p>
      )}

      <div className="detail__field">
        <label htmlFor={textId}>Texte</label>
        <textarea
          id={textId}
          ref={textareaRef}
          value={text}
          onChange={(event) => setText(event.target.value)}
          onKeyDown={handleTextKeyDown}
          readOnly={!editable}
          maxLength={10000}
          rows={6}
          aria-describedby={message ? errorId : undefined}
          aria-invalid={error ? true : undefined}
        />
      </div>

      <div className="detail__field">
        <label htmlFor={destinationFieldId}>Destination (facultatif)</label>
        <select
          id={destinationFieldId}
          value={destinationId}
          onChange={(event) => setDestinationId(event.target.value)}
          disabled={!editable}
        >
          <option value="">Aucune</option>
          <DestinationOptions destinations={destinations} />
          {destinationId !== "" && !destinations.some((d) => d.id === destinationId) && (
            <option value={destinationId}>{destinationId} (indisponible)</option>
          )}
        </select>
      </div>

      <dl className="detail__dates">
        <dt>Créée</dt>
        <dd>
          <time dateTime={new Date(baseline.createdAt).toISOString()}>{formatLong(baseline.createdAt)}</time>
        </dd>
        <dt>Dernière modification</dt>
        <dd>
          {baseline.updatedAt === baseline.createdAt ? (
            "Jamais modifiée"
          ) : (
            <time dateTime={new Date(baseline.updatedAt).toISOString()}>{formatLong(baseline.updatedAt)}</time>
          )}
        </dd>
      </dl>

      {message && (
        <p id={errorId} className={error ? "detail__error" : "detail__notice"} role="alert">
          {message}
        </p>
      )}

      <div className="detail__actions">
        {trashed ? (
          <button type="button" className="detail__primary" onClick={() => void restore()} disabled={busy !== null}>
            {busy === "restore" ? "Restauration…" : "Restaurer"}
          </button>
        ) : (
          <>
            <button
              type="button"
              className="detail__primary"
              onClick={() => void save()}
              disabled={!dirty || busy !== null || text.trim() === "" || gone}
              aria-busy={busy === "save"}
            >
              {busy === "save" ? "Enregistrement…" : "Enregistrer"}
            </button>
            <button type="button" onClick={revert} disabled={!dirty || busy !== null}>
              Annuler les modifications
            </button>
            <button
              type="button"
              className="detail__danger"
              onClick={() => (dirty ? setLeave({ proceed: () => void trash() }) : void trash())}
              disabled={busy !== null || gone}
            >
              Mettre à la corbeille
            </button>
          </>
        )}
      </div>
      {!trashed && <p className="detail__hint">Ctrl+Entrée pour enregistrer · Échap pour fermer</p>}
    </section>
  );
}
