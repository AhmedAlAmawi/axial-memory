import { useCallback, useEffect, useState } from "react";
import { api, checkForUpdate, onFinished, onProgress, type UpdateOffer } from "./api";
import { Mark } from "./components/ui";
import type { AiStatus, AppInfo, Progress, Results as R } from "./types";
import { Home } from "./views/Home";
import { Results } from "./views/Results";
import { Scanning } from "./views/Scanning";
import { Settings } from "./views/Settings";

type View = "home" | "scanning" | "results" | "settings";

export default function App() {
  const [info, setInfo] = useState<AppInfo | null>(null);
  const [view, setView] = useState<View>("home");
  const [results, setResults] = useState<R | null>(null);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [ai, setAi] = useState<AiStatus>({ configured: false, provider: null, provider_label: null, model: null, key_hint: null });
  const [update, setUpdate] = useState<UpdateOffer | null>(null);
  const [installing, setInstalling] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  const showToast = useCallback((m: string) => {
    setToast(m);
    window.setTimeout(() => setToast((t) => (t === m ? null : t)), 5000);
  }, []);

  const reload = useCallback(async () => {
    const r = await api.loadResults();
    setResults(r);
    return r;
  }, []);

  useEffect(() => {
    (async () => {
      const i = await api.appInfo();
      setInfo(i);
      setAi(await api.aiStatus());
      if (i.scanning) {
        setProgress(await api.scanProgress());
        setView("scanning");
      } else {
        const r = await reload();
        setView(r ? "results" : "home");
      }
    })().catch((e) => showToast(String(e)));
    checkForUpdate().then(setUpdate).catch(() => undefined);
  }, [reload, showToast]);

  useEffect(() => {
    const offs = [
      onProgress(setProgress),
      onFinished(async (f) => {
        setProgress(null);
        if (f.status === "failed") showToast(f.error ?? "The search failed.");
        if (f.status === "stopped") showToast("Stopped. What was found so far is shown.");
        setInfo(await api.appInfo());
        const r = await reload();
        setView(r ? "results" : "home");
      }),
    ];
    return () => {
      offs.forEach((p) => p.then((off) => off()));
    };
  }, [reload, showToast]);

  if (!info) {
    return (
      <div className="splash">
        <Mark size={64} spinning />
      </div>
    );
  }

  const nav = (v: View, label: string) => (
    <button className={view === v ? "nav active" : "nav"} disabled={view === "scanning"} onClick={() => setView(v)}>
      {label}
    </button>
  );

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <Mark />
          <span>Axial Memory</span>
        </div>
        <nav>
          {nav("home", "Search")}
          {results && nav("results", "Results")}
          {nav("settings", "Settings")}
        </nav>
        <div className="machine-label">{info.machine_label}</div>
      </header>

      {update && (
        <div className="update">
          A new version ({update.version}) is ready.
          <button
            className="primary small"
            disabled={installing}
            onClick={async () => {
              setInstalling(true);
              try {
                await update.install();
              } catch (e) {
                setInstalling(false);
                showToast(`The update didn't install: ${e}`);
              }
            }}
          >
            {installing ? "Installing…" : "Install and restart"}
          </button>
        </div>
      )}

      <main>
        {view === "home" && <Home info={info} results={results} onStarted={() => setView("scanning")} onOpenResults={() => setView("results")} />}
        {view === "scanning" && <Scanning info={info} progress={progress} onStop={() => api.cancelScan()} />}
        {view === "results" && results && (
          <Results
            info={info}
            results={results}
            ai={ai}
            onRescan={() => setView("home")}
            onReload={reload}
            onToast={showToast}
            onOpenSettings={() => setView("settings")}
          />
        )}
        {view === "settings" && <Settings info={info} ai={ai} onAiChange={setAi} onChanged={reload} onToast={showToast} />}
      </main>

      {toast && (
        <div className="toast" role="status" onClick={() => setToast(null)}>
          {toast}
        </div>
      )}
    </div>
  );
}
