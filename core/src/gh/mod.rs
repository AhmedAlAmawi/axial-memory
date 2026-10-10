//! Reading Grasshopper archives without Rhino.

pub mod archive;
pub mod binary;
pub mod digest;
#[doc(hidden)]
pub mod write;
pub mod xml;

pub use archive::{Chunk, Item, Value};
pub use digest::{Digest, Summary, TextSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Binary,
    Xml,
}

/// Decodes a Grasshopper file's bytes into its archive tree.
pub fn read_archive(bytes: &[u8]) -> Result<(Chunk, Format), String> {
    let head = &bytes[..bytes.len().min(512)];
    if xml::sniff(head) {
        return xml::parse(bytes).map(|c| (c, Format::Xml));
    }
    match binary::parse_compressed(bytes) {
        Ok(c) => Ok((c, Format::Binary)),
        Err(binary::BinaryError::NotCompressed) => binary::parse_raw(bytes)
            .map(|(c, _)| (c, Format::Binary))
            .map_err(|_| binary::BinaryError::NotCompressed.to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Reads a file's bytes all the way to a digest. `ext` is the file's extension
/// without the dot, used only to label the format.
pub fn read_digest(bytes: &[u8], ext: &str) -> Result<Digest, String> {
    let (root, format) = read_archive(bytes)?;
    let label = match (format, ext) {
        (Format::Xml, _) => "ghx",
        (Format::Binary, "ghuser") => "ghuser",
        (Format::Binary, "ghcluster") => "ghcluster",
        _ => "gh",
    };
    Ok(digest::build(&root, label))
}

/// True if a file with no telling name starts like a Grasshopper archive.
pub fn sniff(head: &[u8]) -> bool {
    xml::sniff(head) || binary::sniff(head)
}

#[cfg(test)]
mod tests {
    use super::digest::TextSource;
    use super::write::build::Definition;
    use super::*;

    fn sample() -> Chunk {
        let mut def = Definition::new("palmscape_v3.gh");
        def.slider(10, "Count", 144.0)
            .component("Series", 20, "Ser", &[10])
            .component("Multiplication", 30, "golden angle", &[20])
            .panel(40, "phyllotaxis: 137.5 degrees between seeds")
            .group(50, "Palm stalk generator", &[20, 30])
            .python(60, "spiral", "import rhinoscriptsyntax as rs\nfor i in range(n):\n    rs.AddPoint(i, 0, 0)\n")
            .library("Kangaroo Components", "2.42");
        def.archive()
    }

    fn check(d: &Digest) {
        assert_eq!(d.name.as_deref(), Some("palmscape_v3.gh"));
        assert_eq!(d.gh_version.as_deref(), Some("1.0.7"));
        assert_eq!(d.objects.len(), 6);
        assert_eq!(d.objects[0].detail.as_deref(), Some("144 (range 0 to 500)"));
        assert_eq!(d.edges, vec![(0, 1), (1, 2)]);
        assert_eq!(d.groups.len(), 1);
        assert_eq!(d.groups[0].name, "Palm stalk generator");
        assert_eq!(d.groups[0].members, vec![1, 2]);
        assert_eq!(d.plugins[0].name, "Kangaroo Components");
        assert_eq!(d.scripts.len(), 1);
        assert_eq!(d.scripts[0].language, "python");
        let has = |src: TextSource, needle: &str| d.texts.iter().any(|t| t.source == src && t.text.contains(needle));
        assert!(has(TextSource::Panel, "phyllotaxis"));
        assert!(has(TextSource::Group, "Palm stalk"));
        assert!(has(TextSource::Nickname, "golden angle"));
        assert!(has(TextSource::Definition, "palmscape"));
        assert!(has(TextSource::Script, "rhinoscriptsyntax"));
        assert_eq!(digest::dataflow_order(d)[..3], [0, 1, 2]);
    }

    #[test]
    fn binary_round_trip() {
        let bytes = write::to_gh(&sample());
        let d = read_digest(&bytes, "gh").unwrap();
        assert_eq!(d.format, "gh");
        check(&d);
    }

    #[test]
    fn xml_round_trip() {
        let text = write::to_ghx(&sample());
        let d = read_digest(text.as_bytes(), "ghx").unwrap();
        assert_eq!(d.format, "ghx");
        check(&d);
    }

    #[test]
    fn same_tree_from_both_formats() {
        let root = sample();
        let (from_gh, _) = read_archive(&write::to_gh(&root)).unwrap();
        let (from_ghx, _) = read_archive(write::to_ghx(&root).as_bytes()).unwrap();
        assert_eq!(from_gh, root);
        assert_eq!(from_ghx, root);
    }

    #[test]
    fn sniffs_both_formats_and_rejects_noise() {
        let root = sample();
        let gh = write::to_gh(&root);
        assert!(sniff(&gh[..gh.len().min(4096)]));
        assert!(sniff(&write::to_ghx(&root).as_bytes()[..200]));
        assert!(!sniff(b"just some text that is not an archive at all, really"));
        assert!(!sniff(&[0u8; 64]));
    }

    #[test]
    fn damaged_files_are_errors_not_panics() {
        let mut bytes = write::to_gh(&sample());
        bytes.truncate(bytes.len() / 2);
        assert!(read_digest(&bytes, "gh").is_err());
        assert!(read_digest(b"PK\x03\x04 not a gh", "gh").is_err());
        assert!(read_digest(b"<?xml version=\"1.0\"?><Archive name=\"Root\"><items", "ghx").is_err());
        for seed in 0..200u32 {
            let noise: Vec<u8> = (0..300).map(|i| ((i * 31 + seed * 17) % 251) as u8).collect();
            let _ = read_digest(&noise, "gh");
        }
    }

    #[test]
    fn cluster_contents_are_read() {
        let inner = {
            let mut def = Definition::new("inner");
            def.group(5, "Frond spacing", &[]);
            def.archive()
        };
        let mut outer = Definition::new("outer.gh");
        let mut obj = Chunk::new("Object");
        obj.items.push(write::build::item("Name", write::build::s("Cluster")));
        let mut c = Chunk::new("Container");
        c.items.push(write::build::item("InstanceGuid", write::build::g(&write::build::guid(99))));
        c.items.push(write::build::item("ClusterDocument", Value::Bytes(write::to_gh(&inner))));
        obj.chunks.push(c);
        outer.objects.push(obj);
        let d = read_digest(&write::to_gh(&outer.archive()), "gh").unwrap();
        assert_eq!(d.nested_archives, 1);
        assert!(d.texts.iter().any(|t| t.source == TextSource::Group && t.text == "Frond spacing" && t.in_cluster));
        assert_eq!(d.summary().objects, 1);
    }
}
