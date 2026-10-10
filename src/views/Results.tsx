import { useMemo, useState } from "react";
import { api, pickReportFolder, pickSaveScanFile, pickScanFile } from "../api";
import { Chip, Stat, Tabs } from "../components/ui";
import {
  bytes,
  count,
  coverageLabel,
  issueAdvice,
  issueLabel,
  kindLabel,
  osName,
  shortPath,
  when,
} from "../format";
import type { AiStatus, AppInfo, Confidence, FileView, IssueKind, Kind, Results as R } from "../types";
import { FileDetail } from "./FileDetail";

type Tab = "palm" | "files" | "duplicates" | "gaps" | "laptops";

export function Results({
  info,
  results,
  ai,
  onRescan,
  onReload,
  onToast,
  onOpenSettings,
}: {
  info: AppInfo;
  results: R;
  ai: AiStatus;
  onRescan: () => void;
  onReload: () => void;
  onToast: (msg: string) => void;
  onOpenSettings: () => void;
}) {
  const [tab, setTab] = useState<Tab>("palm");
  const [openFile, setOpenFile] = useState<number | null>(null);
  const t = results.totals;
  const palm = t.strong + t.possible;
  const other = results.machines.find((m) => !m.this_computer);
  const gaps = useMemo(() => realGaps(results, info.home, false), [results, info.home]);
  const gapsListed = useMemo(() => realGaps(results, info.home, true), [results, info.home]);

  const homeFor = (f: FileView) => (results.machines[f.machine]?.this_computer ? info.home : null);
  const where = (f: FileView) => {
    const m = results.machines[f.machine];
    const folder = f.in_archive ? `inside ${shortPath(f.in_archive, homeFor(f), 60)}` : shortPath(f.folder, homeFor(f), 70);
    return results.machines.length > 1 ? `${m?.label ?? ""} · ${folder}` : folder;
  };

  const exportReport = async () => {
    const parent = await pickReportFolder();
    if (!parent) return;
    try {
      const dir = await api.exportReport(parent);
      onToast("Report saved.");
      await api.reveal(dir);
    } catch (e) {
      onToast(String(e));
    }
  };

  return (
    <div className="page results">
      <header className="results-head">
        <div>
          <h1>
            {count(t.definitions, "Grasshopper file")}
            {results.machines.length > 1 ? " on both laptops" : ""}
          </h1>
          <p className="lede">
            {t.unique_definitions.toLocaleString()} different ones once identical copies are grouped.{" "}
            {palm > 0 ? `${count(palm, "looks", "look")} linked to the palm research.` : "None look linked to the palm research yet."}
          </p>
        </div>
        <div className="head-actions">
          <button className="secondary" onClick={exportReport}>
            Save report…
          </button>
          <button className="primary" onClick={onRescan}>
            Search again
          </button>
        </div>
      </header>

      <div className="stats compact">
        <Stat value={t.strong} label="Strong matches" tone="accent" />
        <Stat value={t.possible} label="Possible" />
        <Stat value={t.inconclusive} label="Couldn't check inside" tone={t.inconclusive ? "warn" : undefined} />
        <Stat value={t.cloud_only} label="Only in the cloud" />
        <Stat value={gaps} label="Places it couldn't look" tone={gaps ? "warn" : undefined} />
      </div>

      {!other && (
        <button className="banner" onClick={() => setTab("laptops")}>
          This is only this {osName(info.os)}. Add the other laptop's scan to see everything together →
        </button>
      )}

      <Tabs<Tab>
        value={tab}
        onChange={setTab}
        tabs={[
          { id: "palm", label: "Palm-related", count: results.candidates.length },
          { id: "files", label: "All files", count: results.files.length },
          { id: "duplicates", label: "Copies & versions", count: results.clusters.length + results.families.length },
          { id: "gaps", label: "Gaps", count: gapsListed },
          { id: "laptops", label: "Laptops", count: results.machines.length },
        ]}
      />

      {tab === "palm" && <PalmTab results={results} where={where} onOpen={setOpenFile} />}
      {tab === "files" && <FilesTab results={results} where={where} onOpen={setOpenFile} />}
      {tab === "duplicates" && <DuplicatesTab results={results} where={where} onOpen={setOpenFile} />}
      {tab === "gaps" && <GapsTab info={info} results={results} />}
      {tab === "laptops" && <LaptopsTab results={results} onReload={onReload} onToast={onToast} />}

      <FileDetail
        info={info}
        results={results}
        fileId={openFile}
        ai={ai}
        onClose={() => setOpenFile(null)}
        onOpenSettings={onOpenSettings}
        onToast={onToast}
      />
    </div>
  );
}

type Where = (f: FileView) => string;

function PalmTab({ results, where, onOpen }: { results: R; where: Where; onOpen: (id: number) => void }) {
  const [filter, setFilter] = useState<Confidence | "all">("all");
  const shown = results.candidates.filter((c) => filter === "all" || c.confidence === filter);
  const filters: { id: Confidence | "all"; label: string }[] = [
    { id: "all", label: "All" },
    { id: "strong", label: "Strong" },
    { id: "possible", label: "Possible" },
    { id: "weak", label: "Weak" },
    { id: "inconclusive", label: "Couldn't check inside" },
  ];
  if (results.candidates.length === 0) {
    return (
      <div className="empty">
        <p>Nothing matched the search words.</p>
        <p className="muted">Add more words (other spellings, project names, other languages) in Settings and they apply at once.</p>
      </div>
    );
  }
  return (
    <>
      <div className="filters">
        {filters.map((f) => (
          <button key={f.id} className={filter === f.id ? "filter active" : "filter"} onClick={() => setFilter(f.id)}>
            {f.label}
          </button>
        ))}
      </div>
      <ul className="list">
        {shown.map((c) => {
          const f = results.files[c.file];
          return (
            <li key={c.id}>
              <button className="row-button" onClick={() => onOpen(c.file)}>
                <span className="rank">{c.rank}</span>
                <span className="row-main">
                  <span className="row-title">
                    {c.name}
                    {c.kind !== "definition" && <span className="kind">{kindLabel[c.kind]}</span>}
                  </span>
                  <span className="row-reason">{c.reason}</span>
                  <span className="row-meta">
                    {where(f)} · changed {when(f.modified)}
                    {c.files.length > 1 && ` · ${c.files.length} copies`}
                    {f.status === "cloud_only" && " · only in the cloud"}
                  </span>
                </span>
                <Chip confidence={c.confidence} />
              </button>
            </li>
          );
        })}
      </ul>
    </>
  );
}

const KIND_FILTERS: { id: Kind | "all"; label: string }[] = [
  { id: "all", label: "Everything" },
  { id: "definition", label: "Definitions" },
  { id: "user_object", label: "User objects" },
  { id: "rhino_model", label: "Rhino models" },
  { id: "script", label: "Scripts" },
  { id: "archive", label: "Zips" },
  { id: "document", label: "Documents" },
];

function FilesTab({ results, where, onOpen }: { results: R; where: Where; onOpen: (id: number) => void }) {
  const [query, setQuery] = useState("");
  const [kind, setKind] = useState<Kind | "all">("definition");
  const [hideInstalled, setHideInstalled] = useState(true);
  const [limit, setLimit] = useState(300);
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return results.files.filter(
      (f) =>
        (kind === "all" || f.kind === kind || (kind === "rhino_model" && f.kind === "rhino_backup")) &&
        (!hideInstalled || !f.tags.includes("installed")) &&
        (!q || f.path.toLowerCase().includes(q) || f.groups.some((g) => g.toLowerCase().includes(q))),
    );
  }, [results.files, query, kind, hideInstalled]);
  return (
    <>
      <div className="filters">
        <input className="search" placeholder="Search names, folders, group labels…" value={query} onChange={(e) => setQuery(e.target.value)} />
        <select value={kind} onChange={(e) => setKind(e.target.value as Kind | "all")}>
          {KIND_FILTERS.map((k) => (
            <option key={k.id} value={k.id}>
              {k.label}
            </option>
          ))}
        </select>
        <label className="check inline">
          <input type="checkbox" checked={hideInstalled} onChange={(e) => setHideInstalled(e.target.checked)} />
          Hide files that came with software
        </label>
      </div>
      <table className="table">
        <thead>
          <tr>
            <th>Name</th>
            <th>Where</th>
            <th>Size</th>
            <th>Changed</th>
            <th>Copies</th>
            <th>Palm</th>
          </tr>
        </thead>
        <tbody>
          {shown.slice(0, limit).map((f) => {
            const c = f.candidate !== null ? results.candidates[f.candidate] : null;
            return (
              <tr key={f.id} onClick={() => onOpen(f.id)}>
                <td className="name-cell">
                  {f.name}
                  {f.status !== "read" && <span className="muted small"> · {f.status === "cloud_only" ? "cloud only" : "not read"}</span>}
                </td>
                <td className="muted">{where(f)}</td>
                <td className="num">{bytes(f.size)}</td>
                <td>{when(f.modified)}</td>
                <td className="num">{f.copies > 1 ? f.copies : ""}</td>
                <td>{c && <Chip confidence={c.confidence} />}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
      {shown.length > limit && (
        <button className="secondary small more" onClick={() => setLimit(limit + 500)}>
          Show more ({(shown.length - limit).toLocaleString()} left)
        </button>
      )}
      {shown.length === 0 && <p className="empty muted">No files match.</p>}
    </>
  );
}

function DuplicatesTab({ results, where, onOpen }: { results: R; where: Where; onOpen: (id: number) => void }) {
  const [open, setOpen] = useState<string | null>(null);
  const label = (ids: number[]) => {
    const per = new Map<string, number>();
    for (const id of ids) {
      const m = results.machines[results.files[id].machine]?.label ?? "";
      per.set(m, (per.get(m) ?? 0) + 1);
    }
    return [...per.entries()].map(([m, n]) => (results.machines.length > 1 ? `${m} ×${n}` : `${n} places`)).join(", ");
  };
  return (
    <div className="two-col">
      <section>
        <h2>Identical copies</h2>
        <p className="muted small">The same file, byte for byte, in more than one place.</p>
        {results.clusters.length === 0 && <p className="muted">No identical copies.</p>}
        <ul className="list compact">
          {results.clusters.map((c) => (
            <li key={c.sha256}>
              <button className="row-button" onClick={() => setOpen(open === c.sha256 ? null : c.sha256)}>
                <span className="row-main">
                  <span className="row-title">{c.name}</span>
                  <span className="row-meta">
                    {c.files.length} copies · {label(c.files)} · {bytes(c.size)}
                  </span>
                </span>
                <span className="chevron">{open === c.sha256 ? "−" : "+"}</span>
              </button>
              {open === c.sha256 && (
                <ul className="paths">
                  {c.files.map((id) => (
                    <li key={id}>
                      <button className="link" onClick={() => onOpen(id)}>
                        {results.files[id].name}
                      </button>{" "}
                      <span className="muted small">{where(results.files[id])}</span>
                    </li>
                  ))}
                </ul>
              )}
            </li>
          ))}
        </ul>
      </section>
      <section>
        <h2>Versions of the same project</h2>
        <p className="muted small">Files with the same name once "v2", "final", "copy" and dates are ignored.</p>
        {results.families.length === 0 && <p className="muted">No version families.</p>}
        <ul className="list compact">
          {results.families.map((fam) => (
            <li key={fam.key}>
              <button className="row-button" onClick={() => setOpen(open === fam.key ? null : fam.key)}>
                <span className="row-main">
                  <span className="row-title">{fam.label}</span>
                  <span className="row-meta">
                    {count(fam.versions.length, "version")} · {count(fam.files, "file")}
                    {fam.companions.length > 0 && ` · ${count(fam.companions.length, "Rhino model")} beside it`}
                  </span>
                </span>
                {fam.best && <Chip confidence={fam.best} />}
                <span className="chevron">{open === fam.key ? "−" : "+"}</span>
              </button>
              {open === fam.key && (
                <ol className="paths">
                  {fam.versions.map((v, i) => (
                    <li key={v.sha256 ?? i}>
                      <button className="link" onClick={() => onOpen(v.files[0])}>
                        {results.files[v.files[0]].name}
                      </button>{" "}
                      <span className="muted small">
                        {i === 0 ? "newest" : ""} changed {when(v.newest)} · {v.files.length > 1 ? `${v.files.length} copies` : where(results.files[v.files[0]])}
                      </span>
                    </li>
                  ))}
                </ol>
              )}
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}

type GapKind = IssueKind | "system_locked";

const GAP_ORDER: GapKind[] = ["permission_denied", "cloud_only", "damaged", "unreadable", "archive_limit", "too_large", "vanished", "system_locked", "skipped"];

const SYSTEM_LOCKED = {
  label: "System folders only an administrator can open",
  advice: "These belong to the operating system or to other accounts on the computer, not to your work. Nothing to do.",
};

/** Inside someone's own account folder, where locked places can hold their work. */
function isPersonal(path: string, home: string | null): boolean {
  if (home && (path.startsWith(home + "/") || path.startsWith(home + "\\"))) return true;
  const mac = path.match(/^\/Users\/([^/]+)\//);
  const win = path.match(/^[A-Za-z]:\\Users\\([^\\]+)\\/);
  const account = (mac ?? win)?.[1]?.toLowerCase();
  return account !== undefined && !["shared", "public", "default", "default user", "all users"].includes(account);
}

function gapKind(i: R["issues"][number], results: R, home: string | null): GapKind {
  const own = results.machines[i.machine]?.this_computer ? home : null;
  return i.kind === "permission_denied" && !isPersonal(i.path, own) ? "system_locked" : i.kind;
}

/** Gaps that could hide her work: not system folders, not deliberate skips. */
function realGaps(results: R, home: string | null, withCloud: boolean): number {
  const ignore: GapKind[] = withCloud ? ["skipped", "system_locked"] : ["skipped", "system_locked", "cloud_only"];
  return results.issues.filter((i) => !ignore.includes(gapKind(i, results, home))).length;
}

function GapsTab({ info, results }: { info: AppInfo; results: R }) {
  const [open, setOpen] = useState<GapKind | null>(null);
  const kindOf = (i: R["issues"][number]) => gapKind(i, results, info.home);
  const groups = GAP_ORDER.map((kind) => ({ kind, items: results.issues.filter((i) => kindOf(i) === kind) })).filter((g) => g.items.length);
  const label = (k: GapKind) => (k === "system_locked" ? SYSTEM_LOCKED.label : issueLabel[k]);
  const advice = (k: GapKind) => (k === "system_locked" ? SYSTEM_LOCKED.advice : issueAdvice[k]);
  return (
    <>
      <p className="muted">
        The search can only vouch for places it could actually look. These are the places it couldn't fully check, and
        why.
      </p>
      {info.os === "macos" && info.full_disk_access === false && results.issues.some((i) => kindOf(i) === "permission_denied") && (
        <div className="card note">
          <strong>Turn on Full Disk Access to open the locked folders.</strong>
          <div className="row">
            <button className="secondary small" onClick={() => api.openPrivacySettings()}>
              Open Full Disk Access settings
            </button>
          </div>
        </div>
      )}
      <ul className="list compact">
        {groups.map((g) => (
          <li key={g.kind}>
            <button className="row-button" onClick={() => setOpen(open === g.kind ? null : g.kind)}>
              <span className="row-main">
                <span className="row-title">
                  {label(g.kind)} <span className="muted">· {g.items.length.toLocaleString()}</span>
                </span>
                <span className="row-reason">{advice(g.kind)}</span>
              </span>
              <span className="chevron">{open === g.kind ? "−" : "+"}</span>
            </button>
            {open === g.kind && (
              <ul className="paths mono small">
                {g.items.slice(0, 300).map((i, n) => (
                  <li key={n}>
                    {results.machines.length > 1 && <span className="muted">{results.machines[i.machine]?.label}: </span>}
                    {shortPath(i.path, results.machines[i.machine]?.this_computer ? info.home : null, 110)}
                    {i.detail && <span className="muted"> — {i.detail}</span>}
                  </li>
                ))}
                {g.items.length > 300 && <li className="muted">…and {(g.items.length - 300).toLocaleString()} more in the report.</li>}
              </ul>
            )}
          </li>
        ))}
      </ul>
      <h2>Drives searched</h2>
      <table className="table">
        <thead>
          <tr>
            {results.machines.length > 1 && <th>Laptop</th>}
            <th>Drive</th>
            <th>Status</th>
            <th>Folders</th>
            <th>Files looked at</th>
            <th>Found</th>
            <th>Problems</th>
          </tr>
        </thead>
        <tbody>
          {results.coverage.map((c, i) => (
            <tr key={i} className="static">
              {results.machines.length > 1 && <td>{results.machines[c.machine]?.label}</td>}
              <td>{c.label}</td>
              <td>{coverageLabel[c.status]}</td>
              <td className="num">{c.dirs.toLocaleString()}</td>
              <td className="num">{c.files.toLocaleString()}</td>
              <td className="num">{c.found.toLocaleString()}</td>
              <td className="num">{c.issues.toLocaleString()}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

function LaptopsTab({ results, onReload, onToast }: { results: R; onReload: () => void; onToast: (m: string) => void }) {
  const mine = results.machines.find((m) => m.this_computer);
  const others = results.machines.filter((m) => !m.this_computer);
  const cmp = results.comparison;

  const saveMine = async () => {
    const dest = await pickSaveScanFile(await api.defaultScanFileName());
    if (!dest) return;
    try {
      await api.saveScanFile(dest);
      onToast("Scan file saved. Send it to the other laptop.");
    } catch (e) {
      onToast(String(e));
    }
  };

  const addOther = async () => {
    const path = await pickScanFile();
    if (!path) return;
    try {
      const added = await api.importScan(path);
      onToast(added.this_computer ? "That was this laptop's own scan." : `Added ${added.label}, scanned ${when(added.scanned_at)}.`);
      onReload();
    } catch (e) {
      onToast(String(e));
    }
  };

  return (
    <>
      <div className="machines">
        {results.machines.map((m) => (
          <div key={m.id} className="card machine">
            <div className="eyebrow">{m.this_computer ? "This laptop" : "Other laptop"}</div>
            <h2>{m.label}</h2>
            <p className="muted small">
              {m.device} · {m.os === "macos" ? "macOS" : m.os === "windows" ? "Windows" : m.os} {m.os_version} · searched {when(m.scanned_at)}
              {m.status === "stopped" && " (stopped early)"}
            </p>
            <ul className="facts">
              <li>{count(m.totals.definitions, "definition")} ({m.totals.unique_definitions.toLocaleString()} different)</li>
              <li>{count(m.totals.strong + m.totals.possible, "palm-related file")}</li>
              <li>{count(m.totals.user_objects, "user object")}, {count(m.totals.rhino_models, "Rhino model")}</li>
              <li>{count(m.files_seen, "file")} looked at in {count(m.dirs, "folder")}</li>
            </ul>
            {!m.this_computer && (
              <button
                className="link"
                onClick={async () => {
                  await api.removeMachine(m.id);
                  onReload();
                }}
              >
                Remove this laptop's scan
              </button>
            )}
          </div>
        ))}
        {others.length === 0 && (
          <div className="card machine add">
            <div className="eyebrow">Other laptop</div>
            <h2>Not added yet</h2>
            <ol className="steps">
              <li>Install Axial Memory on the other laptop and search it.</li>
              <li>
                There, open <strong>Laptops</strong> and click <strong>Save this laptop's scan</strong>. Send the file to
                yourself (email, a USB stick, Google Drive or WhatsApp all work).
              </li>
              <li>
                Here, click <strong>Add the other laptop's scan</strong> and choose that file.
              </li>
            </ol>
          </div>
        )}
      </div>
      <div className="row">
        <button className="secondary" onClick={saveMine} disabled={!mine}>
          Save this laptop's scan…
        </button>
        <button className="primary" onClick={addOther}>
          Add the other laptop's scan…
        </button>
      </div>
      {cmp && (
        <section className="card">
          <h2>Comparing the two</h2>
          <p className="muted small">Different definitions (identical copies counted once).</p>
          <div className="stats compact">
            <Stat value={cmp.only_a} label={`Only on ${results.machines[cmp.a]?.label}`} />
            <Stat value={cmp.only_b} label={`Only on ${results.machines[cmp.b]?.label}`} />
            <Stat value={cmp.both} label="On both" />
          </div>
        </section>
      )}
      <p className="muted small">Each laptop keeps its last three searches; the newest one is shown.</p>
    </>
  );
}
