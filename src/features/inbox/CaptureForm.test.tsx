import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
  const keep = screen.getByLabelText("Conserver ce choix") as HTMLInputElement;
  return { onSubmit, field, destination, send, keep };
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

  describe("Conserver ce choix", () => {
    const type = (field: HTMLElement, value: string) => fireEvent.change(field, { target: { value } });

    it("est décochée par défaut, et désactivée tant que la destination est « Aucune »", () => {
      const { keep, destination } = setup();
      expect(keep).not.toBeChecked();
      expect(keep).toBeDisabled();
      fireEvent.change(destination, { target: { value: "moi" } });
      expect(keep).toBeEnabled();
      expect(keep).not.toBeChecked();
    });

    it("décochée : la destination revient à Aucune après un envoi réussi", async () => {
      const { field, destination, keep } = setup();
      fireEvent.change(destination, { target: { value: "moi" } });
      type(field, "Une idée");
      enter(field);
      await waitFor(() => expect(field.value).toBe(""));
      expect(destination.value).toBe("");
      expect(keep).not.toBeChecked();
    });

    it("cochée : la destination reste après plusieurs envois réussis", async () => {
      const { onSubmit, field, destination, keep } = setup();
      fireEvent.change(destination, { target: { value: "finances" } });
      fireEvent.click(keep);
      type(field, "Première");
      enter(field);
      await waitFor(() => expect(field.value).toBe(""));
      type(field, "Seconde");
      enter(field);
      await waitFor(() => expect(field.value).toBe(""));
      expect(onSubmit).toHaveBeenNthCalledWith(1, "Première", "finances");
      expect(onSubmit).toHaveBeenNthCalledWith(2, "Seconde", "finances");
      expect(destination.value).toBe("finances");
      expect(keep).toBeChecked();
      expect(field).toHaveFocus();
    });

    it("cochée : un changement manuel de destination est conservé à son tour", async () => {
      const { onSubmit, field, destination, keep } = setup();
      fireEvent.change(destination, { target: { value: "moi" } });
      fireEvent.click(keep);
      fireEvent.change(destination, { target: { value: "externe" } });
      type(field, "Pour l'externe");
      enter(field);
      await waitFor(() => expect(field.value).toBe(""));
      expect(onSubmit).toHaveBeenCalledWith("Pour l'externe", "externe");
      expect(destination.value).toBe("externe");
      expect(keep).toBeChecked();
    });

    it("choisir « Aucune » désactive la conservation", async () => {
      const { field, destination, keep } = setup();
      fireEvent.change(destination, { target: { value: "moi" } });
      fireEvent.click(keep);
      fireEvent.change(destination, { target: { value: "" } });
      expect(keep).not.toBeChecked();
      expect(keep).toBeDisabled();
      fireEvent.change(destination, { target: { value: "finances" } });
      expect(keep).not.toBeChecked();
      type(field, "Texte");
      enter(field);
      await waitFor(() => expect(field.value).toBe(""));
      expect(destination.value).toBe("");
    });

    it("décocher garde la destination en cours, réinitialisée à l'envoi suivant", async () => {
      const { field, destination, keep } = setup();
      fireEvent.change(destination, { target: { value: "moi" } });
      fireEvent.click(keep);
      fireEvent.click(keep);
      expect(destination.value).toBe("moi");
      type(field, "Texte");
      enter(field);
      await waitFor(() => expect(field.value).toBe(""));
      expect(destination.value).toBe("");
    });

    it("échec de sauvegarde : texte, destination et option conservés, puis maintenus au succès", async () => {
      const onSubmit = vi
        .fn<(c: string, d: string | null) => Promise<void>>()
        .mockRejectedValueOnce(new Error("Échec"))
        .mockResolvedValueOnce();
      const { field, destination, keep } = setup(onSubmit);
      fireEvent.change(destination, { target: { value: "externe" } });
      fireEvent.click(keep);
      type(field, "À garder");
      enter(field);
      await screen.findByRole("alert");
      expect(field.value).toBe("À garder");
      expect(destination.value).toBe("externe");
      expect(keep).toBeChecked();
      enter(field);
      await waitFor(() => expect(field.value).toBe(""));
      expect(destination.value).toBe("externe");
      expect(keep).toBeChecked();
    });

    it("cochée pendant un envoi en cours : la destination n'est pas effacée au retour", async () => {
      const pending = deferred();
      const onSubmit = vi.fn<(c: string, d: string | null) => Promise<void>>().mockReturnValue(pending.promise);
      const { field, destination, keep } = setup(onSubmit);
      fireEvent.change(destination, { target: { value: "moi" } });
      type(field, "Texte");
      enter(field);
      fireEvent.click(keep);
      await act(async () => pending.resolve());
      expect(destination.value).toBe("moi");
    });

    it("n'est pas mémorisée : une nouvelle ouverture repart sans conservation", () => {
      const first = setup();
      fireEvent.change(first.destination, { target: { value: "moi" } });
      fireEvent.click(first.keep);
      cleanup();
      const second = setup();
      expect(second.keep).not.toBeChecked();
      expect(second.destination.value).toBe("");
    });
  });
});
