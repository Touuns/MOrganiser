import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UNDO_DURATION_MS, UndoToast } from "./UndoToast";

describe("UndoToast", () => {
  it("dure environ 8 secondes par défaut", () => {
    expect(UNDO_DURATION_MS).toBe(8000);
  });

  it("annonce le message et propose d'annuler", () => {
    const onAction = vi.fn();
    render(<UndoToast message="Capture déplacée dans la corbeille." actionLabel="Annuler" onAction={onAction} onDismiss={() => {}} />);
    expect(screen.getByText("Capture déplacée dans la corbeille.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Annuler" }));
    expect(onAction).toHaveBeenCalledTimes(1);
  });

  it("disparaît tout seul à l'issue de la durée", async () => {
    const onDismiss = vi.fn();
    render(
      <UndoToast message="Vite" actionLabel="Annuler" onAction={() => {}} onDismiss={onDismiss} durationMs={30} />,
    );
    expect(onDismiss).not.toHaveBeenCalled();
    await waitFor(() => expect(onDismiss).toHaveBeenCalledTimes(1));
  });

  it("ne se déclenche pas après démontage", async () => {
    const onDismiss = vi.fn();
    const { unmount } = render(
      <UndoToast message="Vite" actionLabel="Annuler" onAction={() => {}} onDismiss={onDismiss} durationMs={30} />,
    );
    unmount();
    await new Promise((resolve) => setTimeout(resolve, 80));
    expect(onDismiss).not.toHaveBeenCalled();
  });
});
