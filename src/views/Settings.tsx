import { useEffect, useState } from "react";
import { api, checkForUpdate, inTauri } from "../api";
import { Toggle } from "../components/ui";
import { strengthHelp, strengthLabel } from "../format";
import type { AiStatus, AppInfo, Settings as S, Strength, Term } from "../types";

export function Settings({
  info,
  ai,
  onAiChange,
  onChanged,
  onToast,
}: {
  info: AppInfo;
  ai: AiStatus;
  onAiChange: (a: AiStatus) => void;
  onChanged: () => void;
  onToast: (m: string) => void;
}) {
  const [s, setS] = useState<S | null>(null);
  const [label, setLabel] = useState("");
  const [key, setKey] = useState("");
  const [savingKey, setSavingKey] = useState(false);
  const [keyError, setKeyError] = useState<string | null>(null);

  useEffect(() => {
    api.getSettings().then((x) => {
      setS(x);
      setLabel(x.machine_label ?? "");
    });
  }, []);

  if (!s) return <div className="page">Loading…</div>;

  const save = async (next: S, message?: string) => {
    const saved = await api.saveSettings(next);
    setS(saved);
    onChanged();
    if (message) onToast(message);
  };

  const removeTerm = (t: Term) => save({ ...s, terms: s.terms.filter((x) => x !== t) });
  const addTerm = (word: string, strength: Strength) => {
    const w = word.trim();
    if (!w || s.terms.some((t) => t.word.toLowerCase() === w.toLowerCase())) return;
    save({ ...s, terms: [...s.terms, { word: w, strength, variants: [] }] }, `Added “${w}”. Results update straight away.`);
  };

  const saveKey = async () => {
    setSavingKey(true);
    setKeyError(null);
    try {
      onAiChange(await api.setAiKey(key));
      setKey("");
      onToast("AI key saved and checked.");
    } catch (e) {
      setKeyError(String(e));
    } finally {
      setSavingKey(false);
    }
  };

  const modules = s.modules;
  const setModule = (k: keyof typeof modules, v: boolean) => save({ ...s, modules: { ...modules, [k]: v } }, "Applies to the next search.");

  return (
    <div className="page settings">
      <h1>Settings</h1>

      <section className="card">
        <h2>This laptop's name</h2>
        <p className="muted small">Used in results and reports to tell the two laptops apart.</p>
        <div className="row">
          <input value={label} placeholder={info.os === "macos" ? "Mac" : "Windows laptop"} onChange={(e) => setLabel(e.target.value)} />
          <button className="secondary small" onClick={() => save({ ...s, machine_label: label.trim() || null }, "Saved. Applies to the next search.")}>
            Save
          </button>
        </div>
      </section>

      <section className="card">
        <h2>Search words</h2>
        <p className="muted small">
          Capitals, spaces, dashes and underscores don't matter: “palm tree” also finds Palm_Tree and PalmTree. Changes
          apply to the results straight away.
        </p>
        {(["strong", "palm", "related"] as Strength[]).map((strength) => (
          <TermGroup key={strength} strength={strength} terms={s.terms.filter((t) => t.strength === strength)} onRemove={removeTerm} onAdd={(w) => addTerm(w, strength)} />
        ))}
        <button
          className="link small"
          onClick={async () => {
            setS(await api.resetTerms());
            onChanged();
          }}
        >
          Go back to the original words
        </button>
      </section>

      <section className="card">
        <h2>What to look for</h2>
        <p className="muted small">Grasshopper definitions (.gh, .ghx and their backups) are always found. These come along with them:</p>
        <Toggle checked={modules.user_objects} onChange={(v) => setModule("user_objects", v)} label="User objects (.ghuser)" />
        <Toggle checked={modules.rhino_models} onChange={(v) => setModule("rhino_models", v)} label="Rhino models (.3dm, .3dmbak)" />
        <Toggle checked={modules.scripts} onChange={(v) => setModule("scripts", v)} label="Scripts (.py, .cs)" help="Only those next to a definition or named after the research." />
        <Toggle checked={modules.archives} onChange={(v) => setModule("archives", v)} label="Zip files" help="Opened to find definitions inside, without unzipping anything." />
        <Toggle checked={modules.documents} onChange={(v) => setModule("documents", v)} label="Documents named after the research" help="PDFs, images and notes whose names contain a palm word." />
        <Toggle
          checked={s.read_cloud_files}
          onChange={(v) => save({ ...s, read_cloud_files: v }, "Applies to the next search.")}
          label="Download cloud-only files to look inside them"
          help="Off: files that live only in iCloud, OneDrive, Dropbox or Google Drive are checked by name. On: they're downloaded, which can take a lot of space."
        />
      </section>

      <section className="card">
        <h2>AI explanations (optional)</h2>
        <p className="muted small">
          With a Claude or ChatGPT key, any readable definition can be explained in plain English. Only when you click
          Explain, and only that file's list of components, labels and scripts is sent. The file itself never leaves this
          computer. Everything else works without a key.
        </p>
        {ai.configured ? (
          <div className="row">
            <span className="pill ok">
              Connected to {ai.provider_label}
              {ai.model && ` · ${ai.model}`}
            </span>
            <span className="muted small mono">{ai.key_hint}</span>
            <button className="link small" onClick={async () => onAiChange(await api.clearAiKey())}>
              Remove key
            </button>
          </div>
        ) : (
          <>
            <div className="row">
              <input type="password" className="grow" placeholder="Paste a key: sk-ant-… (Claude) or sk-… (ChatGPT)" value={key} onChange={(e) => setKey(e.target.value)} />
              <button className="primary small" disabled={!key.trim() || savingKey} onClick={saveKey}>
                {savingKey ? "Checking…" : "Save key"}
              </button>
            </div>
            {keyError && <p className="error">{keyError}</p>}
            <p className="muted small">
              Get a Claude key at console.anthropic.com → API Keys, or a ChatGPT key at platform.openai.com → API keys.
              Each explanation costs a few cents.
            </p>
          </>
        )}
      </section>

      <section className="card">
        <h2>About</h2>
        <p className="muted small">
          Axial Memory {info.version}. It only reads: it never changes, moves, renames, deletes, runs or uploads your
          files.
        </p>
        {inTauri && (
          <button
            className="secondary small"
            onClick={async () => {
              try {
                const u = await checkForUpdate();
                if (!u) return onToast("You have the latest version.");
                onToast(`Installing version ${u.version}…`);
                await u.install();
              } catch (e) {
                onToast(`Couldn't check for updates: ${e}`);
              }
            }}
          >
            Check for updates
          </button>
        )}
      </section>
    </div>
  );
}

function TermGroup({ strength, terms, onRemove, onAdd }: { strength: Strength; terms: Term[]; onRemove: (t: Term) => void; onAdd: (w: string) => void }) {
  const [word, setWord] = useState("");
  return (
    <div className="term-group">
      <div className="term-group-head">
        <strong>{strengthLabel[strength]}</strong>
        <span className="muted small">{strengthHelp[strength]}</span>
      </div>
      <div className="terms">
        {terms.map((t) => (
          <span key={t.word} className={`term term-${strength}`} title={t.variants.length ? `Also: ${t.variants.join(", ")}` : undefined}>
            {t.word}
            {t.variants.length > 0 && <span className="variants">+{t.variants.length}</span>}
            <button aria-label={`Remove ${t.word}`} onClick={() => onRemove(t)}>
              ×
            </button>
          </span>
        ))}
        <form
          className="term-add"
          onSubmit={(e) => {
            e.preventDefault();
            onAdd(word);
            setWord("");
          }}
        >
          <input value={word} placeholder="Add a word" onChange={(e) => setWord(e.target.value)} />
        </form>
      </div>
    </div>
  );
}
