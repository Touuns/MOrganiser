import type { ReactNode } from "react";
import "./Badge.css";

export type BadgeTone = "neutral" | "dev" | "stable";

interface BadgeProps {
  tone?: BadgeTone;
  children: ReactNode;
}

/** Petite étiquette d'état (ex. environnement Dev/Stable). */
export function Badge({ tone = "neutral", children }: BadgeProps) {
  return <span className={`badge badge--${tone}`}>{children}</span>;
}
