import { useCallback, useEffect, useRef, useState } from "react";
import {
  createInboxItem,
  listDestinations,
  listInboxItems,
  type Destination,
  type InboxFilter,
  type InboxItem,
} from "./api";
import { CaptureForm } from "./CaptureForm";
import { InboxPanel } from "./InboxPanel";
import "./InboxHome.css";

/** Nombre de captures affichées sur l'accueil (les plus récentes). */
export const HOME_LIMIT = 20;

/**
 * Accueil de la brique 001 : boîte « À organiser » au-dessus de la capture rapide.
 * La base de données reste la source de vérité : après chaque capture, la liste est relue.
 */
export function InboxHome() {
  const [destinations, setDestinations] = useState<Destination[]>([]);
  const [filter, setFilter] = useState<InboxFilter>({ type: "all" });
  const [items, setItems] = useState<InboxItem[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [status, setStatus] = useState("");
  // Seule la réponse de la dernière demande est affichée (changements de filtre rapides).
  const lastRequest = useRef(0);

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
  }, [filter, refresh]);

  async function handleCapture(content: string, destinationId: string | null) {
    const created = await createInboxItem(content, destinationId); // rejette → texte conservé
    const visible =
      filter.type === "all" ||
      (filter.type === "unclassified" && created.destinationId === null) ||
      (filter.type === "destination" && filter.id === created.destinationId);
    setStatus(visible ? "Capture enregistrée." : "Capture enregistrée (masquée par le filtre actuel).");
    void refresh(filter);
  }

  return (
    <div className="inbox-home">
      <InboxPanel
        items={items}
        total={total}
        destinations={destinations}
        filter={filter}
        onFilterChange={setFilter}
        loading={loading}
        loadError={loadError}
        onRetry={() => void refresh(filter)}
      />
      <CaptureForm destinations={destinations} onSubmit={handleCapture} />
      <p className="inbox-home__status" role="status">
        {status}
      </p>
    </div>
  );
}
