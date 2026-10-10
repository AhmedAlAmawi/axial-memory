//! A drawable map of one definition: the components where she placed them on
//! the canvas, the wires between their ports, and her groups in her colours.
//! Everything comes from the file; nothing is laid out by guesswork.

use crate::gh::digest::{is_script_kind, Digest, Obj};
use crate::terms::Matcher;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

const GROUP_PADDING: f32 = 8.0;
const GROUP_LABEL: f32 = 16.0;
const MARGIN: f32 = 30.0;
/// Grasshopper's own default group colour (ARGB 150, 170, 135, 255).
const DEFAULT_GROUP_COLOUR: u32 = 0x96aa_87ff;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Component,
    Param,
    Slider,
    Panel,
    Note,
    Script,
    Cluster,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Pin {
    pub name: String,
    /// Height on the node's edge where wires meet it.
    pub y: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Node {
    pub id: u32,
    pub kind: String,
    pub label: String,
    pub detail: Option<String>,
    pub role: Role,
    pub plugin: Option<String>,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub inputs: Vec<Pin>,
    pub outputs: Vec<Pin>,
    /// A search word was found in this component.
    pub matched: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapGroup {
    pub name: String,
    /// Red, green, blue, alpha.
    pub colour: [u8; 4],
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub members: Vec<u32>,
    /// How many groups it sits inside; outer groups are drawn first.
    pub depth: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapWire {
    pub from: u32,
    pub to: u32,
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Map {
    pub nodes: Vec<Node>,
    pub groups: Vec<MapGroup>,
    pub wires: Vec<MapWire>,
    /// x, y, width and height around everything, with a margin.
    pub bounds: [f32; 4],
    /// Grasshopper's own canvas preview, as base64 PNG.
    pub thumbnail: Option<String>,
    /// Components not drawn: inside clusters, or saved without a position.
    pub hidden: u32,
}

/// The components where a search word was found, to highlight on the map.
pub fn matched_objects(d: &Digest, matcher: &Matcher) -> HashSet<u32> {
    d.texts
        .iter()
        .filter(|t| !t.in_cluster)
        .filter_map(|t| t.obj.filter(|_| !matcher.find(&t.text).is_empty()))
        .collect()
}

fn role(o: &Obj) -> Role {
    let k = o.kind.to_lowercase();
    if k == "panel" {
        Role::Panel
    } else if k.contains("slider") {
        Role::Slider
    } else if k == "scribble" {
        Role::Note
    } else if k == "cluster" {
        Role::Cluster
    } else if is_script_kind(&o.kind) {
        Role::Script
    } else if o.inputs.is_empty() && o.outputs.is_empty() {
        Role::Param
    } else {
        Role::Component
    }
}

fn label(o: &Obj) -> String {
    match o.nick.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.to_string(),
        None => o.kind.clone(),
    }
}

fn pins(names: &[String], rects: &[Option<[f32; 4]>], b: [f32; 4]) -> Vec<Pin> {
    let n = names.len().max(1) as f32;
    names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let y = match rects.get(i).copied().flatten() {
                Some(r) => r[1] + r[3] / 2.0,
                None => b[1] + (i as f32 + 0.5) * b[3] / n,
            };
            Pin { name: name.clone(), y }
        })
        .collect()
}

fn union(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
    let x0 = a[0].min(b[0]);
    let y0 = a[1].min(b[1]);
    let x1 = (a[0] + a[2]).max(b[0] + b[2]);
    let y1 = (a[1] + a[3]).max(b[1] + b[3]);
    [x0, y0, x1 - x0, y1 - y0]
}

fn rgba(argb: u32) -> [u8; 4] {
    [(argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8]
}

pub fn build(d: &Digest, matched: &HashSet<u32>) -> Option<Map> {
    let drawn = |o: &Obj| !o.in_cluster && !o.kind.eq_ignore_ascii_case("group") && o.bounds.is_some();
    let mut nodes = Vec::new();
    let mut hidden = 0;
    for (i, o) in d.objects.iter().enumerate() {
        if o.kind.eq_ignore_ascii_case("group") {
            continue;
        }
        if !drawn(o) {
            hidden += 1;
            continue;
        }
        let b = o.bounds.unwrap();
        nodes.push(Node {
            id: i as u32,
            kind: o.kind.clone(),
            label: label(o),
            detail: o.detail.clone(),
            role: role(o),
            plugin: o.plugin.and_then(|p| d.plugins.get(p as usize)).map(|p| p.name.clone()),
            x: b[0],
            y: b[1],
            w: b[2],
            h: b[3],
            inputs: pins(&o.inputs, &o.in_pins, b),
            outputs: pins(&o.outputs, &o.out_pins, b),
            matched: matched.contains(&(i as u32)),
        });
    }
    if nodes.is_empty() {
        return None;
    }
    let rect_of: HashMap<u32, [f32; 4]> = nodes.iter().map(|n| (n.id, [n.x, n.y, n.w, n.h])).collect();

    let mut wires = Vec::new();
    for w in &d.wires {
        let (Some(from), Some(to)) = (rect_of.get(&w.from), rect_of.get(&w.to)) else { continue };
        let pin = |obj: u32, pin: Option<u16>, output: bool| {
            let o = &d.objects[obj as usize];
            let rects = if output { &o.out_pins } else { &o.in_pins };
            pin.and_then(|p| rects.get(p as usize).copied().flatten())
        };
        let (x1, y1) = match pin(w.from, w.from_pin, true) {
            Some(r) => (r[0] + r[2], r[1] + r[3] / 2.0),
            None => (from[0] + from[2], from[1] + from[3] / 2.0),
        };
        let (x2, y2) = match pin(w.to, w.to_pin, false) {
            Some(r) => (r[0], r[1] + r[3] / 2.0),
            None => (to[0], to[1] + to[3] / 2.0),
        };
        wires.push(MapWire { from: w.from, to: w.to, x1, y1, x2, y2 });
    }

    // Groups wrap their members, which can be other groups.
    let group_of_obj: HashMap<u32, usize> =
        d.groups.iter().enumerate().filter_map(|(gi, g)| g.obj.map(|o| (o, gi))).collect();
    fn rect(
        gi: usize,
        d: &Digest,
        rect_of: &HashMap<u32, [f32; 4]>,
        group_of_obj: &HashMap<u32, usize>,
        memo: &mut HashMap<usize, Option<[f32; 4]>>,
        visiting: &mut HashSet<usize>,
    ) -> Option<[f32; 4]> {
        if let Some(r) = memo.get(&gi) {
            return *r;
        }
        if !visiting.insert(gi) {
            return None;
        }
        let mut acc: Option<[f32; 4]> = None;
        for m in &d.groups[gi].members {
            let r = match group_of_obj.get(m) {
                Some(&inner) => rect(inner, d, rect_of, group_of_obj, memo, visiting),
                None => rect_of.get(m).copied(),
            };
            if let Some(r) = r {
                acc = Some(acc.map_or(r, |a| union(a, r)));
            }
        }
        let padded = acc.map(|a| {
            [
                a[0] - GROUP_PADDING,
                a[1] - GROUP_PADDING - GROUP_LABEL,
                a[2] + 2.0 * GROUP_PADDING,
                a[3] + 2.0 * GROUP_PADDING + GROUP_LABEL,
            ]
        });
        visiting.remove(&gi);
        memo.insert(gi, padded);
        padded
    }
    let mut memo = HashMap::new();
    let mut parents: HashMap<usize, usize> = HashMap::new();
    for (gi, g) in d.groups.iter().enumerate() {
        for m in &g.members {
            if let Some(&inner) = group_of_obj.get(m) {
                parents.insert(inner, gi);
            }
        }
    }
    let depth_of = |mut gi: usize| {
        let mut depth = 0;
        while let Some(&p) = parents.get(&gi) {
            depth += 1;
            gi = p;
            if depth > 16 {
                break;
            }
        }
        depth
    };
    let mut groups = Vec::new();
    for (gi, g) in d.groups.iter().enumerate() {
        let Some(r) = rect(gi, d, &rect_of, &group_of_obj, &mut memo, &mut HashSet::new()) else { continue };
        groups.push(MapGroup {
            name: g.name.clone(),
            colour: rgba(g.colour.unwrap_or(DEFAULT_GROUP_COLOUR)),
            x: r[0],
            y: r[1],
            w: r[2],
            h: r[3],
            members: g.members.iter().copied().filter(|m| rect_of.contains_key(m)).collect(),
            depth: depth_of(gi),
        });
    }
    groups.sort_by_key(|g| g.depth);

    let mut all = nodes.iter().map(|n| [n.x, n.y, n.w, n.h]).chain(groups.iter().map(|g| [g.x, g.y, g.w, g.h]));
    let first = all.next().unwrap();
    let b = all.fold(first, union);
    Some(Map {
        nodes,
        groups,
        wires,
        bounds: [b[0] - MARGIN, b[1] - MARGIN, b[2] + 2.0 * MARGIN, b[3] + 2.0 * MARGIN],
        thumbnail: d.thumbnail.clone(),
        hidden,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gh::write::{build::Definition, to_gh};
    use crate::terms::default_terms;

    #[test]
    fn draws_the_canvas_as_saved() {
        let mut def = Definition::new("palm.gh");
        def.slider(10, "Count", 144.0)
            .component("Series", 20, "Ser", &[10])
            .component("Multiplication", 30, "golden angle", &[20])
            .group(40, "Phyllotaxis spiral", &[20, 30]);
        let mut root = def.archive();
        crate::gh::write::build::Definition::thumbnail(&mut root, b"\x89PNG fake");
        let d = crate::gh::read_digest(&to_gh(&root), "gh").unwrap();
        let map = build(&d, &matched_objects(&d, &Matcher::new(&default_terms()))).unwrap();

        assert_eq!(map.nodes.len(), 3);
        assert_eq!(map.hidden, 0);
        assert_eq!(map.nodes[0].role, Role::Slider);
        assert_eq!(map.nodes[1].label, "Ser");
        assert!(map.nodes[2].matched, "\"golden angle\" is a search word");
        assert!(!map.nodes[1].matched);

        // Slider (x 0, w 140) to Series' input pin (x 180, y 110..130).
        let w = &map.wires[0];
        assert_eq!((w.from, w.to), (0, 1));
        assert_eq!((w.x1, w.y1, w.x2, w.y2), (140.0, 110.0, 180.0, 120.0));
        // Series' output pin (x 260..280) to Multiplication's input pin (x 360).
        assert_eq!((map.wires[1].x1, map.wires[1].x2), (280.0, 360.0));

        let g = &map.groups[0];
        assert_eq!(g.name, "Phyllotaxis spiral");
        assert_eq!(g.colour, [170, 135, 255, 150]);
        assert_eq!(g.members, vec![1, 2]);
        assert!(g.x < 180.0 && g.x + g.w > 460.0 && g.y < 100.0 - GROUP_LABEL);
        assert!(map.thumbnail.is_some());
    }

    #[test]
    fn nothing_to_draw_without_positions() {
        let d = Digest::default();
        assert!(build(&d, &HashSet::new()).is_none());
    }
}
