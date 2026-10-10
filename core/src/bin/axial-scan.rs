//! Headless scan, for development and as a fallback when the app can't run.
//!
//!   axial-scan [--out DIR] [--label NAME] [--add SCANFILE]... [PATH]...
//!
//! With no PATH it searches the whole computer like the app does. It writes a
//! `.axialscan` file and a report folder into DIR (default: current folder).

use axial_core::model::{Machine, ScanOptions};
use axial_core::roots::{self, Root};
use axial_core::scan::{self, Progress, ScanRequest};
use axial_core::{export, results, store, terms};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

fn main() -> anyhow::Result<()> {
    let mut out = PathBuf::from(".");
    let mut label = None;
    let mut extra_scans = Vec::new();
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--out" => out = PathBuf::from(args.next().expect("--out needs a folder")),
            "--label" => label = args.next(),
            "--add" => extra_scans.push(PathBuf::from(args.next().expect("--add needs a scan file"))),
            "-h" | "--help" => {
                println!("axial-scan [--out DIR] [--label NAME] [--add SCANFILE]... [PATH]...");
                return Ok(());
            }
            p => paths.push(p.to_string()),
        }
    }
    let roots: Vec<Root> =
        if paths.is_empty() { roots::discover() } else { paths.iter().map(|p| Root::folder(p)).collect() };
    for r in &roots {
        eprintln!("{} {} ({})", if r.selected { "searching" } else { "skipping " }, r.label, r.path);
    }
    let req = ScanRequest {
        roots,
        options: ScanOptions::default(),
        terms: terms::default_terms(),
        machine: Machine::current("cli".into(), label),
        app_version: env!("CARGO_PKG_VERSION").into(),
    };
    let cancel = AtomicBool::new(false);
    let mut print = |p: &Progress| {
        eprint!(
            "\r{:>7} folders {:>9} files {:>6} definitions {:>6} other  read {}/{}  {:.1}s   ",
            p.dirs,
            p.files_seen,
            p.definitions,
            p.other_found,
            p.read,
            p.queued,
            p.elapsed_ms as f64 / 1000.0
        );
        let _ = std::io::stderr().flush();
    };
    let data = scan::run(&req, &cancel, &mut print);
    eprintln!();
    std::fs::create_dir_all(&out)?;
    let scan_path = out.join(format!("{} scan.{}", data.meta.machine.label, store::EXTENSION));
    store::write(&scan_path, &data)?;
    let mut scans = vec![store::read(&scan_path)?];
    for p in &extra_scans {
        scans.push(store::read(p)?);
    }
    let r = results::build(&scans, &req.terms, &data.meta.machine.id);
    let report = export::write_report(&r, &out, &[])?;
    let t = &r.totals;
    println!(
        "{} definitions ({} different), {} user objects, {} Rhino models, {} scripts, {} archives, {} documents",
        t.definitions, t.unique_definitions, t.user_objects, t.rhino_models, t.scripts, t.archives, t.documents
    );
    println!(
        "palm: {} strong, {} possible, {} weak, {} inconclusive; {} only in the cloud; {} problems; {} skipped on purpose",
        t.strong, t.possible, t.weak, t.inconclusive, t.cloud_only, t.issues, t.skipped
    );
    for c in r.candidates.iter().take(15) {
        println!("  #{} [{}] {} — {}", c.rank, c.confidence.as_str(), c.name, c.reason);
    }
    println!("scan file: {}\nreport:    {}", scan_path.display(), report.display());
    Ok(())
}
