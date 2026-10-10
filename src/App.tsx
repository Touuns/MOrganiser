import { useEffect, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { Badge } from "./components/Badge";
import { Panel } from "./components/Panel";
import { InboxHome } from "./features/inbox/InboxHome";
import { useInitiation } from "./features/initiation/useInitiation";
import { fetchAppInfo, type AppInfo } from "./lib/appInfo";
import "./App.css";

type InfoState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo | null }
  | { status: "error"; message: string };

/**
 * Accueil de la brique 001 : boîte « À organiser » au-dessus de la capture rapide.
 * Prototype limité à ces deux éléments, pas le tableau de bord définitif (brique 006).
 */
export function App() {
  const [state, setState] = useState<InfoState>({ status: "loading" });
  const inApp = isTauri();
  // Initiation facultative : proposée une seule fois, rejouable à tout moment par « Découvrir ».
  const initiation = useInitiation(inApp);

  useEffect(() => {
    let active = true;
    fetchAppInfo()
      .then((info) => active && setState({ status: "ready", info }))
      .catch((error: unknown) =>
        active && setState({ status: "error", message: String(error) }),
      );
    return () => {
      active = false;
    };
  }, []);

  return (
    <div className="app">
      <header className="app__header">
        <span className="app__name">M'Organiser</span>
        <EnvironmentBadge state={state} />
        {inApp && (
          <button type="button" className="app__discover" onClick={initiation.start}>
            Découvrir
          </button>
        )}
      </header>

      <main className="app__main">
        {inApp ? (
          <InboxHome initiation={initiation} />
        ) : (
          <Panel title="Aperçu navigateur">
            <p className="app__muted">
              La capture fonctionne uniquement dans l'application Windows (<code>pnpm app:dev</code>).
              Aucune donnée n'est lue ni écrite ici.
            </p>
          </Panel>
        )}
      </main>

      <footer className="app__footer">
        <EnvironmentDetails state={state} />
      </footer>
    </div>
  );
}

function EnvironmentBadge({ state }: { state: InfoState }) {
  if (state.status !== "ready") return null;
  if (state.info === null) return <Badge>Navigateur</Badge>;
  return state.info.channel === "dev" ? (
    <Badge tone="dev">Dev</Badge>
  ) : (
    <Badge tone="stable">Stable</Badge>
  );
}

function EnvironmentDetails({ state }: { state: InfoState }) {
  switch (state.status) {
    case "loading":
      return null;
    case "error":
      return (
        <p role="alert">
          Impossible de lire l'environnement : <code>{state.message}</code>
        </p>
      );
    case "ready":
      if (state.info === null) return null;
      return (
        <p>
          Version {state.info.version} · Données : <code>{state.info.dataDir}</code>
        </p>
      );
  }
}
