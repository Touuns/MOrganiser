import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

/*
 * jsdom n'implémente pas `HTMLDialogElement.showModal()` / `close()`. Ce shim, limité aux tests,
 * se contente de reproduire ce que l'application observe : l'attribut `open` et l'événement
 * `close`. Il n'émule ni la couche supérieure, ni le piège à focus, ni l'inertie du reste de la
 * page : ces comportements natifs se vérifient dans la vraie fenêtre.
 */
if (typeof HTMLDialogElement !== "undefined" && typeof HTMLDialogElement.prototype.showModal !== "function") {
  HTMLDialogElement.prototype.showModal = function showModal(this: HTMLDialogElement) {
    this.setAttribute("open", "");
  };
  HTMLDialogElement.prototype.close = function close(this: HTMLDialogElement) {
    if (!this.hasAttribute("open")) return;
    this.removeAttribute("open");
    this.dispatchEvent(new Event("close"));
  };
}

afterEach(() => {
  cleanup();
});
