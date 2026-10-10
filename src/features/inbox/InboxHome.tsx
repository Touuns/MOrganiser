import { useCallback, useEffect, useRef, useState } from "react";
import {
  createInboxItem,
  listDestinations,
  listInboxItems,
  listTrashedItems,
  restoreInboxItem,
  type Cursor,
  type Destination,
  type InboxFilter,
  type InboxItem,
} from "./api";
import { CaptureDetail, type CaptureDetailHandle } from "./CaptureDetail";
import { LeaveBanner } from "../../components/LeaveBanner";
import { useCloseGuard } from "../../lib/closeGuard";
import { CaptureForm, type CaptureFormHandle } from "./CaptureForm";
import { FilterSelect, InboxPanel } from "./InboxPanel";
import { PAGE_SIZE, PagedCaptureView } from "./PagedCaptureView";
import { UndoToast } from "./UndoToast";
import "./InboxHome.css";

/** Nombre de captures affichées sur l'accueil (les plus récentes). */
export const HOME_LIMIT = 20;

type View = "home" | "all" | "trash";
type Status = { kind: "ok" | "error"; text: string } | null;

/** Une demande de fermeture de la fenêtre : annulable, et réexaminée à chaque étape. */
interface CloseRequest {
  cancelled: boolean;
  /** L'utilisateur a choisi d'abandonner ce brouillon pour cette fermeture. */
  skipDetail: boolean;
  skipForm: boolean;
  closeWindow: () => void;
}

/**
 * Accueil de la brique 001 : boîte « À organiser » au-dessus de la capture rapide, fiche
 * d'une capture à droite (ou plein écran en fenêtre étroite), « Voir tout » et corbeille.
 * La base de données reste la source de vérité : les listes sont relues après chaque changement.
 */
export function InboxHome() {
  const [destinations, setDestinations] = useState<Destination[]>([]);
  const [filter, setFilter] = useState<InboxFilter>({ type: "all" });
  const [view, setView] = useState<View>("home");
  const [items, setItems] = useState<InboxItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [selected, setSelected] = useState<InboxItem | null>(null);
  // Compteur incrémenté après chaque modification de données : relance les lectures.
  const [version, setVersion] = useState(0);
  const [status, setStatus] = useState<Status>(null);
  const [toast, setToast] = useState<{ id: number; itemId: string } | null>(null);
  // Seule la réponse de la dernière demande est affichée (changements de filtre rapides).
  const lastRequest = useRef(0);
  const detailRef = useRef<CaptureDetailHandle>(null);
  const formRef = useRef<CaptureFormHandle>(null);
  // Fermeture de la fenêtre suspendue par du texte non envoyé dans la capture rapide.
  const [closePrompt, setClosePrompt] = useState<{
    request: CloseRequest;
    error: string | null;
    sending: boolean;
  } | null>(null);
  const closePromptRef = useRef<typeof closePrompt>(null);
  function setPrompt(next: typeof closePrompt) {
    closePromptRef.current = next;
    setClosePrompt(next);
  }
  const opener = useRef<HTMLElement | null>(null);
  // Demande de fermeture en cours (une nouvelle demande annule la précédente).
  const closeRequest = useRef<CloseRequest | null>(null);
  // Écritures en cours (enregistrement, corbeille, restauration, envoi) : la fermeture les attend.
  const operations = useRef(new Set<Promise<unknown>>());
  const track = useCallback(<T,>(operation: Promise<T>): Promise<T> => {
    operations.current.add(operation);
    const done = () => void operations.current.delete(operation);
    operation.then(done, done);
    return operation;
  }, []);
  // Fiche actuellement ouverte, lisible par les réponses tardives (jamais une closure périmée).
  const selectedRef = useRef<InboxItem | null>(null);
  selectedRef.current = selected;

  const refresh = useCallback(async (current: InboxFilter) => {
    const request = ++lastRequest.current;
    setLoading(true);
    try {
      // Rust renvoie les HOME_LIMIT plus récentes, de la plus récente à la plus ancienne.
      // On garde cette sélection (jamais les plus anciennes) et on l'affiche en ordre
      // chronologique : la nouvelle capture arrive en bas, au-dessus du champ.
      const page = await listInboxItems(current, HOME_LIMIT);
      if (request !== lastRequest.current) return;
      setItems([...page.items].reverse());
      setTotal(page.total);
      setLoadError(null);
    } catch (error) {
      if (request !== lastRequest.current) return;
      setLoadError(error instanceof Error ? error.message : "Lecture impossible.");
    } finally {
      if (request === lastRequest.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    listDestinations()
      .then(setDestinations)
      .catch(() => setDestinations([])); // la capture sans destination reste possible
  }, []);

  useEffect(() => {
    void refresh(filter);
  }, [filter, version, refresh]);

  const changed = () => setVersion((v) => v + 1);

  // Fermeture normale de la fenêtre. On examine l'état ACTUEL des brouillons : la fiche
  // d'abord, puis la capture rapide, puis les écritures en cours. Chaque décision relance
  // l'examen (un nouveau texte saisi entre-temps est donc proposé à son tour), et
  // « Continuer » à n'importe quelle étape annule la demande, y compris si une opération
  // précédente n'a pas encore répondu.
  async function advanceClose(request: CloseRequest) {
    for (;;) {
      if (request.cancelled) return;
      if (!request.skipDetail && detailRef.current?.hasUnsaved()) {
        detailRef.current.requestLeave(
          (outcome) => {
            if (outcome === "discarded") request.skipDetail = true;
            void advanceClose(request);
          },
          () => {
            request.cancelled = true;
          },
        );
        return;
      }
      if (!request.skipForm && formRef.current?.hasUnsent()) {
        setPrompt({ request, error: null, sending: false });
        return;
      }
      // Plus rien à proposer : on attend les écritures encore en transit (corbeille,
      // restauration, enregistrement…), puis on réexamine l'état avant de détruire.
      if (operations.current.size > 0) {
        await Promise.allSettled([...operations.current]);
        continue;
      }
      request.cancelled = true; // une seule destruction par demande
      request.closeWindow();
      return;
    }
  }

  useCloseGuard(
    () =>
      Boolean(
        detailRef.current?.hasUnsaved() || formRef.current?.hasUnsent() || operations.current.size > 0,
      ),
    (closeWindow) => {
      if (closeRequest.current) closeRequest.current.cancelled = true;
      setPrompt(null);
      const request: CloseRequest = { cancelled: false, skipDetail: false, skipForm: false, closeWindow };
      closeRequest.current = request;
      void advanceClose(request);
    },
  );

  async function sendBeforeClosing() {
    const prompt = closePrompt;
    if (!prompt || !formRef.current) return;
    setPrompt({ ...prompt, error: null, sending: true });
    const sent = await formRef.current.submit();
    if (prompt.request.cancelled) return; // « Continuer » a été choisi pendant l'attente
    if (sent) {
      setPrompt(null);
      void advanceClose(prompt.request); // réexamine : rien n'est détruit « à l'aveugle »
    } else {
      setPrompt({
        ...prompt,
        sending: false,
        error: "L'envoi a échoué : votre texte est conservé et la fenêtre reste ouverte.",
      });
    }
  }

  /** Exécute `action` après avoir proposé d'enregistrer un brouillon non enregistré. */
  function guard(action: () => void) {
    if (detailRef.current) detailRef.current.requestLeave(() => action());
    else action();
  }

  function openItem(item: InboxItem) {
    if (selected?.id === item.id) return;
    const active = document.activeElement;
    guard(() => {
      if (!selected && active instanceof HTMLElement) opener.current = active;
      setSelected(item);
    });
  }

  function closeDetail() {
    setSelected(null);
    const target = opener.current;
    opener.current = null;
    // Rend le focus à la carte d'où l'on est venu, si elle existe encore.
    window.setTimeout(() => {
      if (target?.isConnected) target.focus();
    }, 0);
  }

  function changeView(next: View) {
    if (next === view) return;
    guard(() => setView(next));
  }

  async function handleCapture(content: string, destinationId: string | null) {
    const created = await track(createInboxItem(content, destinationId)); // rejette → texte conservé
    const visible =
      filter.type === "all" ||
      (filter.type === "unclassified" && created.destinationId === null) ||
      (filter.type === "destination" && filter.id === created.destinationId);
    setStatus({
      kind: "ok",
      text: visible ? "Capture enregistrée." : "Capture enregistrée (masquée par le filtre actuel).",
    });
    changed();
  }

  /** La capture `id` a changé d'état hors de sa fiche : si cette fiche est ouverte, on la relit. */
  function refreshOpenSheet(id: string) {
    if (selectedRef.current?.id === id) void detailRef.current?.refresh();
  }

  async function undoTrash(itemId: string) {
    setToast(null);
    try {
      await track(restoreInboxItem(itemId));
      setStatus({ kind: "ok", text: "Capture restaurée." });
      refreshOpenSheet(itemId);
      changed();
    } catch (error) {
      const message = error instanceof Error ? error.message : "La restauration a échoué.";
      setStatus({ kind: "error", text: `${message} La capture est toujours dans la corbeille.` });
    }
  }

  async function restoreFromTrash(item: InboxItem) {
    try {
      const restored = await track(restoreInboxItem(item.id));
      setStatus({ kind: "ok", text: "Capture restaurée." });
      refreshOpenSheet(restored.id); // la fiche ouverte suit l'état réel ; son brouillon est conservé
      changed();
    } catch (error) {
      const message = error instanceof Error ? error.message : "La restauration a échoué.";
      setStatus({ kind: "error", text: `${message} La capture est toujours dans la corbeille.` });
    }
  }

  const loadAll = useCallback((before: Cursor | null) => listInboxItems(filter, PAGE_SIZE, before), [filter]);
  const loadTrash = useCallback((before: Cursor | null) => listTrashedItems(PAGE_SIZE, before), []);

  return (
    <div className="inbox-home" data-detail={selected ? "open" : "closed"}>
      {closePrompt && (
        <div className="inbox-home__prompt">
          <LeaveBanner
            message="Vous avez du texte non envoyé dans la capture rapide."
            saveLabel="Envoyer"
            discardLabel="Abandonner ce texte"
            keepLabel="Continuer à écrire"
            saving={closePrompt.sending}
            error={closePrompt.error}
            onSave={() => void sendBeforeClosing()}
            onDiscard={() => {
              closePrompt.request.skipForm = true;
              setPrompt(null);
              void advanceClose(closePrompt.request);
            }}
            onKeep={() => {
              closePrompt.request.cancelled = true;
              setPrompt(null);
              formRef.current?.focus();
            }}
          />
        </div>
      )}
      <div className="inbox-home__main">
        {view === "home" && (
          <InboxPanel
            items={items}
            total={total}
            destinations={destinations}
            filter={filter}
            onFilterChange={setFilter}
            loading={loading}
            loadError={loadError}
            onRetry={() => void refresh(filter)}
            selectedId={selected?.id ?? null}
            onOpen={openItem}
            onShowAll={() => changeView("all")}
            onShowTrash={() => changeView("trash")}
          />
        )}
        {view === "all" && (
          <PagedCaptureView
            title="Toutes les captures"
            onBack={() => changeView("home")}
            loader={loadAll}
            reloadKey={version}
            destinations={destinations}
            selectedId={selected?.id ?? null}
            onOpen={openItem}
            toolbar={<FilterSelect destinations={destinations} filter={filter} onChange={setFilter} />}
            emptyText="Aucune capture pour ce filtre."
          />
        )}
        {view === "trash" && (
          <PagedCaptureView
            title="Corbeille"
            onBack={() => changeView("home")}
            loader={loadTrash}
            reloadKey={version}
            destinations={destinations}
            selectedId={selected?.id ?? null}
            onOpen={openItem}
            emptyText="La corbeille est vide."
            dateOf={(item) => item.deletedAt ?? item.createdAt}
            datePrefix="Supprimée le "
            renderAction={(item) => (
              <button type="button" className="inbox__action" onClick={() => void restoreFromTrash(item)}>
                Restaurer
              </button>
            )}
          />
        )}
        <CaptureForm
          ref={formRef}
          destinations={destinations}
          onSubmit={handleCapture}
          onSent={() => {
            // L'avertissement « texte non envoyé » est périmé : la fermeture demandée reprend.
            const prompt = closePromptRef.current;
            if (prompt && !prompt.request.cancelled && !formRef.current?.hasUnsent()) {
              setPrompt(null);
              void advanceClose(prompt.request);
            }
          }}
        />
        <div className="inbox-home__feedback">
          <p
            className={status?.kind === "error" ? "inbox-home__status inbox-home__status--error" : "inbox-home__status"}
            role={status?.kind === "error" ? "alert" : "status"}
          >
            {status?.text ?? ""}
          </p>
          {toast && (
            <UndoToast
              key={toast.id}
              message="Capture mise à la corbeille."
              actionLabel="Annuler"
              onAction={() => void undoTrash(toast.itemId)}
              onDismiss={() => setToast(null)}
            />
          )}
        </div>
      </div>

      {selected && (
        <div className="inbox-home__detail">
          <CaptureDetail
            key={selected.id}
            ref={detailRef}
            item={selected}
            destinations={destinations}
            onClose={closeDetail}
            onSaved={() => {
              setStatus({ kind: "ok", text: "Modifications enregistrées." });
              changed();
            }}
            track={track}
            onTrashed={(item, closeSheet) => {
              // Le résultat concerne la capture `item` : il ne ferme que SA fiche, et seulement
              // si elle est toujours ouverte et sans saisie récente. Jamais une autre fiche.
              if (closeSheet && selectedRef.current?.id === item.id) closeDetail();
              else refreshOpenSheet(item.id);
              setToast({ id: Date.now(), itemId: item.id });
              setStatus(null);
              changed();
            }}
            onRestored={() => {
              setStatus({ kind: "ok", text: "Capture restaurée." });
              changed();
            }}
          />
        </div>
      )}
    </div>
  );
}
