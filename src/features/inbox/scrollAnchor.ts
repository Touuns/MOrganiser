/**
 * Ancrage de défilement : conserve la zone de lecture quand la liste change (suppression,
 * restauration, relecture, entrée d'une capture plus ancienne dans l'ensemble affiché).
 *
 * Principe : on mémorise quelques cartes visibles avec leur position dans le conteneur ; après
 * un changement, la première encore présente est remise exactement à la même position visuelle.
 * Si la carte supprimée servait d'ancre, la suivante de la liste mémorisée la remplace.
 */

export interface Anchor {
  id: string;
  /** Distance entre le haut de la carte et le haut du conteneur visible. */
  offset: number;
}

const MAX_ANCHORS = 6;

const cardsIn = (container: HTMLElement) =>
  container.querySelectorAll<HTMLElement>("[data-capture-id]");

/** Cartes visibles (au moins en partie), dans l'ordre d'affichage. */
export function recordAnchors(container: HTMLElement): Anchor[] {
  const top = container.getBoundingClientRect().top;
  const anchors: Anchor[] = [];
  for (const card of cardsIn(container)) {
    const rect = card.getBoundingClientRect();
    if (rect.height > 0 && rect.bottom > top) {
      anchors.push({ id: card.dataset.captureId as string, offset: rect.top - top });
      if (anchors.length >= MAX_ANCHORS) break;
    }
  }
  return anchors;
}

/**
 * Remet la première ancre encore présente à sa position visuelle d'origine.
 * Renvoie `false` si aucune n'a pu servir (liste vide, rien de mesurable).
 */
export function restoreAnchors(container: HTMLElement, anchors: Anchor[]): boolean {
  if (anchors.length === 0) return false;
  const present = new Map<string, HTMLElement>();
  cardsIn(container).forEach((card) => present.set(card.dataset.captureId as string, card));
  const top = container.getBoundingClientRect().top;
  for (const anchor of anchors) {
    const card = present.get(anchor.id);
    if (!card) continue;
    const rect = card.getBoundingClientRect();
    if (rect.height === 0) continue;
    container.scrollTop += rect.top - top - anchor.offset;
    return true;
  }
  return false;
}
