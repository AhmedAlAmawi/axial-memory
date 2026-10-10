//! What a scan records. One `ScanData` per laptop per scan; it is what gets
//! saved to the portable scan file and carried to the other laptop.

use crate::gh::{Digest, Summary};
use crate::roots::RootKind;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Definition,
    UserObject,
    RhinoModel,
    RhinoBackup,
    Script,
    Archive,
    Document,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Definition => "definition",
            Kind::UserObject => "user_object",
            Kind::RhinoModel => "rhino_model",
            Kind::RhinoBackup => "rhino_backup",
            Kind::Script => "script",
            Kind::Archive => "archive",
            Kind::Document => "document",
        }
    }

    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "definition" => Kind::Definition,
            "user_object" => Kind::UserObject,
            "rhino_model" => Kind::RhinoModel,
            "rhino_backup" => Kind::RhinoBackup,
            "script" => Kind::Script,
            "archive" => Kind::Archive,
            "document" => Kind::Document,
            _ => return None,
        })
    }

    pub fn label(self) -> &'static str {
        match self {
            Kind::Definition => "Grasshopper definition",
            Kind::UserObject => "Grasshopper user object",
            Kind::RhinoModel => "Rhino model",
            Kind::RhinoBackup => "Rhino backup",
            Kind::Script => "Script",
            Kind::Archive => "Archive",
            Kind::Document => "Document",
        }
    }

    /// Files whose insides this app can read.
    pub fn is_grasshopper(self) -> bool {
        matches!(self, Kind::Definition | Kind::UserObject)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    /// Opened, hashed and (for Grasshopper files) read.
    Read,
    /// A cloud placeholder: listed by name, never downloaded.
    CloudOnly,
    /// The operating system refused to open it, or reading failed.
    Unreadable,
    /// Bigger than the hashing limit.
    TooLarge,
    /// Listed by name only (archive types this version doesn't open).
    NameOnly,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Read => "read",
            Status::CloudOnly => "cloud_only",
            Status::Unreadable => "unreadable",
            Status::TooLarge => "too_large",
            Status::NameOnly => "name_only",
        }
    }

    pub fn parse(s: &str) -> Status {
        match s {
            "read" => Status::Read,
            "cloud_only" => Status::CloudOnly,
            "unreadable" => Status::Unreadable,
            "too_large" => Status::TooLarge,
            _ => Status::NameOnly,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GhRead {
    Parsed,
    /// Opened but the inside couldn't be read; never proof the file is irrelevant.
    Inconclusive,
    NotApplicable,
}

impl GhRead {
    pub fn as_str(self) -> &'static str {
        match self {
            GhRead::Parsed => "parsed",
            GhRead::Inconclusive => "inconclusive",
            GhRead::NotApplicable => "not_applicable",
        }
    }

    pub fn parse(s: &str) -> GhRead {
        match s {
            "parsed" => GhRead::Parsed,
            "inconclusive" => GhRead::Inconclusive,
            _ => GhRead::NotApplicable,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct FileRecord {
    pub id: u32,
    /// Full path. Files inside a zip are `<zip path>!/<path inside>`.
    pub path: String,
    pub archive: Option<String>,
    pub name: String,
    pub ext: String,
    pub kind: Kind,
    pub size: Option<u64>,
    pub created: Option<i64>,
    pub modified: Option<i64>,
    pub accessed: Option<i64>,
    pub sha256: Option<String>,
    pub status: Status,
    pub detail: Option<String>,
    pub tags: Vec<String>,
    /// Label of the drive or folder it was found under.
    pub root: String,
}

/// What was inside a file, once per distinct SHA-256.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Content {
    pub sha256: String,
    pub read: GhRead,
    pub error: Option<String>,
    pub summary: Option<Summary>,
    pub digest: Option<Digest>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IssueKind {
    PermissionDenied,
    Unreadable,
    Skipped,
    CloudOnly,
    ArchiveLimit,
    TooLarge,
    Damaged,
    Vanished,
}

impl IssueKind {
    pub fn as_str(self) -> &'static str {
        match self {
            IssueKind::PermissionDenied => "permission_denied",
            IssueKind::Unreadable => "unreadable",
            IssueKind::Skipped => "skipped",
            IssueKind::CloudOnly => "cloud_only",
            IssueKind::ArchiveLimit => "archive_limit",
            IssueKind::TooLarge => "too_large",
            IssueKind::Damaged => "damaged",
            IssueKind::Vanished => "vanished",
        }
    }

    pub fn parse(s: &str) -> IssueKind {
        match s {
            "permission_denied" => IssueKind::PermissionDenied,
            "skipped" => IssueKind::Skipped,
            "cloud_only" => IssueKind::CloudOnly,
            "archive_limit" => IssueKind::ArchiveLimit,
            "too_large" => IssueKind::TooLarge,
            "damaged" => IssueKind::Damaged,
            "vanished" => IssueKind::Vanished,
            _ => IssueKind::Unreadable,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            IssueKind::PermissionDenied => "Locked by the computer",
            IssueKind::Unreadable => "Couldn't be read",
            IssueKind::Skipped => "Skipped on purpose",
            IssueKind::CloudOnly => "Only in the cloud",
            IssueKind::ArchiveLimit => "Archive too big to open fully",
            IssueKind::TooLarge => "Too big to fingerprint",
            IssueKind::Damaged => "Damaged or not really Grasshopper",
            IssueKind::Vanished => "Disappeared during the scan",
        }
    }

    pub fn advice(self) -> &'static str {
        match self {
            IssueKind::PermissionDenied => {
                "On a Mac, turn on Full Disk Access for Axial Memory in System Settings and search again. On Windows these are usually system folders or other people's accounts."
            }
            IssueKind::Unreadable => "The file may be open in another app, or the drive was disconnected. Try again later.",
            IssueKind::Skipped => "Operating-system folders that never hold your work. Nothing to do.",
            IssueKind::CloudOnly => {
                "Not downloaded to this computer, so only the name was checked. Make it available offline (iCloud, OneDrive, Dropbox, Google Drive) and scan again to look inside."
            }
            IssueKind::ArchiveLimit => "Only part of this archive was opened. Unzip it and scan again to check everything.",
            IssueKind::TooLarge => "Listed, but not fingerprinted for duplicates because of its size.",
            IssueKind::Damaged => {
                "The file opened but its contents couldn't be read. It may be damaged or from a format this version doesn't know. Open it in Grasshopper to check."
            }
            IssueKind::Vanished => "The file was moved or deleted while the scan ran.",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Issue {
    pub path: String,
    pub kind: IssueKind,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Scanned,
    Stopped,
    NotSelected,
    Unavailable,
}

impl CoverageStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            CoverageStatus::Scanned => "scanned",
            CoverageStatus::Stopped => "stopped",
            CoverageStatus::NotSelected => "not_selected",
            CoverageStatus::Unavailable => "unavailable",
        }
    }

    pub fn parse(s: &str) -> CoverageStatus {
        match s {
            "scanned" => CoverageStatus::Scanned,
            "stopped" => CoverageStatus::Stopped,
            "unavailable" => CoverageStatus::Unavailable,
            _ => CoverageStatus::NotSelected,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Coverage {
    pub root: String,
    pub label: String,
    pub kind: RootKind,
    pub status: CoverageStatus,
    pub dirs: u64,
    pub files: u64,
    pub found: u64,
    pub issues: u64,
    pub note: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Machine {
    /// Random, stable per install; tells the two laptops apart.
    pub id: String,
    pub label: String,
    pub os: String,
    pub os_version: String,
    pub device: String,
}

impl Machine {
    pub fn current(id: String, label: Option<String>) -> Machine {
        let info = os_info::get();
        let os = std::env::consts::OS.to_string();
        let default_label = match os.as_str() {
            "macos" => "Mac".to_string(),
            "windows" => "Windows laptop".to_string(),
            other => other.to_string(),
        };
        Machine {
            id,
            label: label.filter(|l| !l.trim().is_empty()).unwrap_or(default_label),
            os,
            os_version: info.version().to_string(),
            device: whoami::devicename().unwrap_or_default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScanStatus {
    Complete,
    Stopped,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Modules {
    pub user_objects: bool,
    pub rhino_models: bool,
    pub scripts: bool,
    pub archives: bool,
    pub documents: bool,
}

impl Default for Modules {
    fn default() -> Self {
        Modules { user_objects: true, rhino_models: true, scripts: true, archives: true, documents: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScanOptions {
    /// Downloads cloud-only files in order to read them. Off by default.
    pub read_cloud_files: bool,
    pub modules: Modules,
    /// Files above this size are listed but not hashed.
    pub max_hash_bytes: u64,
}

impl Default for ScanOptions {
    fn default() -> Self {
        ScanOptions { read_cloud_files: false, modules: Modules::default(), max_hash_bytes: 4 * 1024 * 1024 * 1024 }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScanMeta {
    pub scan_id: String,
    pub machine: Machine,
    pub app_version: String,
    pub started_at: i64,
    pub finished_at: i64,
    pub status: ScanStatus,
    pub options: ScanOptions,
    pub dirs: u64,
    pub files_seen: u64,
    pub links_not_followed: u64,
    pub archives_opened: u64,
    pub bytes_read: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ScanData {
    pub meta: ScanMeta,
    pub files: Vec<FileRecord>,
    pub contents: Vec<Content>,
    pub issues: Vec<Issue>,
    pub coverage: Vec<Coverage>,
}

pub fn now_unix() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}
