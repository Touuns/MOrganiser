import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { INITIATION_KEY, markProposalShown, readProposalMemory } from "./initiationStore";

beforeEach(() => window.localStorage.clear());
afterEach(() => vi.restoreAllMocks());

describe("initiationStore", () => {
  it("clé absente : jamais présentée", () => {
    expect(readProposalMemory()).toBe("unseen");
  });

  it("après marquage : présentée, et la valeur survit à une relecture", () => {
    expect(markProposalShown()).toBe(true);
    expect(readProposalMemory()).toBe("seen");
    expect(JSON.parse(window.localStorage.getItem(INITIATION_KEY) as string)).toEqual({
      version: 1,
      parcours: { general: { proposee: true } },
    });
  });

  it("range l'état par parcours sans écraser les autres", () => {
    window.localStorage.setItem(
      INITIATION_KEY,
      JSON.stringify({ version: 1, parcours: { inventaire: { proposee: true } } }),
    );
    expect(readProposalMemory("general")).toBe("unseen");
    expect(readProposalMemory("inventaire")).toBe("seen");
    markProposalShown("general");
    expect(readProposalMemory("inventaire")).toBe("seen");
    expect(readProposalMemory("general")).toBe("seen");
  });

  it("contenu illisible : indisponible, et jamais écrasé", () => {
    window.localStorage.setItem(INITIATION_KEY, "{pas du json");
    expect(readProposalMemory()).toBe("unavailable");
    expect(markProposalShown()).toBe(false);
    expect(window.localStorage.getItem(INITIATION_KEY)).toBe("{pas du json");
  });

  it("stockage qui lève : lecture indisponible, écriture refusée, aucune exception", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("bloqué");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("plein");
    });
    expect(readProposalMemory()).toBe("unavailable");
    expect(markProposalShown()).toBe(false);
  });

  it("écriture silencieusement perdue : non confirmée", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => undefined);
    expect(markProposalShown()).toBe(false);
  });
});
