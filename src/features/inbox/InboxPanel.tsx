import { useEffect, useId, useImperativeHandle, useLayoutEffect, useRef, type Ref } from "react";
import type { Destination, InboxFilter, InboxItem } from "./api";
import { playArrival, snapshotTops, type ArrivalHandle } from "./arrivalAnimation";
import { recordAnchors, restoreAnchors, type Anchor } from "./scrollAnchor";
import { CaptureCard } from "./CaptureCard";
import { DestinationOptions } from "./DestinationOptions";
import "./InboxPanel.css";

/** Une capture vient d'être enregistrée : sa carte doit « monter » depuis la zone de saisie. */
export interface Arrival {
  id: string;
  /** Bord supérieur de la zone de saisie au moment de l'envoi, `null` si non mesurable (pas d'animation). */
  fromTop: number | null;
}

export interface InboxPanelHandle {
  /** Mémorise la position des cartes avant l'insertion (pour les faire glisser sans saut). */
  snapshotPositions: () => void;
}

interface InboxPanelProps {
  ref?: Ref<InboxPanelHandle>;
  arrival: Arrival | null;
  /** Appelé dès que l'arrivée a été prise en charge (animée ou non). */
  onArrivalConsumed: () => void;
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
  onShowTasks: () => void;
  /** Capture à mettre en évidence pendant la visite d'initiation. */
  highlightId?: string | null;
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
  const previousTops = useRef<Map<string, number> | null>(null);
  const running = useRef<ArrivalHandle | null>(null);
  const labels = new Map(destinations.map((d) => [d.id, d.label]));

  // Défilement : « en bas » seulement quand c'est voulu (premier affichage, changement de
  // filtre, arrivée d'une nouvelle capture). Pour tout le reste (suppression, restauration,
  // modification, relecture), la zone de lecture est conservée par ancrage sur une carte
  // voisine encore présente, y compris si une capture plus ancienne entre dans l'ensemble.
  const anchors = useRef<Anchor[]>([]);
  const previous = useRef({ filterKey: "", count: 0 });
  useLayoutEffect(() => {
    const list = listRef.current;
    const filterKey = filterToValue(filter);
    if (list) {
      const toBottom =
        previous.current.filterKey !== filterKey || previous.current.count === 0 || props.arrival !== null;
      if (toBottom) list.scrollTop = list.scrollHeight;
      else restoreAnchors(list, anchors.current);
      anchors.current = recordAnchors(list);
    } else {
      anchors.current = [];
    }
    previous.current = { filterKey, count: items.length };
    // `filter` et `arrival` sont lus à cet instant précis : seul un changement de liste compte.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [items]);

  // Position de lecture mémorisée à chaque défilement de l'utilisateur.
  const hasList = items.length > 0 && !loadError;
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    const remember = () => {
      anchors.current = recordAnchors(list);
    };
    list.addEventListener("scroll", remember, { passive: true });
    return () => list.removeEventListener("scroll", remember);
  }, [hasList]);

  useImperativeHandle(props.ref, () => ({
    snapshotPositions() {
      const list = listRef.current;
      previousTops.current = list ? snapshotTops(list) : null;
    },
  }));

  // Arrivée d'une capture : la vraie carte est déjà dans la liste ; on y fait monter une carte
  // fantôme. Aucune animation (insertion directe) si la liste n'est pas visible ou si le
  // mouvement est réduit. Déclaré APRÈS le défilement ci-dessus : positions finales mesurées.
  const { arrival, onArrivalConsumed } = props;
  useLayoutEffect(() => {
    const list = listRef.current;
    if (!arrival || !list) return;
    const card = [...list.querySelectorAll<HTMLElement>("[data-capture-id]")].find(
      (element) => element.dataset.captureId === arrival.id,
    );
    if (!card) return; // pas encore rendue (liste en cours de relecture) : on attend
    const tops = previousTops.current;
    previousTops.current = null;
    onArrivalConsumed();
    running.current?.cancel(); // une arrivée en cours est terminée net, sa carte reste visible
    // Sans position de départ mesurable, la carte est simplement insérée (déjà visible en bas).
    running.current =
      arrival.fromTop === null
        ? null
        : playArrival({ card, container: list, fromTop: arrival.fromTop, previousTops: tops ?? undefined });
  }, [items, arrival, onArrivalConsumed]);

  // Tout changement qui rendrait le trajet faux annule proprement le mouvement.
  useEffect(() => {
    running.current?.cancel();
  }, [filter, destinations]);
  useEffect(() => {
    const stop = () => running.current?.cancel();
    window.addEventListener("resize", stop);
    return () => {
      window.removeEventListener("resize", stop);
      running.current?.cancel(); // changement de vue ou démontage
    };
  }, []);

  return (
    <section className="inbox" aria-labelledby={headingId}>
      <header className="inbox__header">
        <h2 id={headingId} className="inbox__title">
          À organiser
        </h2>
        <div className="inbox__tools">
          <FilterSelect destinations={destinations} filter={filter} onChange={onFilterChange} />
          <button type="button" className="inbox__link" onClick={props.onShowTasks}>
            Tâches
          </button>
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
                initiationTarget={item.id === props.highlightId}
              />
            ))}
          </ol>
        </>
      )}
    </section>
  );
}
