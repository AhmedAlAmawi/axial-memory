//! Her choices, this laptop's identity, and the AI key, kept in the app's own folders.

use axial_core::model::Modules;
use axial_core::terms::{default_terms, Term};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    pub machine_label: Option<String>,
    pub terms: Vec<Term>,
    pub modules: Modules,
    pub read_cloud_files: bool,
    /// Drives she turned off that are normally searched.
    pub roots_off: Vec<String>,
    /// Drives she turned on that normally aren't (network, backup).
    pub roots_on: Vec<String>,
    /// Folders she added by hand.
    pub folders: Vec<String>,
    /// The model picked when the AI key was added.
    pub ai_model: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            machine_label: None,
            terms: default_terms(),
            modules: Modules::default(),
            read_cloud_files: false,
            roots_off: Vec::new(),
            roots_on: Vec::new(),
            folders: Vec::new(),
            ai_model: None,
        }
    }
}

pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
}

impl Paths {
    pub fn scans(&self) -> PathBuf {
        self.data.join("scans")
    }

    pub fn explanations(&self) -> PathBuf {
        self.data.join("explanations")
    }

    fn settings(&self) -> PathBuf {
        self.config.join("settings.json")
    }

    fn machine_id_file(&self) -> PathBuf {
        self.config.join("machine-id")
    }

    fn ai_key(&self) -> PathBuf {
        self.config.join("ai-key")
    }

    pub fn ensure(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.config)?;
        std::fs::create_dir_all(self.scans())?;
        std::fs::create_dir_all(self.explanations())
    }

    pub fn load_settings(&self) -> Settings {
        std::fs::read_to_string(self.settings()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }

    pub fn save_settings(&self, s: &Settings) -> Result<(), String> {
        let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
        std::fs::write(self.settings(), json).map_err(|e| e.to_string())
    }

    /// A random id made on first run, so the two laptops never get mixed up.
    pub fn machine_id(&self) -> String {
        if let Ok(id) = std::fs::read_to_string(self.machine_id_file()) {
            if !id.trim().is_empty() {
                return id.trim().to_string();
            }
        }
        let id = uuid::Uuid::new_v4().to_string();
        let _ = std::fs::write(self.machine_id_file(), &id);
        id
    }

    pub fn read_key(&self) -> Option<String> {
        std::fs::read_to_string(self.ai_key()).ok().map(|k| k.trim().to_string()).filter(|k| !k.is_empty())
    }

    /// Stored in a file only her account can read, not the keychain: an app
    /// without a paid Apple signature would trigger a keychain prompt after every update.
    pub fn write_key(&self, key: &str) -> Result<(), String> {
        let path = self.ai_key();
        std::fs::write(&path, key.trim()).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn delete_key(&self) {
        let _ = std::fs::remove_file(self.ai_key());
    }
}
