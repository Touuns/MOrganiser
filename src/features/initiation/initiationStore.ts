/**
 * Mémoire locale de l'initiation : « la proposition d'accueil a-t-elle déjà été présentée ? ».
 *
 * Stockée dans le `localStorage` de la WebView. Chaque environnement a son propre dossier
 * WebView2 (`%LOCALAPPDATA%\<identifiant Tauri>\EBWebView`) : Dev et Stable ne partagent donc
 * jamais ce réglage. Aucune donnée personnelle n'y est écrite.
 *
 * Le format est rangé par parcours pour que la brique 007 puisse ajouter d'autres parcours
 * sans migration ; seul le parcours « general » existe pour l'instant.
 */
export const INITIATION_KEY = "morganiser.initiation.v1";

export type ProposalMemory =
  /** Jamais présentée : une première utilisation peut la déclencher. */
  | "unseen"
  /** Déjà présentée (ou installation existante) : ne plus la proposer d'elle-même. */
  | "seen"
  /** Stockage inaccessible ou contenu illisible : ne rien proposer, ne rien écraser. */
  | "unavailable";

interface StoredState {
  version: 1;
  parcours: Record<string, { proposee?: boolean }>;
}

function parse(raw: string): StoredState | null {
  try {
    const value: unknown = JSON.parse(raw);
    if (typeof value !== "object" || value === null) return null;
    const { version, parcours } = value as Partial<StoredState>;
    if (version !== 1 || typeof parcours !== "object" || parcours === null) return null;
    return { version: 1, parcours };
  } catch {
    return null;
  }
}

export function readProposalMemory(parcours = "general"): ProposalMemory {
  try {
    const raw = window.localStorage.getItem(INITIATION_KEY);
    if (raw === null) return "unseen";
    const state = parse(raw);
    if (!state) return "unavailable";
    return state.parcours[parcours]?.proposee === true ? "seen" : "unseen";
  } catch {
    return "unavailable";
  }
}

/** Enregistre que la proposition a été présentée ; `false` si l'écriture n'a pas pu être vérifiée. */
export function markProposalShown(parcours = "general"): boolean {
  try {
    const raw = window.localStorage.getItem(INITIATION_KEY);
    const existing = raw === null ? null : parse(raw);
    if (raw !== null && !existing) return false; // contenu illisible : on ne l'écrase pas
    const next: StoredState = existing ?? { version: 1, parcours: {} };
    next.parcours[parcours] = { ...next.parcours[parcours], proposee: true };
    const serialized = JSON.stringify(next);
    window.localStorage.setItem(INITIATION_KEY, serialized);
    return window.localStorage.getItem(INITIATION_KEY) === serialized;
  } catch {
    return false;
  }
}
