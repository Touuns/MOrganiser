import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Destination } from "./api";
import { CaptureForm } from "./CaptureForm";

const destinations: Destination[] = [
  { id: "moi", label: "Moi", kind: "responsibility" },
  { id: "externe", label: "Externe", kind: "responsibility" },
  { id: "finances", label: "Finances", kind: "section" },
];

/** Promesse contrôlable pour simuler un enregistrement en cours. */
function deferred() {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function setup(onSubmit = vi.fn<(c: string, d: string | null) => Promise<void>>().mockResolvedValue()) {
  render(<CaptureForm destinations={destinations} onSubmit={onSubmit} />);
  const field = screen.getByLabelText("Capture rapide", { selector: "textarea" }) as HTMLTextAreaElement;
  const destination = screen.getByLabelText("Destination (facultatif)") as HTMLSelectElement;
  const send = screen.getByRole("button", { name: /Envoyer/ });
  return { onSubmit, field, destination, send };
}

const enter = (field: HTMLElement, init: KeyboardEventInit = {}) =>
  fireEvent.keyDown(field, { key: "Enter", ...init });

describe("CaptureForm", () => {
  it("a le focus à l'ouverture et propose « Aucune » par défaut", () => {
    const { field, destination } = setup();
    expect(field).toHaveFocus();
    expect(destination.value).toBe("");
    expect(screen.getByRole("option", { name: "Aucune" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Espaces" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Rubriques" })).toBeInTheDocument();
    expect(screen.queryByText(/tags?/i)).not.toBeInTheDocument();
  });

  it("Entrée envoie le texte, sans destination", async () => {
    const { onSubmit, field } = setup();
    fireEvent.change(field, { target: { value: "Vendre ma PlayStation 5" } });
    enter(field);
    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith("Vendre ma PlayStation 5", null));
  });

  it("Maj+Entrée n'envoie pas (retour à la ligne)", () => {
    const { onSubmit, field } = setup();
    fireEvent.change(field, { target: { value: "Ligne 1" } });
    enter(field, { shiftKey: true });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("Entrée pendant une composition (accents IME) n'envoie pas", () => {
    const { onSubmit, field } = setup();
    fireEvent.change(field, { target: { value: "é" } });
    enter(field, { isComposing: true });
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("le bouton Envoyer fait la même chose, avec la destination choisie", async () => {
    const { onSubmit, field, destination, send } = setup();
    fireEvent.change(field, { target: { value: "Rembourser Paul" } });
    fireEvent.change(destination, { target: { value: "finances" } });
    fireEvent.click(send);
    await waitFor(() => expect(onSubmit).toHaveBeenCalledWith("Rembourser Paul", "finances"));
  });

  it("une saisie vide ou d'espaces n'envoie rien", () => {
    const { onSubmit, field, send } = setup();
    enter(field);
    fireEvent.change(field, { target: { value: "   \n  " } });
    enter(field);
    fireEvent.click(send);
    expect(onSubmit).not.toHaveBeenCalled();
  });

  it("après succès : champ vidé, destination remise à Aucune, focus dans le champ", async () => {
    const { field, destination, send } = setup();
    fireEvent.change(field, { target: { value: "Appeler la banque" } });
    fireEvent.change(destination, { target: { value: "moi" } });
    send.focus();
    fireEvent.click(send);
    await waitFor(() => expect(field.value).toBe(""));
    expect(destination.value).toBe("");
    expect(field).toHaveFocus();
  });

  it("des validations répétées pendant l'envoi ne créent qu'un seul envoi", async () => {
    const pending = deferred();
    const onSubmit = vi.fn<(c: string, d: string | null) => Promise<void>>().mockReturnValue(pending.promise);
    const { field, send } = setup(onSubmit);
    fireEvent.change(field, { target: { value: "Une seule fois" } });
    enter(field);
    enter(field);
    fireEvent.click(send);
    enter(field);
    expect(onSubmit).toHaveBeenCalledTimes(1);
    expect(send).toBeDisabled();
    await act(async () => pending.resolve());
    expect(send).not.toBeDisabled();
  });

  it("en cas d'échec : texte et destination conservés, erreur affichée", async () => {
    const onSubmit = vi
      .fn<(c: string, d: string | null) => Promise<void>>()
      .mockRejectedValue(new Error("L'enregistrement local a échoué."));
    const { field, destination } = setup(onSubmit);
    fireEvent.change(field, { target: { value: "Texte précieux" } });
    fireEvent.change(destination, { target: { value: "externe" } });
    enter(field);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "L'enregistrement local a échoué. Votre texte est conservé",
    );
    expect(field.value).toBe("Texte précieux");
    expect(destination.value).toBe("externe");
  });

  it("on peut réessayer après un échec", async () => {
    const onSubmit = vi
      .fn<(c: string, d: string | null) => Promise<void>>()
      .mockRejectedValueOnce(new Error("Échec"))
      .mockResolvedValueOnce();
    const { field } = setup(onSubmit);
    fireEvent.change(field, { target: { value: "Deuxième essai" } });
    enter(field);
    await screen.findByRole("alert");
    enter(field);
    await waitFor(() => expect(field.value).toBe(""));
    expect(onSubmit).toHaveBeenCalledTimes(2);
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("un texte tapé pendant l'envoi n'est pas effacé", async () => {
    const pending = deferred();
    const onSubmit = vi.fn<(c: string, d: string | null) => Promise<void>>().mockReturnValue(pending.promise);
    const { field } = setup(onSubmit);
    fireEvent.change(field, { target: { value: "Première idée" } });
    enter(field);
    fireEvent.change(field, { target: { value: "Seconde idée en cours" } });
    await act(async () => pending.resolve());
    expect(field.value).toBe("Seconde idée en cours");
  });
});
