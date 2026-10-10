import { listInboxItems, listTrashedItems, type InboxPage } from "../inbox/api";

export type FirstUse =
  /** Aucune capture, ni active ni dans la corbeille. */
  | "first"
  /** Au moins une capture existe : installation déjà utilisée. */
  | "existing"
  /** Lecture en échec ou réponse incomplète : jamais traitée comme une base vide. */
  | "unknown";

function readable(page: InboxPage | undefined): page is InboxPage {
  return (
    typeof page === "object" &&
    page !== null &&
    Array.isArray(page.items) &&
    Number.isInteger(page.total) &&
    page.total >= 0
  );
}

/**
 * « Première utilisation » = aucune capture active ni dans la corbeille. C'est une approximation :
 * une base Dev vidée à la main ressemble à une première utilisation, et une installation qui
 * n'a que des données déjà supprimées définitivement aussi.
 */
export async function detectFirstUse(): Promise<FirstUse> {
  try {
    const [active, trashed] = await Promise.all([listInboxItems({ type: "all" }, 1), listTrashedItems(1)]);
    if (!readable(active) || !readable(trashed)) return "unknown";
    if (active.total > 0 || trashed.total > 0 || active.items.length > 0 || trashed.items.length > 0) {
      // Un total à zéro avec des éléments renvoyés est incohérent : on ne conclut rien.
      const inconsistent =
        (active.total === 0 && active.items.length > 0) || (trashed.total === 0 && trashed.items.length > 0);
      return inconsistent ? "unknown" : "existing";
    }
    return "first";
  } catch {
    return "unknown";
  }
}
