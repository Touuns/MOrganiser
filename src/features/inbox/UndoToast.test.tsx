import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UNDO_DURATION_MS, UndoToast } from "./UndoToast";

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function setup(durationMs: number, onDismiss = vi.fn(), onAction = vi.fn()) {
  render(
    <UndoToast
      message="Déplacée dans la corbeille"
      actionLabel="Annuler"
      onAction={onAction}
      onDismiss={onDismiss}
      durationMs={durationMs}
    />,
  );
  return { onDismiss, onAction, toast: screen.getByText("Déplacée dans la corbeille").parentElement! };
}

describe("UndoToast", () => {
  it("dure 4 secondes par défaut", () => {
    expect(UNDO_DURATION_MS).toBe(4000);
  });

  it("affiche le message court et un bouton Annuler identifiable", () => {
    const { onAction } = setup(1000);
    expect(screen.getByText("Déplacée dans la corbeille")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(onAction).toHaveBeenCalledTimes(1);
  });

  it("disparaît tout seul à l'issue de la durée", async () => {
    const { onDismiss } = setup(60);
    expect(onDismiss).not.toHaveBeenCalled();
    await waitFor(() => expect(onDismiss).toHaveBeenCalledTimes(1));
  });

  it("le compte à rebours est suspendu au survol puis reprend avec le temps restant", async () => {
    const { onDismiss, toast } = setup(300);
    await wait(100);
    fireEvent.mouseEnter(toast);
    await wait(500); // bien au-delà de la durée : toujours affichée
    expect(onDismiss).not.toHaveBeenCalled();

    const left = performance.now();
    fireEvent.mouseLeave(toast);
    await waitFor(() => expect(onDismiss).toHaveBeenCalledTimes(1));
    // Reprise avec ≈ 200 ms restantes (et non 300 ms, ni 0).
    const elapsed = performance.now() - left;
    expect(elapsed).toBeGreaterThan(100);
    expect(elapsed).toBeLessThan(290);
  });

  it("le compte à rebours est suspendu tant que le bouton a le focus", async () => {
    const { onDismiss } = setup(150);
    const button = screen.getByRole("button", { name: "Annuler" });
    fireEvent.focus(button);
    await wait(400);
    expect(onDismiss).not.toHaveBeenCalled();
    fireEvent.blur(button);
    await waitFor(() => expect(onDismiss).toHaveBeenCalledTimes(1));
  });

  it("souris et focus cumulés : la reprise attend que les deux aient cessé", async () => {
    const { onDismiss, toast } = setup(120);
    const button = screen.getByRole("button", { name: "Annuler" });
    fireEvent.mouseEnter(toast);
    fireEvent.focus(button);
    fireEvent.mouseLeave(toast);
    await wait(300);
    expect(onDismiss).not.toHaveBeenCalled(); // le focus la retient encore
    fireEvent.blur(button);
    await waitFor(() => expect(onDismiss).toHaveBeenCalledTimes(1));
  });

  it("ne se déclenche pas après démontage", async () => {
    const onDismiss = vi.fn();
    const { unmount } = render(
      <UndoToast message="x" actionLabel="Annuler" onAction={() => {}} onDismiss={onDismiss} durationMs={40} />,
    );
    unmount();
    await wait(120);
    expect(onDismiss).not.toHaveBeenCalled();
  });
});
