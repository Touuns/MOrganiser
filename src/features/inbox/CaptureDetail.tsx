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
  cancelTaskConversion,
  convertInboxItemToTask,
  getInboxItem,
  InboxApiError,
  restoreInboxItem,
  suggestTaskTitle,
  trashInboxItem,
  updateInboxItem,
  type Conversion,
  type Destination,
  type DestinationChoice,
  type InboxItem,
} from "./api";
import { LeaveBanner } from "../../components/LeaveBanner";
import { TaskDraftModal } from "../tasks/TaskDraftModal";
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
  /** Empreinte du brouillon actuel (capture + texte + destination) : sert à reconnaître un abandon. */
  draftKey: () => string;
  /**
   * Demande à quitter la fiche (autre capture, autre vue, fermeture de la fenêtre) :
   * `proceed` est appelé tout de suite s'il n'y a rien à perdre, sinon après le choix de
   * l'utilisateur ; `cancel` s'il choisit de continuer à modifier.
   */
  requestLeave: (proceed: (outcome: LeaveOutcome) => void, cancel?: () => void) => void;
  /** Relit l'état de la capture (ex. restaurée depuis la liste) sans toucher au brouillon. */
  refresh: () => Promise<void>;
}

/**
 * Brouillon de la tâche (modale de conversion) : titre, description et classement sont de vraies
 * saisies de l'utilisateur. Rien n'existe en base tant que la création n'a pas abouti.
 */
interface ConversionDraft {
  open: boolean;
  title: string;
  /** Le titre a été modifié par l'utilisateur (une proposition tardive ne l'écrase jamais). */
  edited: boolean;
  /** Préremplie avec le texte enregistré de la capture ; indépendante de celui-ci ensuite. */
  description: string;
  /** Classement de la tâche (`""` = aucun) ; indépendant de celui de la capture. */
  destinationId: string;
}
const NO_CONVERSION: ConversionDraft = {
  open: false,
  title: "",
  edited: false,
  description: "",
  destinationId: "",
};

interface CaptureDetailProps {
  item: InboxItem;
  destinations: Destination[];
  onClose: () => void;
  onSaved: (item: InboxItem) => void;
  /** `closeSheet` : la fiche d'origine est toujours ouverte, sans saisie récente : elle peut se fermer. */
  onTrashed: (item: InboxItem, closeSheet: boolean) => void;
  onRestored: (item: InboxItem) => void;
  /**
   * La conversion en tâche est CONFIRMÉE par Rust. `closeSheet` : la fiche est toujours là et
   * peut se fermer.
   */
  onConverted: (conversion: Conversion, closeSheet: boolean) => void;
  /** Ouvre la tâche issue de cette capture (capture déjà convertie). */
  onOpenTask: (taskId: string) => void;
  /**
   * La capture est de retour dans la boîte, CONFIRMÉ par Rust (tâche annulée, capture à
   * l'identique). `closeSheet` : cette fiche est toujours là et peut se fermer.
   */
  onReturned: (item: InboxItem, closeSheet: boolean) => void;
  /** L'état a changé hors de cette fiche (ex. conversion déjà annulée) : relire les listes. */
  onStateChanged: () => void;
  /** Déclare une opération d'écriture en cours (la fermeture de la fenêtre l'attend). */
  track: <T>(operation: Promise<T>) => Promise<T>;
  ref?: Ref<CaptureDetailHandle>;
}

type Busy = "save" | "trash" | "restore" | "convert" | "return" | null;
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
  const { item, destinations, onClose, onSaved, onTrashed, onRestored, onConverted, onOpenTask, track } = props;
  const { onReturned, onStateChanged } = props;
  const [baseline, setBaselineState] = useState(item);
  const [text, setTextState] = useState(item.content);
  const [destinationId, setDestinationState] = useState(item.destinationId ?? "");
  const [busy, setBusy] = useState<Busy>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [goneState, setGoneState] = useState(false);
  const [leave, setLeaveState] = useState<PendingLeave | null>(null);
  const [conversion, setConversionState] = useState<ConversionDraft>(NO_CONVERSION);
  const [conversionError, setConversionError] = useState<string | null>(null);
  // La capture a disparu, été supprimée ou convertie ailleurs alors que le brouillon est modifié :
  // la création est impossible, mais le brouillon reste visible (jamais effacé en silence).
  const [conversionBlocked, setConversionBlockedState] = useState<string | null>(null);
  const conversionBlockedRef = useRef<string | null>(null);
  const [suggesting, setSuggesting] = useState(false);
  // Confirmation intégrée de « Remettre dans la boîte » (non modale).
  const [returnPanel, setReturnPanel] = useState(false);
  const keepInTreatedRef = useRef<HTMLButtonElement>(null);
  const returnButtonRef = useRef<HTMLButtonElement>(null);
  // Identifie la demande de titre en cours : une réponse d'une demande périmée est ignorée.
  const suggestion = useRef(0);
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
    conversion: NO_CONVERSION,
  });
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const titleRef = useRef<HTMLInputElement>(null);
  const convertButtonRef = useRef<HTMLButtonElement>(null);
  const panelWasOpen = useRef(false);
  const headingRef = useRef<HTMLHeadingElement>(null);
  const headingId = useId();
  const textId = useId();
  const destinationFieldId = useId();
  const errorId = useId();
  const returnHeadingId = useId();

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
  function setConversion(next: ConversionDraft) {
    model.current.conversion = next;
    setConversionState(next);
  }
  function setConversionBlocked(next: string | null) {
    conversionBlockedRef.current = next;
    setConversionBlockedState(next);
  }
  /** Le brouillon diverge de la référence enregistrée (état immédiat, pas celui du rendu). */
  const isDirty = () =>
    model.current.text !== model.current.baseline.content ||
    model.current.destinationId !== (model.current.baseline.destinationId ?? "");
  /**
   * Le brouillon de tâche diverge de ce que la modale proposait à l'ouverture (titre modifié,
   * description ou classement différents de la capture enregistrée) : il est protégé comme un
   * texte non enregistré. Un brouillon intact se ferme sans avertissement.
   */
  const conversionModified = () => {
    const { conversion, baseline } = model.current;
    return (
      conversion.open &&
      (conversion.edited ||
        conversion.description !== baseline.content ||
        conversion.destinationId !== (baseline.destinationId ?? ""))
    );
  };
  /** Tout ce qu'un départ pourrait faire perdre : texte, destination ou brouillon de tâche. */
  const hasDraft = () => isDirty() || conversionModified();

  const trashed = baseline.deletedAt !== null;
  const converted = baseline.convertedAt !== null;
  const dirty = text !== baseline.content || destinationId !== (baseline.destinationId ?? "");
  const converting = conversion.open;
  const returning = returnPanel && converted && !trashed && !goneState;
  // Pendant la conversion, texte et destination sont figés : on convertit la version enregistrée.
  const editable = !trashed && !goneState && !converted && !converting;

  useImperativeHandle(props.ref, () => ({
    hasUnsaved: () => hasDraft(),
    draftKey: () => {
      const c = model.current.conversion;
      const taskDraft = conversionModified() ? `${c.title}\u0000${c.description}\u0000${c.destinationId}` : "";
      return (
        `${model.current.baseline.id}\u0000${model.current.text}\u0000${model.current.destinationId}` +
        `\u0000${taskDraft}`
      );
    },
    requestLeave(proceed, cancel) {
      if (hasDraft()) setLeave({ proceed, cancel });
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
    (trashed ? headingRef.current : textareaRef.current)?.focus({ preventScroll: true });
    // Une seule fois par capture ouverte (la fiche est remontée avec `key={item.id}`).
  }, []);

  async function refreshBaseline() {
    try {
      const previous = model.current.baseline;
      const fresh = await getInboxItem(previous.id);
      setBaseline(fresh);
      reconcileConversion(previous, fresh);
      return fresh;
    } catch {
      return null;
    }
  }

  /**
   * La capture a été relue pendant que la modale est ouverte. Un brouillon **modifié** n'est
   * jamais effacé en silence : si la création n'est plus possible, la modale reste ouverte avec
   * un message et le brouillon intact. Un brouillon **intact** suit la version actuelle ou se
   * ferme avec un message (rien à perdre).
   */
  function reconcileConversion(previous: InboxItem, fresh: InboxItem) {
    const c = model.current.conversion;
    if (!c.open) return;
    if (fresh.deletedAt !== null || fresh.convertedAt !== null) {
      const reason =
        fresh.convertedAt !== null
          ? "Cette capture est déjà transformée en tâche."
          : "Cette capture a été mise à la corbeille : restaurez-la pour la transformer.";
      // `model.current.baseline` est déjà la version fraîche : on compare à la précédente.
      const modified =
        c.edited ||
        c.description !== previous.content ||
        c.destinationId !== (previous.destinationId ?? "");
      if (modified) {
        setConversionBlocked(`${reason} Votre brouillon reste ici : copiez-le avant de fermer si besoin.`);
      } else {
        closeConversion();
        setNotice(reason);
      }
      return;
    }
    // Toujours convertible. Les champs que l'utilisateur n'a pas touchés suivent la version
    // actuelle ; ceux qu'il a modifiés sont conservés tels quels.
    setConversionBlocked(null);
    const followsText = c.description === previous.content;
    const followsDestination = c.destinationId === (previous.destinationId ?? "");
    if (followsText || followsDestination) {
      setConversion({
        ...c,
        description: followsText ? fresh.content : c.description,
        destinationId: followsDestination ? (fresh.destinationId ?? "") : c.destinationId,
      });
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
    // Un titre de conversion encore non validé retient le départ : la bannière reste affichée.
    if (safe && !conversionModified()) finishLeave("saved");
    return safe;
  }

  // --- Remise dans la boîte (capture traitée) ---

  function openReturnPanel() {
    if (inFlight.current || model.current.baseline.convertedAt === null) return;
    setError(null);
    setNotice(null);
    setReturnPanel(true);
  }

  function closeReturnPanel() {
    setReturnPanel(false);
    // Le panneau disparaît : le focus revient à la commande qui l'a ouvert.
    window.setTimeout(() => returnButtonRef.current?.focus({ preventScroll: true }), 0);
  }

  async function confirmReturn() {
    const version = model.current.baseline;
    const taskId = version.convertedTaskId;
    if (inFlight.current || version.convertedAt === null || taskId === null || model.current.gone) return;
    inFlight.current = true;
    setBusy("return");
    setError(null);
    setNotice(null);
    try {
      // Succès uniquement après la réponse de Rust (les protections y sont l'autorité finale).
      const result = await track(cancelTaskConversion(taskId));
      setBaseline(result.item);
      setReturnPanel(false);
      onReturned(result.item, mounted.current);
    } catch (cause) {
      await explainReturnFailure(cause);
    } finally {
      inFlight.current = false;
      setBusy(null);
    }
  }

  async function explainReturnFailure(cause: unknown) {
    const code = cause instanceof InboxApiError ? cause.code : "unknown";
    const message = cause instanceof Error ? cause.message : "La remise dans la boîte a échoué.";
    if (code === "task_modified") {
      // Travail déjà fait sur la tâche : rien n'est annulé, la capture reste dans « Traitées ».
      setReturnPanel(false);
      setNotice(
        "Cette tâche a déjà été modifiée : la capture reste dans « Traitées ». " +
          "Ouvrez la tâche pour la consulter.",
      );
    } else if (code === "task_not_active") {
      await refreshBaseline();
      setReturnPanel(false);
      setNotice("Cette conversion a déjà été annulée : la capture est de retour dans « À organiser ».");
      onStateChanged();
    } else if (code === "task_not_found") {
      await refreshBaseline();
      setReturnPanel(false);
      setError("Cette tâche n'existe plus. Les listes ont été relues.");
      onStateChanged();
    } else if (code === "not_found") {
      setReturnPanel(false);
      setGone(true);
      setError("Cette capture n'existe plus.");
      onStateChanged();
    } else {
      // inconsistent_state, storage, inconnu : l'état affiché ne change pas ; nouvel essai possible.
      setError(`${message} La capture reste dans « Traitées » : vous pouvez réessayer.`);
    }
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
    } else if (code === "converted") {
      await refreshBaseline();
      setNotice("Cette capture a été transformée en tâche : elle est désormais en lecture seule.");
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
      if (cause instanceof InboxApiError && cause.code === "converted") {
        await refreshBaseline();
        setNotice("Cette capture a été transformée en tâche : elle ne peut plus aller à la corbeille.");
      } else {
        const message = cause instanceof Error ? cause.message : "La mise à la corbeille a échoué.";
        setError(`${message} La capture n'a pas été déplacée.`);
      }
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
    if (hasDraft()) setLeave({ proceed });
    else proceed("clean");
  }

  // --- Conversion en tâche ---

  /** Ferme le panneau et abandonne son brouillon (décision déjà prise par l'appelant). */
  function closeConversion() {
    suggestion.current += 1; // une réponse de titre encore attendue n'a plus d'objet
    setConversion(NO_CONVERSION);
    setConversionError(null);
    setConversionBlocked(null);
    setSuggesting(false);
  }

  /** Ouvre le panneau sur la version ENREGISTRÉE (le brouillon du texte a déjà été réglé). */
  function openConversion() {
    if (model.current.baseline.deletedAt !== null || model.current.baseline.convertedAt !== null) return;
    const mine = ++suggestion.current;
    // Préremplissage : valeurs ENREGISTRÉES de la capture, sans aucune écriture en base.
    const base = model.current.baseline;
    setConversion({
      open: true,
      title: "",
      edited: false,
      description: base.content,
      destinationId: base.destinationId ?? "",
    });
    setConversionError(null);
    setConversionBlocked(null);
    setError(null);
    setNotice(null);
    setSuggesting(true);
    suggestTaskTitle(model.current.baseline.id)
      .then((title) => {
        // Jamais d'écrasement d'un titre que l'utilisateur a commencé à modifier.
        if (mine !== suggestion.current) return;
        const current = model.current.conversion;
        if (current.open && !current.edited) setConversion({ ...current, title });
      })
      .catch((cause: unknown) => {
        if (mine !== suggestion.current || model.current.conversion.edited) return;
        const message = cause instanceof Error ? cause.message : "Le titre n'a pas pu être proposé.";
        setConversionError(`${message} Saisissez le titre de la tâche.`);
      })
      .finally(() => {
        if (mine === suggestion.current) setSuggesting(false);
      });
  }

  /** Clic sur « Transformer en tâche » : le brouillon du texte est réglé AVANT d'ouvrir le panneau. */
  function startConversion() {
    if (inFlight.current || converting) return;
    askLeave((outcome) => {
      if (outcome === "discarded") revert();
      openConversion();
    });
  }

  function editTitle(next: string) {
    setConversion({ ...model.current.conversion, title: next, edited: true });
    setConversionError(null);
  }

  function editDescription(next: string) {
    setConversion({ ...model.current.conversion, description: next });
    setConversionError(null);
  }

  function editDestination(next: string) {
    setConversion({ ...model.current.conversion, destinationId: next });
    setConversionError(null);
  }

  async function convert() {
    const draft = model.current.conversion;
    const version = model.current.baseline;
    if (inFlight.current || !draft.open || version.deletedAt !== null || version.convertedAt !== null || model.current.gone) {
      return;
    }
    if (conversionBlockedRef.current) return;
    if (draft.title.trim() === "") {
      setConversionError("Le titre de la tâche est vide.");
      return;
    }
    // Contrainte actuelle du schéma : jamais remplacée en silence par le texte d'origine.
    if (draft.description.trim() === "") {
      setConversionError("La description ne peut pas être vide pour le moment.");
      return;
    }
    // Classement : « garder » tant que rien n'a changé, sinon le choix explicite de l'utilisateur.
    const destination: DestinationChoice =
      draft.destinationId === (version.destinationId ?? "")
        ? { type: "keep" }
        : draft.destinationId === ""
          ? { type: "clear" }
          : { type: "set", id: draft.destinationId };
    inFlight.current = true;
    setBusy("convert");
    setConversionError(null);
    setError(null);
    setNotice(null);
    try {
      // Version ENREGISTRÉE vue par l'utilisateur ; le succès n'existe qu'après Rust.
      const result = await track(
        convertInboxItemToTask(version.id, version.updatedAt, draft.title, {
          details: draft.description,
          destination,
        }),
      );
      setBaseline(result.item);
      setDraft(result.item.content, result.item.destinationId ?? "");
      closeConversion();
      onConverted(result, mounted.current);
    } catch (cause) {
      await explainConversionFailure(cause);
    } finally {
      inFlight.current = false;
      setBusy(null);
    }
  }

  /**
   * Échec de la création. Le brouillon est TOUJOURS conservé : les messages s'affichent dans la
   * modale, sous les champs, et la capture est relue pour suivre son état réel.
   */
  async function explainConversionFailure(cause: unknown) {
    const code = cause instanceof InboxApiError ? cause.code : "unknown";
    const message = cause instanceof Error ? cause.message : "La création de la tâche a échoué.";
    if (code === "version_conflict") {
      // La capture a changé : la version actuelle devient la référence. Les champs que
      // l'utilisateur n'a pas touchés la suivent ; les autres sont conservés (voir reconcile).
      const fresh = await refreshBaseline();
      if (fresh && fresh.deletedAt === null && fresh.convertedAt === null) {
        setConversionError(
          "Cette capture a changé depuis son ouverture. Votre brouillon est conservé : vérifiez-le " +
            "puis créez la tâche de nouveau.",
        );
      }
    } else if (code === "already_converted" || code === "trashed") {
      // refreshBaseline referme la modale si le brouillon est intact, sinon la garde ouverte
      // avec un message (reconcileConversion).
      await refreshBaseline();
    } else if (code === "not_found") {
      setGone(true);
      if (conversionModified()) {
        setConversionBlocked("Cette capture n'existe plus. Votre brouillon reste ici : copiez-le avant de fermer si besoin.");
      } else {
        closeConversion();
        setError("Cette capture n'existe plus.");
      }
    } else if (
      code === "empty_title" ||
      code === "title_too_long" ||
      code === "empty_details" ||
      code === "details_too_long"
    ) {
      setConversionError(message);
    } else if (code === "unknown_destination") {
      setConversionError(`${message} Choisissez un autre classement : votre brouillon est conservé.`);
    } else {
      setConversionError(`${message} Votre brouillon est conservé : réessayez.`);
    }
  }

  /**
   * ✕, « Abandonner » ou Échap dans la modale. Un brouillon intact se ferme sans avertissement ;
   * un brouillon modifié passe par l'avertissement (jamais d'abandon par un simple clic).
   */
  function requestCloseConversion() {
    if (inFlight.current) return; // création en cours : on n'abandonne pas
    if (model.current.leave) {
      dismissLeave(); // Échap sur l'avertissement = « Continuer »
      titleRef.current?.focus({ preventScroll: true });
      return;
    }
    if (conversionModified()) setLeave({ proceed: () => closeConversion() });
    else closeConversion();
  }

  // Focus : titre à l'ouverture du panneau, bouton « Transformer » à sa fermeture.
  useEffect(() => {
    if (conversion.open && !panelWasOpen.current) titleRef.current?.focus({ preventScroll: true });
    if (!conversion.open && panelWasOpen.current && mounted.current && !converted) {
      convertButtonRef.current?.focus({ preventScroll: true });
    }
    panelWasOpen.current = conversion.open;
  }, [conversion.open, converted]);
  // Confirmation de remise : le focus va au choix sûr (« Garder en Traitées »).
  useEffect(() => {
    if (returning) keepInTreatedRef.current?.focus();
  }, [returning]);
  // La proposition arrivée (et non modifiée) est sélectionnée : une frappe la remplace.
  useEffect(() => {
    if (conversion.open && !suggesting && !conversion.edited) titleRef.current?.select();
    // Seul l'arrivée de la proposition compte.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [suggesting]);

  function decide(choice: "save" | "discard" | "keep") {
    if (!model.current.leave) return;
    if (choice === "keep") {
      dismissLeave();
      // Le bouton disparaît : on rend le focus au champ à poursuivre au clavier. Pendant la
      // conversion le texte est en lecture seule : c'est alors le titre qui est en jeu.
      if (model.current.conversion.open) titleRef.current?.focus({ preventScroll: true });
      else textareaRef.current?.focus({ preventScroll: true });
    } else if (choice === "discard") {
      // Abandon explicite du brouillon de tâche : la modale se ferme, la capture reste intacte.
      if (model.current.conversion.open && !inFlight.current) closeConversion();
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
      else if (model.current.conversion.open) requestCloseConversion();
      else if (returnPanelOpen.current) {
        if (!inFlight.current) closeReturnPanel();
      } else askLeave(onClose);
    }
  }

  function handleTextKeyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
    if (event.key === "Enter" && (event.ctrlKey || event.metaKey) && !event.nativeEvent.isComposing) {
      event.preventDefault();
      void save();
    }
  }

  const message = error ?? notice;
  // Fiche d'une capture traitée : le message (refus, conflit) est en haut, sous les commandes
  // qui l'ont provoqué, jamais hors de la zone visible du panneau défilant.
  const messageOnTop = converted && !trashed;
  const messageNode = message ? (
    <p id={errorId} className={error ? "detail__error" : "detail__notice"} role="alert">
      {message}
    </p>
  ) : null;
  const returnPanelOpen = useRef(false);
  returnPanelOpen.current = returning;

  return (
    <section
      className={returning ? "detail detail--converting" : "detail"}
      aria-labelledby={headingId}
      onKeyDown={handleKeyDown}
    >
      <header className="detail__header">
        <h2 id={headingId} className="detail__title" tabIndex={-1} ref={headingRef}>
          {trashed ? "Capture dans la corbeille" : converted ? "Capture transformée en tâche" : "Fiche de la capture"}
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

      {/* Pendant la modale, l'avertissement s'affiche DANS la modale (le reste est inerte). */}
      {leave && !converting && (
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

      {message && messageOnTop && messageNode}

      {converted && !trashed && !returning && (
        <div className="detail__banner">
          <p>
            Transformée en tâche le {formatLong(baseline.convertedAt!)}. Cette capture est conservée en
            lecture seule.
          </p>
          <div className="detail__convert-actions">
            {baseline.convertedTaskId && (
              <button
                type="button"
                onClick={() => onOpenTask(baseline.convertedTaskId as string)}
                disabled={busy !== null}
              >
                Voir la tâche
              </button>
            )}
            {baseline.convertedTaskId && !returning && (
              <button
                type="button"
                ref={returnButtonRef}
                onClick={openReturnPanel}
                disabled={busy !== null || goneState}
              >
                Remettre dans la boîte
              </button>
            )}
          </div>
        </div>
      )}

      {returning && (
        <section className="detail__convert" aria-labelledby={returnHeadingId}>
          <h3 id={returnHeadingId} className="detail__convert-title">
            Remettre dans la boîte
          </h3>
          <p className="detail__recap">
            La tâche liée sera annulée mais conservée. La capture garde son texte, sa destination et ses
            dates, et revient dans « À organiser » à sa place chronologique : si elle est ancienne,
            « Voir tout » la retrouve.
          </p>
          <div className="detail__convert-actions">
            <button
              type="button"
              className="detail__primary"
              onClick={() => void confirmReturn()}
              disabled={busy !== null}
              aria-busy={busy === "return"}
            >
              {busy === "return" ? "Remise en cours…" : "Confirmer la remise"}
            </button>
            <button
              type="button"
              ref={keepInTreatedRef}
              onClick={closeReturnPanel}
              disabled={busy !== null}
            >
              Garder en Traitées
            </button>
          </div>
        </section>
      )}

      {converting && (
        <TaskDraftModal
          destinations={destinations}
          title={conversion.title}
          description={conversion.description}
          destinationId={conversion.destinationId}
          onTitleChange={editTitle}
          onDescriptionChange={editDescription}
          onDestinationChange={editDestination}
          suggestingTitle={suggesting}
          busy={busy === "convert"}
          message={conversionError ?? conversionBlocked}
          blocked={conversionBlocked !== null}
          leave={leave ? { onDiscard: () => decide("discard"), onKeep: () => decide("keep") } : null}
          onSubmit={() => void convert()}
          onRequestClose={requestCloseConversion}
          titleRef={titleRef}
        />
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

      {message && !messageOnTop && messageNode}

      <div className="detail__actions" hidden={converted && !trashed}>
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
              ref={convertButtonRef}
              className="detail__convert-open"
              onClick={startConversion}
              disabled={busy !== null || goneState || text.trim() === ""}
            >
              Transformer en tâche
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
      {!trashed && !converted && (
        <p className="detail__hint">
          Ctrl+Entrée pour enregistrer · Échap pour fermer
        </p>
      )}
    </section>
  );
}
