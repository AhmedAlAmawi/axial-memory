// Stand-in backend for working on the interface in a browser (`pnpm ui`).
// Never used inside the app.
import type {
  AiStatus,
  AppInfo,
  Candidate,
  Explanation,
  FileView,
  Progress,
  Results,
  Root,
  ScanFinished,
  Settings,
  Totals,
} from "./types";

const HOME = "/Users/studio";
const now = Math.floor(Date.now() / 1000);
const day = 86_400;

let progressListeners: ((p: Progress) => void)[] = [];
let finishedListeners: ((f: ScanFinished) => void)[] = [];
let scanning: Progress | null = null;
let hasResults = new URLSearchParams(location.search).get("empty") === null;
let ai: AiStatus = { configured: false, provider: null, provider_label: null, model: null, key_hint: null };

let settings: Settings = {
  machine_label: null,
  terms: [
    { word: "Palmscapes", strength: "strong", variants: ["palmscape", "palm scape"] },
    { word: "phyllotaxis", strength: "strong", variants: ["phyllotaxy", "phyllotactic", "phylotaxis"] },
    { word: "palm tree", strength: "strong", variants: ["palmtree"] },
    { word: "axial memory", strength: "strong", variants: [] },
    { word: "palm", strength: "palm", variants: ["palms"] },
    { word: "stalk", strength: "palm", variants: ["stalks"] },
    { word: "frond", strength: "related", variants: ["fronds"] },
    { word: "fibonacci", strength: "related", variants: [] },
    { word: "golden angle", strength: "related", variants: ["137.5"] },
  ],
  modules: { user_objects: true, rhino_models: true, scripts: true, archives: true, documents: true },
  read_cloud_files: false,
  roots_off: [],
  roots_on: [],
  folders: [],
  ai_model: null,
};

let roots: Root[] = [
  { path: "/", label: "This Mac", kind: "system", selected: true, available: true, note: null, total_bytes: 494e9, free_bytes: 121e9 },
  { path: "/Volumes/LaCie", label: "LaCie", kind: "external", selected: true, available: true, note: "External drive", total_bytes: 2e12, free_bytes: 1.1e12 },
  { path: "/Volumes/Studio NAS", label: "Studio NAS", kind: "network", selected: false, available: true, note: "Network drive. Searching it can be slow; turn it on if projects live there.", total_bytes: null, free_bytes: null },
];

function file(id: number, machine: number, path: string, extra: Partial<FileView> = {}): FileView {
  const name = path.split(/[\\/]/).pop()!;
  const folder = path.slice(0, path.length - name.length - 1);
  const ext = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
  return {
    id,
    machine,
    path,
    folder,
    name,
    ext,
    kind: "definition",
    size: 180_000 + id * 7_331,
    created: now - (300 - id) * day,
    modified: now - (90 - id) * day,
    sha256: `sha${id}`,
    status: "read",
    detail: null,
    tags: [],
    read: "parsed",
    read_error: null,
    objects: 40 + id * 3,
    groups: [],
    plugins: [],
    gh_version: "1.0.7",
    copies: 1,
    family: name.replace(/[^a-z]/gi, "").toLowerCase(),
    in_archive: null,
    candidate: null,
    ...extra,
  };
}

const files: FileView[] = [
  file(0, 0, `${HOME}/Documents/Thesis/Palmscapes/palm_stalk_v4.gh`, { groups: ["Phyllotaxis spiral", "Stalk taper", "Frond array"], plugins: ["Kangaroo Components 2.42"], sha256: "a1", copies: 3 }),
  file(1, 0, `${HOME}/Desktop/palm_stalk_v4 copy.gh`, { sha256: "a1", copies: 3, tags: ["backup_copy"] }),
  file(2, 1, "C:\\Users\\Studio\\OneDrive\\Thesis\\palm_stalk_v4.gh", { sha256: "a1", copies: 3, tags: ["cloud_folder"] }),
  file(3, 0, `${HOME}/Documents/Thesis/Palmscapes/palm_stalk_v3.gh`, { groups: ["Spiral", "Stalk"], sha256: "a0", modified: now - 140 * day }),
  file(4, 0, `${HOME}/Documents/Studio/seat_pattern.ghx`, { groups: ["Seat"], sha256: "b1" }),
  file(5, 1, "C:\\Users\\Studio\\Downloads\\vogel_sunflower.gh", { sha256: "c1", groups: ["Vogel model"], tags: ["downloads"] }),
  file(6, 0, `${HOME}/Library/Mobile Documents/com~apple~CloudDocs/Old/palmtree_facade.gh`, { status: "cloud_only", sha256: null, read: "inconclusive", objects: null, gh_version: null, tags: ["cloud_folder"], detail: "Only in the cloud: the name was checked, the file wasn't downloaded" }),
  file(7, 0, `${HOME}/Documents/Archive/2019.zip!/workshop/stalks.gh`, { sha256: "d1", in_archive: `${HOME}/Documents/Archive/2019.zip`, tags: ["in_archive"] }),
  file(8, 1, "C:\\Users\\Studio\\AppData\\Roaming\\Grasshopper\\AutoSave\\unnamed_autosave.gh", { sha256: "e1", groups: ["Phyllotaxis test"], tags: ["autosave"] }),
  file(9, 0, `${HOME}/Documents/Studio/tower.gh`, { sha256: "f1" }),
  file(10, 0, `${HOME}/Documents/Studio/broken_export.gh`, { sha256: "g1", read: "inconclusive", read_error: "the file stops part-way through, so it's probably incomplete or damaged", objects: null }),
  file(11, 0, `${HOME}/Documents/Thesis/Palmscapes/site_model.3dm`, { kind: "rhino_model", sha256: "h1", read: "not_applicable", objects: null, gh_version: null, family: null, size: 84_000_000 }),
  file(12, 1, "C:\\Users\\Studio\\Documents\\chair.gh", { sha256: "i1" }),
  file(13, 1, "C:\\Users\\Studio\\Documents\\chair (1).gh", { sha256: "i1", copies: 2 }),
];
files[12].copies = 2;

const ev = (term: string, strength: Candidate["evidence"][0]["strength"], source: Candidate["evidence"][0]["source"], text: string) => ({ term, strength, source, text });

const candidates: Candidate[] = [
  { id: 0, rank: 1, confidence: "strong", score: 4.1, kind: "definition", name: "palm_stalk_v4.gh", file: 0, files: [0, 1, 2], machines: [0, 1], sha256: "a1", terms: ["phyllotaxis", "Palmscapes", "palm", "stalk", "frond"], evidence: [ev("phyllotaxis", "strong", "group", "Phyllotaxis spiral"), ev("Palmscapes", "strong", "folder", "Palmscapes"), ev("palm", "palm", "file_name", "palm_stalk_v4.gh"), ev("stalk", "palm", "group", "Stalk taper"), ev("golden angle", "related", "panel", "angle = 137.5 between each frond base"), ev("frond", "related", "group", "Frond array")], evidence_total: 6, reason: "Group label: “Phyllotaxis spiral”" },
  { id: 1, rank: 2, confidence: "strong", score: 2.4, kind: "definition", name: "palm_stalk_v3.gh", file: 3, files: [3], machines: [0], sha256: "a0", terms: ["Palmscapes", "palm", "stalk"], evidence: [ev("palm", "palm", "file_name", "palm_stalk_v3.gh"), ev("stalk", "palm", "group", "Stalk"), ev("Palmscapes", "strong", "folder", "Palmscapes")], evidence_total: 3, reason: "File name: “palm_stalk_v3.gh”" },
  { id: 2, rank: 3, confidence: "strong", score: 1.9, kind: "definition", name: "unnamed_autosave.gh", file: 8, files: [8], machines: [1], sha256: "e1", terms: ["phyllotaxis"], evidence: [ev("phyllotaxis", "strong", "group", "Phyllotaxis test")], evidence_total: 1, reason: "Group label: “Phyllotaxis test”" },
  { id: 3, rank: 4, confidence: "strong", score: 1.0, kind: "definition", name: "palmtree_facade.gh", file: 6, files: [6], machines: [0], sha256: null, terms: ["palm tree"], evidence: [ev("palm tree", "strong", "file_name", "palmtree_facade.gh")], evidence_total: 1, reason: "File name: “palmtree_facade.gh”" },
  { id: 4, rank: 5, confidence: "possible", score: 0.95, kind: "definition", name: "stalks.gh", file: 7, files: [7], machines: [0], sha256: "d1", terms: ["stalk"], evidence: [ev("stalk", "palm", "file_name", "stalks.gh")], evidence_total: 1, reason: "File name: “stalks.gh”" },
  { id: 5, rank: 6, confidence: "possible", score: 0.85, kind: "definition", name: "seat_pattern.ghx", file: 4, files: [4], machines: [0], sha256: "b1", terms: ["phyllotaxis"], evidence: [ev("phyllotaxis", "strong", "plugin", "Phyllotaxis Tools")], evidence_total: 1, reason: "Plugin: “Phyllotaxis Tools”" },
  { id: 6, rank: 7, confidence: "weak", score: 0.5, kind: "definition", name: "vogel_sunflower.gh", file: 5, files: [5], machines: [1], sha256: "c1", terms: ["vogel", "sunflower"], evidence: [ev("vogel", "related", "file_name", "vogel_sunflower.gh"), ev("sunflower", "related", "file_name", "vogel_sunflower.gh")], evidence_total: 2, reason: "File name: “vogel_sunflower.gh”" },
  { id: 7, rank: 8, confidence: "inconclusive", score: 0, kind: "definition", name: "broken_export.gh", file: 10, files: [10], machines: [0], sha256: "g1", terms: [], evidence: [], evidence_total: 0, reason: "Couldn't look inside: the file stops part-way through, so it's probably incomplete or damaged" },
];
for (const c of candidates) for (const f of c.files) files[f].candidate = c.id;

const totals = (m?: number): Totals => {
  const fs = files.filter((f) => m === undefined || f.machine === m);
  const cs = candidates.filter((c) => m === undefined || c.machines.includes(m));
  return {
    definitions: fs.filter((f) => f.kind === "definition").length,
    unique_definitions: new Set(fs.filter((f) => f.kind === "definition").map((f) => f.sha256 ?? f.path)).size,
    user_objects: 0,
    rhino_models: fs.filter((f) => f.kind === "rhino_model").length,
    scripts: 0,
    archives: 1,
    documents: 0,
    strong: cs.filter((c) => c.confidence === "strong").length,
    possible: cs.filter((c) => c.confidence === "possible").length,
    weak: cs.filter((c) => c.confidence === "weak").length,
    inconclusive: cs.filter((c) => c.confidence === "inconclusive").length,
    cloud_only: fs.filter((f) => f.status === "cloud_only").length,
    unreadable: 0,
    damaged: fs.filter((f) => f.read === "inconclusive" && f.status === "read").length,
    issues: m === 1 ? 2 : 4,
    skipped: 9,
  };
};

const results = (): Results => ({
  machines: [
    { index: 0, id: "mac", label: "Mac", os: "macos", os_version: "15.4", device: "Studio MacBook Air", scan_id: "s1", scanned_at: now - 3600, status: "complete", this_computer: true, dirs: 182_334, files_seen: 1_204_551, totals: totals(0) },
    { index: 1, id: "win", label: "Windows laptop", os: "windows", os_version: "11", device: "STUDIO-PC", scan_id: "s2", scanned_at: now - 2 * day, status: "complete", this_computer: false, dirs: 96_120, files_seen: 640_002, totals: totals(1) },
  ],
  files,
  candidates,
  clusters: [
    { sha256: "a1", name: "palm_stalk_v4.gh", files: [0, 1, 2], machines: [0, 1], size: files[0].size },
    { sha256: "i1", name: "chair.gh", files: [12, 13], machines: [1], size: files[12].size },
  ],
  families: [
    { key: "palmstalk", label: "palm_stalk_v4.gh", versions: [{ sha256: "a1", files: [0, 1, 2], newest: files[0].modified, candidate: 0 }, { sha256: "a0", files: [3], newest: files[3].modified, candidate: 1 }], files: 4, best: "strong", companions: [11] },
    { key: "chair", label: "chair.gh", versions: [{ sha256: "i1", files: [12, 13], newest: files[12].modified, candidate: null }], files: 2, best: null, companions: [] },
  ],
  issues: [
    { machine: 0, path: `${HOME}/Library/Mail`, kind: "permission_denied", detail: "The computer didn't allow reading this" },
    { machine: 0, path: `${HOME}/Library/Messages`, kind: "permission_denied", detail: "The computer didn't allow reading this" },
    { machine: 0, path: files[6].path, kind: "cloud_only", detail: "Only in the cloud: the name was checked, the file wasn't downloaded" },
    { machine: 0, path: files[10].path, kind: "damaged", detail: "the file stops part-way through, so it's probably incomplete or damaged" },
    { machine: 0, path: "/System", kind: "skipped", detail: "operating-system files" },
    { machine: 1, path: "C:\\Windows", kind: "skipped", detail: "Windows system files" },
    { machine: 1, path: "C:\\Users\\Default", kind: "permission_denied", detail: "The computer didn't allow reading this" },
    { machine: 1, path: "C:\\System Volume Information", kind: "skipped", detail: "operating-system index" },
  ],
  coverage: [
    { machine: 0, root: "/", label: "This Mac", kind: "system", status: "scanned", dirs: 160_002, files: 1_100_220, found: 12, issues: 4, note: null },
    { machine: 0, root: "/Volumes/LaCie", label: "LaCie", kind: "external", status: "scanned", dirs: 22_332, files: 104_331, found: 0, issues: 0, note: "External drive" },
    { machine: 0, root: "/Volumes/Studio NAS", label: "Studio NAS", kind: "network", status: "not_selected", dirs: 0, files: 0, found: 0, issues: 0, note: "Network drive" },
    { machine: 1, root: "C:\\", label: "This PC (C:)", kind: "system", status: "scanned", dirs: 96_120, files: 640_002, found: 6, issues: 2, note: null },
  ],
  totals: totals(),
  comparison: { a: 0, b: 1, only_a: 5, only_b: 3, both: 1 },
});

const explanation = (sha: string): Explanation => ({
  sha256: sha,
  file_name: "palm_stalk_v4.gh",
  provider: "Claude",
  model: "claude-sonnet",
  created_at: now - 600,
  markdown: `## What it does
Builds a palm stalk from a spiral of points: a **Phyllotaxis spiral** places frond bases using the golden angle, and the stalk is tapered along them.

## Stages
1. **Phyllotaxis spiral** – "Count" (144) points are spaced by 137.5° and lifted up the stalk.
2. **Stalk taper** – a curve through the points is lofted with a shrinking radius.
3. **Frond array** – fronds are oriented to each point and copied.

## Palm and phyllotaxis
The group "Phyllotaxis spiral" and the panel "angle = 137.5 between each frond base" are the phyllotaxis logic.

## What to change
- "Count" = 144 (0–500)
- "Height" = 8.5

## Needs
Kangaroo Components 2.42.

## Unsure
The frond geometry comes from Rhino and isn't in the extract.`,
});

export async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((r) => setTimeout(r, 120));
  const a = (args ?? {}) as Record<string, any>;
  const out = ((): unknown => {
    switch (cmd) {
      case "app_info":
        return { version: "0.1.0", os: "macos", machine_id: "mac", machine_label: "Mac", full_disk_access: false, scanning: scanning !== null, home: HOME } satisfies AppInfo;
      case "list_roots":
        return roots;
      case "add_folder":
        roots = [...roots, { path: a.path, label: a.path.split("/").pop(), kind: "folder", selected: true, available: true, note: "Folder you added", total_bytes: null, free_bytes: null }];
        return roots;
      case "remove_folder":
        roots = roots.filter((r) => r.path !== a.path);
        return roots;
      case "start_scan":
        simulateScan();
        return null;
      case "cancel_scan":
        if (scanning) finish("stopped");
        return null;
      case "scan_progress":
        return scanning;
      case "load_results":
        return hasResults ? results() : null;
      case "import_scan":
        return { label: "Windows laptop", os: "windows", scanned_at: now - 2 * day, this_computer: false };
      case "default_scan_file_name":
        return "Mac scan 2026-10-10.axialscan";
      case "export_report":
        return `${a.parent}/Axial Memory report 2026-10-10 12.30`;
      case "get_settings":
        return settings;
      case "save_settings":
        settings = a.settings;
        return settings;
      case "reset_terms":
        return settings;
      case "ai_status":
        return ai;
      case "set_ai_key":
        if (!String(a.key).startsWith("sk-")) throw "That doesn't look like a Claude key (sk-ant-…) or a ChatGPT key (sk-…).";
        ai = { configured: true, provider: "anthropic", provider_label: "Claude", model: "claude-sonnet", key_hint: "sk-ant-…9f2c" };
        return ai;
      case "clear_ai_key":
        ai = { configured: false, provider: null, provider_label: null, model: null, key_hint: null };
        return ai;
      case "ai_preview":
        return 'File: palm_stalk_v4.gh\nSaved by Grasshopper 1.0.7\nPlugins: Kangaroo Components 2.42\n\nGroups (component numbers inside):\n- "Phyllotaxis spiral": 2, 3, 4\n\nComponents in dataflow order (37 total).\n1. Number Slider "Count" = 144 (range 0 to 500)\n2. Series "Ser" <- 1\n3. Multiplication "golden angle" <- 2\n…';
      case "get_explanation":
        return a.sha256 === "a1" ? explanation("a1") : null;
      case "explain_file":
        return explanation("a1");
      default:
        return null;
    }
  })();
  return out as T;
}

function simulateScan() {
  const started = Date.now();
  scanning = { phase: "walking", root: "This Mac", current: "/", dirs: 0, files_seen: 0, definitions: 0, other_found: 0, queued: 0, read: 0, issues: 0, elapsed_ms: 0 };
  const folders = [`${HOME}/Documents/Thesis`, `${HOME}/Library/Application Support/McNeel/Rhinoceros/8.0`, `${HOME}/Desktop`, "/Volumes/LaCie/Projects 2019"];
  const tick = setInterval(() => {
    if (!scanning) return clearInterval(tick);
    const t = Date.now() - started;
    const walking = t < 5000;
    scanning = {
      ...scanning,
      phase: walking ? "walking" : "reading",
      current: folders[Math.floor(t / 1300) % folders.length] + "/…",
      dirs: Math.floor(t * 31),
      files_seen: Math.floor(t * 212),
      definitions: Math.floor(t / 420),
      other_found: Math.floor(t / 900),
      queued: Math.floor(t / 300),
      read: Math.floor(t / 330),
      issues: Math.floor(t / 2000),
      elapsed_ms: t,
    };
    progressListeners.forEach((l) => l(scanning!));
    if (t > 6500) finish("complete");
  }, 200);
}

function finish(status: ScanFinished["status"]) {
  scanning = null;
  hasResults = true;
  finishedListeners.forEach((l) => l({ status, error: null }));
}

export async function onProgress(cb: (p: Progress) => void) {
  progressListeners.push(cb);
  return () => {
    progressListeners = progressListeners.filter((l) => l !== cb);
  };
}

export async function onFinished(cb: (f: ScanFinished) => void) {
  finishedListeners.push(cb);
  return () => {
    finishedListeners = finishedListeners.filter((l) => l !== cb);
  };
}
