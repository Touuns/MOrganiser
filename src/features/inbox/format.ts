const short = new Intl.DateTimeFormat("fr-FR", {
  day: "numeric",
  month: "short",
  hour: "2-digit",
  minute: "2-digit",
});

const long = new Intl.DateTimeFormat("fr-FR", { dateStyle: "long", timeStyle: "short" });

/** « 10 oct., 02:18 » : cartes de la liste. */
export const formatShort = (ms: number) => short.format(ms);

/** « 10 octobre 2026 à 02:18 » : fiche. */
export const formatLong = (ms: number) => long.format(ms);

export const isoDate = (ms: number) => new Date(ms).toISOString();
