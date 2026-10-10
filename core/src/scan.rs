//! Walks the chosen drives read-only and records every Grasshopper file, the
//! files around them, and every place it couldn't look.
//!
//! One thread walks; a few workers open, fingerprint and read files; one
//! collects. Nothing is ever written, moved, renamed or executed, and links
//! are never followed, so a loop of links can't trap the scan.

use crate::classify::{self, Classified};
use crate::cloud;
use crate::gh;
use crate::model::*;
use crate::roots::{Root, RootKind};
use crate::terms::{Matcher, Strength, Term};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};
use walkdir::WalkDir;

/// Grasshopper files are read whole up to this size.
const MAX_GH_BYTES: u64 = 512 * 1024 * 1024;
const MAX_SCRIPT_TEXT: u64 = 1024 * 1024;
const MAX_SNIFF_BYTES: u64 = 256 * 1024 * 1024;
const MAX_MEMBER_BYTES: u64 = 256 * 1024 * 1024;
/// Total decompressed bytes read out of one archive, nested ones included.
const MAX_ARCHIVE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_ARCHIVE_MEMBERS: usize = 200_000;
const MAX_NESTED_ZIP_BYTES: u64 = 128 * 1024 * 1024;
const MAX_ZIP_DEPTH: usize = 2;

pub struct ScanRequest {
    /// Every root on the computer; unselected ones are recorded as not searched.
    pub roots: Vec<Root>,
    pub options: ScanOptions,
    pub terms: Vec<Term>,
    pub machine: Machine,
    pub app_version: String,
}

#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    #[default]
    Walking,
    Reading,
    Done,
}

#[derive(Serialize, Clone, Debug, Default)]
pub struct Progress {
    pub phase: Phase,
    pub root: String,
    pub current: String,
    pub dirs: u64,
    pub files_seen: u64,
    pub definitions: u64,
    pub other_found: u64,
    pub queued: u64,
    pub read: u64,
    pub issues: u64,
    pub elapsed_ms: u64,
}

#[derive(Default)]
struct Counters {
    dirs: AtomicU64,
    files_seen: AtomicU64,
    definitions: AtomicU64,
    other_found: AtomicU64,
    queued: AtomicU64,
    read: AtomicU64,
    issues: AtomicU64,
    links: AtomicU64,
    archives_opened: AtomicU64,
    bytes_read: AtomicU64,
}

struct Job {
    path: PathBuf,
    display: String,
    class: Classified,
    size: u64,
    times: Times,
    tags: Vec<String>,
    root: String,
    /// Peek first; only keep it if it turns out to be a Grasshopper archive.
    sniff: bool,
}

#[derive(Clone, Copy, Default)]
struct Times {
    created: Option<i64>,
    modified: Option<i64>,
    accessed: Option<i64>,
}

enum Out {
    File(Box<(FileRecord, Option<Content>)>),
    Issue(Issue),
}

fn file_out(record: FileRecord, content: Option<Content>) -> Out {
    Out::File(Box::new((record, content)))
}

struct Shared<'a> {
    options: &'a ScanOptions,
    matcher: &'a Matcher,
    counters: &'a Counters,
    cancel: &'a AtomicBool,
    /// Contents already read, so identical copies are only parsed once.
    parsed: Mutex<HashSet<String>>,
}

pub fn run(req: &ScanRequest, cancel: &AtomicBool, on_progress: &mut dyn FnMut(&Progress)) -> ScanData {
    let started_at = now_unix();
    let clock = Instant::now();
    let counters = Counters::default();
    let matcher = Matcher::new(&req.terms);
    let shared = Shared {
        options: &req.options,
        matcher: &matcher,
        counters: &counters,
        cancel,
        parsed: Mutex::new(HashSet::new()),
    };
    let workers = std::thread::available_parallelism().map(|n| n.get() / 2).unwrap_or(2).clamp(2, 6);

    let (mut files, contents, mut issues, coverage) = std::thread::scope(|s| {
        let (job_tx, job_rx) = bounded::<Job>(4096);
        let (out_tx, out_rx) = unbounded::<Out>();
        for _ in 0..workers {
            let rx = job_rx.clone();
            let tx = out_tx.clone();
            let shared = &shared;
            s.spawn(move || {
                for job in rx {
                    if !shared.cancel.load(Relaxed) {
                        work(job, shared, &tx);
                    }
                    shared.counters.read.fetch_add(1, Relaxed);
                }
            });
        }
        drop(job_rx);
        let collector = s.spawn(|| collect(out_rx, &counters));

        let mut walker = Walker {
            req,
            shared: &shared,
            jobs: &job_tx,
            out: &out_tx,
            nearby_scripts: HashMap::new(),
            definition_dirs: HashSet::new(),
            clock,
            last_emit: Instant::now(),
            on_progress: &mut *on_progress,
            root_label: String::new(),
            current: String::new(),
        };
        let mut coverage = Vec::new();
        for root in &req.roots {
            coverage.push(walker.walk_root(root));
        }
        walker.queue_nearby_scripts();
        drop(walker);
        drop(job_tx);
        drop(out_tx);

        while !collector.is_finished() {
            on_progress(&snapshot(&counters, Phase::Reading, "", "", clock));
            std::thread::sleep(Duration::from_millis(150));
        }
        let (files, contents, issues) = collector.join().expect("collector thread");
        (files, contents, issues, coverage)
    });

    files.sort_by(|a, b| a.path.cmp(&b.path));
    for (i, f) in files.iter_mut().enumerate() {
        f.id = i as u32 + 1;
    }
    let mut coverage = coverage;
    for c in coverage.iter_mut() {
        c.found = files.iter().filter(|f| f.root == c.label).count() as u64;
    }
    issues.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.path.cmp(&b.path)));
    let stopped = cancel.load(Relaxed);
    on_progress(&snapshot(&counters, Phase::Done, "", "", clock));
    ScanData {
        meta: ScanMeta {
            scan_id: uuid::Uuid::new_v4().to_string(),
            machine: req.machine.clone(),
            app_version: req.app_version.clone(),
            started_at,
            finished_at: now_unix(),
            status: if stopped { ScanStatus::Stopped } else { ScanStatus::Complete },
            options: req.options.clone(),
            dirs: counters.dirs.load(Relaxed),
            files_seen: counters.files_seen.load(Relaxed),
            links_not_followed: counters.links.load(Relaxed),
            archives_opened: counters.archives_opened.load(Relaxed),
            bytes_read: counters.bytes_read.load(Relaxed),
        },
        files,
        contents,
        issues,
        coverage,
    }
}

fn snapshot(c: &Counters, phase: Phase, root: &str, current: &str, clock: Instant) -> Progress {
    Progress {
        phase,
        root: root.to_string(),
        current: current.to_string(),
        dirs: c.dirs.load(Relaxed),
        files_seen: c.files_seen.load(Relaxed),
        definitions: c.definitions.load(Relaxed),
        other_found: c.other_found.load(Relaxed),
        queued: c.queued.load(Relaxed),
        read: c.read.load(Relaxed),
        issues: c.issues.load(Relaxed),
        elapsed_ms: clock.elapsed().as_millis() as u64,
    }
}

fn collect(rx: Receiver<Out>, counters: &Counters) -> (Vec<FileRecord>, Vec<Content>, Vec<Issue>) {
    let mut files = Vec::new();
    let mut contents: HashMap<String, Content> = HashMap::new();
    let mut issues = Vec::new();
    for msg in rx {
        match msg {
            Out::File(boxed) => {
                let (f, content) = *boxed;
                if f.kind == Kind::Definition {
                    counters.definitions.fetch_add(1, Relaxed);
                } else {
                    counters.other_found.fetch_add(1, Relaxed);
                }
                if let Some(c) = content {
                    contents.entry(c.sha256.clone()).or_insert(c);
                }
                files.push(f);
            }
            Out::Issue(i) => {
                if i.kind != IssueKind::Skipped {
                    counters.issues.fetch_add(1, Relaxed);
                }
                issues.push(i);
            }
        }
    }
    let mut contents: Vec<Content> = contents.into_values().collect();
    contents.sort_by(|a, b| a.sha256.cmp(&b.sha256));
    (files, contents, issues)
}

struct Walker<'a, 'b> {
    req: &'a ScanRequest,
    shared: &'a Shared<'a>,
    jobs: &'a Sender<Job>,
    out: &'a Sender<Out>,
    /// Scripts are only kept when they sit next to a definition (or match a search word).
    nearby_scripts: HashMap<PathBuf, Vec<Job>>,
    definition_dirs: HashSet<PathBuf>,
    clock: Instant,
    last_emit: Instant,
    on_progress: &'b mut dyn FnMut(&Progress),
    root_label: String,
    current: String,
}

impl Walker<'_, '_> {
    fn emit(&mut self) {
        if self.last_emit.elapsed() >= Duration::from_millis(120) {
            self.last_emit = Instant::now();
            let p = snapshot(self.shared.counters, Phase::Walking, &self.root_label, &self.current, self.clock);
            (self.on_progress)(&p);
        }
    }

    fn issue(&self, path: String, kind: IssueKind, detail: impl Into<String>) {
        let _ = self.out.send(Out::Issue(Issue { path, kind, detail: detail.into() }));
    }

    fn queue(&self, job: Job) {
        self.shared.counters.queued.fetch_add(1, Relaxed);
        let _ = self.jobs.send(job);
    }

    fn name_matches(&self, text: &str, min: Strength) -> bool {
        self.shared.matcher.find(text).iter().any(|(t, _)| t.strength <= min)
    }

    fn walk_root(&mut self, root: &Root) -> Coverage {
        let mut cov = Coverage {
            root: root.path.clone(),
            label: root.label.clone(),
            kind: root.kind,
            status: CoverageStatus::Scanned,
            dirs: 0,
            files: 0,
            found: 0,
            issues: 0,
            note: root.note.clone(),
        };
        if !root.selected {
            cov.status = CoverageStatus::NotSelected;
            return cov;
        }
        if !root.available || !Path::new(&root.path).exists() {
            cov.status = CoverageStatus::Unavailable;
            return cov;
        }
        if self.shared.cancel.load(Relaxed) {
            cov.status = CoverageStatus::Stopped;
            return cov;
        }
        self.root_label = root.label.clone();
        let names_only = root.kind == RootKind::Cloud && !self.req.options.read_cloud_files;
        let mut it = WalkDir::new(&root.path).follow_links(false).into_iter();
        while let Some(next) = it.next() {
            if self.shared.cancel.load(Relaxed) {
                cov.status = CoverageStatus::Stopped;
                break;
            }
            let entry = match next {
                Ok(e) => e,
                Err(err) => {
                    let path =
                        err.path().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| root.path.clone());
                    let (kind, detail) = describe_io(err.io_error());
                    self.issue(path, kind, detail);
                    cov.issues += 1;
                    continue;
                }
            };
            let file_type = entry.file_type();
            if file_type.is_dir() {
                let path = entry.path().to_string_lossy().into_owned();
                if entry.depth() > 0 {
                    if let Some(reason) = classify::skip_reason(&path, root.is_system_root()) {
                        self.issue(path, IssueKind::Skipped, reason);
                        it.skip_current_dir();
                        continue;
                    }
                }
                cov.dirs += 1;
                self.shared.counters.dirs.fetch_add(1, Relaxed);
                self.current = path;
                self.emit();
                continue;
            }
            if file_type.is_symlink() {
                self.shared.counters.links.fetch_add(1, Relaxed);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }
            cov.files += 1;
            self.shared.counters.files_seen.fetch_add(1, Relaxed);
            let name = entry.file_name().to_string_lossy().into_owned();
            let (class, sniff) = match classify::classify(&name) {
                Some(c) => (c, false),
                None if classify::worth_sniffing(&name) => (
                    Classified {
                        kind: Kind::Definition,
                        ext: "gh".into(),
                        name: name.clone(),
                        backup: true,
                        icloud_stub: false,
                        name_only: false,
                    },
                    true,
                ),
                None => continue,
            };
            let display = entry.path().to_string_lossy().into_owned();
            if !sniff && !self.wanted(&class, &display) {
                continue;
            }
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(err) => {
                    let (kind, detail) = describe_io(err.io_error());
                    self.issue(display, kind, detail);
                    cov.issues += 1;
                    continue;
                }
            };
            let cloud_only = cloud::is_cloud_only(&meta) || class.icloud_stub || names_only;
            if sniff && (cloud_only || meta.len() < 64 || meta.len() > MAX_SNIFF_BYTES) {
                continue;
            }
            let mut tags = classify::location_tags(&display, class.kind);
            match root.kind {
                RootKind::External => tags.push("external_drive".into()),
                RootKind::Network => tags.push("network_drive".into()),
                RootKind::Cloud => tags.push("cloud_drive".into()),
                RootKind::Backup => tags.push("backup_drive".into()),
                RootKind::System | RootKind::Folder => {}
            }
            if class.backup {
                tags.push("backup_copy".into());
            }
            let job = Job {
                path: entry.path().to_path_buf(),
                display,
                size: if class.icloud_stub { 0 } else { meta.len() },
                times: times(&meta),
                class,
                tags,
                root: root.label.clone(),
                sniff,
            };
            if cloud_only && !self.req.options.read_cloud_files {
                self.list_cloud_only(job);
                continue;
            }
            if job.class.kind == Kind::Definition && !sniff {
                if let Some(dir) = job.path.parent() {
                    self.definition_dirs.insert(dir.to_path_buf());
                }
            }
            if job.class.kind == Kind::Script && !self.name_matches(&job.display, Strength::Related) {
                let installed = job.tags.iter().any(|t| t == "installed");
                if let (Some(dir), false) = (job.path.parent(), installed) {
                    self.nearby_scripts.entry(dir.to_path_buf()).or_default().push(job);
                }
                continue;
            }
            self.queue(job);
        }
        cov
    }

    /// Whether a file of this kind belongs in the scan at all.
    fn wanted(&self, class: &Classified, path: &str) -> bool {
        let m = &self.req.options.modules;
        match class.kind {
            Kind::Definition => true,
            Kind::UserObject => m.user_objects,
            Kind::RhinoModel | Kind::RhinoBackup => m.rhino_models,
            Kind::Script => m.scripts,
            Kind::Archive if class.name_only => m.archives && self.name_matches(path, Strength::Related),
            Kind::Archive => {
                m.archives && !classify::location_tags(path, Kind::Archive).iter().any(|t| t == "installed")
            }
            Kind::Document => {
                let images = ["jpg", "jpeg", "png", "tif", "tiff", "heic", "psd", "mp4", "mov"];
                let needed = if images.contains(&class.ext.as_str()) { Strength::Strong } else { Strength::Palm };
                m.documents && self.name_matches(&class.name, needed)
            }
        }
    }

    fn list_cloud_only(&self, job: Job) {
        let detail = "Only in the cloud: the name was checked, the file wasn't downloaded";
        self.issue(job.display.clone(), IssueKind::CloudOnly, detail);
        let record = record_for(&job, None, Status::CloudOnly, Some(detail.to_string()));
        let _ = self.out.send(file_out(record, None));
    }

    fn queue_nearby_scripts(&mut self) {
        let scripts = std::mem::take(&mut self.nearby_scripts);
        for (dir, jobs) in scripts {
            if self.definition_dirs.contains(&dir) {
                for job in jobs {
                    self.queue(job);
                }
            }
        }
    }
}

fn times(meta: &std::fs::Metadata) -> Times {
    let unix = |t: std::io::Result<SystemTime>| {
        t.ok().and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok()).map(|d| d.as_secs() as i64)
    };
    Times { created: unix(meta.created()), modified: unix(meta.modified()), accessed: unix(meta.accessed()) }
}

fn describe_io(err: Option<&std::io::Error>) -> (IssueKind, String) {
    match err {
        Some(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            (IssueKind::PermissionDenied, "The computer didn't allow reading this".to_string())
        }
        Some(e) if e.kind() == std::io::ErrorKind::NotFound => {
            (IssueKind::Vanished, "Gone before it could be read".to_string())
        }
        Some(e) => (IssueKind::Unreadable, e.to_string()),
        None => (IssueKind::Unreadable, "Couldn't be read".to_string()),
    }
}

fn record_for(job: &Job, sha256: Option<String>, status: Status, detail: Option<String>) -> FileRecord {
    FileRecord {
        id: 0,
        path: job.display.clone(),
        archive: None,
        name: job.class.name.clone(),
        ext: job.class.ext.clone(),
        kind: job.class.kind,
        size: (!job.class.icloud_stub).then_some(job.size),
        created: job.times.created,
        modified: job.times.modified,
        accessed: job.times.accessed,
        sha256,
        status,
        detail,
        tags: job.tags.clone(),
        root: job.root.clone(),
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn hash_reader(mut r: impl Read, cancel: &AtomicBool, counters: &Counters) -> std::io::Result<Option<String>> {
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        if cancel.load(Relaxed) {
            return Ok(None);
        }
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        counters.bytes_read.fetch_add(n as u64, Relaxed);
        h.update(&buf[..n]);
    }
    Ok(Some(hex(&h.finalize())))
}

fn work(mut job: Job, s: &Shared, out: &Sender<Out>) {
    if job.sniff {
        let mut head = vec![0u8; 4096];
        let Ok(mut f) = File::open(&job.path) else { return };
        let Ok(n) = f.read(&mut head) else { return };
        if !gh::sniff(&head[..n]) {
            return;
        }
        if gh::xml::sniff(&head[..n]) {
            job.class.ext = "ghx".into();
        }
        job.tags.push("renamed_grasshopper".into());
    }
    if let Some(note) = recycle_bin_origin(&job.path) {
        job.tags.push("deleted".into());
        job.class.name = format!("{} ({})", job.class.name, note.original_name);
        let detail = note.describe();
        return finish(job, s, out, Some(detail));
    }
    finish(job, s, out, None)
}

fn finish(job: Job, s: &Shared, out: &Sender<Out>, note: Option<String>) {
    let result = match job.class.kind {
        Kind::Definition | Kind::UserObject => read_grasshopper(&job, s),
        Kind::Archive if !job.class.name_only => return read_zip(&job, s, out),
        Kind::Script => read_script(&job, s),
        _ => hash_only(&job, s),
    };
    let (mut record, content, issue) = result;
    if let Some(n) = note {
        record.detail = Some(match record.detail {
            Some(d) => format!("{n}. {d}"),
            None => n,
        });
    }
    if let Some(i) = issue {
        let _ = out.send(Out::Issue(i));
    }
    let _ = out.send(file_out(record, content));
}

type Outcome = (FileRecord, Option<Content>, Option<Issue>);

fn unreadable(job: &Job, err: &std::io::Error) -> Outcome {
    let (kind, detail) = describe_io(Some(err));
    let record = record_for(job, None, Status::Unreadable, Some(detail.clone()));
    (record, None, Some(Issue { path: job.display.clone(), kind, detail }))
}

fn too_large(job: &Job) -> Outcome {
    let detail = format!("{} MB: listed, not fingerprinted", job.size / (1024 * 1024));
    let record = record_for(job, None, Status::TooLarge, Some(detail.clone()));
    (record, None, Some(Issue { path: job.display.clone(), kind: IssueKind::TooLarge, detail }))
}

fn hash_only(job: &Job, s: &Shared) -> Outcome {
    if job.size > s.options.max_hash_bytes {
        return too_large(job);
    }
    let file = match File::open(&job.path) {
        Ok(f) => f,
        Err(e) => return unreadable(job, &e),
    };
    match hash_reader(BufReader::new(file), s.cancel, s.counters) {
        Ok(sha) => {
            let status = if job.class.name_only { Status::NameOnly } else { Status::Read };
            let detail = job
                .class
                .name_only
                .then(|| "This kind of archive isn't opened yet; only its name was checked".to_string());
            (record_for(job, sha, status, detail), None, None)
        }
        Err(e) => unreadable(job, &e),
    }
}

/// Reads a definition whole, fingerprints it and reads its inside.
fn read_grasshopper(job: &Job, s: &Shared) -> Outcome {
    if job.size > MAX_GH_BYTES {
        let (record, _, issue) = hash_only(job, s);
        let content = record.sha256.clone().map(|sha| Content {
            sha256: sha,
            read: GhRead::Inconclusive,
            error: Some("too large to read inside".into()),
            summary: None,
            digest: None,
        });
        return (record, content, issue);
    }
    let bytes = match std::fs::read(&job.path) {
        Ok(b) => b,
        Err(e) => return unreadable(job, &e),
    };
    s.counters.bytes_read.fetch_add(bytes.len() as u64, Relaxed);
    let sha = sha256_hex(&bytes);
    let (content, issue) = grasshopper_content(&bytes, &job.class.ext, &sha, &job.display, s);
    (record_for(job, Some(sha), Status::Read, None), content, issue)
}

fn grasshopper_content(
    bytes: &[u8],
    ext: &str,
    sha: &str,
    display: &str,
    s: &Shared,
) -> (Option<Content>, Option<Issue>) {
    if !s.parsed.lock().unwrap().insert(sha.to_string()) {
        return (None, None);
    }
    match gh::read_digest(bytes, ext) {
        Ok(d) => (
            Some(Content {
                sha256: sha.into(),
                read: GhRead::Parsed,
                error: None,
                summary: Some(d.summary()),
                digest: Some(d),
            }),
            None,
        ),
        Err(e) => (
            Some(Content {
                sha256: sha.into(),
                read: GhRead::Inconclusive,
                error: Some(e.clone()),
                summary: None,
                digest: None,
            }),
            Some(Issue { path: display.into(), kind: IssueKind::Damaged, detail: e }),
        ),
    }
}

fn read_script(job: &Job, s: &Shared) -> Outcome {
    if job.size > MAX_SCRIPT_TEXT {
        return hash_only(job, s);
    }
    let bytes = match std::fs::read(&job.path) {
        Ok(b) => b,
        Err(e) => return unreadable(job, &e),
    };
    s.counters.bytes_read.fetch_add(bytes.len() as u64, Relaxed);
    let sha = sha256_hex(&bytes);
    let content = s.parsed.lock().unwrap().insert(sha.clone()).then(|| script_content(&bytes, &sha));
    (record_for(job, Some(sha), Status::Read, None), content, None)
}

fn script_content(bytes: &[u8], sha: &str) -> Content {
    let text: String = String::from_utf8_lossy(bytes).chars().take(40_000).collect();
    let digest = gh::Digest {
        format: "script".into(),
        texts: vec![gh::digest::Text { source: gh::TextSource::Script, text, obj: None, in_cluster: false }],
        ..gh::Digest::default()
    };
    Content { sha256: sha.into(), read: GhRead::NotApplicable, error: None, summary: None, digest: Some(digest) }
}

/// Lists and reads the Grasshopper files (and Rhino models) inside a zip,
/// without extracting anything to disk.
fn read_zip(job: &Job, s: &Shared, out: &Sender<Out>) {
    let opened = File::open(&job.path)
        .map_err(|e| e.to_string())
        .and_then(|f| zip::ZipArchive::new(BufReader::new(f)).map_err(|e| e.to_string()));
    let mut found = 0;
    let mut open_error = None;
    match opened {
        Ok(mut archive) => {
            s.counters.archives_opened.fetch_add(1, Relaxed);
            let mut budget = MAX_ARCHIVE_BYTES;
            found = read_zip_members(&mut archive, &job.display, job, 0, &mut budget, s, out);
        }
        Err(e) => open_error = Some(format!("Couldn't open the archive: {e}")),
    }
    // Most zips on a computer have nothing to do with Grasshopper; only keep the ones that do.
    if found == 0 && s.matcher.find(&job.display).is_empty() {
        return;
    }
    let (mut record, _, issue) = hash_only(job, s);
    if let Some(i) = issue {
        let _ = out.send(Out::Issue(i));
    }
    match open_error {
        Some(e) => {
            record.detail = Some(e.clone());
            let _ = out.send(Out::Issue(Issue { path: job.display.clone(), kind: IssueKind::Damaged, detail: e }));
        }
        None if found > 0 => record.detail = Some(format!("{found} Grasshopper or related file(s) inside")),
        None => {}
    }
    let _ = out.send(file_out(record, None));
}

fn read_zip_members<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    zip_display: &str,
    job: &Job,
    depth: usize,
    budget: &mut u64,
    s: &Shared,
    out: &Sender<Out>,
) -> usize {
    let total = archive.len();
    if total > MAX_ARCHIVE_MEMBERS {
        let _ = out.send(Out::Issue(Issue {
            path: zip_display.into(),
            kind: IssueKind::ArchiveLimit,
            detail: format!("{total} files inside; only the first {MAX_ARCHIVE_MEMBERS} were checked"),
        }));
    }
    let mut found = 0;
    for i in 0..total.min(MAX_ARCHIVE_MEMBERS) {
        if s.cancel.load(Relaxed) {
            break;
        }
        let (name, size, is_dir, encrypted, modified) = match archive.by_index_raw(i) {
            Ok(f) => (
                f.name().map(|n| n.into_owned()).unwrap_or_else(|_| format!("file {i}")),
                f.size(),
                f.is_dir(),
                f.encrypted(),
                f.last_modified().and_then(zip_time),
            ),
            Err(_) => continue,
        };
        if is_dir {
            continue;
        }
        let file_name = name.rsplit('/').next().unwrap_or(&name).to_string();
        let Some(class) = classify::classify(&file_name) else { continue };
        let member_display = format!("{zip_display}!/{name}");
        if class.kind == Kind::Archive && !class.name_only {
            if depth + 1 < MAX_ZIP_DEPTH && size <= MAX_NESTED_ZIP_BYTES && size <= *budget && !encrypted {
                let mut buf = Vec::with_capacity(size as usize);
                if let Ok(f) = archive.by_index(i) {
                    if f.take(MAX_NESTED_ZIP_BYTES + 1).read_to_end(&mut buf).is_ok() {
                        *budget = budget.saturating_sub(buf.len() as u64);
                        if let Ok(mut inner) = zip::ZipArchive::new(Cursor::new(buf)) {
                            found += read_zip_members(&mut inner, &member_display, job, depth + 1, budget, s, out);
                        }
                    }
                }
            }
            continue;
        }
        let keep = match class.kind {
            Kind::Definition | Kind::UserObject => true,
            Kind::RhinoModel | Kind::RhinoBackup => s.options.modules.rhino_models,
            Kind::Script | Kind::Document => {
                s.matcher.find(&file_name).iter().any(|(t, _)| t.strength <= Strength::Palm)
            }
            Kind::Archive => false,
        };
        if !keep {
            continue;
        }
        found += 1;
        let mut tags = job.tags.clone();
        tags.push("in_archive".into());
        let mut record = FileRecord {
            id: 0,
            path: member_display.clone(),
            archive: Some(zip_display.to_string()),
            name: file_name,
            ext: class.ext.clone(),
            kind: class.kind,
            size: Some(size),
            created: None,
            modified,
            accessed: None,
            sha256: None,
            status: Status::Read,
            detail: None,
            tags,
            root: job.root.clone(),
        };
        let fail = |record: &mut FileRecord, status: Status, kind: IssueKind, detail: String| {
            record.status = status;
            record.detail = Some(detail.clone());
            let _ = out.send(Out::Issue(Issue { path: member_display.clone(), kind, detail }));
        };
        if encrypted {
            fail(
                &mut record,
                Status::Unreadable,
                IssueKind::Unreadable,
                "Password-protected inside the archive".into(),
            );
        } else if size > MAX_MEMBER_BYTES || size > *budget {
            fail(&mut record, Status::TooLarge, IssueKind::ArchiveLimit, "Too large to read inside the archive".into());
        } else {
            let mut buf = Vec::with_capacity(size as usize);
            let read = archive
                .by_index(i)
                .map_err(|e| e.to_string())
                .and_then(|f| f.take(MAX_MEMBER_BYTES + 1).read_to_end(&mut buf).map_err(|e| e.to_string()));
            match read {
                Ok(_) => {
                    *budget = budget.saturating_sub(buf.len() as u64);
                    s.counters.bytes_read.fetch_add(buf.len() as u64, Relaxed);
                    let sha = sha256_hex(&buf);
                    let mut content = None;
                    if class.kind.is_grasshopper() {
                        let (c, issue) = grasshopper_content(&buf, &class.ext, &sha, &member_display, s);
                        content = c;
                        if let Some(i) = issue {
                            let _ = out.send(Out::Issue(i));
                        }
                    }
                    record.sha256 = Some(sha);
                    let _ = out.send(file_out(record, content));
                    continue;
                }
                Err(e) => fail(
                    &mut record,
                    Status::Unreadable,
                    IssueKind::Unreadable,
                    format!("Couldn't be read inside the archive: {e}"),
                ),
            }
        }
        let _ = out.send(file_out(record, None));
    }
    found
}

fn zip_time(t: zip::DateTime) -> Option<i64> {
    use chrono::TimeZone;
    let date = chrono::NaiveDate::from_ymd_opt(t.year() as i32, t.month() as u32, t.day() as u32)?;
    let dt = date.and_hms_opt(t.hour() as u32, t.minute() as u32, t.second() as u32)?;
    chrono::Local.from_local_datetime(&dt).earliest().map(|d| d.timestamp())
}

/// Where a file in the Windows Recycle Bin came from, read from its `$I` partner.
pub struct RecycleNote {
    pub original_path: String,
    pub original_name: String,
    pub deleted_at: Option<i64>,
}

impl RecycleNote {
    fn describe(&self) -> String {
        let when = self
            .deleted_at
            .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
            .map(|d| format!(" on {}", d.with_timezone(&chrono::Local).format("%Y-%m-%d")))
            .unwrap_or_default();
        format!("Deleted{when}; it was {}", self.original_path)
    }
}

pub fn recycle_bin_origin(path: &Path) -> Option<RecycleNote> {
    let name = path.file_name()?.to_string_lossy();
    let in_bin = path.to_string_lossy().to_lowercase().contains("$recycle.bin");
    let rest = name.strip_prefix("$R")?;
    if !in_bin {
        return None;
    }
    let info = std::fs::read(path.with_file_name(format!("$I{rest}"))).ok()?;
    parse_recycle_info(&info)
}

/// `$I` layout: version (8 bytes), size (8), deletion FILETIME (8), then the
/// original path as UTF-16: fixed 260 characters in version 1, length-prefixed in 2.
pub fn parse_recycle_info(info: &[u8]) -> Option<RecycleNote> {
    if info.len() < 24 {
        return None;
    }
    let version = i64::from_le_bytes(info[0..8].try_into().ok()?);
    let filetime = i64::from_le_bytes(info[16..24].try_into().ok()?);
    let utf16 = match version {
        1 => info.get(24..24 + 520)?,
        2 => {
            let chars = u32::from_le_bytes(info.get(24..28)?.try_into().ok()?) as usize;
            info.get(28..28 + chars * 2)?
        }
        _ => return None,
    };
    let units: Vec<u16> =
        utf16.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).take_while(|&u| u != 0).collect();
    let original_path = String::from_utf16_lossy(&units);
    let original_name = original_path.rsplit(['\\', '/']).next().unwrap_or(&original_path).to_string();
    // FILETIME counts 100 ns ticks since 1601.
    let deleted_at = (filetime > 0).then(|| filetime / 10_000_000 - 11_644_473_600);
    Some(RecycleNote { original_path, original_name, deleted_at })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_recycle_bin_info_v2() {
        let path = "C:\\Users\\h\\Desktop\\palm tree.gh";
        let units: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut info = Vec::new();
        info.extend_from_slice(&2i64.to_le_bytes());
        info.extend_from_slice(&1234i64.to_le_bytes());
        info.extend_from_slice(&133_500_000_000_000_000i64.to_le_bytes());
        info.extend_from_slice(&(units.len() as u32).to_le_bytes());
        for u in units {
            info.extend_from_slice(&u.to_le_bytes());
        }
        let note = parse_recycle_info(&info).unwrap();
        assert_eq!(note.original_path, path);
        assert_eq!(note.original_name, "palm tree.gh");
        assert!(note.deleted_at.unwrap() > 1_600_000_000);
    }
}
