#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai;
mod settings;

use axial_core::model::{Machine, ScanData, ScanMeta, ScanOptions, ScanStatus};
use axial_core::results::Results;
use axial_core::roots::{self, Root};
use axial_core::scan::{self, Phase, Progress, ScanRequest};
use axial_core::terms::default_terms;
use axial_core::{explain, export, results, store};
use serde::{Deserialize, Serialize};
use settings::{Paths, Settings};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

/// Older scans of this laptop kept besides the newest.
const KEEP_OLD_SCANS: usize = 2;

struct Running {
    cancel: Arc<AtomicBool>,
    progress: Arc<Mutex<Progress>>,
}

struct Loaded {
    scans: Vec<ScanData>,
    results: Results,
}

struct AppState {
    paths: Paths,
    machine_id: String,
    settings: Mutex<Settings>,
    running: Mutex<Option<Running>>,
    loaded: Mutex<Option<Arc<Loaded>>>,
    http: reqwest::Client,
}

type Res<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

impl AppState {
    fn settings(&self) -> Settings {
        self.settings.lock().unwrap().clone()
    }

    fn forget_results(&self) {
        *self.loaded.lock().unwrap() = None;
    }

    /// Every scan file kept, newest first.
    fn scan_files(&self) -> Vec<(PathBuf, ScanMeta)> {
        let mut out: Vec<(PathBuf, ScanMeta)> = std::fs::read_dir(self.paths.scans())
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().is_some_and(|x| x == store::EXTENSION))
                    .filter_map(|p| store::read_meta(&p).ok().map(|m| (p, m)))
                    .collect()
            })
            .unwrap_or_default();
        out.sort_by_key(|(_, meta)| std::cmp::Reverse(meta.finished_at));
        out
    }

    /// The newest scan of each laptop, this one first.
    fn load(&self) -> Res<Option<Arc<Loaded>>> {
        if let Some(l) = self.loaded.lock().unwrap().clone() {
            return Ok(Some(l));
        }
        let mut seen = std::collections::HashSet::new();
        let mut latest: Vec<PathBuf> = Vec::new();
        for (path, meta) in self.scan_files() {
            if seen.insert(meta.machine.id.clone()) {
                latest.push(path);
            }
        }
        if latest.is_empty() {
            return Ok(None);
        }
        let mut scans: Vec<ScanData> = latest.iter().filter_map(|p| store::read(p).ok()).collect();
        scans.sort_by_key(|s| (s.meta.machine.id != self.machine_id, s.meta.machine.label.clone()));
        let terms = self.settings().terms;
        let results = results::build(&scans, &terms, &self.machine_id);
        let loaded = Arc::new(Loaded { scans, results });
        *self.loaded.lock().unwrap() = Some(loaded.clone());
        Ok(Some(loaded))
    }

    fn roots(&self) -> Vec<Root> {
        let s = self.settings();
        let mut roots = roots::discover();
        for r in roots.iter_mut() {
            if s.roots_off.contains(&r.path) {
                r.selected = false;
            }
            if s.roots_on.contains(&r.path) && r.available {
                r.selected = true;
            }
        }
        for folder in &s.folders {
            let mut r = Root::folder(folder);
            r.available = Path::new(folder).exists();
            r.selected = r.available && !s.roots_off.contains(folder);
            r.note = Some("Folder you added".into());
            roots.push(r);
        }
        roots
    }
}

#[derive(Serialize)]
struct AppInfo {
    version: String,
    os: String,
    machine_id: String,
    machine_label: String,
    full_disk_access: Option<bool>,
    scanning: bool,
    /// Her home folder, so paths can be shown as `~/Documents/…`.
    home: Option<String>,
}

/// `None` off macOS. On a Mac, Safari's and Mail's folders open only with Full Disk Access.
fn full_disk_access() -> Option<bool> {
    #[cfg(target_os = "macos")]
    {
        let home = std::env::var("HOME").ok()?;
        for probe in [format!("{home}/Library/Safari"), format!("{home}/Library/Mail")] {
            match std::fs::read_dir(&probe) {
                Ok(_) => return Some(true),
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => return Some(false),
                Err(_) => continue,
            }
        }
        match std::fs::File::open("/Library/Application Support/com.apple.TCC/TCC.db") {
            Ok(_) => Some(true),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => Some(false),
            Err(_) => None,
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        None
    }
}

#[tauri::command]
fn app_info(app: AppHandle, state: State<AppState>) -> AppInfo {
    let machine = Machine::current(state.machine_id.clone(), state.settings().machine_label);
    AppInfo {
        version: app.package_info().version.to_string(),
        os: std::env::consts::OS.into(),
        machine_id: state.machine_id.clone(),
        machine_label: machine.label,
        full_disk_access: full_disk_access(),
        scanning: state.running.lock().unwrap().is_some(),
        home: std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok(),
    }
}

#[tauri::command]
fn list_roots(state: State<AppState>) -> Vec<Root> {
    state.roots()
}

#[tauri::command]
fn add_folder(path: String, state: State<AppState>) -> Res<Vec<Root>> {
    {
        let mut s = state.settings.lock().unwrap();
        if !s.folders.contains(&path) {
            s.folders.push(path.clone());
        }
        s.roots_off.retain(|p| p != &path);
        state.paths.save_settings(&s)?;
    }
    Ok(state.roots())
}

#[tauri::command]
fn remove_folder(path: String, state: State<AppState>) -> Res<Vec<Root>> {
    {
        let mut s = state.settings.lock().unwrap();
        s.folders.retain(|p| p != &path);
        state.paths.save_settings(&s)?;
    }
    Ok(state.roots())
}

#[derive(Serialize, Clone)]
struct ScanFinished {
    status: &'static str,
    error: Option<String>,
}

#[tauri::command]
fn start_scan(selected: Vec<String>, app: AppHandle, state: State<AppState>) -> Res<()> {
    let mut running = state.running.lock().unwrap();
    if running.is_some() {
        return Err("A scan is already running.".into());
    }
    let mut roots = state.roots();
    {
        let mut s = state.settings.lock().unwrap();
        let defaults = roots::discover();
        let folders_off: Vec<String> = s.folders.iter().filter(|f| !selected.contains(f)).cloned().collect();
        s.roots_off =
            defaults.iter().filter(|r| r.selected && !selected.contains(&r.path)).map(|r| r.path.clone()).collect();
        s.roots_off.extend(folders_off);
        s.roots_on =
            defaults.iter().filter(|r| !r.selected && selected.contains(&r.path)).map(|r| r.path.clone()).collect();
        state.paths.save_settings(&s)?;
    }
    for r in roots.iter_mut() {
        r.selected = r.available && selected.contains(&r.path);
    }
    if !roots.iter().any(|r| r.selected) {
        return Err("Choose at least one place to search.".into());
    }
    let s = state.settings();
    let req = ScanRequest {
        roots,
        options: ScanOptions {
            read_cloud_files: s.read_cloud_files,
            modules: s.modules.clone(),
            ..ScanOptions::default()
        },
        terms: s.terms.clone(),
        machine: Machine::current(state.machine_id.clone(), s.machine_label.clone()),
        app_version: app.package_info().version.to_string(),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let progress = Arc::new(Mutex::new(Progress::default()));
    *running = Some(Running { cancel: cancel.clone(), progress: progress.clone() });
    drop(running);

    let scans_dir = state.paths.scans();
    std::thread::spawn(move || {
        let mut last = Instant::now() - Duration::from_secs(1);
        let data = scan::run(&req, &cancel, &mut |p| {
            *progress.lock().unwrap() = p.clone();
            if last.elapsed() >= Duration::from_millis(200) || p.phase == Phase::Done {
                last = Instant::now();
                let _ = app.emit("scan-progress", p);
            }
        });
        let path = scans_dir.join(format!("{}.{}", data.meta.scan_id, store::EXTENSION));
        let saved = store::write(&path, &data);
        let state = app.state::<AppState>();
        if saved.is_ok() {
            let mine: Vec<PathBuf> = state
                .scan_files()
                .into_iter()
                .filter(|(_, m)| m.machine.id == state.machine_id)
                .map(|(p, _)| p)
                .collect();
            for old in mine.into_iter().skip(1 + KEEP_OLD_SCANS) {
                let _ = std::fs::remove_file(old);
            }
        }
        *state.running.lock().unwrap() = None;
        state.forget_results();
        let finished = match saved {
            Ok(()) if data.meta.status == ScanStatus::Stopped => ScanFinished { status: "stopped", error: None },
            Ok(()) => ScanFinished { status: "complete", error: None },
            Err(e) => {
                ScanFinished { status: "failed", error: Some(format!("The scan finished but couldn't be saved: {e}")) }
            }
        };
        let _ = app.emit("scan-finished", finished);
    });
    Ok(())
}

#[tauri::command]
fn cancel_scan(state: State<AppState>) {
    if let Some(r) = state.running.lock().unwrap().as_ref() {
        r.cancel.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
fn scan_progress(state: State<AppState>) -> Option<Progress> {
    state.running.lock().unwrap().as_ref().map(|r| r.progress.lock().unwrap().clone())
}

#[tauri::command]
async fn load_results(app: AppHandle) -> Res<Option<Results>> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        state.load().map(|l| l.map(|l| l.results.clone()))
    })
    .await
    .map_err(err)?
}

#[derive(Serialize)]
struct Imported {
    label: String,
    os: String,
    scanned_at: i64,
    this_computer: bool,
}

#[tauri::command]
fn import_scan(path: String, state: State<AppState>) -> Res<Imported> {
    let meta = store::read_meta(Path::new(&path)).map_err(|e| format!("That file can't be added: {e}"))?;
    let dest = state.paths.scans().join(format!("{}.{}", meta.scan_id, store::EXTENSION));
    std::fs::copy(&path, &dest).map_err(err)?;
    state.forget_results();
    Ok(Imported {
        label: meta.machine.label,
        os: meta.machine.os,
        scanned_at: meta.finished_at,
        this_computer: meta.machine.id == state.machine_id,
    })
}

#[tauri::command]
fn remove_machine(machine_id: String, state: State<AppState>) -> Res<()> {
    if machine_id == state.machine_id {
        return Err("This laptop's own scans stay; scan again to replace them.".into());
    }
    for (path, meta) in state.scan_files() {
        if meta.machine.id == machine_id {
            std::fs::remove_file(path).map_err(err)?;
        }
    }
    state.forget_results();
    Ok(())
}

#[tauri::command]
fn default_scan_file_name(state: State<AppState>) -> String {
    let label = Machine::current(state.machine_id.clone(), state.settings().machine_label).label;
    format!("{label} scan {}.{}", chrono::Local::now().format("%Y-%m-%d"), store::EXTENSION)
}

#[tauri::command]
fn save_scan_file(dest: String, state: State<AppState>) -> Res<()> {
    let newest = state
        .scan_files()
        .into_iter()
        .find(|(_, m)| m.machine.id == state.machine_id)
        .ok_or("Scan this laptop first.")?;
    std::fs::copy(newest.0, dest).map_err(err)?;
    Ok(())
}

#[derive(Serialize, Deserialize, Clone)]
struct Explanation {
    sha256: String,
    file_name: String,
    markdown: String,
    model: String,
    provider: String,
    created_at: i64,
}

fn read_explanations(paths: &Paths) -> Vec<Explanation> {
    std::fs::read_dir(paths.explanations())
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .filter_map(|e| std::fs::read_to_string(e.path()).ok())
                .filter_map(|s| serde_json::from_str(&s).ok())
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command]
async fn export_report(parent: String, app: AppHandle) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        let loaded = state.load()?.ok_or("Scan this laptop first.")?;
        let explanations: Vec<export::Explanation> = read_explanations(&state.paths)
            .into_iter()
            .filter(|e| loaded.results.files.iter().any(|f| f.sha256.as_deref() == Some(e.sha256.as_str())))
            .map(|e| export::Explanation { sha256: e.sha256, file_name: e.file_name, markdown: e.markdown })
            .collect();
        let dir = export::write_report(&loaded.results, Path::new(&parent), &explanations).map_err(err)?;
        Ok(dir.to_string_lossy().into_owned())
    })
    .await
    .map_err(err)?
}

#[tauri::command]
fn reveal(path: String, app: AppHandle) -> Res<()> {
    let target = path.split("!/").next().unwrap_or(&path).to_string();
    if !Path::new(&target).exists() {
        return Err("That file isn't on this computer. It may be on the other laptop, or it was moved.".into());
    }
    app.opener().reveal_item_in_dir(target).map_err(err)
}

#[tauri::command]
fn open_privacy_settings(app: AppHandle) -> Res<()> {
    app.opener()
        .open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles", None::<&str>)
        .map_err(err)
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
fn save_settings(settings: Settings, state: State<AppState>) -> Res<Settings> {
    let mut s = state.settings.lock().unwrap();
    let ai_model = s.ai_model.clone();
    *s = Settings { ai_model, ..settings };
    state.paths.save_settings(&s)?;
    drop(s);
    state.forget_results();
    Ok(state.settings())
}

#[tauri::command]
fn reset_terms(state: State<AppState>) -> Res<Settings> {
    let mut s = state.settings.lock().unwrap();
    s.terms = default_terms();
    state.paths.save_settings(&s)?;
    drop(s);
    state.forget_results();
    Ok(state.settings())
}

#[derive(Serialize)]
struct AiStatus {
    configured: bool,
    provider: Option<ai::Provider>,
    provider_label: Option<&'static str>,
    model: Option<String>,
    key_hint: Option<String>,
}

fn ai_status_of(state: &AppState) -> AiStatus {
    let key = state.paths.read_key();
    let provider = key.as_deref().and_then(ai::Provider::detect);
    AiStatus {
        configured: key.is_some() && provider.is_some(),
        provider,
        provider_label: provider.map(ai::Provider::label),
        model: state.settings().ai_model,
        key_hint: key.map(|k| format!("{}…{}", &k[..k.len().min(7)], &k[k.len().saturating_sub(4)..])),
    }
}

#[tauri::command]
fn ai_status(state: State<AppState>) -> AiStatus {
    ai_status_of(&state)
}

#[tauri::command]
async fn set_ai_key(key: String, app: AppHandle) -> Res<AiStatus> {
    let key = key.trim().to_string();
    let provider =
        ai::Provider::detect(&key).ok_or("That doesn't look like a Claude key (sk-ant-…) or a ChatGPT key (sk-…).")?;
    let http = app.state::<AppState>().http.clone();
    let model = ai::choose_model(&http, provider, &key).await?;
    let state = app.state::<AppState>();
    state.paths.write_key(&key)?;
    {
        let mut s = state.settings.lock().unwrap();
        s.ai_model = Some(model);
        state.paths.save_settings(&s)?;
    }
    Ok(ai_status_of(&state))
}

#[tauri::command]
fn clear_ai_key(state: State<AppState>) -> Res<AiStatus> {
    state.paths.delete_key();
    {
        let mut s = state.settings.lock().unwrap();
        s.ai_model = None;
        state.paths.save_settings(&s)?;
    }
    Ok(ai_status_of(&state))
}

/// The exact text an explanation would send, for her to see first.
fn extract_for(state: &AppState, file_id: u32) -> Res<(String, String, String)> {
    let loaded = state.load()?.ok_or("Scan this laptop first.")?;
    let f = loaded.results.files.get(file_id as usize).ok_or("That file isn't in the results any more.")?;
    let sha = f.sha256.clone().ok_or("Nothing inside this file could be read, so there's nothing to explain.")?;
    let digest = loaded
        .scans
        .iter()
        .flat_map(|s| s.contents.iter())
        .find(|c| c.sha256 == sha)
        .and_then(|c| c.digest.clone())
        .filter(|d| !d.objects.is_empty())
        .ok_or("Nothing inside this file could be read, so there's nothing to explain.")?;
    let evidence = f.candidate.map(|c| loaded.results.candidates[c as usize].evidence.clone()).unwrap_or_default();
    Ok((sha, f.name.clone(), explain::extract(&digest, &f.name, &evidence)))
}

#[tauri::command]
async fn ai_preview(file_id: u32, app: AppHandle) -> Res<String> {
    tauri::async_runtime::spawn_blocking(move || {
        extract_for(&app.state::<AppState>(), file_id).map(|(_, _, text)| text)
    })
    .await
    .map_err(err)?
}

#[tauri::command]
fn get_explanation(sha256: String, state: State<AppState>) -> Option<Explanation> {
    let path = state.paths.explanations().join(format!("{sha256}.json"));
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok())
}

#[tauri::command]
async fn explain_file(file_id: u32, app: AppHandle) -> Res<Explanation> {
    let handle = app.clone();
    let (sha, name, extract) =
        tauri::async_runtime::spawn_blocking(move || extract_for(&handle.state::<AppState>(), file_id))
            .await
            .map_err(err)??;
    let state = app.state::<AppState>();
    let key = state.paths.read_key().ok_or("Add an AI key in Settings first.")?;
    let provider = ai::Provider::detect(&key).ok_or("The saved AI key isn't recognised; add it again in Settings.")?;
    let model = match state.settings().ai_model {
        Some(m) => m,
        None => ai::choose_model(&state.http, provider, &key).await?,
    };
    let markdown = ai::complete(&state.http, provider, &key, &model, explain::SYSTEM_PROMPT, &extract).await?;
    let e = Explanation {
        sha256: sha.clone(),
        file_name: name,
        markdown,
        model,
        provider: provider.label().into(),
        created_at: axial_core::model::now_unix(),
    };
    let json = serde_json::to_string_pretty(&e).map_err(err)?;
    std::fs::write(state.paths.explanations().join(format!("{sha}.json")), json).map_err(err)?;
    Ok(e)
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let paths = Paths { config: app.path().app_config_dir()?, data: app.path().app_data_dir()? };
            paths.ensure()?;
            let state = AppState {
                machine_id: paths.machine_id(),
                settings: Mutex::new(paths.load_settings()),
                paths,
                running: Mutex::new(None),
                loaded: Mutex::new(None),
                http: reqwest::Client::builder().timeout(Duration::from_secs(180)).build()?,
            };
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_roots,
            add_folder,
            remove_folder,
            start_scan,
            cancel_scan,
            scan_progress,
            load_results,
            import_scan,
            remove_machine,
            default_scan_file_name,
            save_scan_file,
            export_report,
            reveal,
            open_privacy_settings,
            get_settings,
            save_settings,
            reset_terms,
            ai_status,
            set_ai_key,
            clear_ai_key,
            ai_preview,
            get_explanation,
            explain_file,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Axial Memory");
}
