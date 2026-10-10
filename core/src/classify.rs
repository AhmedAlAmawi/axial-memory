//! What a file is, from its name, and what kind of place it sits in.

use crate::model::Kind;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq)]
pub struct Classified {
    pub kind: Kind,
    /// Lowercase extension of the real file type (`gh` for `palm.gh.bak`).
    pub ext: String,
    /// Name with an iCloud placeholder's wrapping removed.
    pub name: String,
    /// A copy Grasshopper or the user made (`.gh.bak`, `.ghx~`…).
    pub backup: bool,
    /// An old-style iCloud placeholder (`.palm.gh.icloud`): the real file isn't on this computer.
    pub icloud_stub: bool,
    /// An archive type this version only lists (`.rar`, `.7z`).
    pub name_only: bool,
}

const DOCUMENTS: &[&str] = &[
    "pdf", "doc", "docx", "txt", "md", "rtf", "pages", "key", "ppt", "pptx", "xls", "xlsx", "csv", "numbers", "jpg",
    "jpeg", "png", "tif", "tiff", "heic", "psd", "ai", "indd", "svg", "dwg", "dxf", "obj", "stl", "skp", "mp4", "mov",
];

fn backup_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)\.(ghx?)(?:~|\.(?:bak|backup|old|orig|tmp|temp|autosave|copy|\d{1,3}))$").unwrap()
    })
}

pub fn classify(file_name: &str) -> Option<Classified> {
    if let Some(inner) = file_name.strip_prefix('.').and_then(|n| n.strip_suffix(".icloud")) {
        let mut c = classify(inner)?;
        c.icloud_stub = true;
        return Some(c);
    }
    let lower = file_name.to_lowercase();
    let make = |kind: Kind, ext: &str| Classified {
        kind,
        ext: ext.to_string(),
        name: file_name.to_string(),
        backup: false,
        icloud_stub: false,
        name_only: false,
    };
    if let Some(caps) = backup_pattern().captures(&lower) {
        let mut c = make(Kind::Definition, &caps[1]);
        c.backup = true;
        return Some(c);
    }
    let ext = lower.rsplit_once('.').map(|(_, e)| e)?;
    let c = match ext {
        "gh" | "ghx" | "ghcluster" => make(Kind::Definition, ext),
        "ghuser" => make(Kind::UserObject, ext),
        "3dm" => make(Kind::RhinoModel, ext),
        "3dmbak" => make(Kind::RhinoBackup, ext),
        "py" | "cs" | "vb" => make(Kind::Script, ext),
        "zip" => make(Kind::Archive, ext),
        "rar" | "7z" => Classified { name_only: true, ..make(Kind::Archive, ext) },
        e if DOCUMENTS.contains(&e) => make(Kind::Document, e),
        _ => return None,
    };
    Some(c)
}

/// Names worth peeking inside even though they don't say they're Grasshopper:
/// generic backup and temp extensions.
pub fn worth_sniffing(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    matches!(
        lower.rsplit_once('.').map(|(_, e)| e),
        Some("bak" | "backup" | "old" | "orig" | "tmp" | "temp" | "autosave" | "copy")
    )
}

/// Plain-language facts about where a file lives, from its path.
pub fn location_tags(path: &str, kind: Kind) -> Vec<String> {
    let p = format!("{}/", path.replace('\\', "/").to_lowercase());
    let name = p.trim_end_matches('/').rsplit('/').next().unwrap_or("").to_string();
    let has = |s: &str| p.contains(s);
    let mut tags = Vec::new();
    let mut tag = |t: &str| {
        if !tags.iter().any(|x: &String| x == t) {
            tags.push(t.to_string());
        }
    };
    if (has("/grasshopper") && has("/autosave/")) || name.contains("autosave") {
        tag("autosave");
    }
    if has("/userobjects/") {
        tag("user_objects_folder");
    }
    if has("/grasshopper/libraries/")
        || has("/rhinoceros/packages/")
        || has(".app/contents/")
        || has("/program files")
        || has("/appdata/local/programs/")
        || has("/mcneel/rhinoceros/") && has("/plug-ins/") && !has("/autosave/") && !has("/userobjects/")
    {
        tag("installed");
    }
    if has("/.trash/") || has("/.trashes/") || has("/$recycle.bin/") {
        tag("trash");
    }
    if has("/private/var/folders/") || p.starts_with("/tmp/") || has("/appdata/local/temp/") || has("/temporaryitems/")
    {
        tag("temp");
    }
    if has("/downloads/") {
        tag("downloads");
    }
    if has("/library/mobile documents/")
        || has("/library/cloudstorage/")
        || has("/onedrive")
        || has("/dropbox")
        || has("/google drive")
        || has("/googledrive")
        || has("/icloud drive")
        || has("/iclouddrive")
    {
        tag("cloud_folder");
    }
    if has("/library/mail/") || has("/mail downloads/") || has("/com.apple.mail/") || has("/outlook") {
        tag("email");
    }
    if has("/messages/attachments/") || has("whatsapp") || has("telegram") || has("/microsoft/teams/") || has("/slack/")
    {
        tag("messages");
    }
    if kind == Kind::RhinoBackup || has("/backups.backupdb/") || has("/backup") {
        tag("backup");
    }
    tags
}

/// Folders that never hold someone's work, skipped by design and reported as such.
pub fn skip_reason(path: &str, is_system_root: bool) -> Option<&'static str> {
    let p = path.replace('\\', "/");
    let lower = p.to_lowercase();
    let name = lower.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    if matches!(name, ".spotlight-v100" | ".fseventsd" | ".documentrevisions-v100" | "system volume information") {
        return Some("operating-system index");
    }
    if !is_system_root {
        return None;
    }
    #[cfg(windows)]
    {
        let rest = lower.get(2..).unwrap_or("");
        for prefix in [
            "/windows",
            "/$winreagent",
            "/recovery",
            "/$sysreset",
            "/windows.old/windows",
            "/$windows.~bt",
            "/$windows.~ws",
        ] {
            if rest == prefix || rest.starts_with(&format!("{prefix}/")) {
                return Some("Windows system files");
            }
        }
        None
    }
    #[cfg(not(windows))]
    {
        const SYSTEM: &[&str] = &[
            "/system",
            "/dev",
            "/volumes",
            "/home",
            "/net",
            "/network",
            "/cores",
            "/usr",
            "/bin",
            "/sbin",
            "/proc",
            "/sys",
            "/library/apple",
            "/private/var/vm",
            "/private/var/db",
            "/private/var/protected",
        ];
        for prefix in SYSTEM {
            if lower == *prefix || lower.starts_with(&format!("{prefix}/")) {
                return Some(if *prefix == "/volumes" {
                    "searched as its own drive"
                } else {
                    "operating-system files"
                });
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_grasshopper_and_friends() {
        assert_eq!(classify("Palm_v2.GH").unwrap().kind, Kind::Definition);
        assert_eq!(classify("palm.ghx").unwrap().ext, "ghx");
        let b = classify("palm final.gh.bak").unwrap();
        assert!(b.backup && b.kind == Kind::Definition && b.ext == "gh");
        assert!(classify("palm.ghx~").unwrap().backup);
        assert_eq!(classify("frond.ghuser").unwrap().kind, Kind::UserObject);
        assert_eq!(classify("site.3dmbak").unwrap().kind, Kind::RhinoBackup);
        assert!(classify("old.rar").unwrap().name_only);
        let stub = classify(".phyllotaxis.gh.icloud").unwrap();
        assert!(stub.icloud_stub && stub.name == "phyllotaxis.gh");
        assert!(classify("notes.ghost").is_none());
        assert!(classify("Makefile").is_none());
        assert!(worth_sniffing("palm.tmp") && !worth_sniffing("palm.gh"));
    }

    #[test]
    fn tags_places() {
        let t = location_tags(
            "/Users/h/Library/Application Support/McNeel/Rhinoceros/8.0/Plug-ins/Grasshopper (b45a)/AutoSave/x.gh",
            Kind::Definition,
        );
        assert!(t.contains(&"autosave".to_string()) && !t.contains(&"installed".to_string()));
        assert!(location_tags("C:\\Users\\h\\OneDrive\\Thesis\\a.gh", Kind::Definition)
            .contains(&"cloud_folder".to_string()));
        assert!(location_tags("C:\\$Recycle.Bin\\S-1-5\\$RAB12.gh", Kind::Definition).contains(&"trash".to_string()));
        assert!(location_tags("/Applications/Rhino 8.app/Contents/x.gh", Kind::Definition)
            .contains(&"installed".to_string()));
    }

    #[cfg(not(windows))]
    #[test]
    fn skips_system_folders_only_at_the_system_root() {
        assert!(skip_reason("/System", true).is_some());
        assert!(skip_reason("/System/Volumes/Data", true).is_some());
        assert!(skip_reason("/Users/h/System", true).is_none());
        assert!(skip_reason("/Volumes/Backup/System", false).is_none());
        assert!(skip_reason("/Volumes/Backup/.Spotlight-V100", false).is_some());
        assert!(skip_reason("/private/var/folders/xy/T", true).is_none());
    }
}
