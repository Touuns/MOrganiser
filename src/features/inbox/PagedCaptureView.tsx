import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import type { Cursor, Destination, InboxItem, InboxPage } from "./api";
import { CaptureCard } from "./CaptureCard";
import "./PagedCaptureView.css";

/** Taille d'un lot de l'historique complet. */
export const PAGE_SIZE = 50;

interface PagedCaptureViewProps {
  title: string;
  onBack: () => void;
  /** Charge une page ; `null` = la plus récente. Doit rester stable tant que le filtre ne change pas. */
  loader: (before: Cursor | null) => Promise<InboxPage>;
  /** Change après chaque modification de données : la vue repart de la page la plus récente. */
  reloadKey: number;
  destinations: Destination[];
  selectedId: string | null;
  onOpen: (item: InboxItem) => void;
  toolbar?: ReactNode;
  emptyText: string;
  /** Date et préfixe affichés sur chaque carte (corbeille : date de suppression). */
  dateOf?: (item: InboxItem) => number;
  datePrefix?: string;
  renderAction?: (item: InboxItem) => ReactNode;
}

/**
 * Liste complète chargée par lots, en ordre chronologique (la plus récente en bas). Les lots
 * plus anciens s'ajoutent au-dessus ; la position de défilement est conservée.
 */
export function PagedCaptureView(props: PagedCaptureViewProps) {
  const { title, onBack, loader, reloadKey, destinations, selectedId, onOpen, toolbar } = props;
  const [items, setItems] = useState<InboxItem[]>([]);
  const [total, setTotal] = useState(0);
  const [nextCursor, setNextCursor] = useState<Cursor | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const generation = useRef(0);
  const scrollRef = useRef<HTMLDivElement>(null);
  // Action de défilement à exécuter après le prochain affichage de la liste.
  const pendingScroll = useRef<{ kind: "bottom" } | { kind: "keep"; height: number; top: number } | null>(
    null,
  );
  const labels = new Map(destinations.map((d) => [d.id, d.label]));

  const loadFirst = useCallback(async () => {
    const current = ++generation.current;
    setLoading(true);
    try {
      const page = await loader(null);
      if (current !== generation.current) return;
      pendingScroll.current = { kind: "bottom" };
      setItems([...page.items].reverse());
      setTotal(page.total);
      setNextCursor(page.nextCursor);
      setError(null);
    } catch (cause) {
      if (current !== generation.current) return;
      setError(cause instanceof Error ? cause.message : "Lecture impossible.");
    } finally {
      if (current === generation.current) setLoading(false);
    }
  }, [loader]);

  useEffect(() => {
    void loadFirst();
  }, [loadFirst, reloadKey]);

  async function loadMore() {
    if (!nextCursor || loadingMore) return;
    const current = generation.current;
    const container = scrollRef.current;
    setLoadingMore(true);
    try {
      const page = await loader(nextCursor);
      if (current !== generation.current) return;
      if (container) {
        pendingScroll.current = { kind: "keep", height: container.scrollHeight, top: container.scrollTop };
      }
      setItems((existing) => {
        const known = new Set(existing.map((item) => item.id));
        const older = [...page.items].reverse().filter((item) => !known.has(item.id));
        return [...older, ...existing];
      });
      setNextCursor(page.nextCursor);
      setTotal(page.total);
      setError(null);
    } catch (cause) {
      if (current !== generation.current) return;
      setError(cause instanceof Error ? cause.message : "Chargement impossible.");
    } finally {
      setLoadingMore(false);
    }
  }

  useLayoutEffect(() => {
    const container = scrollRef.current;
    const action = pendingScroll.current;
    if (!container || !action) return;
    pendingScroll.current = null;
    if (action.kind === "bottom") container.scrollTop = container.scrollHeight;
    // Les éléments ajoutés au-dessus ne doivent pas faire « sauter » ce que l'on lisait.
    else container.scrollTop = action.top + (container.scrollHeight - action.height);
  }, [items]);

  const remaining = total - items.length;

  return (
    <section className="paged" aria-label={title}>
      <header className="paged__header">
        <button type="button" className="paged__back" onClick={onBack}>
          ← Retour
        </button>
        <h2 className="paged__title">{title}</h2>
        {toolbar}
      </header>

      {error && (
        <div className="paged__message" role="alert">
          <p>{error}</p>
          <button type="button" className="paged__button" onClick={() => void loadFirst()}>
            Réessayer
          </button>
        </div>
      )}

      {!error && items.length === 0 ? (
        <p className="paged__message">{loading ? "Chargement…" : props.emptyText}</p>
      ) : (
        <div className="paged__scroll" ref={scrollRef}>
          {nextCursor && (
            <button
              type="button"
              className="paged__button paged__more"
              onClick={() => void loadMore()}
              disabled={loadingMore}
            >
              {loadingMore
                ? "Chargement…"
                : `Charger les ${Math.min(PAGE_SIZE, remaining)} plus anciennes (${remaining} restantes)`}
            </button>
          )}
          <ol className="paged__list" aria-busy={loading || loadingMore}>
            {items.map((item) => (
              <CaptureCard
                key={item.id}
                item={item}
                destinationLabel={item.destinationId ? (labels.get(item.destinationId) ?? item.destinationId) : null}
                selected={item.id === selectedId}
                onOpen={onOpen}
                date={props.dateOf?.(item)}
                datePrefix={props.datePrefix}
                action={props.renderAction?.(item)}
              />
            ))}
          </ol>
        </div>
      )}
      {items.length > 0 && (
        <p className="paged__count">
          {items.length} affichée{items.length > 1 ? "s" : ""} sur {total}
        </p>
      )}
    </section>
  );
}
