import { fireEvent, render, screen } from "@testing-library/react";
import { useRef, useState } from "react";
import { describe, expect, it, vi } from "vitest";
import { Modal } from "./Modal";

function Content({ onClose }: { onClose: () => void }) {
  const first = useRef<HTMLInputElement>(null);
  return (
    <Modal labelledBy="t" describedBy="d" onRequestClose={onClose} initialFocus={first}>
      <h2 id="t">Titre de la modale</h2>
      <p id="d">Description</p>
      <button type="button">Premier</button>
      <input ref={first} aria-label="Champ" />
    </Modal>
  );
}

describe("Modal générique", () => {
  it("s'ouvre comme une vraie modale nommée et décrite, focus sur l'élément demandé", () => {
    render(<Content onClose={() => undefined} />);
    const dialog = screen.getByRole("dialog", { name: "Titre de la modale" });
    expect(dialog).toHaveAttribute("open");
    expect(dialog).toHaveAccessibleDescription("Description");
    expect(screen.getByLabelText("Champ")).toHaveFocus();
  });

  it("Échap demande la fermeture une seule fois et n'atteint pas les parents", () => {
    const onClose = vi.fn();
    const parentKey = vi.fn();
    render(
      <div onKeyDown={parentKey}>
        <Content onClose={onClose} />
      </div>,
    );
    fireEvent.keyDown(screen.getByLabelText("Champ"), { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(parentKey).not.toHaveBeenCalled(); // les événements React suivent l'arbre : arrêtés
    // Le navigateur déclenche aussi `cancel` : pas de seconde demande pour la même touche.
    fireEvent(screen.getByRole("dialog"), new Event("cancel", { cancelable: true }));
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("une demande de fermeture du système (cancel) est annulée puis transmise une fois", () => {
    const onClose = vi.fn();
    render(<Content onClose={onClose} />);
    const event = new Event("cancel", { cancelable: true });
    fireEvent(screen.getByRole("dialog"), event);
    expect(event.defaultPrevented).toBe(true); // jamais de fermeture native : l'appelant décide
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("un clic sur l'arrière-plan (cible : le dialogue) ne demande aucune fermeture", () => {
    const onClose = vi.fn();
    render(<Content onClose={onClose} />);
    const dialog = screen.getByRole("dialog");
    fireEvent.mouseDown(dialog);
    fireEvent.mouseUp(dialog);
    fireEvent.click(dialog);
    expect(onClose).not.toHaveBeenCalled();
    expect(dialog).toHaveAttribute("open");
  });

  it("rend le focus à l'élément qui l'a ouverte quand elle disparaît", () => {
    function Host() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button type="button" onClick={() => setOpen(true)}>
            Ouvrir
          </button>
          {open && <Content onClose={() => setOpen(false)} />}
        </>
      );
    }
    render(<Host />);
    const opener = screen.getByRole("button", { name: "Ouvrir" });
    opener.focus();
    fireEvent.click(opener);
    expect(screen.getByLabelText("Champ")).toHaveFocus();

    fireEvent.keyDown(screen.getByLabelText("Champ"), { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(opener).toHaveFocus();
  });

  it("ne s'ouvre qu'une fois malgré un double montage", () => {
    const show = vi.spyOn(HTMLDialogElement.prototype, "showModal");
    const { rerender } = render(<Content onClose={() => undefined} />);
    rerender(<Content onClose={() => undefined} />);
    expect(show).toHaveBeenCalledTimes(1);
    show.mockRestore();
  });
});
