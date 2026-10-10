//! The tree every Grasshopper archive decodes to, whether it came from a
//! binary `.gh`/`.ghuser` or an XML `.ghx`: named chunks holding named items.

#[derive(Debug, Clone, PartialEq)]
pub struct Chunk {
    pub name: String,
    pub index: Option<i32>,
    pub items: Vec<Item>,
    pub chunks: Vec<Chunk>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub name: String,
    pub index: Option<i32>,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    /// Lowercase, hyphenated, in .NET's textual order.
    Guid(String),
    Bytes(Vec<u8>),
    Version(i32, i32, i32),
    Date(i64),
    /// A rectangle on the canvas: x, y, width, height.
    Rect([f32; 4]),
    /// A point on the canvas: x, y.
    Point([f32; 2]),
    /// A colour, as .NET packs it: alpha, red, green, blue from the high byte down.
    Color(u32),
    /// An image, such as the canvas preview Grasshopper saves in every file.
    Bitmap(Vec<u8>),
    /// A value type this reader keeps no payload for (3D points, planes, intervals…).
    Other(i32),
}

/// GH_IO type codes, shared by the binary and XML formats.
pub mod types {
    pub const BOOL: i32 = 1;
    pub const BYTE: i32 = 2;
    pub const INT32: i32 = 3;
    pub const INT64: i32 = 4;
    pub const SINGLE: i32 = 5;
    pub const DOUBLE: i32 = 6;
    pub const DECIMAL: i32 = 7;
    pub const DATE: i32 = 8;
    pub const GUID: i32 = 9;
    pub const STRING: i32 = 10;
    pub const BYTE_ARRAY: i32 = 20;
    pub const DOUBLE_ARRAY: i32 = 21;
    pub const POINT: i32 = 30;
    pub const POINTF: i32 = 31;
    pub const RECT: i32 = 34;
    pub const RECTF: i32 = 35;
    pub const COLOR: i32 = 36;
    pub const BITMAP: i32 = 37;
    pub const VERSION: i32 = 80;

    /// Byte width of the fixed-size types; variable-size ones are handled by the readers.
    pub fn fixed_size(code: i32) -> Option<usize> {
        Some(match code {
            1 | 2 => 1,
            3 | 5 | 36 => 4,
            4 | 6 | 8 | 30 | 31 | 32 | 33 => 8,
            7 | 9 | 34 | 35 | 50 | 60 => 16,
            51 => 24,
            52 | 61 => 32,
            70 | 71 => 48,
            72 => 72,
            80 => 12,
            _ => return None,
        })
    }
}

impl Chunk {
    pub fn new(name: impl Into<String>) -> Self {
        Chunk { name: name.into(), index: None, items: Vec::new(), chunks: Vec::new() }
    }

    pub fn item(&self, name: &str) -> Option<&Item> {
        self.items.iter().find(|i| i.name.eq_ignore_ascii_case(name))
    }

    pub fn items_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Item> + 'a {
        self.items.iter().filter(move |i| i.name.eq_ignore_ascii_case(name))
    }

    pub fn str(&self, name: &str) -> Option<&str> {
        match &self.item(name)?.value {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn guid(&self, name: &str) -> Option<&str> {
        match &self.item(name)?.value {
            Value::Guid(g) => Some(g.as_str()),
            _ => None,
        }
    }

    pub fn number(&self, name: &str) -> Option<f64> {
        match &self.item(name)?.value {
            Value::Int(n) => Some(*n as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        match &self.item(name)?.value {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn rect(&self, name: &str) -> Option<[f32; 4]> {
        match &self.item(name)?.value {
            Value::Rect(r) => Some(*r),
            _ => None,
        }
    }

    pub fn color(&self, name: &str) -> Option<u32> {
        match &self.item(name)?.value {
            Value::Color(c) => Some(*c),
            _ => None,
        }
    }

    /// Where this object or parameter sits on the canvas, from its `Attributes`.
    pub fn canvas_bounds(&self) -> Option<[f32; 4]> {
        self.chunk("Attributes")?
            .rect("Bounds")
            .filter(|r| r.iter().all(|v| v.is_finite()) && r[2] >= 0.0 && r[3] >= 0.0)
    }

    pub fn chunk(&self, name: &str) -> Option<&Chunk> {
        self.chunks.iter().find(|c| c.name.eq_ignore_ascii_case(name))
    }

    pub fn chunks_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Chunk> + 'a {
        self.chunks.iter().filter(move |c| c.name.eq_ignore_ascii_case(name))
    }

    /// Depth-first visit of this chunk and everything under it.
    pub fn walk<'a>(&'a self, visit: &mut dyn FnMut(&'a Chunk, usize)) {
        fn go<'a>(c: &'a Chunk, depth: usize, visit: &mut dyn FnMut(&'a Chunk, usize)) {
            visit(c, depth);
            for child in &c.chunks {
                go(child, depth + 1, visit);
            }
        }
        go(self, 0, visit);
    }
}

/// Formats 16 GUID bytes the way .NET writes them (first three groups little-endian).
pub fn guid_from_dotnet_bytes(b: &[u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        b[3], b[2], b[1], b[0], b[5], b[4], b[7], b[6], b[8], b[9], b[10], b[11], b[12], b[13], b[14], b[15]
    )
}

/// The inverse of [`guid_from_dotnet_bytes`]; `None` for anything that isn't a GUID.
pub fn guid_to_dotnet_bytes(s: &str) -> Option<[u8; 16]> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let mut raw = [0u8; 16];
    for (i, byte) in raw.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).ok()?;
    }
    let mut b = raw;
    b[0..4].reverse();
    b[4..6].reverse();
    b[6..8].reverse();
    Some(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_round_trips_through_dotnet_byte_order() {
        let s = "6bb8ff08-9dd5-cccd-7191-72c85ccf8b6a";
        let bytes = guid_to_dotnet_bytes(s).unwrap();
        assert_eq!(bytes[0], 0x08);
        assert_eq!(guid_from_dotnet_bytes(&bytes), s);
    }
}
