import { useEffect, type ReactNode } from "react";
import { confidenceLabel } from "../format";
import type { Confidence } from "../types";

/** The phyllotaxis mark: seeds placed at the golden angle, as in Vogel's model. */
export function Mark({ size = 28, spinning = false }: { size?: number; spinning?: boolean }) {
  const dots = [];
  const n = 64;
  for (let i = 1; i <= n; i++) {
    const a = i * 2.39996323;
    const r = 4.6 * Math.sqrt(i);
    dots.push(<circle key={i} cx={50 + r * Math.cos(a)} cy={50 + r * Math.sin(a)} r={1.1 + i / 26} />);
  }
  return (
    <svg className={spinning ? "mark spinning" : "mark"} width={size} height={size} viewBox="0 0 100 100" aria-hidden>
      {dots}
    </svg>
  );
}

export function Chip({ confidence }: { confidence: Confidence }) {
  return <span className={`chip chip-${confidence}`}>{confidenceLabel[confidence]}</span>;
}

export function Stat({ value, label, tone }: { value: number | string; label: string; tone?: "accent" | "warn" }) {
  return (
    <div className={`stat ${tone ?? ""}`}>
      <div className="stat-value">{typeof value === "number" ? value.toLocaleString() : value}</div>
      <div className="stat-label">{label}</div>
    </div>
  );
}

export function Tabs<T extends string>({ tabs, value, onChange }: { tabs: { id: T; label: string; count?: number }[]; value: T; onChange: (t: T) => void }) {
  return (
    <div className="tabs" role="tablist">
      {tabs.map((t) => (
        <button key={t.id} role="tab" aria-selected={t.id === value} className={t.id === value ? "tab active" : "tab"} onClick={() => onChange(t.id)}>
          {t.label}
          {t.count !== undefined && <span className="tab-count">{t.count.toLocaleString()}</span>}
        </button>
      ))}
    </div>
  );
}

export function Toggle({ checked, onChange, label, help }: { checked: boolean; onChange: (v: boolean) => void; label: ReactNode; help?: ReactNode }) {
  return (
    <label className="toggle">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span className="toggle-track" aria-hidden />
      <span className="toggle-text">
        <span>{label}</span>
        {help && <span className="muted small">{help}</span>}
      </span>
    </label>
  );
}

export function Drawer({ open, onClose, children }: { open: boolean; onClose: () => void; children: ReactNode }) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  if (!open) return null;
  return (
    <div className="drawer-backdrop" onClick={onClose}>
      <aside className="drawer" onClick={(e) => e.stopPropagation()}>
        <button className="drawer-close" onClick={onClose} aria-label="Close">
          ×
        </button>
        {children}
      </aside>
    </div>
  );
}

export function Modal({ title, children, onClose }: { title: string; children: ReactNode; onClose: () => void }) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal" role="dialog" aria-label={title} onClick={(e) => e.stopPropagation()}>
        <h3>{title}</h3>
        {children}
      </div>
    </div>
  );
}

export function ProgressBar({ value, max }: { value: number; max: number }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div className="bar" role="progressbar" aria-valuenow={value} aria-valuemax={max}>
      <div className="bar-fill" style={{ width: `${pct}%` }} />
    </div>
  );
}

/** Renders the model's markdown (headings, lists, bold) as elements, never as raw HTML. */
export function Markdown({ text }: { text: string }) {
  const inline = (s: string, key: string) => {
    const parts = s.split(/(\*\*[^*]+\*\*|`[^`]+`)/g).filter(Boolean);
    return parts.map((p, i) =>
      p.startsWith("**") ? <strong key={`${key}-${i}`}>{p.slice(2, -2)}</strong> : p.startsWith("`") ? <code key={`${key}-${i}`}>{p.slice(1, -1)}</code> : <span key={`${key}-${i}`}>{p}</span>,
    );
  };
  const blocks: ReactNode[] = [];
  let list: { ordered: boolean; items: string[] } | null = null;
  const flush = () => {
    if (!list) return;
    const items = list.items.map((it, i) => <li key={i}>{inline(it, `li${blocks.length}-${i}`)}</li>);
    blocks.push(list.ordered ? <ol key={blocks.length}>{items}</ol> : <ul key={blocks.length}>{items}</ul>);
    list = null;
  };
  for (const raw of text.split("\n")) {
    const line = raw.trimEnd();
    const heading = line.match(/^#{1,4}\s+(.*)$/);
    const ordered = line.match(/^\s*\d+[.)]\s+(.*)$/);
    const bullet = line.match(/^\s*[-*•]\s+(.*)$/);
    if (heading) {
      flush();
      blocks.push(<h4 key={blocks.length}>{inline(heading[1], `h${blocks.length}`)}</h4>);
    } else if (ordered || bullet) {
      const isOrdered = Boolean(ordered);
      if (!list || list.ordered !== isOrdered) {
        flush();
        list = { ordered: isOrdered, items: [] };
      }
      list.items.push((ordered ?? bullet)![1]);
    } else if (line.trim() === "") {
      flush();
    } else {
      flush();
      blocks.push(<p key={blocks.length}>{inline(line, `p${blocks.length}`)}</p>);
    }
  }
  flush();
  return <div className="markdown">{blocks}</div>;
}
