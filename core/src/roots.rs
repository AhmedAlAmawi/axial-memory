//! The drives and volumes a scan can cover on this computer.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RootKind {
    /// The computer's own disk.
    System,
    /// USB sticks, external and secondary disks, mounted disk images.
    External,
    Network,
    /// A drive that streams files from the cloud (Google Drive's G:).
    Cloud,
    /// Time Machine and similar backup volumes.
    Backup,
    /// A folder picked by hand.
    Folder,
}

impl RootKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RootKind::System => "system",
            RootKind::External => "external",
            RootKind::Network => "network",
            RootKind::Cloud => "cloud",
            RootKind::Backup => "backup",
            RootKind::Folder => "folder",
        }
    }

    pub fn parse(s: &str) -> RootKind {
        match s {
            "system" => RootKind::System,
            "external" => RootKind::External,
            "network" => RootKind::Network,
            "cloud" => RootKind::Cloud,
            "backup" => RootKind::Backup,
            _ => RootKind::Folder,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Root {
    pub path: String,
    pub label: String,
    pub kind: RootKind,
    /// Whether it's searched unless she turns it off.
    pub selected: bool,
    pub available: bool,
    pub note: Option<String>,
    pub total_bytes: Option<u64>,
    pub free_bytes: Option<u64>,
}

impl Root {
    pub fn folder(path: &str) -> Root {
        let label = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());
        Root {
            path: path.to_string(),
            label,
            kind: RootKind::Folder,
            selected: true,
            available: true,
            note: None,
            total_bytes: None,
            free_bytes: None,
        }
    }

    /// The root of the computer's own disk, where operating-system folders are skipped.
    pub fn is_system_root(&self) -> bool {
        self.kind == RootKind::System
    }
}

pub fn discover() -> Vec<Root> {
    #[cfg(target_os = "macos")]
    {
        mac::discover()
    }
    #[cfg(windows)]
    {
        win::discover()
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        vec![Root { kind: RootKind::System, label: "This computer".into(), ..Root::folder("/") }]
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use super::{Root, RootKind};
    use std::ffi::{CStr, CString};

    struct Fs {
        fstype: String,
        total: u64,
        free: u64,
    }

    fn statfs(path: &str) -> Option<Fs> {
        let c = CString::new(path).ok()?;
        let mut s: libc::statfs = unsafe { std::mem::zeroed() };
        if unsafe { libc::statfs(c.as_ptr(), &mut s) } != 0 {
            return None;
        }
        let fstype = unsafe { CStr::from_ptr(s.f_fstypename.as_ptr()) }.to_string_lossy().into_owned();
        Some(Fs {
            fstype,
            total: s.f_blocks.saturating_mul(s.f_bsize as u64),
            free: s.f_bavail.saturating_mul(s.f_bsize as u64),
        })
    }

    pub fn discover() -> Vec<Root> {
        let data = statfs("/System/Volumes/Data").or_else(|| statfs("/"));
        let mut roots = vec![Root {
            path: "/".into(),
            label: "This Mac".into(),
            kind: RootKind::System,
            selected: true,
            available: true,
            note: None,
            total_bytes: data.as_ref().map(|f| f.total),
            free_bytes: data.as_ref().map(|f| f.free),
        }];
        let Ok(entries) = std::fs::read_dir("/Volumes") else { return roots };
        let mut volumes: Vec<_> = entries.flatten().collect();
        volumes.sort_by_key(|e| e.file_name());
        for entry in volumes {
            let name = entry.file_name().to_string_lossy().into_owned();
            // The startup disk appears here as a link back to "/".
            let is_link = entry.file_type().map(|t| t.is_symlink()).unwrap_or(true);
            if name.starts_with('.') || is_link {
                continue;
            }
            let path = format!("/Volumes/{name}");
            let fs = statfs(&path);
            let fstype = fs.as_ref().map(|f| f.fstype.to_lowercase()).unwrap_or_default();
            let (kind, selected, note) = if ["smbfs", "afpfs", "nfs", "webdav", "ftp", "cifs"]
                .contains(&fstype.as_str())
            {
                (
                    RootKind::Network,
                    false,
                    Some("Network drive. Searching it can be slow; turn it on if projects live there.".to_string()),
                )
            } else if name.starts_with("Backups of ") || name.to_lowercase().contains("time machine") {
                (RootKind::Backup, false, Some("Backup disk. Turn on to search old backups too (slow).".to_string()))
            } else if name.to_lowercase().contains("googledrive") || name.to_lowercase() == "google drive" {
                (
                    RootKind::Cloud,
                    true,
                    Some("Streams from the cloud: names are checked, files aren't downloaded.".to_string()),
                )
            } else {
                (RootKind::External, true, Some("External drive".to_string()))
            };
            roots.push(Root {
                path,
                label: name,
                kind,
                selected,
                available: fs.is_some(),
                note,
                total_bytes: fs.as_ref().map(|f| f.total),
                free_bytes: fs.as_ref().map(|f| f.free),
            });
        }
        roots
    }
}

#[cfg(windows)]
mod win {
    use super::{Root, RootKind};
    use windows_sys::Win32::Storage::FileSystem::{
        GetDiskFreeSpaceExW, GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW,
    };
    use windows_sys::Win32::System::Diagnostics::Debug::{SetErrorMode, SEM_FAILCRITICALERRORS};

    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOTE: u32 = 4;
    const DRIVE_CDROM: u32 = 5;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    pub fn discover() -> Vec<Root> {
        // Without this, probing an empty card reader pops up "There is no disk in the drive".
        unsafe { SetErrorMode(SEM_FAILCRITICALERRORS) };
        let mut buf = vec![0u16; 512];
        let n = unsafe { GetLogicalDriveStringsW(buf.len() as u32, buf.as_mut_ptr()) } as usize;
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".into()).to_uppercase();
        let mut roots = Vec::new();
        for drive in buf[..n.min(buf.len())].split(|&c| c == 0).filter(|s| !s.is_empty()) {
            let path = String::from_utf16_lossy(drive);
            let w = wide(&path);
            let drive_type = unsafe { GetDriveTypeW(w.as_ptr()) };
            let mut label_buf = [0u16; 261];
            let mut fs_buf = [0u16; 261];
            let ok = unsafe {
                GetVolumeInformationW(
                    w.as_ptr(),
                    label_buf.as_mut_ptr(),
                    label_buf.len() as u32,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    fs_buf.as_mut_ptr(),
                    fs_buf.len() as u32,
                )
            } != 0;
            let label = from_wide(&label_buf);
            let (mut free, mut total) = (0u64, 0u64);
            let sized =
                ok && unsafe { GetDiskFreeSpaceExW(w.as_ptr(), &mut free, &mut total, std::ptr::null_mut()) } != 0;
            let letter = path.trim_end_matches('\\').to_string();
            let is_system = letter.eq_ignore_ascii_case(&system_drive);
            let (kind, selected, note) = if is_system {
                (RootKind::System, true, None)
            } else if label.to_lowercase().contains("google drive") {
                (
                    RootKind::Cloud,
                    true,
                    Some("Streams from the cloud: names are checked, files aren't downloaded.".to_string()),
                )
            } else {
                match drive_type {
                    DRIVE_REMOTE => (
                        RootKind::Network,
                        false,
                        Some("Network drive. Searching it can be slow; turn it on if projects live there.".to_string()),
                    ),
                    DRIVE_REMOVABLE => (RootKind::External, true, Some("Removable drive".to_string())),
                    DRIVE_CDROM => (RootKind::External, ok, Some("CD/DVD".to_string())),
                    DRIVE_FIXED => (RootKind::External, true, Some("Another disk in this computer".to_string())),
                    _ => (RootKind::External, ok, None),
                }
            };
            let pretty = if is_system {
                format!("This PC ({letter})")
            } else if label.is_empty() {
                letter.clone()
            } else {
                format!("{label} ({letter})")
            };
            roots.push(Root {
                path,
                label: pretty,
                kind,
                selected: selected && ok,
                available: ok,
                note: if ok { note } else { Some("Nothing in this drive right now".to_string()) },
                total_bytes: sized.then_some(total),
                free_bytes: sized.then_some(free),
            });
        }
        roots
    }
}
