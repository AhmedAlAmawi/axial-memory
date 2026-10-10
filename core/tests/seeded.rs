//! The spec's acceptance test: every .gh/.ghx in a seeded folder is listed,
//! nothing is altered, every unreadable place is reported, duplicates are
//! grouped, and every palm match points at concrete evidence.

use axial_core::gh::write::{build::Definition, to_gh, to_ghx};
use axial_core::model::{GhRead, IssueKind, Kind, Machine, ScanData, ScanOptions, Status};
use axial_core::roots::Root;
use axial_core::scan::{self, ScanRequest};
use axial_core::terms::{default_terms, Confidence, Source};
use axial_core::{export, results, store};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

fn put(root: &Path, rel: &str, bytes: &[u8]) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, bytes).unwrap();
}

fn definition(name: &str, group: &str, panel: &str) -> Vec<u8> {
    let mut d = Definition::new(name);
    d.slider(10, "Count", 144.0).component("Series", 20, "Ser", &[10]).group(30, group, &[20]);
    if !panel.is_empty() {
        d.panel(40, panel);
    }
    to_gh(&d.archive())
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in entries {
            z.start_file(*name, opts).unwrap();
            z.write_all(bytes).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

/// Seeds the folder and returns the definitions a complete scan must list.
fn seed(root: &Path) -> Vec<String> {
    let palm_v1 = definition("palm_v1.gh", "Trunk", "");
    let palm_v2 = definition("palm_v2.gh", "Phyllotaxis spiral", "");
    put(root, "Projects/Palmscapes 2023/palm_v1.gh", &palm_v1);
    put(root, "Projects/Palmscapes 2023/palm_v2_final.gh", &palm_v2);
    put(root, "Projects/Palmscapes 2023/palm_v2_final copy.gh", &palm_v2);
    put(root, "Projects/Palmscapes 2023/model.3dm", b"3D Geometry File Format  fake model");
    put(root, "Projects/Palmscapes 2023/notes.pdf", b"%PDF-1.4 not about anything");
    put(root, "Projects/unrelated/chair.gh", &definition("chair.gh", "Legs", ""));
    let chair_x = {
        let mut d = Definition::new("chair.ghx");
        d.panel(40, "phyllotaxis study for the seat pattern");
        to_ghx(&d.archive())
    };
    put(root, "Projects/unrelated/chair.ghx", chair_x.as_bytes());
    let tower = {
        let mut d = Definition::new("tower");
        d.slider(1, "Floors", 30.0);
        to_ghx(&d.archive())
    };
    put(root, "Projects/unrelated/tower.GHX", tower.as_bytes());
    put(root, ".hidden/secret_stalks.gh", &definition("s.gh", "Beams", ""));
    put(root, "deep/a/b/c/d/e/f/g/h/i/j/deep.gh", &definition("deep.gh", "Deep", ""));
    put(root, "Ünïcode 中文/نخيل.gh", &definition("n.gh", "Base", ""));
    let cafe = {
        let mut d = Definition::new("café.gh");
        d.python(5, "growth", "import math\n# fibonacci growth\nfor i in range(10):\n    x = math.sqrt(i)\n");
        to_gh(&d.archive())
    };
    put(root, "Ünïcode 中文/café.gh", &cafe);
    put(root, "backups/palm_v1.gh.bak", &palm_v1);
    put(root, "backups/mystery.tmp", &definition("mystery.gh", "Lost work", ""));
    put(root, "backups/notgh.tmp", b"just a temp file that is not grasshopper at all, nothing to see here");
    let frond = definition("frond.gh", "Frond", "");
    let nested = zip_of(&[("deep_zip.gh", &definition("deep_zip.gh", "Inner", ""))]);
    put(root, "archive.zip", &zip_of(&[("inner/frond.gh", &frond), ("readme.txt", b"hello"), ("nested.zip", &nested)]));
    put(root, "unrelated.zip", &zip_of(&[("photo.jpg", b"jpeg bytes")]));
    put(root, "broken.gh", &[0x13, 0x37, 0xde, 0xad, 0xbe, 0xef, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    put(root, "fake.gh", b"hello, I am not a grasshopper file");
    put(root, "scripts/near.gh", &definition("near.gh", "Helpers", ""));
    put(root, "scripts/helper.py", b"def helper():\n    return 42\n");
    put(root, "lonely/helper2.py", b"def nothing():\n    pass\n");

    [
        "Projects/Palmscapes 2023/palm_v1.gh",
        "Projects/Palmscapes 2023/palm_v2_final.gh",
        "Projects/Palmscapes 2023/palm_v2_final copy.gh",
        "Projects/unrelated/chair.gh",
        "Projects/unrelated/chair.ghx",
        "Projects/unrelated/tower.GHX",
        ".hidden/secret_stalks.gh",
        "deep/a/b/c/d/e/f/g/h/i/j/deep.gh",
        "Ünïcode 中文/نخيل.gh",
        "Ünïcode 中文/café.gh",
        "backups/palm_v1.gh.bak",
        "backups/mystery.tmp",
        "archive.zip!/inner/frond.gh",
        "archive.zip!/nested.zip!/deep_zip.gh",
        "broken.gh",
        "fake.gh",
        "scripts/near.gh",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn fingerprints(root: &Path) -> BTreeMap<PathBuf, (Vec<u8>, std::time::SystemTime)> {
    walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            let bytes = std::fs::read(e.path()).unwrap();
            let modified = e.metadata().unwrap().modified().unwrap();
            (e.path().to_path_buf(), (bytes, modified))
        })
        .collect()
}

fn scan_folder(root: &Path) -> ScanData {
    let req = ScanRequest {
        roots: vec![Root::folder(root.to_str().unwrap())],
        options: ScanOptions::default(),
        terms: default_terms(),
        machine: Machine::current("test-machine".into(), Some("Test laptop".into())),
        app_version: "test".into(),
    };
    scan::run(&req, &AtomicBool::new(false), &mut |_| {})
}

fn rel(root: &Path, path: &str) -> String {
    let prefix = format!("{}{}", root.display(), std::path::MAIN_SEPARATOR);
    path.strip_prefix(&prefix).unwrap_or(path).replace('\\', "/")
}

#[test]
fn seeded_folder_meets_the_acceptance_criteria() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("seed");
    let expected = seed(&root);

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&root, root.join("Projects/loop")).unwrap();
        put(&root, "locked/hidden.gh", &definition("hidden.gh", "Locked", ""));
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
    }

    let before = fingerprints(&root);
    let data = scan_folder(&root);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    // 1. Every definition in the folder is listed.
    let mut listed: Vec<String> =
        data.files.iter().filter(|f| f.kind == Kind::Definition).map(|f| rel(&root, &f.path)).collect();
    listed.sort();
    let mut want = expected.clone();
    want.sort();
    assert_eq!(listed, want);

    // 2. No originals altered, nothing added.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o000)).unwrap();
    }
    let after = fingerprints(&root);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(root.join("locked"), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    assert_eq!(before, after);

    // 3. Every place it couldn't look is reported.
    #[cfg(unix)]
    if !is_root_user() {
        assert!(data.issues.iter().any(|i| i.kind == IssueKind::PermissionDenied && i.path.ends_with("locked")));
    }
    let damaged: Vec<String> =
        data.issues.iter().filter(|i| i.kind == IssueKind::Damaged).map(|i| rel(&root, &i.path)).collect();
    assert!(damaged.contains(&"broken.gh".to_string()) && damaged.contains(&"fake.gh".to_string()));

    // Scripts only next to definitions; unrelated zips and documents left out.
    let names: Vec<&str> = data.files.iter().map(|f| f.name.as_str()).collect();
    assert!(names.contains(&"helper.py") && !names.contains(&"helper2.py"));
    assert!(names.contains(&"model.3dm") && !names.contains(&"notes.pdf") && !names.contains(&"unrelated.zip"));
    assert!(data.files.iter().any(|f| f.name == "archive.zip" && f.kind == Kind::Archive));

    // The scan file round-trips exactly.
    let out = tempfile::tempdir().unwrap();
    let scan_file = out.path().join(format!("test.{}", store::EXTENSION));
    store::write(&scan_file, &data).unwrap();
    assert_eq!(store::read(&scan_file).unwrap(), data);

    // 4. Duplicates grouped, across names and backup copies.
    let r = results::build(std::slice::from_ref(&data), &default_terms(), "test-machine");
    let cluster_of = |name: &str| {
        r.clusters
            .iter()
            .find(|c| c.files.iter().any(|&id| r.files[id as usize].path.ends_with(name)))
            .map(|c| c.files.len())
    };
    assert_eq!(cluster_of("palm_v2_final copy.gh"), Some(2));
    assert_eq!(cluster_of("palm_v1.gh.bak"), Some(2));
    assert_eq!(r.totals.definitions, 17);
    assert_eq!(r.totals.unique_definitions, 15);

    // 5. Palm matches, each traceable to evidence.
    let candidate = |name: &str| {
        r.candidates
            .iter()
            .find(|c| c.files.iter().any(|&id| r.files[id as usize].path.ends_with(name)))
            .unwrap_or_else(|| panic!("{name} not a candidate"))
    };
    let v2 = candidate("palm_v2_final.gh");
    assert_eq!(v2.confidence, Confidence::Strong);
    assert!(v2.evidence.iter().any(|e| e.source == Source::Group && e.text.contains("Phyllotaxis spiral")));
    assert!(v2.evidence.iter().any(|e| e.source == Source::Folder && e.term == "Palmscapes"));
    assert_eq!(candidate("chair.ghx").confidence, Confidence::Strong);
    assert!(candidate("chair.ghx").evidence.iter().any(|e| e.source == Source::Panel));
    assert_eq!(candidate("secret_stalks.gh").confidence, Confidence::Possible);
    assert_eq!(candidate("نخيل.gh").confidence, Confidence::Possible);
    assert_eq!(candidate("café.gh").confidence, Confidence::Weak);
    assert!(candidate("café.gh").evidence.iter().any(|e| e.source == Source::Script && e.term == "fibonacci"));
    assert!(candidate("archive.zip!/inner/frond.gh").evidence.iter().any(|e| e.term == "frond"));
    assert_eq!(candidate("broken.gh").confidence, Confidence::Inconclusive);
    assert_eq!(candidate("fake.gh").confidence, Confidence::Inconclusive);
    for c in &r.candidates {
        assert!(c.confidence == Confidence::Inconclusive || !c.evidence.is_empty(), "{} has no evidence", c.name);
    }
    assert!(r.candidates.iter().all(|c| !c.name.contains("tower") && !c.name.contains("deep.gh")));

    // Everything readable was read; the sniffed temp file is a real definition.
    let mystery = data.files.iter().find(|f| f.name == "mystery.tmp").unwrap();
    assert!(mystery.tags.contains(&"renamed_grasshopper".to_string()) && mystery.status == Status::Read);
    let parsed = |name: &str| {
        let f = data.files.iter().find(|f| f.name == name).unwrap();
        data.contents.iter().find(|c| Some(&c.sha256) == f.sha256.as_ref()).map(|c| c.read)
    };
    assert_eq!(parsed("deep_zip.gh"), Some(GhRead::Parsed));
    assert_eq!(parsed("fake.gh"), Some(GhRead::Inconclusive));

    // The report folder has every file the spec names, and the master catalog lists all 17.
    let report = export::write_report(&r, out.path(), &[]).unwrap();
    for f in [
        "ALL_GRASSHOPPER_FILES.csv",
        "PALM_CANDIDATES.csv",
        "PALM_EVIDENCE.csv",
        "ACCESS_ERRORS.csv",
        "COVERAGE.csv",
        "DUPLICATE_CLUSTERS.csv",
        "RELATED_FILES.csv",
        "REVIEW_SET.json",
        "axial_memory.sqlite",
        "README.txt",
    ] {
        assert!(report.join(f).exists(), "{f} missing");
    }
    let catalog = std::fs::read_to_string(report.join("ALL_GRASSHOPPER_FILES.csv")).unwrap();
    assert_eq!(catalog.lines().count(), 1 + 17);
}

#[test]
fn two_laptops_merge_and_compare() {
    let dir = tempfile::tempdir().unwrap();
    let mac = dir.path().join("mac");
    let win = dir.path().join("win");
    let shared = definition("shared.gh", "Phyllotaxis", "");
    put(&mac, "Thesis/shared.gh", &shared);
    put(&mac, "Thesis/mac_only.gh", &definition("m.gh", "A", ""));
    put(&win, "Desktop/shared (1).gh", &shared);
    put(&win, "Desktop/win_only.gh", &definition("w.gh", "B", ""));
    let mut a = scan_folder(&mac);
    a.meta.machine.id = "mac".into();
    let mut b = scan_folder(&win);
    b.meta.machine.id = "win".into();
    let r = results::build(&[a, b], &default_terms(), "mac");
    let cmp = r.comparison.unwrap();
    assert_eq!((cmp.only_a, cmp.only_b, cmp.both), (1, 1, 1));
    let cluster = r.clusters.iter().find(|c| c.files.len() == 2).unwrap();
    assert_eq!(cluster.machines, vec![0, 1]);
    let c = r.candidates.iter().find(|c| c.name.starts_with("shared")).unwrap();
    assert_eq!(c.machines, vec![0, 1]);
    assert!(r.machines[0].this_computer && !r.machines[1].this_computer);
}

#[test]
fn stopping_reports_a_partial_scan() {
    let dir = tempfile::tempdir().unwrap();
    put(dir.path(), "a/palm.gh", &definition("palm.gh", "A", ""));
    let req = ScanRequest {
        roots: vec![Root::folder(dir.path().to_str().unwrap())],
        options: ScanOptions::default(),
        terms: default_terms(),
        machine: Machine::current("m".into(), None),
        app_version: "test".into(),
    };
    let data = scan::run(&req, &AtomicBool::new(true), &mut |_| {});
    assert_eq!(data.meta.status, axial_core::model::ScanStatus::Stopped);
    assert_eq!(data.coverage[0].status, axial_core::model::CoverageStatus::Stopped);
}

#[cfg(unix)]
fn is_root_user() -> bool {
    std::env::var("USER").map(|u| u == "root").unwrap_or(false)
}
