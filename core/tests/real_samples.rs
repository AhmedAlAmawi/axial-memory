//! Reads real Grasshopper files from `AXIAL_SAMPLES_DIR` (not committed: they
//! belong to other people). Skips when the variable isn't set.

use axial_core::gh;
use std::path::Path;

#[test]
fn real_files_read_cleanly() {
    let Ok(dir) = std::env::var("AXIAL_SAMPLES_DIR") else {
        eprintln!("AXIAL_SAMPLES_DIR not set; skipping");
        return;
    };
    let mut read = 0;
    for entry in std::fs::read_dir(Path::new(&dir)).unwrap() {
        let path = entry.unwrap().path();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if !["gh", "ghx", "ghuser"].contains(&ext.as_str()) {
            continue;
        }
        let bytes = std::fs::read(&path).unwrap();
        let d = gh::read_digest(&bytes, &ext).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let s = d.summary();
        eprintln!(
            "{}: {} objects, {} wires, groups {:?}, plugins {:?}, {} scripts, {} texts, nested {}",
            path.file_name().unwrap().to_string_lossy(),
            s.objects,
            d.edges.len(),
            s.groups,
            s.plugins,
            s.scripts,
            d.texts.len(),
            s.nested_archives
        );
        read += 1;
    }
    assert!(read > 0, "no Grasshopper files in {dir}");
}
