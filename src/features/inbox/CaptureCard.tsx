import type { ReactNode } from "react";
import { Badge } from "../../components/Badge";
import type { InboxItem } from "./api";
import { formatShort, isoDate } from "./format";
import "./CaptureCard.css";

interface CaptureCardProps {
  item: InboxItem;
  /** Libellé de la destination, ou `null` si la capture n'en a pas (« À classer »). */
  destinationLabel: string | null;
  selected: boolean;
  onOpen: (item: InboxItem) => void;
  /** Date affichée (création par défaut ; suppression dans la corbeille). */
  date?: number;
  datePrefix?: string;
  /** Bouton supplémentaire (ex. « Restaurer »), à côté de la zone cliquable. */
  action?: ReactNode;
}

/** Carte d'une capture : ouvrir la fiche au clic ou à `Entrée`. */
export function CaptureCard(props: CaptureCardProps) {
  const { item, destinationLabel, selected, onOpen, action, datePrefix } = props;
  const date = props.date ?? item.createdAt;
  return (
    <li className="inbox__item" data-capture-id={item.id} data-selected={selected || undefined}>
      <button
        type="button"
        className="inbox__open"
        aria-current={selected ? "true" : undefined}
        onClick={() => onOpen(item)}
      >
        <span className="inbox__content">{item.content}</span>
        <span className="inbox__meta">
          {destinationLabel ? (
            <Badge tone="accent">{destinationLabel}</Badge>
          ) : (
            <Badge>À classer</Badge>
          )}
          <time dateTime={isoDate(date)}>
            {datePrefix}
            {formatShort(date)}
          </time>
        </span>
      </button>
      {action}
    </li>
  );
}
