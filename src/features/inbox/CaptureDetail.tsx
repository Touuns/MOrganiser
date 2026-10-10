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

/** Issue d'une demande de départ : rien à perdre, brouillon enregistré, ou abandonné. */
export type LeaveOutcome = "clean" | "saved" | "discarded";

export interface CaptureDetailHandle {
  /**
   * Un brouillon diverge de la version enregistrée. Indépendant du droit d'enregistrer :
   * un texte reste protégé même si la capture a été supprimée ailleurs.
   */
  hasUnsaved: () => boolean;
  /**
   * Demande à quitter la fiche (autre capture, autre vue, fermeture de la fenêtre) :
   * `proceed` est appelé tout de suite s'il n'y a rien à perdre, sinon après le choix de
   * l'utilisateur ; `cancel` s'il choisit de continuer à modifier.
   */
  requestLeave: (proceed: (outcome: LeaveOutcome) => void, cancel?: () => void) => void;
  /** Relit l'état de la capture (ex. restaurée depuis la liste) sans toucher au brouillon. */
  refresh: () => Promise<void>;
}

interface CaptureDetailProps {
  item: InboxItem;
  destinations: Destination[];
  onClose: () => void;
  onSaved: (item: InboxItem) => void;
  /** `closeSheet` : la fiche d'origine est toujours ouverte, sans saisie récente : elle peut se fermer. */
  onTrashed: (item: InboxItem, closeSheet: boolean) => void;
  onRestored: (item: InboxItem) => void;
  /** Déclare une opération d'écriture en cours (la fermeture de la fenêtre l'attend). */
  track: <T>(operation: Promise<T>) => Promise<T>;
  ref?: Ref<CaptureDetailHandle>;
}

type Busy = "save" | "trash" | "restore" | null;
interface PendingLeave {
  proceed: (outcome: LeaveOutcome) => void;
  cancel?: () => void;
}

/**
 * Fiche d'une capture : texte et destination directement modifiables.
 *
 * Trois notions distinctes : la **référence enregistrée** (`baseline`), le **brouillon**
 * (`text`, `destinationId`) et les **opérations en cours** (`inFlight`). Une réponse
 * tardive met à jour la référence ; elle ne touche au brouillon que s'il n'a pas changé
 * depuis l'envoi. À remonter avec `key={item.id}` pour changer de capture.
 */
export function CaptureDetail(props: CaptureDetailProps) {
  const { item, destinations, onClose, onSaved, onTrashed, onRestored, track } = props;
  const [baseline, setBaselineState] = useState(item);
  const [text, setTextState] = useState(item.content);
  const [destinationId, setDestinationState] = useState(item.destinationId ?? "");
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [goneState, setGoneState] = useState(false);
  const [leave, setLeaveState] = useState<PendingLeave | null>(null);
  const inFlight = useRef(false);
  const mounted = useRef(true);
  // Modèle « source de vérité immédiate » : mis à jour au même instant que l'état React, donc
  // lisible par une réponse tardive ou une demande de fermeture avant le prochain rendu.
  const model = useRef({
    baseline: item,
    text: item.content,
    destinationId: item.destinationId ?? "",
    gone: false,
    leave: null as PendingLeave | null,
  });
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const headingId = useId();
  const textId = useId();
  const destinationFieldId = useId();
  const errorId = useId();

  function setBaseline(next: InboxItem) {
    model.current.baseline = next;
    setBaselineState(next);
  }
  function setDraft(nextText: string, nextDestination: string) {
    model.current.text = nextText;
    model.current.destinationId = nextDestination;
    setTextState(nextText);
    setDestinationState(nextDestination);
  }
  function setGone(next: boolean) {
    model.current.gone = next;
    setGoneState(next);
  }
  function setLeave(next: PendingLeave | null) {
    model.current.leave = next;
    setLeaveState(next);
  }
  /** Le brouillon diverge de la référence enregistrée (état immédiat, pas celui du rendu). */
  const isDirty = () =>
    model.current.text !== model.current.baseline.content ||
    model.current.destinationId !== (model.current.baseline.destinationId ?? "");

  const trashed = baseline.deletedAt !== null;
  const dirty = text !== baseline.content || destinationId !== (baseline.destinationId ?? "");
  const editable = !trashed && !goneState;

  useImperativeHandle(props.ref, () => ({
    hasUnsaved: () => isDirty(),
    requestLeave(proceed, cancel) {
      if (isDirty()) setLeave({ proceed, cancel });
      else proceed("clean");
    },
    refresh: async () => {
      const fresh = await refreshBaseline();
      if (fresh) setGone(false);
    },
  }));

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  // À l'ouverture : le texte (modifiable) ou le titre (corbeille, lecture seule).
  useEffect(() => {
    (trashed ? headingRef.current : textareaRef.current)?.focus();
    // Une seule fois par capture ouverte (la fiche est remontée avec `key={item.id}`).
  }, []);

  async function refreshBaseline() {
    try {
      const fresh = await getInboxItem(model.current.baseline.id);
      setBaseline(fresh);
      return fresh;
    } catch {
      return null;
    }
  }

  /** Le départ en attente a lieu une seule fois, quand le brouillon est sûr. */
  function finishLeave(outcome: LeaveOutcome) {
    const pending = model.current.leave;
    if (!pending) return;
    setLeave(null);
    pending.proceed(outcome);
  }

  function dismissLeave() {
    const pending = model.current.leave;
    setLeave(null);
    pending?.cancel?.();
  }

  /** `true` si plus rien n'est à perdre (enregistré, ou rien à enregistrer). */
  async function save(): Promise<boolean> {
    if (inFlight.current) return false;
    const current = model.current;
    if (current.baseline.deletedAt !== null || current.gone) {
      setNotice(
        current.gone
          ? "Cette capture n'existe plus : vos modifications ne peuvent pas être enregistrées."
          : "Cette capture est dans la corbeille : restaurez-la pour enregistrer vos modifications.",
      );
      return false;
    }
    if (!isDirty()) return true;
    if (current.text.trim() === "") {
      setError("Le texte ne peut pas être vide.");
      return false;
    }
    const sent = { text: current.text, destinationId: current.destinationId };
    const version = current.baseline;
    inFlight.current = true;
    setBusy("save");
    setError(null);
    setNotice(null);
    let safe = false;
    try {
      const saved = await track(
        updateInboxItem(
          version.id,
          sent.text,
          sent.destinationId === "" ? null : sent.destinationId,
          version.updatedAt,
        ),
      );
      // La référence passe à la version enregistrée. Le brouillon n'est remplacé que s'il
      // n'a pas changé depuis l'envoi : une saisie faite pendant l'attente reste visible.
      const now = model.current;
      setBaseline(saved);
      setDraft(
        now.text === sent.text ? saved.content : now.text,
        now.destinationId === sent.destinationId ? (saved.destinationId ?? "") : now.destinationId,
      );
      safe = !isDirty();
      onSaved(saved);
    } catch (cause) {
      await explainSaveFailure(cause);
    } finally {
      inFlight.current = false;
      setBusy(null);
    }
    // Après la libération du verrou : l'action enchaînée (ex. corbeille) peut s'exécuter.
    if (safe) finishLeave("saved");
    return safe;
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
    if (inFlight.current || model.current.baseline.deletedAt !== null || model.current.gone) return;
    inFlight.current = true;
    setBusy("trash");
    setError(null);
    // Brouillon au départ : seule une saisie faite PENDANT l'attente est à conserver
    // (un brouillon explicitement abandonné avant ne l'est pas).
    const start = { text: model.current.text, destinationId: model.current.destinationId };
    try {
      const trashedItem = await track(trashInboxItem(model.current.baseline.id));
      setBaseline(trashedItem);
      const now = model.current;
      const typedMeanwhile = now.text !== start.text || now.destinationId !== start.destinationId;
      if (typedMeanwhile) {
        setNotice("Cette capture a été mise à la corbeille. Votre nouveau texte est conservé.");
      }
      // La fiche ne se ferme que si elle est toujours là et sans saisie récente.
      onTrashed(trashedItem, mounted.current && !typedMeanwhile);
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
      const restored = await track(restoreInboxItem(model.current.baseline.id));
      setBaseline(restored); // le brouillon éventuel reste tel quel
      setGone(false);
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
    dismissLeave(); // l'utilisateur reprend la main : tout départ en attente est annulé
    setDraft(model.current.baseline.content, model.current.baseline.destinationId ?? "");
    setError(null);
    setNotice(null);
  }

  function askLeave(proceed: (outcome: LeaveOutcome) => void) {
    if (isDirty()) setLeave({ proceed });
    else proceed("clean");
  }

  function decide(choice: "save" | "discard" | "keep") {
    if (!model.current.leave) return;
    if (choice === "keep") {
      dismissLeave();
      // Le bouton disparaît : on rend le focus au texte, pour poursuivre au clavier.
      textareaRef.current?.focus();
    } else if (choice === "discard") {
      finishLeave("discarded");
    } else {
      // Enregistrement déjà en cours : `save()` répond non et le départ aura lieu à sa fin,
      // si aucun nouveau brouillon ne subsiste.
      void save();
    }
  }

  function handleKeyDown(event: KeyboardEvent<HTMLElement>) {
    if (event.key === "Escape") {
      event.stopPropagation();
      if (model.current.leave) dismissLeave();
      else askLeave(onClose);
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
          onClick={() => askLeave(onClose)}
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
          onChange={(event) => setDraft(event.target.value, model.current.destinationId)}
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
          onChange={(event) => setDraft(model.current.text, event.target.value)}
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
          <>
            <button type="button" className="detail__primary" onClick={() => void restore()} disabled={busy !== null}>
              {busy === "restore" ? "Restauration…" : "Restaurer"}
            </button>
            {dirty && (
              <button type="button" onClick={revert} disabled={busy !== null}>
                Annuler les modifications
              </button>
            )}
          </>
        ) : (
          <>
            <button
              type="button"
              className="detail__primary"
              onClick={() => void save()}
              disabled={!dirty || busy !== null || text.trim() === "" || goneState}
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
              onClick={() => askLeave(() => void trash())}
              disabled={busy !== null || goneState}
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
