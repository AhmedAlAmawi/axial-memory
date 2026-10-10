//! Turns an archive tree into what a person (or a model) needs: the components,
//! how they're wired, the groups and their labels, panel text, scripts, plugins
//! and file references. Also the user-written text the palm search runs over.

use super::archive::{Chunk, Value};
use super::binary;
use base64::Engine;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

const MAX_OBJECTS: usize = 50_000;
const MAX_TEXTS: usize = 20_000;
const MAX_TEXT_CHARS: usize = 4_000;
const MAX_SCRIPT_CHARS: usize = 40_000;
const MAX_REFERENCES: usize = 200;
const MAX_NESTING: usize = 3;

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Digest {
    pub format: String,
    pub archive_version: Option<String>,
    pub gh_version: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub plugins: Vec<Plugin>,
    pub objects: Vec<Obj>,
    pub groups: Vec<Group>,
    pub edges: Vec<(u32, u32)>,
    pub texts: Vec<Text>,
    pub scripts: Vec<Script>,
    pub references: Vec<String>,
    pub nested_archives: u32,
    pub truncated: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Plugin {
    pub name: String,
    pub version: Option<String>,
    pub author: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Obj {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nick: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub in_cluster: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Group {
    pub name: String,
    pub members: Vec<u32>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TextSource {
    /// The definition's own name and description, as saved by Grasshopper.
    Definition,
    Group,
    Panel,
    Note,
    Nickname,
    Script,
    Value,
    Component,
    Plugin,
    Port,
    UserObject,
}

impl TextSource {
    pub fn label(self) -> &'static str {
        match self {
            TextSource::Definition => "saved definition name",
            TextSource::Group => "group label",
            TextSource::Panel => "panel text",
            TextSource::Note => "scribble note",
            TextSource::Nickname => "component name she gave",
            TextSource::Script => "script code",
            TextSource::Value => "value typed into a parameter",
            TextSource::Component => "component type",
            TextSource::Plugin => "plugin",
            TextSource::Port => "script input/output name",
            TextSource::UserObject => "user object details",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Text {
    pub source: TextSource,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub obj: Option<u32>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub in_cluster: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Script {
    pub obj: u32,
    pub language: String,
    pub code: String,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// Short facts about a definition, kept for every file (the digest is kept too).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Summary {
    pub gh_version: Option<String>,
    pub name: Option<String>,
    pub objects: u32,
    pub groups: Vec<String>,
    pub plugins: Vec<String>,
    pub references: Vec<String>,
    pub scripts: u32,
    pub nested_archives: u32,
}

impl Digest {
    pub fn summary(&self) -> Summary {
        let mut groups: Vec<String> = Vec::new();
        for g in &self.groups {
            let name = g.name.trim();
            if !name.is_empty() && !name.eq_ignore_ascii_case("group") && !groups.iter().any(|x| x == name) {
                groups.push(name.to_string());
            }
        }
        Summary {
            gh_version: self.gh_version.clone(),
            name: self.name.clone(),
            objects: self.objects.iter().filter(|o| !o.in_cluster).count() as u32,
            groups,
            plugins: self.plugins.iter().map(plugin_label).collect(),
            references: self.references.clone(),
            scripts: self.scripts.len() as u32,
            nested_archives: self.nested_archives,
        }
    }
}

pub fn plugin_label(p: &Plugin) -> String {
    match &p.version {
        Some(v) if !v.is_empty() => format!("{} {}", p.name, v),
        _ => p.name.clone(),
    }
}

#[derive(Default)]
struct Ctx {
    guid_to_obj: HashMap<String, u32>,
    pending_edges: Vec<(String, u32)>,
    pending_groups: Vec<(usize, Vec<String>)>,
    libs: HashMap<String, u32>,
    seen_texts: HashSet<(TextSource, String, Option<u32>)>,
}

pub fn build(root: &Chunk, format: &str) -> Digest {
    let mut d = Digest { format: format.to_string(), ..Digest::default() };
    let mut ctx = Ctx::default();
    d.archive_version = root.item("ArchiveVersion").and_then(|i| version_text(&i.value));
    build_any(root, &mut d, &mut ctx, 0, false);
    resolve(&mut d, &mut ctx);
    d.references = find_references(&d);
    d
}

fn build_any(root: &Chunk, d: &mut Digest, ctx: &mut Ctx, depth: usize, in_cluster: bool) {
    let name = root.name.to_ascii_lowercase();
    if name == "root" {
        if let Some(def) = root.chunk("Definition") {
            build_definition(def, d, ctx, depth, in_cluster);
        } else {
            harvest_generic(root, d, ctx, in_cluster);
        }
    } else if name == "definition" {
        build_definition(root, d, ctx, depth, in_cluster);
    } else if name == "userobject" && depth == 0 {
        build_user_object(root, d, ctx);
    } else if let Some(c) = root.chunk("Container") {
        let kind = root.str("Name").unwrap_or("Unknown component").to_string();
        component(&kind, root.guid("Lib"), c, d, ctx, depth, in_cluster);
    } else if root.item("InstanceGuid").is_some() {
        // The component inside a user object is stored as its container alone.
        let kind = root.str("Name").unwrap_or("Unknown component").to_string();
        component(&kind, None, root, d, ctx, depth, in_cluster);
    } else {
        harvest_generic(root, d, ctx, in_cluster);
    }
}

fn build_definition(def: &Chunk, d: &mut Digest, ctx: &mut Ctx, depth: usize, in_cluster: bool) {
    if depth == 0 {
        d.gh_version = def.item("plugin_version").and_then(|i| version_text(&i.value));
        if let Some(props) = def.chunk("DefinitionProperties") {
            d.name = props.str("Name").map(str::to_string).filter(|s| !s.trim().is_empty());
            d.description = props.str("Description").map(str::to_string).filter(|s| !s.trim().is_empty());
            for t in [d.name.clone(), d.description.clone()].into_iter().flatten() {
                push_text(d, ctx, TextSource::Definition, &t, None, false);
            }
        }
    }
    if let Some(libs) = def.chunk("GHALibraries") {
        for lib in libs.chunks_named("Library") {
            let Some(name) = lib.str("Name").filter(|s| !s.trim().is_empty()) else { continue };
            let version = lib.str("Version").or(lib.str("AssemblyVersion")).map(str::to_string);
            let idx = match d.plugins.iter().position(|p| p.name == name) {
                Some(i) => i as u32,
                None => {
                    d.plugins.push(Plugin {
                        name: name.to_string(),
                        version,
                        author: lib.str("Author").map(str::to_string).filter(|s| !s.is_empty()),
                    });
                    push_text(d, ctx, TextSource::Plugin, name, None, in_cluster);
                    (d.plugins.len() - 1) as u32
                }
            };
            if let Some(id) = lib.guid("Id") {
                ctx.libs.insert(id.to_string(), idx);
            }
        }
    }
    if let Some(objects) = def.chunk("DefinitionObjects") {
        for obj in objects.chunks_named("Object") {
            if d.objects.len() >= MAX_OBJECTS {
                d.truncated = true;
                break;
            }
            build_any(obj, d, ctx, depth, in_cluster);
        }
    }
}

fn build_user_object(root: &Chunk, d: &mut Digest, ctx: &mut Ctx) {
    d.name = root.str("Name").map(str::to_string);
    d.description = root.str("Description").map(str::to_string).filter(|s| !s.trim().is_empty());
    for key in ["Name", "Description", "Category", "SubCategory"] {
        if let Some(t) = root.str(key) {
            push_text(d, ctx, TextSource::UserObject, t, None, false);
        }
    }
    for item in &root.items {
        if let Value::Bytes(b) = &item.value {
            if let Some(inner) = binary::try_nested(b) {
                d.nested_archives += 1;
                build_any(&inner, d, ctx, 1, false);
            }
        }
    }
}

const IGNORED_ITEMS: &[&str] = &[
    "name",
    "nickname",
    "description",
    "usertext",
    "text",
    "codeinput",
    "assemblyfullname",
    "assemblyversion",
    "author",
    "version",
    "id",
    "lib",
    "guid",
    "instanceguid",
    "source",
    "category",
    "subcategory",
    "fontname",
    "font",
];

fn is_script_kind(kind: &str) -> bool {
    let k = kind.to_ascii_lowercase();
    ["python", "c#", "csharp", "vb ", "vb.net", "script", "ironpython", "code"].iter().any(|w| k.contains(w))
}

/// One component (or floating parameter, group, panel…) from its container chunk.
fn component(kind: &str, lib: Option<&str>, c: &Chunk, d: &mut Digest, ctx: &mut Ctx, depth: usize, in_cluster: bool) {
    let kind = kind.to_string();
    let idx = d.objects.len() as u32;
    let plugin = lib.and_then(|g| ctx.libs.get(g).copied());
    let mut o = Obj {
        kind: kind.clone(),
        nick: None,
        plugin,
        detail: None,
        inputs: Vec::new(),
        outputs: Vec::new(),
        in_cluster,
    };
    let kind_lc = kind.to_ascii_lowercase();
    let scripty = is_script_kind(&kind) || c.item("CodeInput").is_some() || c.item("ScriptSource").is_some();
    push_text(d, ctx, TextSource::Component, &kind, Some(idx), in_cluster);

    if let Some(g) = c.guid("InstanceGuid") {
        ctx.guid_to_obj.insert(g.to_string(), idx);
    }
    let nick = c.str("NickName").map(str::trim).filter(|s| !s.is_empty()).map(str::to_string);
    if let Some(n) = &nick {
        let default_name = c.str("Name").unwrap_or(&kind);
        if !n.eq_ignore_ascii_case(&kind) && !n.eq_ignore_ascii_case(default_name) {
            let source = if kind_lc == "group" { TextSource::Group } else { TextSource::Nickname };
            push_text(d, ctx, source, n, Some(idx), in_cluster);
        } else if kind_lc == "group" {
            push_text(d, ctx, TextSource::Group, n, Some(idx), in_cluster);
        }
    }
    o.nick = nick.clone();

    // Wires: every input parameter lists the guids of the outputs feeding it.
    for src in c.items_named("Source") {
        if let Value::Guid(g) = &src.value {
            ctx.pending_edges.push((g.clone(), idx));
        }
    }
    let mut ports: Vec<(&Chunk, bool)> = Vec::new();
    c.walk(&mut |ch, level| {
        if level == 0 || level > 3 {
            return;
        }
        let n = ch.name.to_ascii_lowercase();
        if n == "param_input" || n == "inputparam" {
            ports.push((ch, true));
        } else if n == "param_output" || n == "outputparam" {
            ports.push((ch, false));
        }
    });
    for (p, is_input) in ports {
        if let Some(g) = p.guid("InstanceGuid") {
            ctx.guid_to_obj.insert(g.to_string(), idx);
        }
        let label = p.str("NickName").or(p.str("Name")).unwrap_or("").trim().to_string();
        if !label.is_empty() {
            if is_input {
                o.inputs.push(label.clone())
            } else {
                o.outputs.push(label.clone())
            }
            if scripty && label.chars().count() > 1 {
                push_text(d, ctx, TextSource::Port, &label, Some(idx), in_cluster);
            }
        }
        if is_input {
            for src in p.items_named("Source") {
                if let Value::Guid(g) = &src.value {
                    ctx.pending_edges.push((g.clone(), idx));
                }
            }
        }
    }

    match kind_lc.as_str() {
        "group" => {
            let members = c
                .items_named("ID")
                .filter_map(|i| match &i.value {
                    Value::Guid(g) => Some(g.clone()),
                    _ => None,
                })
                .collect();
            d.groups.push(Group { name: nick.clone().unwrap_or_default(), members: Vec::new() });
            ctx.pending_groups.push((d.groups.len() - 1, members));
        }
        "panel" => {
            if let Some(t) = c.str("UserText").filter(|t| !t.trim().is_empty()) {
                o.detail = Some(excerpt(t, 160));
                push_text(d, ctx, TextSource::Panel, t, Some(idx), in_cluster);
            }
        }
        "scribble" => {
            if let Some(t) = c.str("Text").filter(|t| !t.trim().is_empty()) {
                o.detail = Some(excerpt(t, 160));
                push_text(d, ctx, TextSource::Note, t, Some(idx), in_cluster);
            }
        }
        "boolean toggle" => {
            if let Some(b) = c.bool("ToggleValue") {
                o.detail = Some(if b { "True".into() } else { "False".into() });
            }
        }
        "value list" => {
            let entries: Vec<String> =
                c.chunks_named("ListItem").filter_map(|li| li.str("Name").map(str::to_string)).collect();
            for e in &entries {
                push_text(d, ctx, TextSource::Value, e, Some(idx), in_cluster);
            }
            if !entries.is_empty() {
                o.detail = Some(excerpt(&entries.join(", "), 160));
            }
        }
        _ => {}
    }
    if let Some(slider) = c.chunk("Slider") {
        if let Some(v) = slider.number("Value") {
            let range = match (slider.number("Min"), slider.number("Max")) {
                (Some(a), Some(b)) => format!(" (range {} to {})", num(a), num(b)),
                _ => String::new(),
            };
            o.detail = Some(format!("{}{}", num(v), range));
        }
    }

    // Scripts, typed values, expressions and anything else she wrote in the component.
    let mut strings: Vec<(String, String)> = Vec::new();
    let mut blobs: Vec<Vec<u8>> = Vec::new();
    c.walk(&mut |ch, _| {
        for item in &ch.items {
            match &item.value {
                Value::Str(s) if !s.trim().is_empty() => strings.push((item.name.to_ascii_lowercase(), s.clone())),
                Value::Bytes(b) => blobs.push(b.clone()),
                _ => {}
            }
        }
    });
    for (item_name, s) in strings {
        if matches!(item_name.as_str(), "name" | "nickname" | "description") {
            continue;
        }
        let code = if item_name == "codeinput" || item_name == "scriptsource" || (scripty && looks_like_code(&s)) {
            Some(s.clone())
        } else if scripty && looks_like_base64(&s) {
            decode_base64_text(&s).filter(|t| looks_like_code(t))
        } else if looks_like_code(&s) && s.lines().count() > 2 {
            Some(s.clone())
        } else {
            None
        };
        if let Some(code) = code {
            let language = script_language(&kind, &code);
            if !d.scripts.iter().any(|x| x.obj == idx && x.code == code) {
                d.scripts.push(Script { obj: idx, language, code: truncate(&code, MAX_SCRIPT_CHARS) });
            }
            push_text(d, ctx, TextSource::Script, &code, Some(idx), in_cluster);
            continue;
        }
        if IGNORED_ITEMS.contains(&item_name.as_str()) {
            continue;
        }
        push_text(d, ctx, TextSource::Value, &s, Some(idx), in_cluster);
    }
    d.objects.push(o);

    // Clusters keep their inside as a nested archive.
    if depth < MAX_NESTING {
        for b in blobs {
            if let Some(inner) = binary::try_nested(&b) {
                d.nested_archives += 1;
                build_any(&inner, d, ctx, depth + 1, true);
            }
        }
    }
}

fn harvest_generic(root: &Chunk, d: &mut Digest, ctx: &mut Ctx, in_cluster: bool) {
    let mut strings = Vec::new();
    root.walk(&mut |ch, _| {
        for item in &ch.items {
            if let Value::Str(s) = &item.value {
                if !IGNORED_ITEMS.contains(&item.name.to_ascii_lowercase().as_str())
                    || item.name.eq_ignore_ascii_case("name")
                {
                    strings.push(s.clone());
                }
            }
        }
    });
    for s in strings {
        push_text(d, ctx, TextSource::Value, &s, None, in_cluster);
    }
}

fn resolve(d: &mut Digest, ctx: &mut Ctx) {
    let mut seen = HashSet::new();
    for (src, dst) in std::mem::take(&mut ctx.pending_edges) {
        if let Some(&from) = ctx.guid_to_obj.get(&src) {
            if from != dst && seen.insert((from, dst)) {
                d.edges.push((from, dst));
            }
        }
    }
    for (gi, members) in std::mem::take(&mut ctx.pending_groups) {
        let mut ids: Vec<u32> = members.iter().filter_map(|m| ctx.guid_to_obj.get(m).copied()).collect();
        ids.sort_unstable();
        ids.dedup();
        d.groups[gi].members = ids;
    }
}

fn push_text(d: &mut Digest, ctx: &mut Ctx, source: TextSource, text: &str, obj: Option<u32>, in_cluster: bool) {
    let t = text.trim();
    if t.is_empty() || d.texts.len() >= MAX_TEXTS {
        if d.texts.len() >= MAX_TEXTS {
            d.truncated = true;
        }
        return;
    }
    let t = truncate(t, if source == TextSource::Script { MAX_SCRIPT_CHARS } else { MAX_TEXT_CHARS });
    if ctx.seen_texts.insert((source, t.clone(), obj)) {
        d.texts.push(Text { source, text: t, obj, in_cluster });
    }
}

fn looks_like_code(s: &str) -> bool {
    if s.len() < 40 || !s.contains('\n') {
        return false;
    }
    let markers = [
        "import ", "def ", "return", "for ", "if ", "using ", "void ", "class ", "rs.", "rg.", " = ", ";", "{",
        "print(",
    ];
    markers.iter().filter(|m| s.contains(*m)).count() >= 2
}

fn looks_like_base64(s: &str) -> bool {
    s.len() >= 40
        && s.len().is_multiple_of(4)
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/' || b == b'=')
}

fn decode_base64_text(s: &str) -> Option<String> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
    String::from_utf8(bytes).ok()
}

fn script_language(kind: &str, code: &str) -> String {
    let k = kind.to_ascii_lowercase();
    if k.contains("python") || code.contains("import rhinoscriptsyntax") || code.contains("def ") {
        "python".into()
    } else if k.contains("c#") || k.contains("csharp") || code.contains("using System") || code.contains("RunScript(") {
        "csharp".into()
    } else if k.contains("vb") {
        "vb".into()
    } else {
        "script".into()
    }
}

fn version_text(v: &Value) -> Option<String> {
    match v {
        Value::Version(a, b, c) => Some(format!("{a}.{b}.{c}")),
        _ => None,
    }
}

fn num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{:.4}", v).trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

pub fn excerpt(s: &str, max: usize) -> String {
    let flat: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate(&flat, max)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push('…');
        t
    }
}

fn reference_pattern() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r#"(?i)(?:[a-z]:\\|\\\\|/(?:users|volumes|home|private|tmp)/|~/)[^\r\n"<>|*?]{1,240}?\.(?:3dm|3dmbak|ghx?|ghuser|csv|txt|xlsx?|json|xml|obj|stl|dwg|dxf|jpe?g|png|tiff?|bmp|pdf|shp|geojson|ply|fbx|step|stp|iges|igs|py|cs|gha)\b"#,
        )
        .unwrap()
    })
}

fn find_references(d: &Digest) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in &d.texts {
        if matches!(t.source, TextSource::Component | TextSource::Plugin) {
            continue;
        }
        for m in reference_pattern().find_iter(&t.text) {
            let p = m.as_str().trim().to_string();
            if !out.contains(&p) {
                out.push(p);
                if out.len() >= MAX_REFERENCES {
                    return out;
                }
            }
        }
    }
    out
}

/// Components in the order data flows through them: nothing comes before what
/// feeds it, and otherwise canvas order is kept.
pub fn dataflow_order(d: &Digest) -> Vec<u32> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let n = d.objects.len();
    let mut indegree = vec![0usize; n];
    let mut next: Vec<Vec<u32>> = vec![Vec::new(); n];
    for &(a, b) in &d.edges {
        if (a as usize) < n && (b as usize) < n {
            next[a as usize].push(b);
            indegree[b as usize] += 1;
        }
    }
    let mut ready: BinaryHeap<Reverse<u32>> =
        (0..n as u32).filter(|&i| indegree[i as usize] == 0).map(Reverse).collect();
    let mut order = Vec::with_capacity(n);
    let mut placed = vec![false; n];
    while let Some(Reverse(i)) = ready.pop() {
        order.push(i);
        placed[i as usize] = true;
        for &j in &next[i as usize] {
            indegree[j as usize] -= 1;
            if indegree[j as usize] == 0 {
                ready.push(Reverse(j));
            }
        }
    }
    // Loops (rare, e.g. Anemone) keep their canvas order.
    order.extend((0..n as u32).filter(|&i| !placed[i as usize]));
    order
}
