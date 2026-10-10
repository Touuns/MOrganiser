import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef } from "react";

/**
 * Protège la fermeture normale de la fenêtre (bouton `×`, Alt+F4, « Quitter ») : si
 * `shouldBlock()` répond oui, la fermeture est suspendue et `onBlocked` reçoit
 * `closeWindow`, à appeler seulement quand l'utilisateur a tranché.
 *
 * Sans blocage, le gestionnaire ne fait rien : Tauri ferme la fenêtre comme d'habitude.
 * `closeWindow` détruit la fenêtre directement : aucune nouvelle demande de fermeture
 * n'est émise, donc aucune boucle de confirmation.
 *
 * Limite : un arrêt forcé du processus ou une coupure de courant ne sont pas interceptables.
 */
export function useCloseGuard(
  shouldBlock: () => boolean,
  onBlocked: (closeWindow: () => void) => void,
) {
  // Toujours les dernières fonctions, sans réinscrire l'écouteur à chaque rendu.
  const latest = useRef({ shouldBlock, onBlocked });
  latest.current = { shouldBlock, onBlocked };

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    try {
      const appWindow = getCurrentWindow();
      appWindow
        .onCloseRequested((event) => {
          if (!latest.current.shouldBlock()) return; // fermeture normale
          event.preventDefault();
          latest.current.onBlocked(() => void appWindow.destroy());
        })
        .then((stop) => {
          if (cancelled) stop();
          else unlisten = stop;
        })
        .catch(() => undefined);
    } catch {
      // Hors de la fenêtre Tauri (tests, navigateur) : pas de protection à poser.
    }

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);
}
