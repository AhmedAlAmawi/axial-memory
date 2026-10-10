//! Puts one or more laptop scans together: every file, the palm candidates
//! with their evidence, exact duplicates, version families, and the gaps.

use crate::gh::digest::excerpt;
use crate::model::*;
use crate::terms::{judge, Confidence, Evidence, Matcher, Source, Term};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::OnceLock;

const MAX_EVIDENCE: usize = 40;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Totals {
    pub definitions: u32,
    pub unique_definitions: u32,
    pub user_objects: u32,
    pub rhino_models: u32,
    pub scripts: u32,
    pub archives: u32,
    pub documents: u32,
    pub strong: u32,
    pub possible: u32,
    pub weak: u32,
    pub inconclusive: u32,
    pub cloud_only: u32,
    pub unreadable: u32,
    pub damaged: u32,
    pub issues: u32,
    pub skipped: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MachineView {
    pub index: u32,
    pub id: String,
    pub label: String,
    pub os: String,
    pub os_version: String,
    pub device: String,
    pub scan_id: String,
    pub scanned_at: i64,
    pub status: ScanStatus,
    pub this_computer: bool,
    pub dirs: u64,
    pub files_seen: u64,
    pub totals: Totals,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FileView {
    pub id: u32,
    pub machine: u32,
    pub path: String,
    pub folder: String,
    pub name: String,
    pub ext: String,
    pub kind: Kind,
    pub size: Option<u64>,
    pub created: Option<i64>,
    pub modified: Option<i64>,
    pub sha256: Option<String>,
    pub status: Status,
    pub detail: Option<String>,
    pub tags: Vec<String>,
    pub read: GhRead,
    pub read_error: Option<String>,
    pub objects: Option<u32>,
    pub groups: Vec<String>,
    pub plugins: Vec<String>,
    pub gh_version: Option<String>,
    pub copies: u32,
    pub family: Option<String>,
    pub in_archive: Option<String>,
    pub candidate: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Candidate {
    pub id: u32,
    pub rank: u32,
    pub confidence: Confidence,
    pub score: f64,
    pub kind: Kind,
    pub name: String,
    pub file: u32,
    pub files: Vec<u32>,
    pub machines: Vec<u32>,
    pub sha256: Option<String>,
    pub terms: Vec<String>,
    pub evidence: Vec<Evidence>,
    pub evidence_total: u32,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Cluster {
    pub sha256: String,
    pub name: String,
    pub files: Vec<u32>,
    pub machines: Vec<u32>,
    pub size: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Version {
    pub sha256: Option<String>,
    pub files: Vec<u32>,
    pub newest: Option<i64>,
    pub candidate: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Family {
    pub key: String,
    pub label: String,
    pub versions: Vec<Version>,
    pub files: u32,
    pub best: Option<Confidence>,
    /// Rhino models sitting next to the definitions: likely what they were built on.
    pub companions: Vec<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct IssueView {
    pub machine: u32,
    pub path: String,
    pub kind: IssueKind,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CoverageView {
    pub machine: u32,
    #[serde(flatten)]
    pub coverage: Coverage,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Comparison {
    pub a: u32,
    pub b: u32,
    pub only_a: u32,
    pub only_b: u32,
    pub both: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Results {
    pub machines: Vec<MachineView>,
    pub files: Vec<FileView>,
    pub candidates: Vec<Candidate>,
    pub clusters: Vec<Cluster>,
    pub families: Vec<Family>,
    pub issues: Vec<IssueView>,
    pub coverage: Vec<CoverageView>,
    pub totals: Totals,
    pub comparison: Option<Comparison>,
}

pub fn build(scans: &[ScanData], terms: &[Term], this_machine: &str) -> Results {
    let matcher = Matcher::new(terms);
    let mut contents: HashMap<&str, &Content> = HashMap::new();
    for s in scans {
        for c in &s.contents {
            contents.entry(c.sha256.as_str()).or_insert(c);
        }
    }

    let mut files = Vec::new();
    for (mi, scan) in scans.iter().enumerate() {
        for f in &scan.files {
            let content = f.sha256.as_deref().and_then(|s| contents.get(s)).copied();
            let summary = content.and_then(|c| c.summary.as_ref());
            files.push(FileView {
                id: files.len() as u32,
                machine: mi as u32,
                path: f.path.clone(),
                folder: folder_of(f),
                name: f.name.clone(),
                ext: f.ext.clone(),
                kind: f.kind,
                size: f.size,
                created: f.created,
                modified: f.modified,
                sha256: f.sha256.clone(),
                status: f.status,
                detail: f.detail.clone(),
                tags: f.tags.clone(),
                read: content.map(|c| c.read).unwrap_or(if f.kind.is_grasshopper() {
                    GhRead::Inconclusive
                } else {
                    GhRead::NotApplicable
                }),
                read_error: content.and_then(|c| c.error.clone()),
                objects: summary.map(|s| s.objects),
                groups: summary.map(|s| s.groups.iter().take(8).cloned().collect()).unwrap_or_default(),
                plugins: summary.map(|s| s.plugins.clone()).unwrap_or_default(),
                gh_version: summary.and_then(|s| s.gh_version.clone()),
                copies: 1,
                family: f.kind.is_grasshopper().then(|| family_key(&f.name)),
                in_archive: f.archive.clone(),
                candidate: None,
            });
        }
    }

    let mut by_sha: HashMap<String, Vec<u32>> = HashMap::new();
    for f in &files {
        if let Some(sha) = &f.sha256 {
            by_sha.entry(sha.clone()).or_default().push(f.id);
        }
    }
    for ids in by_sha.values() {
        for &id in ids {
            files[id as usize].copies = ids.len() as u32;
        }
    }

    // Evidence: what the names say, plus what's written inside (once per content).
    let mut inside: HashMap<String, Vec<Evidence>> = HashMap::new();
    let mut evidence_of = Vec::with_capacity(files.len());
    for f in &files {
        let mut ev = name_evidence(&matcher, f);
        if let Some(sha) = &f.sha256 {
            if !inside.contains_key(sha) {
                let found = contents.get(sha.as_str()).map(|c| content_evidence(&matcher, c)).unwrap_or_default();
                inside.insert(sha.clone(), found);
            }
            ev.extend(inside[sha].iter().cloned());
        }
        evidence_of.push(ev);
    }

    // Candidates: one per distinct content (copies grouped), plus unreadable look-alikes.
    let mut groups: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for f in &files {
        let key = f.sha256.clone().unwrap_or_else(|| format!("path:{}:{}", f.machine, f.path));
        groups.entry(key).or_default().push(f.id);
    }
    let mut candidates = Vec::new();
    for (key, ids) in &groups {
        let mut ev: Vec<Evidence> = Vec::new();
        let mut seen = HashSet::new();
        for &id in ids {
            for e in &evidence_of[id as usize] {
                if seen.insert((e.term.clone(), e.source, e.text.clone())) {
                    ev.push(e.clone());
                }
            }
        }
        let rep = representative(&files, ids);
        let f = &files[rep as usize];
        let confidence = if ev.is_empty() {
            let unreadable = f.kind.is_grasshopper()
                && ids.iter().all(|&id| {
                    let x = &files[id as usize];
                    x.read == GhRead::Inconclusive || x.status != Status::Read
                });
            if !unreadable {
                continue;
            }
            Confidence::Inconclusive
        } else {
            judge(&ev).0
        };
        let score = if ev.is_empty() { 0.0 } else { judge(&ev).1 };
        ev.sort_by(|a, b| {
            let w = |e: &Evidence| e.strength.weight() * e.source.weight();
            w(b).partial_cmp(&w(a)).unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut terms: Vec<String> = Vec::new();
        for e in &ev {
            if !terms.contains(&e.term) {
                terms.push(e.term.clone());
            }
        }
        let reason = match ev.first() {
            Some(e) => format!("{}: “{}”", capitalize(e.source.label()), e.text),
            None => format!(
                "Couldn't look inside: {}",
                f.read_error.clone().or_else(|| f.detail.clone()).unwrap_or_else(|| status_words(f.status).into())
            ),
        };
        let mut machines: Vec<u32> = ids.iter().map(|&id| files[id as usize].machine).collect();
        machines.sort_unstable();
        machines.dedup();
        let total = ev.len() as u32;
        ev.truncate(MAX_EVIDENCE);
        candidates.push(Candidate {
            id: 0,
            rank: 0,
            confidence,
            score,
            kind: f.kind,
            name: f.name.clone(),
            file: rep,
            files: ids.clone(),
            machines,
            sha256: (!key.starts_with("path:")).then(|| key.clone()),
            terms,
            evidence: ev,
            evidence_total: total,
            reason,
        });
    }
    candidates.sort_by(|a, b| {
        a.confidence
            .cmp(&b.confidence)
            .then(b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal))
            .then(b.kind.is_grasshopper().cmp(&a.kind.is_grasshopper()))
            .then(files[b.file as usize].modified.cmp(&files[a.file as usize].modified))
    });
    for (i, c) in candidates.iter_mut().enumerate() {
        c.id = i as u32;
        c.rank = i as u32 + 1;
        for &fid in &c.files {
            files[fid as usize].candidate = Some(c.id);
        }
    }

    let mut clusters: Vec<Cluster> = by_sha
        .iter()
        .filter(|(_, ids)| ids.len() > 1)
        .map(|(sha, ids)| {
            let mut machines: Vec<u32> = ids.iter().map(|&i| files[i as usize].machine).collect();
            machines.sort_unstable();
            machines.dedup();
            let mut ids = ids.clone();
            ids.sort_by_key(|&i| (files[i as usize].machine, files[i as usize].path.clone()));
            Cluster {
                sha256: sha.clone(),
                name: files[representative(&files, &ids) as usize].name.clone(),
                size: files[ids[0] as usize].size,
                files: ids,
                machines,
            }
        })
        .collect();
    clusters.sort_by(|a, b| b.files.len().cmp(&a.files.len()).then(a.name.cmp(&b.name)));

    let families = families(&files, &candidates);
    let issues: Vec<IssueView> = scans
        .iter()
        .enumerate()
        .flat_map(|(mi, s)| {
            s.issues.iter().map(move |i| IssueView {
                machine: mi as u32,
                path: i.path.clone(),
                kind: i.kind,
                detail: i.detail.clone(),
            })
        })
        .collect();
    let coverage: Vec<CoverageView> = scans
        .iter()
        .enumerate()
        .flat_map(|(mi, s)| s.coverage.iter().map(move |c| CoverageView { machine: mi as u32, coverage: c.clone() }))
        .collect();

    let machines: Vec<MachineView> = scans
        .iter()
        .enumerate()
        .map(|(mi, s)| MachineView {
            index: mi as u32,
            id: s.meta.machine.id.clone(),
            label: s.meta.machine.label.clone(),
            os: s.meta.machine.os.clone(),
            os_version: s.meta.machine.os_version.clone(),
            device: s.meta.machine.device.clone(),
            scan_id: s.meta.scan_id.clone(),
            scanned_at: s.meta.finished_at,
            status: s.meta.status,
            this_computer: s.meta.machine.id == this_machine,
            dirs: s.meta.dirs,
            files_seen: s.meta.files_seen,
            totals: totals(&files, &candidates, &issues, Some(mi as u32)),
        })
        .collect();

    let comparison = (machines.len() >= 2).then(|| {
        let unique = |m: u32| -> HashSet<&str> {
            files
                .iter()
                .filter(|f| f.machine == m && f.kind == Kind::Definition)
                .filter_map(|f| f.sha256.as_deref())
                .collect()
        };
        let (a, b) = (unique(0), unique(1));
        Comparison {
            a: 0,
            b: 1,
            only_a: a.difference(&b).count() as u32,
            only_b: b.difference(&a).count() as u32,
            both: a.intersection(&b).count() as u32,
        }
    });

    let totals = totals(&files, &candidates, &issues, None);
    Results { machines, files, candidates, clusters, families, issues, coverage, totals, comparison }
}

fn folder_of(f: &FileRecord) -> String {
    let p = f.path.as_str();
    let cut = p.rfind(['/', '\\']).unwrap_or(0);
    p[..cut].to_string()
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn status_words(s: Status) -> &'static str {
    match s {
        Status::CloudOnly => "only in the cloud",
        Status::Unreadable => "couldn't be opened",
        Status::TooLarge => "too large",
        Status::NameOnly => "not opened",
        Status::Read => "not readable",
    }
}

/// Matches in the file's own name, its folders, and (for files inside a zip) the zip and its folders.
fn name_evidence(matcher: &Matcher, f: &FileView) -> Vec<Evidence> {
    let mut out: Vec<Evidence> = Vec::new();
    let push = |source: Source, text: &str, out: &mut Vec<Evidence>| {
        for (term, excerpt) in matcher.find(text) {
            if !out.iter().any(|e| e.term == term.word && e.source == source) {
                out.push(Evidence {
                    term: term.word.clone(),
                    strength: term.strength,
                    source,
                    text: excerpt,
                    in_cluster: false,
                });
            }
        }
    };
    push(Source::FileName, &f.name, &mut out);
    let (outer, inner) = match f.path.split_once("!/") {
        Some((zip, member)) => (zip.to_string(), Some(member.to_string())),
        None => (f.folder.clone(), None),
    };
    let parts: Vec<&str> = outer.split(['/', '\\']).filter(|p| !p.is_empty()).collect();
    for (i, part) in parts.iter().enumerate() {
        // The account folder (/Users/<name>) says nothing about the project.
        let account =
            i > 0 && (parts[i - 1].eq_ignore_ascii_case("users") || parts[i - 1].eq_ignore_ascii_case("home"));
        if account {
            continue;
        }
        let is_zip_itself = inner.is_some() && i == parts.len() - 1;
        push(if is_zip_itself { Source::InsideArchivePath } else { Source::Folder }, part, &mut out);
    }
    if let Some(member) = inner {
        let segments: Vec<&str> = member.split('/').collect();
        for seg in &segments[..segments.len().saturating_sub(1)] {
            push(Source::InsideArchivePath, seg, &mut out);
        }
    }
    out
}

fn content_evidence(matcher: &Matcher, c: &Content) -> Vec<Evidence> {
    let Some(d) = &c.digest else { return Vec::new() };
    let mut out: Vec<Evidence> = Vec::new();
    let mut seen = HashSet::new();
    for t in &d.texts {
        let source: Source = t.source.into();
        for (term, ex) in matcher.find(&t.text) {
            if seen.insert((term.word.clone(), source, ex.clone())) {
                out.push(Evidence {
                    term: term.word.clone(),
                    strength: term.strength,
                    source,
                    text: ex,
                    in_cluster: t.in_cluster,
                });
            }
        }
        if out.len() >= 400 {
            break;
        }
    }
    out
}

/// The copy to show first: readable, in a working folder, most recently changed.
fn representative(files: &[FileView], ids: &[u32]) -> u32 {
    let penalty = |f: &FileView| {
        let mut p = 0;
        if f.status != Status::Read {
            p += 4;
        }
        for t in &f.tags {
            p += match t.as_str() {
                "installed" => 3,
                "trash" | "deleted" => 2,
                "temp" | "autosave" | "in_archive" => 1,
                _ => 0,
            };
        }
        p
    };
    *ids.iter()
        .min_by(|&&a, &&b| {
            let (fa, fb) = (&files[a as usize], &files[b as usize]);
            penalty(fa).cmp(&penalty(fb)).then(fb.modified.cmp(&fa.modified))
        })
        .unwrap_or(&ids[0])
}

fn trailing_noise() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"(?i)[\s_\-.]*(?:\(\d+\)|copy(?:\s*\d+)?|v(?:er(?:sion)?)?[\s_\-.]*\d+[a-z]?|rev[\s_\-.]*\d+|final|finale|last|latest|new|old|backup|bak|edit(?:ed)?|test|draft|wip|autosave|\d{4}[\-_.]?\d{2}[\-_.]?\d{2}|\d{1,4})$",
        )
        .unwrap()
    })
}

/// Groups `palm_v2_final.gh`, `palm v3.gh` and `palm (1).gh` as one family.
pub fn family_key(name: &str) -> String {
    let lower = name.to_lowercase();
    let mut stem = lower.as_str();
    for ext in [".bak", ".backup", ".old", ".tmp", "~", ".gh", ".ghx", ".ghuser", ".ghcluster"] {
        if let Some(s) = stem.strip_suffix(ext) {
            stem = s;
        }
    }
    let mut s = stem.to_string();
    loop {
        let next = trailing_noise().replace(&s, "").to_string();
        if next == s || next.is_empty() {
            break;
        }
        s = next;
    }
    let squashed: String = s.chars().filter(|c| c.is_alphanumeric()).collect();
    if squashed.is_empty() {
        stem.chars().filter(|c| c.is_alphanumeric()).collect()
    } else {
        squashed
    }
}

fn families(files: &[FileView], candidates: &[Candidate]) -> Vec<Family> {
    let mut by_key: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    for f in files {
        if let Some(k) = &f.family {
            by_key.entry(k.clone()).or_default().push(f.id);
        }
    }
    let mut models_by_folder: HashMap<(u32, &str), Vec<u32>> = HashMap::new();
    for f in files.iter().filter(|f| f.kind == Kind::RhinoModel) {
        models_by_folder.entry((f.machine, f.folder.as_str())).or_default().push(f.id);
    }
    let mut out = Vec::new();
    for (key, ids) in by_key {
        if ids.len() < 2 && files[ids[0] as usize].candidate.is_none() {
            continue;
        }
        let mut versions: BTreeMap<String, Version> = BTreeMap::new();
        for &id in &ids {
            let f = &files[id as usize];
            let vkey = f.sha256.clone().unwrap_or_else(|| format!("path:{}", f.id));
            let v = versions.entry(vkey).or_insert(Version {
                sha256: f.sha256.clone(),
                files: Vec::new(),
                newest: None,
                candidate: f.candidate,
            });
            v.files.push(id);
            v.newest = v.newest.max(f.modified);
        }
        let mut versions: Vec<Version> = versions.into_values().collect();
        versions.sort_by_key(|v| std::cmp::Reverse(v.newest));
        let best =
            ids.iter().filter_map(|&id| files[id as usize].candidate).map(|c| candidates[c as usize].confidence).min();
        let mut labels: HashMap<String, u32> = HashMap::new();
        for &id in &ids {
            *labels.entry(files[id as usize].name.clone()).or_default() += 1;
        }
        let label =
            labels.into_iter().max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0))).map(|(n, _)| n).unwrap_or(key.clone());
        let mut companions: Vec<u32> = Vec::new();
        for &id in &ids {
            let f = &files[id as usize];
            if let Some(models) = models_by_folder.get(&(f.machine, f.folder.as_str())) {
                for &m in models {
                    if !companions.contains(&m) {
                        companions.push(m);
                    }
                }
            }
        }
        out.push(Family { key, label: excerpt(&label, 80), files: ids.len() as u32, versions, best, companions });
    }
    out.sort_by(|a, b| {
        let rank = |c: Option<Confidence>| c.map(|c| c as u8).unwrap_or(9);
        rank(a.best).cmp(&rank(b.best)).then(b.versions.len().cmp(&a.versions.len())).then(a.label.cmp(&b.label))
    });
    out
}

fn totals(files: &[FileView], candidates: &[Candidate], issues: &[IssueView], machine: Option<u32>) -> Totals {
    let mut t = Totals::default();
    let on = |m: u32| machine.is_none_or(|x| x == m);
    let mut unique = HashSet::new();
    for f in files.iter().filter(|f| on(f.machine)) {
        match f.kind {
            Kind::Definition => {
                t.definitions += 1;
                match &f.sha256 {
                    Some(sha) => {
                        unique.insert(sha.clone());
                    }
                    None => {
                        unique.insert(format!("path:{}", f.id));
                    }
                }
            }
            Kind::UserObject => t.user_objects += 1,
            Kind::RhinoModel | Kind::RhinoBackup => t.rhino_models += 1,
            Kind::Script => t.scripts += 1,
            Kind::Archive => t.archives += 1,
            Kind::Document => t.documents += 1,
        }
        match f.status {
            Status::CloudOnly => t.cloud_only += 1,
            Status::Unreadable => t.unreadable += 1,
            _ => {}
        }
        if f.kind.is_grasshopper() && f.status == Status::Read && f.read == GhRead::Inconclusive {
            t.damaged += 1;
        }
    }
    t.unique_definitions = unique.len() as u32;
    for c in candidates {
        if !c.files.iter().any(|&id| on(files[id as usize].machine)) {
            continue;
        }
        match c.confidence {
            Confidence::Strong => t.strong += 1,
            Confidence::Possible => t.possible += 1,
            Confidence::Weak => t.weak += 1,
            Confidence::Inconclusive => t.inconclusive += 1,
        }
    }
    for i in issues.iter().filter(|i| on(i.machine)) {
        if i.kind == IssueKind::Skipped {
            t.skipped += 1;
        } else {
            t.issues += 1;
        }
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn families_ignore_version_noise() {
        assert_eq!(family_key("palm_v2_final.gh"), "palm");
        assert_eq!(family_key("Palm Tree (1).gh"), "palmtree");
        assert_eq!(family_key("palm tree 2023-05-01.ghx"), "palmtree");
        assert_eq!(family_key("spiral-001.gh"), "spiral");
        assert_eq!(family_key("palm copy 2.gh.bak"), "palm");
        assert_eq!(family_key("001.gh"), "001");
    }
}
