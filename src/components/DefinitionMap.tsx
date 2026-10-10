import { useEffect, useRef, useState } from "react";
import type { DefinitionMap, MapGroup, MapNode } from "../types";

type View = { x: number; y: number; w: number; h: number };
type Bounds = DefinitionMap["bounds"];

const fit = (b: Bounds): View => ({ x: b[0], y: b[1], w: b[2], h: b[3] });

function zoom(v: View, factor: number, cx: number, cy: number, b: Bounds): View {
  const w = Math.min(b[2] * 3, Math.max(Math.max(b[2] / 40, 120), v.w * factor));
  const k = w / v.w;
  return { x: cx - (cx - v.x) * k, y: cy - (cy - v.y) * k, w, h: v.h * k };
}

function rgba([r, g, b, a]: MapGroup["colour"], k = 1): string {
  return `rgba(${r}, ${g}, ${b}, ${Math.min(1, (a / 255) * k).toFixed(3)})`;
}

function cut(s: string, max: number): string {
  return s.length <= max ? s : `${s.slice(0, Math.max(1, max - 1))}…`;
}

function wirePath(x1: number, y1: number, x2: number, y2: number): string {
  const dx = Math.max(30, Math.abs(x2 - x1) * 0.5);
  return `M${x1},${y1} C${x1 + dx},${y1} ${x2 - dx},${y2} ${x2},${y2}`;
}

/** Wraps text into lines of about `width` characters. */
function lines(text: string, width: number, max: number): string[] {
  const out: string[] = [];
  let line = "";
  for (const word of text.split(/\s+/)) {
    if ((line + " " + word).trim().length > width && line) {
      out.push(line);
      line = word;
    } else {
      line = (line + " " + word).trim();
    }
    if (out.length >= max) break;
  }
  if (line && out.length < max) out.push(line);
  return out;
}

function NodeShape({ n, detailed, dim, onHover }: { n: MapNode; detailed: boolean; dim: boolean; onHover: (id: number | null) => void }) {
  const chars = Math.max(3, Math.floor(n.w / 6));
  const cx = n.x + n.w / 2;
  const cy = n.y + n.h / 2;
  const hasPins = n.inputs.length + n.outputs.length > 0 && n.role !== "slider" && n.role !== "param";
  const title = [
    n.label !== n.kind ? `${n.kind} “${n.label}”` : n.kind,
    n.detail,
    n.plugin ? `Plugin: ${n.plugin}` : null,
    n.matched ? "Has a search word" : null,
  ]
    .filter(Boolean)
    .join("\n");
  let body;
  if (n.role === "panel" || n.role === "note") {
    const text = n.detail ?? n.label;
    body = lines(text, Math.max(8, Math.floor(n.w / 5)), Math.max(1, Math.floor((n.h - 6) / 11))).map((l, i) => (
      <text key={i} x={n.x + 5} y={n.y + 11 + i * 11} className="panel-text">
        {l}
      </text>
    ));
  } else if (n.role === "slider") {
    // Like Grasshopper: a slider shows its value, after its name if she gave it one.
    const value = n.detail?.split(" (")[0] ?? "";
    const text = n.label === n.kind ? value || n.kind : value ? `${n.label} = ${value}` : n.label;
    body = (
      <text x={n.x + 6} y={cy} className="node-label left">
        {cut(text, chars)}
      </text>
    );
  } else if (detailed && hasPins && n.h > n.w * 0.6) {
    // Grasshopper writes a component's name sideways between its inputs and outputs.
    body = (
      <text x={cx} y={cy} transform={`rotate(-90 ${cx} ${cy})`} className="node-label">
        {cut(n.label, Math.max(3, Math.floor(n.h / 6)))}
      </text>
    );
  } else {
    body = (
      <text x={cx} y={cy} className="node-label">
        {cut(n.label, chars)}
      </text>
    );
  }
  return (
    <g
      className={`node ${n.role}${n.matched ? " matched" : ""}${dim ? " dim" : ""}`}
      onPointerEnter={() => onHover(n.id)}
      onPointerLeave={() => onHover(null)}
    >
      <title>{title}</title>
      <rect x={n.x} y={n.y} width={n.w} height={n.h} rx={4} />
      {body}
      {n.inputs.map((p, i) => (
        <circle key={`i${i}`} cx={n.x} cy={p.y} r={2.4} className="pin" />
      ))}
      {n.outputs.map((p, i) => (
        <circle key={`o${i}`} cx={n.x + n.w} cy={p.y} r={2.4} className="pin" />
      ))}
      {detailed &&
        hasPins &&
        n.inputs.map((p, i) => (
          <text key={`li${i}`} x={n.x + 4} y={p.y} className="pin-label">
            {cut(p.name, 6)}
          </text>
        ))}
      {detailed &&
        hasPins &&
        n.outputs.map((p, i) => (
          <text key={`lo${i}`} x={n.x + n.w - 4} y={p.y} className="pin-label end">
            {cut(p.name, 6)}
          </text>
        ))}
    </g>
  );
}

/** The definition's canvas as she laid it out: drag to move, scroll to zoom, double-click to fit. */
export function DefinitionMapView({ map, height, onExpand }: { map: DefinitionMap; height: number | string; onExpand?: () => void }) {
  const svgRef = useRef<SVGSVGElement>(null);
  const [view, setView] = useState<View>(() => fit(map.bounds));
  const [hover, setHover] = useState<number | null>(null);
  const [unitPx, setUnitPx] = useState(1);
  const drag = useRef<{ x: number; y: number; view: View; scale: number } | null>(null);

  useEffect(() => setView(fit(map.bounds)), [map]);

  useEffect(() => {
    const el = svgRef.current;
    if (!el) return;
    const update = () => {
      const r = el.getBoundingClientRect();
      setUnitPx(Math.min(r.width / view.w, r.height / view.h));
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(el);
    return () => ro.disconnect();
  }, [view]);

  useEffect(() => {
    const el = svgRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const ctm = el.getScreenCTM();
      if (!ctm) return;
      const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(ctm.inverse());
      setView((v) => zoom(v, Math.exp(e.deltaY * 0.0015), p.x, p.y, map.bounds));
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [map.bounds]);

  const step = (factor: number) => setView((v) => zoom(v, factor, v.x + v.w / 2, v.y + v.h / 2, map.bounds));
  const linked =
    hover === null ? null : new Set(map.wires.filter((w) => w.from === hover || w.to === hover).flatMap((w) => [w.from, w.to]));

  return (
    <div className="map">
      <div className="map-tools">
        <button onClick={() => step(0.75)} aria-label="Zoom in">
          +
        </button>
        <button onClick={() => step(1 / 0.75)} aria-label="Zoom out">
          −
        </button>
        <button onClick={() => setView(fit(map.bounds))}>Fit</button>
        {onExpand && <button onClick={onExpand}>Open large</button>}
      </div>
      <svg
        ref={svgRef}
        className="map-canvas"
        style={{ height }}
        viewBox={`${view.x} ${view.y} ${view.w} ${view.h}`}
        preserveAspectRatio="xMidYMid meet"
        onPointerDown={(e) => {
          const ctm = svgRef.current?.getScreenCTM();
          if (e.button !== 0 || !ctm) return;
          svgRef.current?.setPointerCapture(e.pointerId);
          drag.current = { x: e.clientX, y: e.clientY, view, scale: 1 / ctm.a };
        }}
        onPointerMove={(e) => {
          const d = drag.current;
          if (!d) return;
          setView({ ...d.view, x: d.view.x - (e.clientX - d.x) * d.scale, y: d.view.y - (e.clientY - d.y) * d.scale });
        }}
        onPointerUp={() => (drag.current = null)}
        onDoubleClick={() => setView(fit(map.bounds))}
      >
        <g>
          {map.groups.map((g, i) => (
            <g key={i}>
              <rect x={g.x} y={g.y} width={g.w} height={g.h} rx={10} fill={rgba(g.colour, 0.5)} stroke={rgba(g.colour, 1.6)} />
              <text x={g.x + 8} y={g.y + 12} className="map-group-label">
                {g.name}
              </text>
            </g>
          ))}
        </g>
        <g>
          {map.wires.map((w, i) => {
            const lit = hover !== null && (w.from === hover || w.to === hover);
            return <path key={i} d={wirePath(w.x1, w.y1, w.x2, w.y2)} className={lit ? "wire lit" : hover !== null ? "wire dim" : "wire"} />;
          })}
        </g>
        <g>
          {map.nodes.map((n) => (
            <NodeShape key={n.id} n={n} detailed={unitPx >= 1.1} dim={linked !== null && !linked.has(n.id) && n.id !== hover} onHover={setHover} />
          ))}
        </g>
      </svg>
      {map.groups.length > 0 && (
        <div className="map-legend">
          {map.groups.map((g, i) => (
            <button key={i} onClick={() => setView({ x: g.x - 24, y: g.y - 24, w: g.w + 48, h: g.h + 48 })}>
              <span className="swatch" style={{ background: rgba(g.colour, 1.6) }} />
              {g.name || "Unnamed group"}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
