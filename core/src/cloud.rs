//! Cloud placeholders. Opening one makes iCloud/OneDrive/Dropbox/Google Drive
//! download it, so they are listed by name and never read unless the user opts in.

use std::fs::Metadata;

/// macOS `SF_DATALESS`: the file's contents live only with its File Provider.
#[cfg(target_os = "macos")]
const SF_DATALESS: u32 = 0x4000_0000;

/// Windows Cloud Files attributes: `RECALL_ON_DATA_ACCESS`, `RECALL_ON_OPEN`, `OFFLINE`.
#[cfg(windows)]
const CLOUD_ATTRIBUTES: u32 = 0x0040_0000 | 0x0004_0000 | 0x0000_1000;

pub fn is_cloud_only(meta: &Metadata) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        meta.st_flags() & SF_DATALESS != 0
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & CLOUD_ATTRIBUTES != 0
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        let _ = meta;
        false
    }
}
