import { duration, shortPath } from "../format";
import type { AppInfo, Progress } from "../types";
import { Mark, ProgressBar, Stat } from "../components/ui";

export function Scanning({ info, progress, onStop }: { info: AppInfo; progress: Progress | null; onStop: () => void }) {
  const p = progress;
  const reading = p?.phase === "reading";
  return (
    <div className="page scanning">
      <Mark size={72} spinning />
      <h1>{reading ? "Reading what it found…" : "Searching…"}</h1>
      <p className="lede">
        You can keep working while it searches.
        {info.os === "macos" && " If your Mac asks whether Axial Memory may look in a folder, click Allow."}
      </p>
      <div className="stats">
        <Stat value={p?.definitions ?? 0} label="Grasshopper files found" tone="accent" />
        <Stat value={p?.other_found ?? 0} label="Related files" />
        <Stat value={p?.dirs ?? 0} label="Folders searched" />
        <Stat value={p?.files_seen ?? 0} label="Files looked at" />
      </div>
      <div className="scan-status">
        {reading ? (
          <>
            <ProgressBar value={p?.read ?? 0} max={p?.queued ?? 0} />
            <div className="muted small">
              {(p?.read ?? 0).toLocaleString()} of {(p?.queued ?? 0).toLocaleString()} read
            </div>
          </>
        ) : (
          <div className="current mono" title={p?.current}>
            {p?.current ? shortPath(p.current, info.home, 80) : "Starting…"}
          </div>
        )}
        <div className="muted small">{duration(p?.elapsed_ms ?? 0)}</div>
      </div>
      <button className="secondary" onClick={onStop}>
        Stop
      </button>
    </div>
  );
}
