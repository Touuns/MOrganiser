/**
 * Animation d'arrivée d'une capture : une carte fantôme monte de la zone de saisie jusqu'à
 * l'emplacement de la vraie carte, déjà insérée dans la liste (la base de données et la liste
 * sont la source de vérité ; ce mouvement est purement visuel).
 *
 * Technique : Web Animations API, sans dépendance. La carte fantôme est un clone de la vraie
 * carte, en `position: fixed`, non interactif et ignoré des technologies d'assistance. La vraie
 * carte n'est que masquée visuellement (`visibility`) pendant le trajet et TOUJOURS rétablie :
 * fin normale, annulation ou délai de sécurité.
 */

/** Durée du trajet, validée par le propriétaire. */
export const ARRIVAL_DURATION_MS = 350;
export const ARRIVAL_EASING = "cubic-bezier(0.2, 0.8, 0.2, 1)";
/** Marge au-delà de la durée : si l'animation ne signale jamais sa fin, on rétablit quand même. */
const SAFETY_MARGIN_MS = 300;

export interface ArrivalHandle {
  /** Arrête le mouvement et rend la vraie carte visible immédiatement. Idempotent. */
  cancel: () => void;
}

export function prefersReducedMotion(): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Positions (bord supérieur, repère de la fenêtre) des cartes présentes, par identifiant. */
export function snapshotTops(list: HTMLElement): Map<string, number> {
  const tops = new Map<string, number>();
  list.querySelectorAll<HTMLElement>("[data-capture-id]").forEach((card) => {
    const rect = card.getBoundingClientRect();
    if (rect.height > 0) tops.set(card.dataset.captureId as string, rect.top);
  });
  return tops;
}

function isVisibleIn(rect: DOMRect, container: DOMRect): boolean {
  return (
    rect.width > 0 &&
    rect.height > 0 &&
    container.width > 0 &&
    rect.bottom > container.top &&
    rect.top < container.bottom
  );
}

interface PlayOptions {
  /** La vraie carte, déjà présente dans la liste. */
  card: HTMLElement;
  /** Le conteneur défilant de la liste. */
  container: HTMLElement;
  /** Bord supérieur de la zone de saisie (repère de la fenêtre). */
  fromTop: number;
  /** Positions des cartes avant l'insertion : elles glissent vers leur nouvelle place. */
  previousTops?: Map<string, number>;
  durationMs?: number;
}

/**
 * Lance l'animation. Renvoie `null` (et ne touche à rien) si elle n'a pas lieu d'être :
 * mouvement réduit, API indisponible, ou destination non visible (fenêtre étroite, liste
 * masquée, carte hors de la zone visible) — la carte est alors simplement insérée.
 */
export function playArrival(options: PlayOptions): ArrivalHandle | null {
  const { card, container, fromTop, previousTops } = options;
  const duration = options.durationMs ?? ARRIVAL_DURATION_MS;
  if (prefersReducedMotion() || typeof card.animate !== "function") return null;

  const end = card.getBoundingClientRect();
  if (!isVisibleIn(end, container.getBoundingClientRect())) return null;

  const timing: KeyframeAnimationOptions = { duration, easing: ARRIVAL_EASING, fill: "both" };
  const running: Animation[] = [];
  let ghost: HTMLElement | null = null;
  let timer: number | undefined;
  let finished = false;

  const finish = () => {
    if (finished) return;
    finished = true;
    window.clearTimeout(timer);
    running.forEach((animation) => {
      try {
        animation.cancel();
      } catch {
        // déjà terminée
      }
    });
    ghost?.remove();
    card.style.visibility = ""; // la vraie carte est toujours rétablie
  };

  try {
    // Carte fantôme : clone fidèle de la vraie carte (même taille, même apparence).
    ghost = card.cloneNode(true) as HTMLElement;
    ghost.removeAttribute("data-capture-id");
    ghost.removeAttribute("data-selected");
    ghost.querySelectorAll("[id]").forEach((node) => node.removeAttribute("id"));
    ghost.classList.add("arrival-ghost");
    ghost.setAttribute("aria-hidden", "true");
    ghost.setAttribute("inert", "");
    Object.assign(ghost.style, {
      position: "fixed",
      top: `${end.top}px`,
      left: `${end.left}px`,
      width: `${end.width}px`,
      height: `${end.height}px`,
      margin: "0",
    });
    document.body.appendChild(ghost);
    card.style.visibility = "hidden";

    // Trajet : de la zone de saisie (en bas) vers la place définitive, principalement vertical.
    const dy = fromTop - end.top;
    running.push(
      ghost.animate(
        [
          { transform: `translateY(${dy}px)`, opacity: 0.55 },
          { transform: "translateY(0)", opacity: 1 },
        ],
        timing,
      ),
    );

    // Les cartes déjà présentes glissent de leur ancienne place vers la nouvelle (FLIP) au
    // lieu de sauter brutalement lorsque la liste défile jusqu'en bas.
    if (previousTops) {
      const containerRect = container.getBoundingClientRect();
      container.querySelectorAll<HTMLElement>("[data-capture-id]").forEach((other) => {
        if (other === card) return;
        const before = previousTops.get(other.dataset.captureId as string);
        if (before === undefined) return;
        const rect = other.getBoundingClientRect();
        const delta = before - rect.top;
        if (Math.abs(delta) < 1 || !isVisibleIn(rect, containerRect)) return;
        running.push(other.animate([{ transform: `translateY(${delta}px)` }, { transform: "none" }], timing));
      });
    }

    timer = window.setTimeout(finish, duration + SAFETY_MARGIN_MS);
    const first = running[0];
    first.finished.then(finish, finish);
  } catch {
    finish(); // quoi qu'il arrive, aucune carte ne reste masquée
    return null;
  }

  return { cancel: finish };
}

// ---------------------------------------------------------------------------------------
// Sortie d'une carte (mise à la corbeille)
// ---------------------------------------------------------------------------------------

/** Durée de la disparition d'une carte mise à la corbeille. */
export const DEPARTURE_DURATION_MS = 220;
export const DEPARTURE_EASING = "cubic-bezier(0.4, 0, 1, 1)";

export interface DepartureHandle {
  /**
   * Arrête la transition et retire la carte fantôme. La carte réelle n'est JAMAIS laissée
   * invisible par l'animation : si elle est encore dans le DOM (liste pas encore relue ou relecture
   * en échec), elle est rendue visible mais marquée « sortie » (atténuée, non interactive).
   * `restoreCard` (restauration) la remet à l'état normal et interactif. Idempotent, y compris
   * après la fin de la transition.
   */
  cancel: (options?: { restoreCard?: boolean }) => void;
}

/**
 * Fait disparaître en fondu (léger rétrécissement) une carte déjà retirée par la base de
 * données. La mise à la corbeille n'est JAMAIS retardée : la carte fantôme n'est qu'un reflet
 * visuel, non interactif, qui ne peut pas rendre la capture de nouveau active. Quand la liste
 * est relue et que la vraie carte quitte le DOM, les cartes restantes glissent vers leur place.
 *
 * Renvoie `null` (sans rien toucher) si la transition n'a pas lieu d'être : mouvement réduit,
 * API indisponible ou carte non visible.
 */
export function playDeparture(options: { card: HTMLElement; durationMs?: number }): DepartureHandle | null {
  const { card } = options;
  const duration = options.durationMs ?? DEPARTURE_DURATION_MS;
  const container = card.closest<HTMLElement>(".inbox__list, .paged__scroll");
  if (!container || prefersReducedMotion() || typeof card.animate !== "function") return null;

  const rect = card.getBoundingClientRect();
  if (!isVisibleIn(rect, container.getBoundingClientRect())) return null;

  // Positions des autres cartes avant la relecture de la liste.
  const before = new Map<string, number>();
  container.querySelectorAll<HTMLElement>("[data-capture-id]").forEach((other) => {
    if (other === card) return;
    const otherRect = other.getBoundingClientRect();
    if (otherRect.height > 0) before.set(other.dataset.captureId as string, otherRect.top);
  });

  const timing: KeyframeAnimationOptions = { duration, easing: DEPARTURE_EASING, fill: "forwards" };
  const shifts: Animation[] = [];
  let ghost: HTMLElement | null = null;
  let ghostAnimation: Animation | null = null;
  let observer: MutationObserver | null = null;
  let timer: number | undefined;
  let observerTimer: number | undefined;
  let done = false;

  const stopObserving = () => {
    observer?.disconnect();
    window.clearTimeout(observerTimer);
  };
  /**
   * État de la carte réelle quand le mouvement s'arrête : toujours visible. Si la liste ne l'a
   * pas (encore) retirée, elle est marquée comme sortie : la base la sait à la corbeille, la
   * liste périmée ne doit pas la présenter comme active.
   */
  const settleCard = (restore: boolean) => {
    if (!card.isConnected) return;
    card.style.visibility = "";
    if (restore) {
      card.removeAttribute("data-departed");
      card.removeAttribute("inert");
    } else {
      card.setAttribute("data-departed", "");
      card.setAttribute("inert", "");
    }
  };
  const finish = (restoreCard = false) => {
    if (done) {
      if (restoreCard) settleCard(true); // restauration après la fin : on retire la marque
      return;
    }
    done = true;
    window.clearTimeout(timer);
    stopObserving();
    try {
      ghostAnimation?.cancel();
      shifts.forEach((animation) => animation.cancel());
    } catch {
      // déjà terminées
    }
    ghost?.remove();
    settleCard(restoreCard);
  };

  try {
    ghost = card.cloneNode(true) as HTMLElement;
    ghost.removeAttribute("data-capture-id");
    ghost.removeAttribute("data-selected");
    ghost.querySelectorAll("[id]").forEach((node) => node.removeAttribute("id"));
    ghost.classList.add("arrival-ghost", "departure-ghost");
    ghost.setAttribute("aria-hidden", "true");
    ghost.setAttribute("inert", "");
    Object.assign(ghost.style, {
      position: "fixed",
      top: `${rect.top}px`,
      left: `${rect.left}px`,
      width: `${rect.width}px`,
      height: `${rect.height}px`,
      margin: "0",
    });
    document.body.appendChild(ghost);
    card.style.visibility = "hidden";

    ghostAnimation = ghost.animate(
      [
        { opacity: 1, transform: "scale(1)" },
        { opacity: 0, transform: "scale(0.96)" },
      ],
      timing,
    );

    // Dès que la liste a retiré la vraie carte, les autres glissent de leur ancienne place
    // vers la nouvelle (FLIP) au lieu de sauter.
    observer = new MutationObserver(() => {
      if (card.isConnected || !container.isConnected) return;
      stopObserving();
      const containerRect = container.getBoundingClientRect();
      container.querySelectorAll<HTMLElement>("[data-capture-id]").forEach((other) => {
        const top = before.get(other.dataset.captureId as string);
        if (top === undefined) return;
        const otherRect = other.getBoundingClientRect();
        const delta = top - otherRect.top;
        if (Math.abs(delta) < 1 || !isVisibleIn(otherRect, containerRect)) return;
        shifts.push(other.animate([{ transform: `translateY(${delta}px)` }, { transform: "none" }], timing));
      });
    });
    observer.observe(container, { childList: true, subtree: true });
    observerTimer = window.setTimeout(stopObserving, 2000);

    timer = window.setTimeout(() => finish(false), duration + SAFETY_MARGIN_MS);
    ghostAnimation.finished.then(() => finish(false), () => finish(false));
  } catch {
    finish(false);
    return null;
  }

  return { cancel: (cancelOptions) => finish(cancelOptions?.restoreCard ?? false) };
}
