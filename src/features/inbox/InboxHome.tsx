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
  const [closePrompt, setClosePrompt] = useState<{ closeWindow: () => void; error: string | null; sending: boolean } | null>(null);
  const opener = useRef<HTMLElement | null>(null);

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

  // Fermeture normale de la fenêtre : on propose d'abord d'enregistrer la fiche modifiée,
  // puis d'envoyer le texte de la capture rapide. « Continuer » à une étape annule tout.
  useCloseGuard(
    () => Boolean(detailRef.current?.hasUnsaved() || formRef.current?.hasUnsent()),
    (closeWindow) => {
      const checkCapture = () => {
        if (formRef.current?.hasUnsent()) setClosePrompt({ closeWindow, error: null, sending: false });
        else closeWindow();
      };
      if (detailRef.current?.hasUnsaved()) detailRef.current.requestLeave(checkCapture);
      else checkCapture();
    },
  );

  async function sendBeforeClosing() {
    const prompt = closePrompt;
    if (!prompt || !formRef.current) return;
    setClosePrompt({ ...prompt, error: null, sending: true });
    const sent = await formRef.current.submit();
    if (sent) {
      setClosePrompt(null);
      prompt.closeWindow();
    } else {
      setClosePrompt({
        ...prompt,
        sending: false,
        error: "L'envoi a échoué : votre texte est conservé et la fenêtre reste ouverte.",
      });
    }
  }

  /** Exécute `action` après avoir proposé d'enregistrer un brouillon non enregistré. */
  function guard(action: () => void) {
    if (detailRef.current) detailRef.current.requestLeave(action);
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
    const created = await createInboxItem(content, destinationId); // rejette → texte conservé
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

  async function undoTrash(itemId: string) {
    setToast(null);
    try {
      await restoreInboxItem(itemId);
      setStatus({ kind: "ok", text: "Capture restaurée." });
      changed();
    } catch (error) {
      const message = error instanceof Error ? error.message : "La restauration a échoué.";
      setStatus({ kind: "error", text: `${message} La capture est toujours dans la corbeille.` });
    }
  }

  async function restoreFromTrash(item: InboxItem) {
    try {
      const restored = await restoreInboxItem(item.id);
      setStatus({ kind: "ok", text: "Capture restaurée." });
      if (selected?.id === restored.id) setSelected(restored);
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
              const { closeWindow } = closePrompt;
              setClosePrompt(null);
              closeWindow();
            }}
            onKeep={() => {
              setClosePrompt(null);
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
        <CaptureForm ref={formRef} destinations={destinations} onSubmit={handleCapture} />
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
            onTrashed={(item) => {
              closeDetail();
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
