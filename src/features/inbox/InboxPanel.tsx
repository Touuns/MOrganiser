import { useId, useLayoutEffect, useRef } from "react";
import { Badge } from "../../components/Badge";
import type { Destination, InboxFilter, InboxItem } from "./api";
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
}

const timeFormat = new Intl.DateTimeFormat("fr-FR", {
  day: "numeric",
  month: "short",
  hour: "2-digit",
  minute: "2-digit",
});

function filterToValue(filter: InboxFilter): string {
  return filter.type === "destination" ? `destination:${filter.id}` : filter.type;
}

function valueToFilter(value: string): InboxFilter {
  if (value.startsWith("destination:")) return { type: "destination", id: value.slice(12) };
  return value === "unclassified" ? { type: "unclassified" } : { type: "all" };
}

/**
 * Boîte « À organiser », placée au-dessus du champ de capture : les plus anciennes en
 * haut, la plus récente en bas, juste au-dessus du champ d'où elle vient.
 */
export function InboxPanel(props: InboxPanelProps) {
  const { items, total, destinations, filter, onFilterChange, loading, loadError, onRetry } = props;
  const headingId = useId();
  const filterId = useId();
  const listRef = useRef<HTMLOListElement>(null);
  const labels = new Map(destinations.map((d) => [d.id, d.label]));

  // La liste n'est rechargée qu'au démarrage, au changement de filtre et après une
  // capture réussie : on amène alors la plus récente (en bas) dans la zone visible.
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
        <label className="inbox__filter" htmlFor={filterId}>
          <span>Afficher</span>
          <select
            id={filterId}
            value={filterToValue(filter)}
            onChange={(event) => onFilterChange(valueToFilter(event.target.value))}
          >
            <option value="all">Toutes</option>
            <option value="unclassified">À classer</option>
            <DestinationOptions
              destinations={destinations.map((d) => ({ ...d, id: `destination:${d.id}` }))}
            />
          </select>
        </label>
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
              Les {items.length} plus récentes sur {total} ; les plus anciennes ne sont pas
              affichées ici.
            </p>
          )}
          <ol className="inbox__list" ref={listRef} aria-busy={loading}>
            {items.map((item) => (
              <li key={item.id} className="inbox__item">
                <p className="inbox__content">{item.content}</p>
                <div className="inbox__meta">
                  {item.destinationId ? (
                    <Badge tone="accent">{labels.get(item.destinationId) ?? item.destinationId}</Badge>
                  ) : (
                    <Badge>À classer</Badge>
                  )}
                  <time dateTime={new Date(item.createdAt).toISOString()}>
                    {timeFormat.format(item.createdAt)}
                  </time>
                </div>
              </li>
            ))}
          </ol>
        </>
      )}
    </section>
  );
}
