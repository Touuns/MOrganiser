import { useId, useLayoutEffect, useRef } from "react";
import type { Destination, InboxFilter, InboxItem } from "./api";
import { CaptureCard } from "./CaptureCard";
import { DestinationOptions } from "./DestinationOptions";
import "./InboxPanel.css";

interface InboxPanelProps {
  /** Captures à afficher, en ordre chronologique croissant (la plus récente en dernier). */
  items: InboxItem[];
  total: number;
  destinations: Destination[];
  filter: InboxFilter;
  onFilterChange: (filter: InboxFilter) => void;
  loading: boolean;
  loadError: string | null;
  onRetry: () => void;
  selectedId: string | null;
  onOpen: (item: InboxItem) => void;
  onShowAll: () => void;
  onShowTrash: () => void;
}

export function filterToValue(filter: InboxFilter): string {
  return filter.type === "destination" ? `destination:${filter.id}` : filter.type;
}

export function valueToFilter(value: string): InboxFilter {
  if (value.startsWith("destination:")) return { type: "destination", id: value.slice(12) };
  return value === "unclassified" ? { type: "unclassified" } : { type: "all" };
}

/** Sélecteur de filtre partagé par la boîte et la vue complète. */
export function FilterSelect(props: {
  destinations: Destination[];
  filter: InboxFilter;
  onChange: (filter: InboxFilter) => void;
}) {
  const id = useId();
  return (
    <label className="inbox__filter" htmlFor={id}>
      <span>Afficher</span>
      <select
        id={id}
        value={filterToValue(props.filter)}
        onChange={(event) => props.onChange(valueToFilter(event.target.value))}
      >
        <option value="all">Toutes</option>
        <option value="unclassified">À classer</option>
        <DestinationOptions
          destinations={props.destinations.map((d) => ({ ...d, id: `destination:${d.id}` }))}
        />
      </select>
    </label>
  );
}

/**
 * Boîte « À organiser », placée au-dessus du champ de capture : les plus anciennes en
 * haut, la plus récente en bas, juste au-dessus du champ d'où elle vient.
 */
export function InboxPanel(props: InboxPanelProps) {
  const { items, total, destinations, filter, onFilterChange, loading, loadError, onRetry } = props;
  const headingId = useId();
  const listRef = useRef<HTMLOListElement>(null);
  const labels = new Map(destinations.map((d) => [d.id, d.label]));

  // La liste n'est rechargée qu'au démarrage, au changement de filtre et après une
  // modification : on amène alors la plus récente (en bas) dans la zone visible.
  useLayoutEffect(() => {
    const list = listRef.current;
    if (list) list.scrollTop = list.scrollHeight;
  }, [items]);

  return (
    <section className="inbox" aria-labelledby={headingId}>
      <header className="inbox__header">
        <h2 id={headingId} className="inbox__title">
          À organiser
        </h2>
        <div className="inbox__tools">
          <FilterSelect destinations={destinations} filter={filter} onChange={onFilterChange} />
          <button type="button" className="inbox__link" onClick={props.onShowTrash}>
            Corbeille
          </button>
        </div>
      </header>

      {loadError ? (
        <div className="inbox__message" role="alert">
          <p>{loadError}</p>
          <button type="button" className="inbox__retry" onClick={onRetry}>
            Réessayer
          </button>
        </div>
      ) : items.length === 0 ? (
        <p className="inbox__message">
          {loading
            ? "Chargement…"
            : filter.type === "all"
              ? "Rien à organiser pour l'instant. Écrivez ci-dessous."
              : "Aucune capture pour ce filtre."}
        </p>
      ) : (
        <>
          {total > items.length && (
            <p className="inbox__more">
              Les {items.length} plus récentes sur {total}.{" "}
              <button type="button" className="inbox__link" onClick={props.onShowAll}>
                Voir tout
              </button>
            </p>
          )}
          <ol className="inbox__list" ref={listRef} aria-busy={loading}>
            {items.map((item) => (
              <CaptureCard
                key={item.id}
                item={item}
                destinationLabel={
                  item.destinationId ? (labels.get(item.destinationId) ?? item.destinationId) : null
                }
                selected={item.id === props.selectedId}
                onOpen={props.onOpen}
              />
            ))}
          </ol>
        </>
      )}
    </section>
  );
}
