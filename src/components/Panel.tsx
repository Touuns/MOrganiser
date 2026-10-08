import { useId, type ReactNode } from "react";
import "./Panel.css";

interface PanelProps {
  title: string;
  children: ReactNode;
}

/** Surface de contenu titrée. Brique de base des futures sections du tableau de bord. */
export function Panel({ title, children }: PanelProps) {
  const headingId = useId();
  return (
    <section className="panel" aria-labelledby={headingId}>
      <h2 id={headingId} className="panel__title">
        {title}
      </h2>
      <div className="panel__body">{children}</div>
    </section>
  );
}
