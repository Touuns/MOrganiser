import { useEffect, useState } from "react";
import { Badge } from "./components/Badge";
import { Panel } from "./components/Panel";
import { fetchAppInfo, type AppInfo } from "./lib/appInfo";
import "./App.css";

type InfoState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo | null }
  | { status: "error"; message: string };

/**
 * Fenêtre de fondation (brique 000). Elle vérifie seulement que l'application démarre
 * et dans quel environnement ; ce n'est pas le tableau de bord définitif.
 */
export function App() {
  const [state, setState] = useState<InfoState>({ status: "loading" });

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
      </header>

      <main className="app__main">
        <Panel title="Fondations">
          <p>L'application démarre correctement.</p>
          <p className="app__muted">
            Aucune fonctionnalité n'est encore disponible : la capture rapide arrivera avec la
            brique 001.
          </p>
        </Panel>

        <Panel title="Environnement">
          <EnvironmentDetails state={state} />
        </Panel>
      </main>
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
      return <p className="app__muted">Lecture de l'environnement…</p>;
    case "error":
      return (
        <p role="alert">
          Impossible de lire l'environnement : <code>{state.message}</code>
        </p>
      );
    case "ready":
      if (state.info === null) {
        return (
          <p className="app__muted">
            Interface ouverte dans un navigateur, hors de l'application Windows. Aucune donnée
            n'est lue ni écrite.
          </p>
        );
      }
      return (
        <dl className="app__facts">
          <dt>Version</dt>
          <dd>{state.info.version}</dd>
          <dt>Dossier de données</dt>
          <dd>
            <code>{state.info.dataDir}</code>
          </dd>
        </dl>
      );
  }
}
