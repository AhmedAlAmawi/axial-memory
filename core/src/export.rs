//! The report folder: the spec's CSV catalogs, a merged SQLite database, the
//! review set for the interpreter stage, and any AI explanations.

use crate::model::*;
use crate::results::{Candidate, FileView, Results};
use anyhow::Result;
use rusqlite::{params, Connection};
use serde_json::json;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct Explanation {
    pub sha256: String,
    pub file_name: String,
    pub markdown: String,
}

pub fn date(t: Option<i64>) -> String {
    t.and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .map(|d| d.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

fn csv_writer(path: &Path) -> Result<csv::Writer<std::fs::File>> {
    let mut f = std::fs::File::create(path)?;
    // A byte-order mark makes Excel read accents and Arabic correctly.
    f.write_all("\u{feff}".as_bytes())?;
    Ok(csv::Writer::from_writer(f))
}

fn machine_label(r: &Results, f: &FileView) -> String {
    r.machines.get(f.machine as usize).map(|m| m.label.clone()).unwrap_or_default()
}

fn confidence_of(r: &Results, f: &FileView) -> String {
    f.candidate.map(|c| r.candidates[c as usize].confidence.as_str().to_string()).unwrap_or_default()
}

/// Writes `Axial Memory report <date>` inside `parent` and returns its path.
pub fn write_report(r: &Results, parent: &Path, explanations: &[Explanation]) -> Result<PathBuf> {
    let stamp = chrono::Local::now().format("%Y-%m-%d %H.%M").to_string();
    let mut dir = parent.join(format!("Axial Memory report {stamp}"));
    let mut n = 2;
    while dir.exists() {
        dir = parent.join(format!("Axial Memory report {stamp} ({n})"));
        n += 1;
    }
    std::fs::create_dir_all(&dir)?;

    std::fs::write(dir.join("README.txt"), readme(r))?;
    write_files(r, &dir.join("ALL_GRASSHOPPER_FILES.csv"), |f| f.kind == Kind::Definition)?;
    write_files(r, &dir.join("RELATED_FILES.csv"), |f| f.kind != Kind::Definition)?;
    write_candidates(r, &dir.join("PALM_CANDIDATES.csv"))?;
    write_evidence(r, &dir.join("PALM_EVIDENCE.csv"))?;
    write_issues(r, &dir.join("ACCESS_ERRORS.csv"))?;
    write_coverage(r, &dir.join("COVERAGE.csv"))?;
    write_clusters(r, &dir.join("DUPLICATE_CLUSTERS.csv"))?;
    std::fs::write(dir.join("REVIEW_SET.json"), serde_json::to_string_pretty(&review_set(r))?)?;
    write_sqlite(r, &dir.join("axial_memory.sqlite"))?;
    if !explanations.is_empty() {
        let ex = dir.join("EXPLANATIONS");
        std::fs::create_dir_all(&ex)?;
        for e in explanations {
            let safe: String =
                e.file_name.chars().map(|c| if c.is_alphanumeric() || " -_.".contains(c) { c } else { '_' }).collect();
            std::fs::write(ex.join(format!("{} ({}).md", safe, &e.sha256[..8.min(e.sha256.len())])), &e.markdown)?;
        }
    }
    Ok(dir)
}

fn readme(r: &Results) -> String {
    let mut s = String::from("Axial Memory report\n===================\n\n");
    for m in &r.machines {
        let t = &m.totals;
        s.push_str(&format!(
            "{} (scanned {}{}): {} Grasshopper definitions ({} different), {} strong and {} possible palm matches, {} only in the cloud, {} places it couldn't look.\n",
            m.label,
            date(Some(m.scanned_at)),
            if m.status == ScanStatus::Stopped { ", stopped early" } else { "" },
            t.definitions,
            t.unique_definitions,
            t.strong,
            t.possible,
            t.cloud_only,
            t.issues
        ));
    }
    s.push_str(
        "
What each file is
-----------------
ALL_GRASSHOPPER_FILES.csv  Every .gh/.ghx definition found (and backup copies), with where it is, size, dates, fingerprint (SHA-256) and what's inside.
RELATED_FILES.csv          User objects, Rhino models, scripts and zips found next to or about the definitions.
PALM_CANDIDATES.csv        Files that may belong to the palm research, ranked. Every row says why.
PALM_EVIDENCE.csv          Every match behind those rankings: the word, where it was found, and the text around it.
ACCESS_ERRORS.csv          Every place the scan couldn't look or read, and what to do about it.
COVERAGE.csv               Which drives were searched, how much, and what was skipped.
DUPLICATE_CLUSTERS.csv     Identical files (same fingerprint) found in more than one place.
REVIEW_SET.json            Project families with their distinct versions, plugins and referenced files, for the next stage.
axial_memory.sqlite        All of the above in one database.
EXPLANATIONS/              AI explanations, if any were asked for.

Confidence: strong = a search word appears in the file's name or inside it (a group, panel, script...);
possible = only in a folder name, or a word with other meanings like \"palm\"; weak = only related words;
inconclusive = the inside couldn't be read and the name doesn't match, so check it by hand.
Nothing was changed, moved or uploaded to make this report.
",
    );
    s
}

fn write_files(r: &Results, path: &Path, keep: impl Fn(&FileView) -> bool) -> Result<()> {
    let mut w = csv_writer(path)?;
    w.write_record([
        "machine",
        "full_path",
        "inside_archive",
        "file_name",
        "file_type",
        "kind",
        "size_bytes",
        "created",
        "modified",
        "sha256",
        "status",
        "status_detail",
        "read_inside",
        "components",
        "groups",
        "plugins",
        "grasshopper_version",
        "location_tags",
        "copies_total",
        "family",
        "palm_confidence",
    ])?;
    for f in r.files.iter().filter(|f| keep(f)) {
        w.write_record([
            machine_label(r, f),
            f.path.clone(),
            f.in_archive.clone().unwrap_or_default(),
            f.name.clone(),
            f.ext.clone(),
            f.kind.as_str().to_string(),
            f.size.map(|s| s.to_string()).unwrap_or_default(),
            date(f.created),
            date(f.modified),
            f.sha256.clone().unwrap_or_default(),
            f.status.as_str().to_string(),
            f.detail.clone().or_else(|| f.read_error.clone()).unwrap_or_default(),
            f.read.as_str().to_string(),
            f.objects.map(|n| n.to_string()).unwrap_or_default(),
            f.groups.join("; "),
            f.plugins.join("; "),
            f.gh_version.clone().unwrap_or_default(),
            f.tags.join("; "),
            f.copies.to_string(),
            f.family.clone().unwrap_or_default(),
            confidence_of(r, f),
        ])?;
    }
    w.flush()?;
    Ok(())
}

fn paths_of(r: &Results, c: &Candidate) -> String {
    c.files
        .iter()
        .map(|&id| {
            let f = &r.files[id as usize];
            format!("{}: {}", machine_label(r, f), f.path)
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn write_candidates(r: &Results, path: &Path) -> Result<()> {
    let mut w = csv_writer(path)?;
    w.write_record([
        "rank",
        "confidence",
        "score",
        "kind",
        "file_name",
        "machine",
        "full_path",
        "matched_terms",
        "evidence_sources",
        "reason",
        "copies",
        "all_copies",
        "sha256",
        "modified",
    ])?;
    for c in &r.candidates {
        let f = &r.files[c.file as usize];
        let mut sources: Vec<&str> = c.evidence.iter().map(|e| e.source.label()).collect();
        sources.dedup();
        w.write_record([
            c.rank.to_string(),
            c.confidence.as_str().to_string(),
            format!("{:.2}", c.score),
            c.kind.as_str().to_string(),
            c.name.clone(),
            machine_label(r, f),
            f.path.clone(),
            c.terms.join("; "),
            sources.join("; "),
            c.reason.clone(),
            c.files.len().to_string(),
            paths_of(r, c),
            c.sha256.clone().unwrap_or_default(),
            date(f.modified),
        ])?;
    }
    w.flush()?;
    Ok(())
}

fn write_evidence(r: &Results, path: &Path) -> Result<()> {
    let mut w = csv_writer(path)?;
    w.write_record(["rank", "file_name", "full_path", "term", "strength", "found_in", "text", "inside_cluster"])?;
    for c in &r.candidates {
        let f = &r.files[c.file as usize];
        for e in &c.evidence {
            w.write_record([
                c.rank.to_string(),
                c.name.clone(),
                f.path.clone(),
                e.term.clone(),
                e.strength.as_str().to_string(),
                e.source.label().to_string(),
                e.text.clone(),
                if e.in_cluster { "yes".into() } else { String::new() },
            ])?;
        }
    }
    w.flush()?;
    Ok(())
}

fn write_issues(r: &Results, path: &Path) -> Result<()> {
    let mut w = csv_writer(path)?;
    w.write_record(["machine", "path", "problem", "detail", "what_to_do"])?;
    for i in &r.issues {
        let m = r.machines.get(i.machine as usize).map(|m| m.label.clone()).unwrap_or_default();
        w.write_record([m, i.path.clone(), i.kind.label().to_string(), i.detail.clone(), i.kind.advice().to_string()])?;
    }
    w.flush()?;
    Ok(())
}

fn write_coverage(r: &Results, path: &Path) -> Result<()> {
    let mut w = csv_writer(path)?;
    w.write_record([
        "machine",
        "location",
        "label",
        "kind",
        "status",
        "folders_searched",
        "files_seen",
        "found",
        "problems",
        "note",
    ])?;
    for c in &r.coverage {
        let m = r.machines.get(c.machine as usize).map(|m| m.label.clone()).unwrap_or_default();
        let v = &c.coverage;
        w.write_record([
            m,
            v.root.clone(),
            v.label.clone(),
            v.kind.as_str().to_string(),
            v.status.as_str().to_string(),
            v.dirs.to_string(),
            v.files.to_string(),
            v.found.to_string(),
            v.issues.to_string(),
            v.note.clone().unwrap_or_default(),
        ])?;
    }
    for m in &r.machines {
        let t = &m.totals;
        w.write_record([
            m.label.clone(),
            "TOTAL".into(),
            format!("{} definitions ({} different)", t.definitions, t.unique_definitions),
            String::new(),
            if m.status == ScanStatus::Stopped { "stopped early".into() } else { "complete".into() },
            m.dirs.to_string(),
            m.files_seen.to_string(),
            (t.definitions + t.user_objects + t.rhino_models + t.scripts + t.archives + t.documents).to_string(),
            t.issues.to_string(),
            format!("{} skipped on purpose; {} only in the cloud", t.skipped, t.cloud_only),
        ])?;
    }
    w.flush()?;
    Ok(())
}

fn write_clusters(r: &Results, path: &Path) -> Result<()> {
    let mut w = csv_writer(path)?;
    w.write_record(["cluster", "sha256", "copies", "machines", "machine", "full_path", "size_bytes", "modified"])?;
    for (i, c) in r.clusters.iter().enumerate() {
        let machines = c
            .machines
            .iter()
            .filter_map(|&m| r.machines.get(m as usize))
            .map(|m| m.label.clone())
            .collect::<Vec<_>>()
            .join("; ");
        for &id in &c.files {
            let f = &r.files[id as usize];
            w.write_record([
                (i + 1).to_string(),
                c.sha256.clone(),
                c.files.len().to_string(),
                machines.clone(),
                machine_label(r, f),
                f.path.clone(),
                f.size.map(|s| s.to_string()).unwrap_or_default(),
                date(f.modified),
            ])?;
        }
    }
    w.flush()?;
    Ok(())
}

/// Prioritized project families for the interpreter stage.
pub fn review_set(r: &Results) -> serde_json::Value {
    let copy = |id: u32| {
        let f = &r.files[id as usize];
        json!({ "machine": machine_label(r, f), "path": f.path, "modified": date(f.modified), "tags": f.tags })
    };
    let families: Vec<_> = r
        .families
        .iter()
        .filter(|fam| fam.best.is_some() || fam.versions.len() > 1)
        .map(|fam| {
            let versions: Vec<_> = fam
                .versions
                .iter()
                .map(|v| {
                    let first = &r.files[v.files[0] as usize];
                    let candidate = v.candidate.map(|c| &r.candidates[c as usize]);
                    json!({
                        "sha256": v.sha256,
                        "newest": date(v.newest),
                        "components": first.objects,
                        "groups": first.groups,
                        "plugins": first.plugins,
                        "grasshopper_version": first.gh_version,
                        "palm_rank": candidate.map(|c| c.rank),
                        "palm_confidence": candidate.map(|c| c.confidence.as_str()),
                        "copies": v.files.iter().map(|&id| copy(id)).collect::<Vec<_>>(),
                    })
                })
                .collect();
            json!({
                "family": fam.label,
                "best_confidence": fam.best.map(|c| c.as_str()),
                "distinct_versions": fam.versions.len(),
                "versions": versions,
                "companion_models": fam.companions.iter().map(|&id| copy(id)).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "generated_at": chrono::Local::now().to_rfc3339(),
        "machines": r.machines.iter().map(|m| json!({ "label": m.label, "os": m.os, "scanned_at": date(Some(m.scanned_at)), "complete": m.status == ScanStatus::Complete })).collect::<Vec<_>>(),
        "families": families,
    })
}

fn write_sqlite(r: &Results, path: &Path) -> Result<()> {
    let _ = std::fs::remove_file(path);
    let mut conn = Connection::open(path)?;
    conn.execute_batch(
        "CREATE TABLE machines (id INTEGER PRIMARY KEY, label TEXT, os TEXT, os_version TEXT, device TEXT, scan_id TEXT, scanned_at TEXT, status TEXT);
         CREATE TABLE files (id INTEGER PRIMARY KEY, machine INTEGER, path TEXT, inside_archive TEXT, name TEXT, ext TEXT, kind TEXT,
           size INTEGER, created TEXT, modified TEXT, sha256 TEXT, status TEXT, detail TEXT, read_inside TEXT, components INTEGER,
           groups_json TEXT, plugins_json TEXT, gh_version TEXT, tags_json TEXT, copies INTEGER, family TEXT, candidate INTEGER);
         CREATE TABLE candidates (id INTEGER PRIMARY KEY, rank INTEGER, confidence TEXT, score REAL, kind TEXT, name TEXT, file INTEGER,
           files_json TEXT, sha256 TEXT, terms_json TEXT, reason TEXT);
         CREATE TABLE evidence (candidate INTEGER, term TEXT, strength TEXT, source TEXT, text TEXT, in_cluster INTEGER);
         CREATE TABLE clusters (sha256 TEXT, copies INTEGER, files_json TEXT);
         CREATE TABLE issues (machine INTEGER, path TEXT, kind TEXT, detail TEXT);
         CREATE TABLE coverage (machine INTEGER, root TEXT, label TEXT, kind TEXT, status TEXT, dirs INTEGER, files INTEGER, found INTEGER, issues INTEGER, note TEXT);",
    )?;
    let tx = conn.transaction()?;
    for m in &r.machines {
        tx.execute(
            "INSERT INTO machines VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                m.index,
                m.label,
                m.os,
                m.os_version,
                m.device,
                m.scan_id,
                date(Some(m.scanned_at)),
                format!("{:?}", m.status).to_lowercase()
            ],
        )?;
    }
    {
        let mut st = tx.prepare(
            "INSERT INTO files VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
        )?;
        for f in &r.files {
            st.execute(params![
                f.id,
                f.machine,
                f.path,
                f.in_archive,
                f.name,
                f.ext,
                f.kind.as_str(),
                f.size.map(|s| s as i64),
                date(f.created),
                date(f.modified),
                f.sha256,
                f.status.as_str(),
                f.detail,
                f.read.as_str(),
                f.objects,
                serde_json::to_string(&f.groups)?,
                serde_json::to_string(&f.plugins)?,
                f.gh_version,
                serde_json::to_string(&f.tags)?,
                f.copies,
                f.family,
                f.candidate,
            ])?;
        }
    }
    {
        let mut st = tx.prepare("INSERT INTO candidates VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)")?;
        let mut ev = tx.prepare("INSERT INTO evidence VALUES (?1,?2,?3,?4,?5,?6)")?;
        for c in &r.candidates {
            st.execute(params![
                c.id,
                c.rank,
                c.confidence.as_str(),
                c.score,
                c.kind.as_str(),
                c.name,
                c.file,
                serde_json::to_string(&c.files)?,
                c.sha256,
                serde_json::to_string(&c.terms)?,
                c.reason,
            ])?;
            for e in &c.evidence {
                ev.execute(params![c.id, e.term, e.strength.as_str(), e.source.as_str(), e.text, e.in_cluster])?;
            }
        }
    }
    {
        let mut st = tx.prepare("INSERT INTO clusters VALUES (?1,?2,?3)")?;
        for c in &r.clusters {
            st.execute(params![c.sha256, c.files.len() as i64, serde_json::to_string(&c.files)?])?;
        }
        let mut st = tx.prepare("INSERT INTO issues VALUES (?1,?2,?3,?4)")?;
        for i in &r.issues {
            st.execute(params![i.machine, i.path, i.kind.as_str(), i.detail])?;
        }
        let mut st = tx.prepare("INSERT INTO coverage VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)")?;
        for c in &r.coverage {
            let v = &c.coverage;
            st.execute(params![
                c.machine,
                v.root,
                v.label,
                v.kind.as_str(),
                v.status.as_str(),
                v.dirs as i64,
                v.files as i64,
                v.found as i64,
                v.issues as i64,
                v.note
            ])?;
        }
    }
    tx.commit()?;
    Ok(())
}
