// Mirrors the Rust types the app sends (core/src/model.rs, results.rs, terms.rs, scan.rs).

export type Kind = "definition" | "user_object" | "rhino_model" | "rhino_backup" | "script" | "archive" | "document";
export type Status = "read" | "cloud_only" | "unreadable" | "too_large" | "name_only";
export type GhRead = "parsed" | "inconclusive" | "not_applicable";
export type Confidence = "strong" | "possible" | "weak" | "inconclusive";
export type Strength = "strong" | "palm" | "related";
export type Source =
  | "file_name"
  | "folder"
  | "inside_archive_path"
  | "definition"
  | "group"
  | "panel"
  | "note"
  | "nickname"
  | "script"
  | "value"
  | "component"
  | "plugin"
  | "port"
  | "user_object";
export type IssueKind =
  | "permission_denied"
  | "unreadable"
  | "skipped"
  | "cloud_only"
  | "archive_limit"
  | "too_large"
  | "damaged"
  | "vanished";
export type RootKind = "system" | "external" | "network" | "cloud" | "backup" | "folder";
export type CoverageStatus = "scanned" | "stopped" | "not_selected" | "unavailable";

export interface Root {
  path: string;
  label: string;
  kind: RootKind;
  selected: boolean;
  available: boolean;
  note: string | null;
  total_bytes: number | null;
  free_bytes: number | null;
}

export interface Progress {
  phase: "walking" | "reading" | "done";
  root: string;
  current: string;
  dirs: number;
  files_seen: number;
  definitions: number;
  other_found: number;
  queued: number;
  read: number;
  issues: number;
  elapsed_ms: number;
}

export interface ScanFinished {
  status: "complete" | "stopped" | "failed";
  error: string | null;
}

export interface Totals {
  definitions: number;
  unique_definitions: number;
  user_objects: number;
  rhino_models: number;
  scripts: number;
  archives: number;
  documents: number;
  strong: number;
  possible: number;
  weak: number;
  inconclusive: number;
  cloud_only: number;
  unreadable: number;
  damaged: number;
  issues: number;
  skipped: number;
}

export interface MachineView {
  index: number;
  id: string;
  label: string;
  os: string;
  os_version: string;
  device: string;
  scan_id: string;
  scanned_at: number;
  status: "complete" | "stopped";
  this_computer: boolean;
  dirs: number;
  files_seen: number;
  totals: Totals;
}

export interface FileView {
  id: number;
  machine: number;
  path: string;
  folder: string;
  name: string;
  ext: string;
  kind: Kind;
  size: number | null;
  created: number | null;
  modified: number | null;
  sha256: string | null;
  status: Status;
  detail: string | null;
  tags: string[];
  read: GhRead;
  read_error: string | null;
  objects: number | null;
  groups: string[];
  plugins: string[];
  gh_version: string | null;
  copies: number;
  family: string | null;
  in_archive: string | null;
  candidate: number | null;
}

export interface Evidence {
  term: string;
  strength: Strength;
  source: Source;
  text: string;
  in_cluster?: boolean;
}

export interface Candidate {
  id: number;
  rank: number;
  confidence: Confidence;
  score: number;
  kind: Kind;
  name: string;
  file: number;
  files: number[];
  machines: number[];
  sha256: string | null;
  terms: string[];
  evidence: Evidence[];
  evidence_total: number;
  reason: string;
}

export interface Cluster {
  sha256: string;
  name: string;
  files: number[];
  machines: number[];
  size: number | null;
}

export interface Version {
  sha256: string | null;
  files: number[];
  newest: number | null;
  candidate: number | null;
}

export interface Family {
  key: string;
  label: string;
  versions: Version[];
  files: number;
  best: Confidence | null;
  companions: number[];
}

export interface IssueView {
  machine: number;
  path: string;
  kind: IssueKind;
  detail: string;
}

export interface CoverageView {
  machine: number;
  root: string;
  label: string;
  kind: RootKind;
  status: CoverageStatus;
  dirs: number;
  files: number;
  found: number;
  issues: number;
  note: string | null;
}

export interface Comparison {
  a: number;
  b: number;
  only_a: number;
  only_b: number;
  both: number;
}

export interface Results {
  machines: MachineView[];
  files: FileView[];
  candidates: Candidate[];
  clusters: Cluster[];
  families: Family[];
  issues: IssueView[];
  coverage: CoverageView[];
  totals: Totals;
  comparison: Comparison | null;
}

export interface Term {
  word: string;
  strength: Strength;
  variants: string[];
}

export interface Modules {
  user_objects: boolean;
  rhino_models: boolean;
  scripts: boolean;
  archives: boolean;
  documents: boolean;
}

export interface Settings {
  machine_label: string | null;
  terms: Term[];
  modules: Modules;
  read_cloud_files: boolean;
  roots_off: string[];
  roots_on: string[];
  folders: string[];
  ai_model: string | null;
}

export interface AiStatus {
  configured: boolean;
  provider: "anthropic" | "openai" | null;
  provider_label: string | null;
  model: string | null;
  key_hint: string | null;
}

export interface Explanation {
  sha256: string;
  file_name: string;
  markdown: string;
  model: string;
  provider: string;
  created_at: number;
}

export interface AppInfo {
  version: string;
  os: string;
  machine_id: string;
  machine_label: string;
  full_disk_access: boolean | null;
  scanning: boolean;
  home: string | null;
}

export interface Imported {
  label: string;
  os: string;
  scanned_at: number;
  this_computer: boolean;
}
