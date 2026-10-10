import { useEffect, useState } from "react";
import { api, pickFolder } from "../api";
import { bytes, count, osName, when } from "../format";
import type { AppInfo, Results, Root } from "../types";

export function Home({
  info,
  results,
  onStarted,
  onOpenResults,
}: {
  info: AppInfo;
  results: Results | null;
  onStarted: () => void;
  onOpenResults: () => void;
}) {
  const [roots, setRoots] = useState<Root[] | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const applyRoots = (rs: Root[]) => {
    setRoots(rs);
    setSelected(new Set(rs.filter((r) => r.selected).map((r) => r.path)));
  };

  useEffect(() => {
    api.listRoots().then(applyRoots).catch((e) => setError(String(e)));
  }, []);

  const toggle = (path: string, on: boolean) => {
    const next = new Set(selected);
    if (on) next.add(path);
    else next.delete(path);
    setSelected(next);
  };

  const addFolder = async () => {
    const path = await pickFolder();
    if (path) applyRoots(await api.addFolder(path));
  };

  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      await api.startScan([...selected]);
      onStarted();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const mine = results?.machines.find((m) => m.this_computer);
  const isMac = info.os === "macos";

  return (
    <div className="page home">
      <section className="hero">
        <h1>Find every Grasshopper file on this {osName(info.os)}.</h1>
        <p className="lede">
          Axial Memory searches the whole computer, reads inside each definition without opening Rhino, and shows which
          ones belong to the palm research. Nothing is changed, moved or uploaded.
        </p>
      </section>

      {mine && (
        <section className="card last-scan">
          <div>
            <div className="eyebrow">Last search on this {osName(info.os)}</div>
            <div className="last-scan-line">
              {when(mine.scanned_at)} · {count(mine.totals.definitions, "Grasshopper file")} ·{" "}
              {count(mine.totals.strong + mine.totals.possible, "palm-related file")}
              {mine.status === "stopped" && " · stopped early"}
            </div>
          </div>
          <button className="secondary" onClick={onOpenResults}>
            See results
          </button>
        </section>
      )}

      <section className="card">
        <h2>Where to search</h2>
        {!roots && <p className="muted">Looking for drives…</p>}
        <ul className="roots">
          {roots?.map((r) => (
            <li key={r.path} className={r.available ? "" : "unavailable"}>
              <label className="check">
                <input
                  type="checkbox"
                  disabled={!r.available}
                  checked={selected.has(r.path)}
                  onChange={(e) => toggle(r.path, e.target.checked)}
                />
                <span className="root-text">
                  <span className="root-label">{r.label}</span>
                  <span className="muted small">
                    {[r.note, r.total_bytes ? `${bytes(r.total_bytes)}` : null].filter(Boolean).join(" · ")}
                  </span>
                </span>
              </label>
              {r.kind === "folder" && (
                <button className="link" onClick={async () => applyRoots(await api.removeFolder(r.path))}>
                  Remove
                </button>
              )}
            </li>
          ))}
        </ul>
        <button className="secondary small" onClick={addFolder}>
          Add a folder…
        </button>
      </section>

      {isMac && (
        <section className="card note">
          <h2>Your Mac will ask for permission</h2>
          <p>
            While it searches, macOS will ask a few times whether Axial Memory may look in Desktop, Documents, Downloads
            and other apps' files. Click <strong>Allow</strong> each time.
          </p>
          <p>
            For the most complete search, with no questions, turn on <strong>Full Disk Access</strong>: open the
            settings, click <strong>+</strong>, choose Axial Memory from Applications, switch it on, then reopen the app.
          </p>
          <div className="row">
            <button className="secondary small" onClick={() => api.openPrivacySettings()}>
              Open Full Disk Access settings
            </button>
            <span className={info.full_disk_access ? "pill ok" : "pill"}>
              Full Disk Access: {info.full_disk_access ? "on" : "off"}
            </span>
          </div>
        </section>
      )}

      {error && <p className="error">{error}</p>}
      <div className="actions">
        <button className="primary big" disabled={busy || selected.size === 0} onClick={start}>
          {mine ? "Search again" : "Start searching"}
        </button>
      </div>
    </div>
  );
}
