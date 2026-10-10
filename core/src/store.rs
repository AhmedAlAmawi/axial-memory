//! The scan file: one SQLite database per laptop scan. It is what the app keeps
//! between runs and what she carries to the other laptop.

use crate::model::*;
use crate::roots::RootKind;
use anyhow::{bail, Context, Result};
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use rusqlite::{params, Connection, OpenFlags};
use std::io::{Read, Write};
use std::path::Path;

pub const VERSION: i32 = 1;
pub const EXTENSION: &str = "axialscan";

const SCHEMA: &str = "
CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE files (
  id INTEGER PRIMARY KEY, path TEXT NOT NULL, archive TEXT, name TEXT NOT NULL, ext TEXT NOT NULL,
  kind TEXT NOT NULL, size INTEGER, created INTEGER, modified INTEGER, accessed INTEGER, sha256 TEXT,
  status TEXT NOT NULL, detail TEXT, tags TEXT NOT NULL, root TEXT NOT NULL
);
CREATE INDEX files_sha256 ON files(sha256);
CREATE TABLE contents (sha256 TEXT PRIMARY KEY, read TEXT NOT NULL, error TEXT, summary TEXT, digest BLOB);
CREATE TABLE issues (path TEXT NOT NULL, kind TEXT NOT NULL, detail TEXT NOT NULL);
CREATE TABLE coverage (
  root TEXT NOT NULL, label TEXT NOT NULL, kind TEXT NOT NULL, status TEXT NOT NULL,
  dirs INTEGER NOT NULL, files INTEGER NOT NULL, found INTEGER NOT NULL, issues INTEGER NOT NULL, note TEXT
);
";

fn i(v: Option<u64>) -> Option<i64> {
    v.map(|n| n.min(i64::MAX as u64) as i64)
}

fn pack(value: &impl serde::Serialize) -> Result<Vec<u8>> {
    let mut enc = DeflateEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&serde_json::to_vec(value)?)?;
    Ok(enc.finish()?)
}

fn unpack<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    let mut json = Vec::new();
    DeflateDecoder::new(bytes).take(512 * 1024 * 1024).read_to_end(&mut json)?;
    Ok(serde_json::from_slice(&json)?)
}

/// Writes atomically: a half-written scan file never replaces a good one.
pub fn write(path: &Path, data: &ScanData) -> Result<()> {
    let tmp = path.with_extension("partial");
    let _ = std::fs::remove_file(&tmp);
    {
        let mut conn = Connection::open(&tmp)?;
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", VERSION)?;
        let tx = conn.transaction()?;
        tx.execute("INSERT INTO meta (key, value) VALUES ('scan', ?1)", params![serde_json::to_string(&data.meta)?])?;
        {
            let mut st = tx.prepare(
                "INSERT INTO files (id, path, archive, name, ext, kind, size, created, modified, accessed, sha256, status, detail, tags, root)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
            )?;
            for f in &data.files {
                st.execute(params![
                    f.id,
                    f.path,
                    f.archive,
                    f.name,
                    f.ext,
                    f.kind.as_str(),
                    i(f.size),
                    f.created,
                    f.modified,
                    f.accessed,
                    f.sha256,
                    f.status.as_str(),
                    f.detail,
                    serde_json::to_string(&f.tags)?,
                    f.root,
                ])?;
            }
        }
        {
            let mut st =
                tx.prepare("INSERT INTO contents (sha256, read, error, summary, digest) VALUES (?1, ?2, ?3, ?4, ?5)")?;
            for c in &data.contents {
                let summary = c.summary.as_ref().map(serde_json::to_string).transpose()?;
                let digest = c.digest.as_ref().map(pack).transpose()?;
                st.execute(params![c.sha256, c.read.as_str(), c.error, summary, digest])?;
            }
        }
        {
            let mut st = tx.prepare("INSERT INTO issues (path, kind, detail) VALUES (?1, ?2, ?3)")?;
            for issue in &data.issues {
                st.execute(params![issue.path, issue.kind.as_str(), issue.detail])?;
            }
        }
        {
            let mut st = tx.prepare(
                "INSERT INTO coverage (root, label, kind, status, dirs, files, found, issues, note) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for c in &data.coverage {
                st.execute(params![
                    c.root,
                    c.label,
                    c.kind.as_str(),
                    c.status.as_str(),
                    i(Some(c.dirs)),
                    i(Some(c.files)),
                    i(Some(c.found)),
                    i(Some(c.issues)),
                    c.note
                ])?;
            }
        }
        tx.commit()?;
    }
    std::fs::rename(&tmp, path).with_context(|| format!("saving {}", path.display()))?;
    Ok(())
}

fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .with_context(|| format!("opening {}", path.display()))?;
    let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version == 0 {
        bail!("this isn't an Axial Memory scan file");
    }
    if version > VERSION {
        bail!("this scan file was made by a newer version of Axial Memory; update the app first");
    }
    Ok(conn)
}

pub fn read_meta(path: &Path) -> Result<ScanMeta> {
    let conn = open(path)?;
    let json: String = conn.query_row("SELECT value FROM meta WHERE key = 'scan'", [], |r| r.get(0))?;
    Ok(serde_json::from_str(&json)?)
}

pub fn read(path: &Path) -> Result<ScanData> {
    let conn = open(path)?;
    let json: String = conn.query_row("SELECT value FROM meta WHERE key = 'scan'", [], |r| r.get(0))?;
    let meta: ScanMeta = serde_json::from_str(&json)?;

    let mut st = conn.prepare(
        "SELECT id, path, archive, name, ext, kind, size, created, modified, accessed, sha256, status, detail, tags, root FROM files ORDER BY id",
    )?;
    let files = st
        .query_map([], |r| {
            let kind: String = r.get(5)?;
            let status: String = r.get(11)?;
            let tags: String = r.get(13)?;
            Ok(FileRecord {
                id: r.get(0)?,
                path: r.get(1)?,
                archive: r.get(2)?,
                name: r.get(3)?,
                ext: r.get(4)?,
                kind: Kind::parse(&kind).unwrap_or(Kind::Document),
                size: r.get::<_, Option<i64>>(6)?.map(|n| n as u64),
                created: r.get(7)?,
                modified: r.get(8)?,
                accessed: r.get(9)?,
                sha256: r.get(10)?,
                status: Status::parse(&status),
                detail: r.get(12)?,
                tags: serde_json::from_str(&tags).unwrap_or_default(),
                root: r.get(14)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut st = conn.prepare("SELECT sha256, read, error, summary, digest FROM contents ORDER BY sha256")?;
    let rows = st
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<String>>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<Vec<u8>>>(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut contents = Vec::with_capacity(rows.len());
    for (sha256, read, error, summary, digest) in rows {
        contents.push(Content {
            sha256,
            read: GhRead::parse(&read),
            error,
            summary: summary.and_then(|s| serde_json::from_str(&s).ok()),
            digest: digest.and_then(|d| unpack(&d).ok()),
        });
    }

    let mut st = conn.prepare("SELECT path, kind, detail FROM issues ORDER BY rowid")?;
    let issues = st
        .query_map([], |r| {
            let kind: String = r.get(1)?;
            Ok(Issue { path: r.get(0)?, kind: IssueKind::parse(&kind), detail: r.get(2)? })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut st = conn
        .prepare("SELECT root, label, kind, status, dirs, files, found, issues, note FROM coverage ORDER BY rowid")?;
    let coverage = st
        .query_map([], |r| {
            let kind: String = r.get(2)?;
            let status: String = r.get(3)?;
            Ok(Coverage {
                root: r.get(0)?,
                label: r.get(1)?,
                kind: RootKind::parse(&kind),
                status: CoverageStatus::parse(&status),
                dirs: r.get::<_, i64>(4)? as u64,
                files: r.get::<_, i64>(5)? as u64,
                found: r.get::<_, i64>(6)? as u64,
                issues: r.get::<_, i64>(7)? as u64,
                note: r.get(8)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(ScanData { meta, files, contents, issues, coverage })
}
