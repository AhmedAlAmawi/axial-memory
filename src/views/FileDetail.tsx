import { useEffect, useState } from "react";
import { api } from "../api";
import { DefinitionMapView } from "../components/DefinitionMap";
import { Chip, Drawer, Markdown, Modal } from "../components/ui";
import { bytes, count, kindLabel, revealLabel, shortPath, sourceLabel, statusLabel, tagLabel, when } from "../format";
import type { AiStatus, AppInfo, DefinitionMap, Explanation, Results } from "../types";

export function FileDetail({
  info,
  results,
  fileId,
  ai,
  onClose,
  onOpenSettings,
  onToast,
}: {
  info: AppInfo;
  results: Results;
  fileId: number | null;
  ai: AiStatus;
  onClose: () => void;
  onOpenSettings: () => void;
  onToast: (m: string) => void;
}) {
  const f = fileId !== null ? results.files[fileId] : null;
  const c = f && f.candidate !== null ? results.candidates[f.candidate] : null;
  const copies = c ? c.files : f ? results.files.filter((x) => f.sha256 && x.sha256 === f.sha256).map((x) => x.id) : [];
  const [explanation, setExplanation] = useState<Explanation | null>(null);
  const [preview, setPreview] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  const [aiError, setAiError] = useState<string | null>(null);
  const [map, setMap] = useState<DefinitionMap | null>(null);
  const [large, setLarge] = useState(false);

  useEffect(() => {
    setExplanation(null);
    setAiError(null);
    setPreview(null);
    setMap(null);
    setLarge(false);
    if (f?.sha256) api.getExplanation(f.sha256).then(setExplanation);
    if (f && f.read === "parsed" && f.kind.match(/definition|user_object/)) {
      api.definitionMap(f.id).then(setMap).catch(() => setMap(null));
    }
  }, [f?.id, f?.sha256]);

  if (!f) return null;
  const readable = f.read === "parsed" && f.kind !== "script";
  const local = (id: number) => results.machines[results.files[id].machine]?.this_computer;
  const homeFor = (id: number) => (local(id) ? info.home : null);

  const reveal = async (path: string) => {
    try {
      await api.reveal(path);
    } catch (e) {
      onToast(String(e));
    }
  };

  const showPreview = async () => {
    setAiError(null);
    try {
      setPreview(await api.aiPreview(f.id));
    } catch (e) {
      setAiError(String(e));
    }
  };

  const send = async () => {
    setPreview(null);
    setAsking(true);
    setAiError(null);
    try {
      setExplanation(await api.explainFile(f.id));
    } catch (e) {
      setAiError(String(e));
    } finally {
      setAsking(false);
    }
  };

  return (
    <Drawer open onClose={onClose}>
      <div className="detail">
        <div className="eyebrow">{kindLabel[f.kind]}</div>
        <h2 className="detail-title">{f.name}</h2>
        {c && (
          <div className="detail-verdict">
            <Chip confidence={c.confidence} />
            <span>{c.reason}</span>
          </div>
        )}

        {c && c.evidence.length > 0 && (
          <section>
            <h3>Why it's here</h3>
            <ul className="evidence">
              {c.evidence.map((e, i) => (
                <li key={i}>
                  <span className={`term term-${e.strength}`}>{e.term}</span>
                  <span className="muted small">
                    {sourceLabel[e.source]}
                    {e.in_cluster ? " (inside a cluster)" : ""}
                  </span>
                  <q>{e.text}</q>
                </li>
              ))}
            </ul>
            {c.evidence_total > c.evidence.length && (
              <p className="muted small">…and {c.evidence_total - c.evidence.length} more in the report.</p>
            )}
          </section>
        )}

        {map && (
          <section>
            <h3>Map</h3>
            <DefinitionMapView map={map} height={300} onExpand={() => setLarge(true)} />
            <div className="map-caption">
              <p className="muted small">
                Drawn from the file: components where they sit on the canvas, the wires between them and your groups.
                {map.nodes.some((n) => n.matched) && " Outlined in green: components with a search word."}
                {map.hidden > 0 && ` ${count(map.hidden, "component")} inside clusters aren't shown.`} Drag to move, scroll to
                zoom.
              </p>
              {map.thumbnail && (
                <figure className="thumb">
                  <img src={`data:image/png;base64,${map.thumbnail}`} alt="Grasshopper's own preview of the canvas" />
                  <figcaption className="muted small">Grasshopper's own preview</figcaption>
                </figure>
              )}
            </div>
          </section>
        )}

        <section>
          <h3>{copies.length > 1 ? `${copies.length} copies` : "Where it is"}</h3>
          <ul className="copies">
            {(copies.length ? copies : [f.id]).map((id) => {
              const x = results.files[id];
              return (
                <li key={id}>
                  <div className="mono small path">{shortPath(x.path, homeFor(id), 140)}</div>
                  <div className="muted small">
                    {results.machines.length > 1 && `${results.machines[x.machine]?.label} · `}
                    changed {when(x.modified)} · {bytes(x.size)}
                    {x.status !== "read" && ` · ${statusLabel[x.status]}`}
                    {x.tags.length > 0 && ` · ${x.tags.map((t) => tagLabel[t] ?? t).join(", ")}`}
                  </div>
                  {x.detail && <div className="muted small">{x.detail}</div>}
                  {local(id) && (
                    <button className="link small" onClick={() => reveal(x.path)}>
                      {revealLabel(info.os)}
                    </button>
                  )}
                </li>
              );
            })}
          </ul>
        </section>

        <section>
          <h3>Inside</h3>
          {f.read === "parsed" && f.objects !== null ? (
            <ul className="facts">
              <li>
                {f.objects.toLocaleString()} components{f.gh_version && `, saved by Grasshopper ${f.gh_version}`}
              </li>
              {f.groups.length > 0 && (
                <li>
                  Groups:{" "}
                  {f.groups.map((g) => (
                    <span key={g} className="tag">
                      {g}
                    </span>
                  ))}
                </li>
              )}
              {f.plugins.length > 0 && <li>Plugins: {f.plugins.join(", ")}</li>}
            </ul>
          ) : f.read === "inconclusive" ? (
            <p className="muted">
              {f.status === "cloud_only"
                ? "This file is only in the cloud, so its inside wasn't checked."
                : `Couldn't read inside: ${f.read_error ?? f.detail ?? "unknown reason"}. That doesn't mean it's unrelated; open it in Grasshopper to check.`}
            </p>
          ) : (
            <p className="muted">Only Grasshopper files are read inside.</p>
          )}
        </section>

        {f.kind !== "rhino_model" && f.kind !== "document" && (
          <section className="ai">
            <h3>Explain with AI</h3>
            {explanation ? (
              <>
                <Markdown text={explanation.markdown} />
                <p className="muted small">
                  Explained by {explanation.provider} ({explanation.model}) {when(explanation.created_at)}. AI can be wrong;
                  check it against the file.
                </p>
                {ai.configured && readable && (
                  <button className="link small" onClick={showPreview} disabled={asking}>
                    Explain again
                  </button>
                )}
              </>
            ) : !readable ? (
              <p className="muted">Only definitions that could be read inside can be explained.</p>
            ) : !ai.configured ? (
              <p className="muted">
                Add a Claude or ChatGPT key in{" "}
                <button className="link" onClick={onOpenSettings}>
                  Settings
                </button>{" "}
                to get a plain-English explanation of what this definition does.
              </p>
            ) : (
              <button className="primary" onClick={showPreview} disabled={asking}>
                {asking ? `Asking ${ai.provider_label}…` : `Explain with ${ai.provider_label}`}
              </button>
            )}
            {aiError && <p className="error">{aiError}</p>}
          </section>
        )}
      </div>

      {large && map && (
        <Modal title={f.name} onClose={() => setLarge(false)} wide>
          <DefinitionMapView map={map} height="calc(90vh - 140px)" />
        </Modal>
      )}

      {preview !== null && (
        <Modal title={`Send this to ${ai.provider_label}?`} onClose={() => setPreview(null)}>
          <p className="small">
            Only this summary of the file is sent: the list of components, their wiring, group labels, panel text and
            scripts. The file itself stays on this computer.
          </p>
          <pre className="preview">{preview}</pre>
          <div className="row end">
            <button className="secondary" onClick={() => setPreview(null)}>
              Cancel
            </button>
            <button className="primary" onClick={send}>
              Send
            </button>
          </div>
        </Modal>
      )}
    </Drawer>
  );
}
