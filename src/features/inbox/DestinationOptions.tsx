import type { Destination } from "./api";

/** Options d'un <select>, regroupées : espaces de responsabilité puis rubriques. */
export function DestinationOptions({ destinations }: { destinations: Destination[] }) {
  const spaces = destinations.filter((d) => d.kind === "responsibility");
  const sections = destinations.filter((d) => d.kind === "section");
  return (
    <>
      {spaces.length > 0 && (
        <optgroup label="Espaces">
          {spaces.map((d) => (
            <option key={d.id} value={d.id}>
              {d.label}
            </option>
          ))}
        </optgroup>
      )}
      {sections.length > 0 && (
        <optgroup label="Rubriques">
          {sections.map((d) => (
            <option key={d.id} value={d.id}>
              {d.label}
            </option>
          ))}
        </optgroup>
      )}
    </>
  );
}
