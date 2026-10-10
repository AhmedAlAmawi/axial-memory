//! XML GH_IO archives (`.ghx`): the same chunk/item tree as the binary format,
//! written as `<Archive>`, `<chunk name index>` and `<item name index type_code>`.

use super::archive::{types, Chunk, Item, Value};
use base64::Engine;
use roxmltree::{Document, Node, ParsingOptions};

const MAX_DEPTH: usize = 96;

/// True if these bytes start like a `.ghx` file.
pub fn sniff(head: &[u8]) -> bool {
    let text = String::from_utf8_lossy(head);
    let t = text.trim_start_matches('\u{feff}').trim_start();
    (t.starts_with("<?xml") || t.starts_with("<Archive")) && t.contains("<Archive")
}

pub fn parse(bytes: &[u8]) -> Result<Chunk, String> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.trim_start_matches('\u{feff}');
    let opts = ParsingOptions { allow_dtd: false, ..ParsingOptions::default() };
    let doc = Document::parse_with_options(text, opts).map_err(|e| format!("its text is damaged ({e})"))?;
    let root = doc.root_element();
    if !root.tag_name().name().eq_ignore_ascii_case("Archive") {
        return Err("it's XML, but not a Grasshopper file".into());
    }
    chunk_from(root, 0)
}

fn chunk_from(node: Node, depth: usize) -> Result<Chunk, String> {
    if depth > MAX_DEPTH {
        return Err("archive nests deeper than the reader allows".into());
    }
    let mut chunk = Chunk::new(node.attribute("name").unwrap_or("Root"));
    chunk.index = node.attribute("index").and_then(|s| s.trim().parse().ok());
    for child in node.children().filter(Node::is_element) {
        let tag = child.tag_name().name();
        if tag.eq_ignore_ascii_case("items") {
            for item in child.children().filter(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("item"))
            {
                chunk.items.push(item_from(item));
            }
        } else if tag.eq_ignore_ascii_case("chunks") {
            for sub in child.children().filter(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("chunk"))
            {
                chunk.chunks.push(chunk_from(sub, depth + 1)?);
            }
        }
    }
    Ok(chunk)
}

fn all_text(node: Node) -> String {
    node.descendants().filter(Node::is_text).filter_map(|n| n.text()).collect()
}

fn child_text(node: Node, name: &str) -> Option<String> {
    node.children().find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case(name)).map(all_text)
}

fn item_from(node: Node) -> Item {
    let name = node.attribute("name").unwrap_or("").to_string();
    let index = node.attribute("index").and_then(|s| s.trim().parse().ok());
    let code: i32 = node.attribute("type_code").and_then(|s| s.trim().parse().ok()).unwrap_or(0);
    let text = || all_text(node);
    let value = match code {
        types::STRING => Value::Str(text()),
        types::GUID => Value::Guid(text().trim().to_ascii_lowercase()),
        types::BOOL => Value::Bool(text().trim().eq_ignore_ascii_case("true")),
        types::BYTE | types::INT32 | types::INT64 => {
            text().trim().parse().map(Value::Int).unwrap_or(Value::Other(code))
        }
        types::SINGLE | types::DOUBLE | types::DECIMAL => {
            text().trim().parse().map(Value::Float).unwrap_or(Value::Other(code))
        }
        types::DATE => text().trim().parse().map(Value::Date).unwrap_or(Value::Other(code)),
        types::VERSION => {
            let part = |n: &str| child_text(node, n).and_then(|t| t.trim().parse().ok()).unwrap_or(0);
            Value::Version(part("Major"), part("Minor"), part("Revision"))
        }
        types::BYTE_ARRAY | types::BITMAP => {
            let compact: String = text().chars().filter(|c| !c.is_whitespace()).collect();
            match base64::engine::general_purpose::STANDARD.decode(compact.as_bytes()) {
                Ok(b) if code == types::BITMAP => Value::Bitmap(b),
                Ok(b) => Value::Bytes(b),
                Err(_) => Value::Other(code),
            }
        }
        types::POINT | types::POINTF => {
            let part = |n: &str| child_text(node, n).and_then(|t| t.trim().parse::<f32>().ok());
            match (part("X"), part("Y")) {
                (Some(x), Some(y)) => Value::Point([x, y]),
                _ => Value::Other(code),
            }
        }
        types::RECT | types::RECTF => {
            let part = |n: &str| child_text(node, n).and_then(|t| t.trim().parse::<f32>().ok());
            match (part("X"), part("Y"), part("W"), part("H")) {
                (Some(x), Some(y), Some(w), Some(h)) => Value::Rect([x, y, w, h]),
                _ => Value::Other(code),
            }
        }
        types::COLOR => {
            let argb: Vec<u32> = child_text(node, "ARGB")
                .map(|t| t.split(';').filter_map(|p| p.trim().parse().ok()).collect())
                .unwrap_or_default();
            match argb[..] {
                [a, r, g, b] => Value::Color((a & 0xff) << 24 | (r & 0xff) << 16 | (g & 0xff) << 8 | (b & 0xff)),
                _ => Value::Other(code),
            }
        }
        other => Value::Other(other),
    };
    Item { name, index, value }
}
