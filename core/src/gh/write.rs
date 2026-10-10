//! Writes archives in both formats. The app never writes Grasshopper files;
//! this exists so tests can build seeded definitions without Rhino.

use super::archive::{guid_to_dotnet_bytes, types, Chunk, Item, Value};
use base64::Engine;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use std::io::Write;

pub fn to_raw(root: &Chunk) -> Vec<u8> {
    let mut out = Vec::new();
    write_chunk(&mut out, root);
    out
}

/// The bytes of a `.gh` file.
pub fn to_gh(root: &Chunk) -> Vec<u8> {
    let mut enc = DeflateEncoder::new(Vec::new(), Compression::default());
    enc.write_all(&to_raw(root)).unwrap();
    enc.finish().unwrap()
}

/// The text of a `.ghx` file.
pub fn to_ghx(root: &Chunk) -> String {
    let mut s = String::from("\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\" standalone=\"yes\"?>\n");
    s.push_str(&format!("<Archive name=\"{}\">\n", esc(&root.name)));
    write_body(&mut s, root, 1);
    s.push_str("</Archive>\n");
    s
}

fn write_string(out: &mut Vec<u8>, s: &str) {
    let mut n = s.len();
    loop {
        let mut byte = (n & 0x7f) as u8;
        n >>= 7;
        if n != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if n == 0 {
            break;
        }
    }
    out.extend_from_slice(s.as_bytes());
}

fn write_chunk(out: &mut Vec<u8>, c: &Chunk) {
    write_string(out, &c.name);
    out.extend_from_slice(&c.index.unwrap_or(-1).to_le_bytes());
    out.extend_from_slice(&(c.items.len() as i32).to_le_bytes());
    out.extend_from_slice(&(c.chunks.len() as i32).to_le_bytes());
    for item in &c.items {
        write_item(out, item);
    }
    for sub in &c.chunks {
        write_chunk(out, sub);
    }
}

fn write_item(out: &mut Vec<u8>, item: &Item) {
    write_string(out, &item.name);
    out.extend_from_slice(&item.index.unwrap_or(-1).to_le_bytes());
    match &item.value {
        Value::Bool(b) => {
            out.extend_from_slice(&types::BOOL.to_le_bytes());
            out.push(*b as u8);
        }
        Value::Int(n) => match i32::try_from(*n) {
            Ok(small) => {
                out.extend_from_slice(&types::INT32.to_le_bytes());
                out.extend_from_slice(&small.to_le_bytes());
            }
            Err(_) => {
                out.extend_from_slice(&types::INT64.to_le_bytes());
                out.extend_from_slice(&n.to_le_bytes());
            }
        },
        Value::Float(f) => {
            out.extend_from_slice(&types::DOUBLE.to_le_bytes());
            out.extend_from_slice(&f.to_le_bytes());
        }
        Value::Str(s) => {
            out.extend_from_slice(&types::STRING.to_le_bytes());
            write_string(out, s);
        }
        Value::Guid(g) => {
            out.extend_from_slice(&types::GUID.to_le_bytes());
            out.extend_from_slice(&guid_to_dotnet_bytes(g).unwrap_or([0; 16]));
        }
        Value::Bytes(b) => {
            out.extend_from_slice(&types::BYTE_ARRAY.to_le_bytes());
            out.extend_from_slice(&(b.len() as i32).to_le_bytes());
            out.extend_from_slice(b);
        }
        Value::Version(a, b, c) => {
            out.extend_from_slice(&types::VERSION.to_le_bytes());
            for part in [a, b, c] {
                out.extend_from_slice(&part.to_le_bytes());
            }
        }
        Value::Date(t) => {
            out.extend_from_slice(&types::DATE.to_le_bytes());
            out.extend_from_slice(&t.to_le_bytes());
        }
        Value::Rect(r) => {
            out.extend_from_slice(&types::RECTF.to_le_bytes());
            for v in r {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        Value::Point(p) => {
            out.extend_from_slice(&types::POINTF.to_le_bytes());
            for v in p {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        Value::Color(c) => {
            out.extend_from_slice(&types::COLOR.to_le_bytes());
            out.extend_from_slice(&c.to_le_bytes());
        }
        Value::Bitmap(b) => {
            out.extend_from_slice(&types::BITMAP.to_le_bytes());
            out.extend_from_slice(&(b.len() as i32).to_le_bytes());
            out.extend_from_slice(b);
        }
        Value::Other(code) => {
            out.extend_from_slice(&code.to_le_bytes());
            out.extend(std::iter::repeat_n(0u8, types::fixed_size(*code).unwrap_or(0)));
        }
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn index_attr(index: Option<i32>) -> String {
    index.map(|i| format!(" index=\"{i}\"")).unwrap_or_default()
}

fn write_body(s: &mut String, c: &Chunk, depth: usize) {
    let pad = "  ".repeat(depth);
    s.push_str(&format!("{pad}<items count=\"{}\">\n", c.items.len()));
    for item in &c.items {
        let (type_name, code, body) = match &item.value {
            Value::Bool(b) => ("gh_bool", types::BOOL, b.to_string()),
            Value::Int(n) => ("gh_int32", types::INT32, n.to_string()),
            Value::Float(f) => ("gh_double", types::DOUBLE, f.to_string()),
            Value::Str(t) => ("gh_string", types::STRING, esc(t)),
            Value::Guid(g) => ("gh_guid", types::GUID, g.clone()),
            Value::Bytes(b) => (
                "gh_bytearray",
                types::BYTE_ARRAY,
                format!("<stream>{}</stream>", base64::engine::general_purpose::STANDARD.encode(b)),
            ),
            Value::Version(a, b, c) => {
                ("gh_version", types::VERSION, format!("<Major>{a}</Major><Minor>{b}</Minor><Revision>{c}</Revision>"))
            }
            Value::Date(t) => ("gh_date", types::DATE, t.to_string()),
            Value::Rect([x, y, w, h]) => {
                ("gh_drawing_rectanglef", types::RECTF, format!("<X>{x}</X><Y>{y}</Y><W>{w}</W><H>{h}</H>"))
            }
            Value::Point([x, y]) => ("gh_drawing_pointf", types::POINTF, format!("<X>{x}</X><Y>{y}</Y>")),
            Value::Color(c) => (
                "gh_drawing_color",
                types::COLOR,
                format!("<ARGB>{};{};{};{}</ARGB>", c >> 24, (c >> 16) & 0xff, (c >> 8) & 0xff, c & 0xff),
            ),
            Value::Bitmap(b) => (
                "gh_drawing_bitmap",
                types::BITMAP,
                format!(
                    "<bitmap length=\"{}\">{}</bitmap>",
                    b.len(),
                    base64::engine::general_purpose::STANDARD.encode(b)
                ),
            ),
            Value::Other(code) => ("gh_unknown", *code, String::new()),
        };
        s.push_str(&format!(
            "{pad}  <item name=\"{}\"{} type_name=\"{type_name}\" type_code=\"{code}\">{body}</item>\n",
            esc(&item.name),
            index_attr(item.index)
        ));
    }
    s.push_str(&format!("{pad}</items>\n"));
    s.push_str(&format!("{pad}<chunks count=\"{}\">\n", c.chunks.len()));
    for sub in &c.chunks {
        s.push_str(&format!("{pad}  <chunk name=\"{}\"{}>\n", esc(&sub.name), index_attr(sub.index)));
        write_body(s, sub, depth + 2);
        s.push_str(&format!("{pad}  </chunk>\n"));
    }
    s.push_str(&format!("{pad}</chunks>\n"));
}

/// Small builder for test definitions: components, wires, groups, panels, scripts.
pub mod build {
    use super::super::archive::{Chunk, Item, Value};

    pub fn item(name: &str, value: Value) -> Item {
        Item { name: name.into(), index: None, value }
    }

    pub fn indexed(name: &str, index: i32, value: Value) -> Item {
        Item { name: name.into(), index: Some(index), value }
    }

    pub fn s(v: &str) -> Value {
        Value::Str(v.into())
    }

    pub fn g(v: &str) -> Value {
        Value::Guid(v.into())
    }

    pub fn guid(n: u32) -> String {
        format!("{n:08x}-0000-4000-8000-000000000000")
    }

    pub struct Definition {
        pub name: String,
        pub objects: Vec<Chunk>,
        pub libraries: Vec<(String, String)>,
    }

    impl Definition {
        pub fn new(name: &str) -> Self {
            Definition { name: name.into(), objects: Vec::new(), libraries: Vec::new() }
        }

        fn object(&mut self, type_name: &str, container: Chunk) -> &mut Self {
            let mut obj = Chunk::new("Object");
            obj.index = Some(self.objects.len() as i32);
            obj.items.push(item("GUID", g(&guid(0xabc0_0000 + self.objects.len() as u32))));
            obj.items.push(item("Name", s(type_name)));
            obj.chunks.push(container);
            self.objects.push(obj);
            self
        }

        /// Objects are laid out left to right, one per column.
        fn slot(&self) -> (f32, f32) {
            (180.0 * self.objects.len() as f32, 100.0)
        }

        fn attributes(x: f32, y: f32, w: f32, h: f32) -> Chunk {
            let mut a = Chunk::new("Attributes");
            a.items.push(item("Bounds", Value::Rect([x, y, w, h])));
            a.items.push(item("Pivot", Value::Point([x, y])));
            a
        }

        fn container(id: u32, nick: &str) -> Chunk {
            let mut c = Chunk::new("Container");
            c.items.push(item("InstanceGuid", g(&guid(id))));
            c.items.push(item("NickName", s(nick)));
            c
        }

        /// A component with one input (wired from `sources`) and one output (`id + 1`).
        pub fn component(&mut self, type_name: &str, id: u32, nick: &str, sources: &[u32]) -> &mut Self {
            let (x, y) = self.slot();
            let mut c = Self::container(id, nick);
            c.chunks.push(Self::attributes(x, y, 100.0, 40.0));
            let mut input = Chunk::new("param_input");
            input.index = Some(0);
            input.items.push(item("InstanceGuid", g(&guid(id + 1000))));
            input.items.push(item("NickName", s("A")));
            for (i, src) in sources.iter().enumerate() {
                input.items.push(indexed("Source", i as i32, g(&guid(src + 1))));
            }
            input.chunks.push(Self::attributes(x, y + 10.0, 20.0, 20.0));
            let mut output = Chunk::new("param_output");
            output.index = Some(0);
            output.items.push(item("InstanceGuid", g(&guid(id + 1))));
            output.items.push(item("NickName", s("R")));
            output.chunks.push(Self::attributes(x + 80.0, y + 10.0, 20.0, 20.0));
            c.chunks.push(input);
            c.chunks.push(output);
            self.object(type_name, c)
        }

        pub fn slider(&mut self, id: u32, nick: &str, value: f64) -> &mut Self {
            let (x, y) = self.slot();
            let mut c = Self::container(id, nick);
            // Floating parameters expose their own guid as the output.
            c.items[0] = item("InstanceGuid", g(&guid(id + 1)));
            c.chunks.push(Self::attributes(x, y, 140.0, 20.0));
            let mut slider = Chunk::new("Slider");
            slider.items.push(item("Min", Value::Float(0.0)));
            slider.items.push(item("Max", Value::Float(500.0)));
            slider.items.push(item("Value", Value::Float(value)));
            c.chunks.push(slider);
            self.object("Number Slider", c)
        }

        pub fn panel(&mut self, id: u32, text: &str) -> &mut Self {
            let (x, y) = self.slot();
            let mut c = Self::container(id, "Panel");
            c.items.push(item("UserText", s(text)));
            c.chunks.push(Self::attributes(x, y, 160.0, 60.0));
            self.object("Panel", c)
        }

        /// Groups have no position of their own: they wrap their members.
        pub fn group(&mut self, id: u32, label: &str, members: &[u32]) -> &mut Self {
            let mut c = Self::container(id, label);
            c.items.push(item("Colour", Value::Color(0x96aa_87ff)));
            for (i, m) in members.iter().enumerate() {
                c.items.push(indexed("ID", i as i32, g(&guid(*m))));
            }
            self.object("Group", c)
        }

        pub fn python(&mut self, id: u32, nick: &str, code: &str) -> &mut Self {
            let (x, y) = self.slot();
            let mut c = Self::container(id, nick);
            c.items.push(item("CodeInput", s(code)));
            c.chunks.push(Self::attributes(x, y, 100.0, 40.0));
            self.object("GhPython Script", c)
        }

        /// The canvas preview Grasshopper stores beside the definition.
        pub fn thumbnail(root: &mut Chunk, png: &[u8]) {
            let mut t = Chunk::new("Thumbnail");
            t.items.push(item("Thumbnail", Value::Bitmap(png.to_vec())));
            root.chunks.push(t);
        }

        pub fn library(&mut self, name: &str, version: &str) -> &mut Self {
            self.libraries.push((name.into(), version.into()));
            self
        }

        pub fn archive(&self) -> Chunk {
            let mut root = Chunk::new("Root");
            root.items.push(item("ArchiveVersion", Value::Version(0, 2, 2)));
            let mut def = Chunk::new("Definition");
            def.items.push(item("plugin_version", Value::Version(1, 0, 7)));
            let mut props = Chunk::new("DefinitionProperties");
            props.items.push(item("Name", s(&self.name)));
            props.items.push(item("Description", s("")));
            def.chunks.push(props);
            if !self.libraries.is_empty() {
                let mut libs = Chunk::new("GHALibraries");
                libs.items.push(item("Count", Value::Int(self.libraries.len() as i64)));
                for (i, (name, version)) in self.libraries.iter().enumerate() {
                    let mut lib = Chunk::new("Library");
                    lib.index = Some(i as i32);
                    lib.items.push(item("Name", s(name)));
                    lib.items.push(item("Version", s(version)));
                    lib.items.push(item("Id", g(&guid(0xfeed_0000 + i as u32))));
                    libs.chunks.push(lib);
                }
                def.chunks.push(libs);
            }
            let mut objects = Chunk::new("DefinitionObjects");
            objects.items.push(item("ObjectCount", Value::Int(self.objects.len() as i64)));
            objects.chunks = self.objects.clone();
            def.chunks.push(objects);
            root.chunks.push(def);
            root
        }
    }
}
