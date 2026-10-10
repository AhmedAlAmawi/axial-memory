import type { Confidence, CoverageStatus, IssueKind, Kind, Source, Status, Strength } from "./types";

export function bytes(n: number | null | undefined): string {
  if (n == null) return "";
  if (n < 1024) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export function count(n: number, one: string, many = `${one}s`): string {
  return `${n.toLocaleString()} ${n === 1 ? one : many}`;
}

export function when(unix: number | null | undefined): string {
  if (!unix) return "";
  const d = new Date(unix * 1000);
  const now = new Date();
  const time = d.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const days = Math.round((day(now) - day(d)) / 86_400_000);
  if (days === 0) return `today ${time}`;
  if (days === 1) return `yesterday ${time}`;
  return d.toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });
}

export function duration(ms: number): string {
  const s = Math.floor(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  return `${m}m ${String(s % 60).padStart(2, "0")}s`;
}

/** `~/Documents/…` instead of the full home path; the middle of long paths elided. */
export function shortPath(path: string, home: string | null, max = 90): string {
  let p = path;
  if (home && (p === home || p.startsWith(home + "/") || p.startsWith(home + "\\"))) p = "~" + p.slice(home.length);
  if (p.length <= max) return p;
  const head = Math.floor(max * 0.4);
  return `${p.slice(0, head)}…${p.slice(p.length - (max - head - 1))}`;
}

export const confidenceLabel: Record<Confidence, string> = {
  strong: "Strong match",
  possible: "Possible",
  weak: "Weak",
  inconclusive: "Couldn't check inside",
};

export const strengthLabel: Record<Strength, string> = {
  strong: "Strong words",
  palm: "Palm words",
  related: "Related words",
};

export const strengthHelp: Record<Strength, string> = {
  strong: "Almost certainly the palm research. One in a file's name or inside it makes a strong match.",
  palm: "Palm words that also have other meanings, so they count for less on their own.",
  related: "Neighbouring vocabulary. Weak alone, but supports the other words.",
};

export const sourceLabel: Record<Source, string> = {
  file_name: "File name",
  folder: "Folder name",
  inside_archive_path: "Inside the zip",
  definition: "Name saved in the file",
  group: "Group label",
  panel: "Panel text",
  note: "Scribble note",
  nickname: "Renamed component",
  script: "Script code",
  value: "Typed value",
  component: "Component",
  plugin: "Plugin",
  port: "Script input",
  user_object: "User object",
};

export const kindLabel: Record<Kind, string> = {
  definition: "Definition",
  user_object: "User object",
  rhino_model: "Rhino model",
  rhino_backup: "Rhino backup",
  script: "Script",
  archive: "Zip",
  document: "Document",
};

export const statusLabel: Record<Status, string> = {
  read: "Read",
  cloud_only: "Only in the cloud",
  unreadable: "Couldn't be opened",
  too_large: "Too large to fingerprint",
  name_only: "Name checked only",
};

export const coverageLabel: Record<CoverageStatus, string> = {
  scanned: "Searched",
  stopped: "Stopped early",
  not_selected: "Not searched",
  unavailable: "Not connected",
};

export const issueLabel: Record<IssueKind, string> = {
  permission_denied: "Locked by the computer",
  unreadable: "Couldn't be read",
  skipped: "Skipped on purpose",
  cloud_only: "Only in the cloud",
  archive_limit: "Zip too big to open fully",
  too_large: "Too big to fingerprint",
  damaged: "Damaged, or not really Grasshopper",
  vanished: "Disappeared during the search",
};

export const issueAdvice: Record<IssueKind, string> = {
  permission_denied:
    "On a Mac, turn on Full Disk Access for Axial Memory in System Settings and search again. On Windows these are usually system folders or other people's accounts.",
  unreadable: "The file may be open in another app, or a drive was unplugged. Try again later.",
  skipped: "Operating-system folders that never hold your work. Nothing to do.",
  cloud_only:
    "Not downloaded to this computer, so only the name was checked. Make the folder available offline in iCloud, OneDrive, Dropbox or Google Drive and search again to look inside.",
  archive_limit: "Only part of this zip was opened. Unzip it and search again to check everything.",
  too_large: "Listed, but not fingerprinted for duplicates because of its size.",
  damaged:
    "It opened, but what's inside couldn't be read. It may be damaged or newer than this app understands. Open it in Grasshopper to check.",
  vanished: "It was moved or deleted while the search ran.",
};

export const tagLabel: Record<string, string> = {
  autosave: "Autosave",
  user_objects_folder: "User objects folder",
  installed: "Came with software",
  trash: "In the Trash",
  deleted: "Deleted",
  temp: "Temporary folder",
  downloads: "Downloads",
  cloud_folder: "Cloud folder",
  email: "Email attachment",
  messages: "Messages attachment",
  backup: "Backup",
  backup_copy: "Backup copy",
  renamed_grasshopper: "Grasshopper file with another name",
  in_archive: "Inside a zip",
  external_drive: "External drive",
  network_drive: "Network drive",
  cloud_drive: "Cloud drive",
  backup_drive: "Backup drive",
};

export function osName(os: string): string {
  return os === "macos" ? "Mac" : os === "windows" ? "PC" : "computer";
}

export function revealLabel(os: string): string {
  return os === "windows" ? "Show in Explorer" : "Show in Finder";
}
