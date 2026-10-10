import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  AiStatus,
  AppInfo,
  Explanation,
  Imported,
  Progress,
  Results,
  Root,
  ScanFinished,
  Settings,
} from "./types";
import * as mock from "./mock";

/** False in a plain browser (`pnpm ui`), where a mock backend stands in. */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return inTauri ? invoke<T>(cmd, args) : mock.call<T>(cmd, args);
}

export const api = {
  appInfo: () => call<AppInfo>("app_info"),
  listRoots: () => call<Root[]>("list_roots"),
  addFolder: (path: string) => call<Root[]>("add_folder", { path }),
  removeFolder: (path: string) => call<Root[]>("remove_folder", { path }),
  startScan: (selected: string[]) => call<void>("start_scan", { selected }),
  cancelScan: () => call<void>("cancel_scan"),
  scanProgress: () => call<Progress | null>("scan_progress"),
  loadResults: () => call<Results | null>("load_results"),
  importScan: (path: string) => call<Imported>("import_scan", { path }),
  removeMachine: (machineId: string) => call<void>("remove_machine", { machineId }),
  defaultScanFileName: () => call<string>("default_scan_file_name"),
  saveScanFile: (dest: string) => call<void>("save_scan_file", { dest }),
  exportReport: (parent: string) => call<string>("export_report", { parent }),
  reveal: (path: string) => call<void>("reveal", { path }),
  openPrivacySettings: () => call<void>("open_privacy_settings"),
  getSettings: () => call<Settings>("get_settings"),
  saveSettings: (settings: Settings) => call<Settings>("save_settings", { settings }),
  resetTerms: () => call<Settings>("reset_terms"),
  aiStatus: () => call<AiStatus>("ai_status"),
  setAiKey: (key: string) => call<AiStatus>("set_ai_key", { key }),
  clearAiKey: () => call<AiStatus>("clear_ai_key"),
  aiPreview: (fileId: number) => call<string>("ai_preview", { fileId }),
  getExplanation: (sha256: string) => call<Explanation | null>("get_explanation", { sha256 }),
  explainFile: (fileId: number) => call<Explanation>("explain_file", { fileId }),
};

export function onProgress(cb: (p: Progress) => void): Promise<() => void> {
  return inTauri ? listen<Progress>("scan-progress", (e) => cb(e.payload)) : mock.onProgress(cb);
}

export function onFinished(cb: (f: ScanFinished) => void): Promise<() => void> {
  return inTauri ? listen<ScanFinished>("scan-finished", (e) => cb(e.payload)) : mock.onFinished(cb);
}

export async function pickFolder(): Promise<string | null> {
  if (!inTauri) return "/Users/studio/Old projects";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: false, title: "Choose a folder to search" });
  return typeof picked === "string" ? picked : null;
}

export async function pickScanFile(): Promise<string | null> {
  if (!inTauri) return "/Users/studio/Downloads/Windows laptop scan.axialscan";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({
    multiple: false,
    title: "Choose the other laptop's scan file",
    filters: [{ name: "Axial Memory scan", extensions: ["axialscan"] }],
  });
  return typeof picked === "string" ? picked : null;
}

export async function pickSaveScanFile(defaultName: string): Promise<string | null> {
  if (!inTauri) return `/Users/studio/Desktop/${defaultName}`;
  const { save } = await import("@tauri-apps/plugin-dialog");
  return save({ defaultPath: defaultName, filters: [{ name: "Axial Memory scan", extensions: ["axialscan"] }] });
}

export async function pickReportFolder(): Promise<string | null> {
  if (!inTauri) return "/Users/studio/Desktop";
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: false, title: "Where should the report folder go?" });
  return typeof picked === "string" ? picked : null;
}

export interface UpdateOffer {
  version: string;
  install: () => Promise<void>;
}

export async function checkForUpdate(): Promise<UpdateOffer | null> {
  if (!inTauri) return null;
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check();
  if (!update) return null;
  return {
    version: update.version,
    install: async () => {
      await update.downloadAndInstall();
      const { relaunch } = await import("@tauri-apps/plugin-process");
      await relaunch();
    },
  };
}
