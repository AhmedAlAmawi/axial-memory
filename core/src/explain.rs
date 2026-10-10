//! What gets sent when she asks an AI to explain one definition: a compact
//! text extract of the components, wiring, labels and scripts. Never the file.
//! The app does the sending; this module only builds the words.

use crate::gh::digest::{dataflow_order, plugin_label, Digest, TextSource};
use crate::terms::Evidence;

const MAX_EXTRACT_CHARS: usize = 60_000;
const MAX_SCRIPT_CHARS: usize = 6_000;

pub const SYSTEM_PROMPT: &str = "You help an architecture researcher understand her own Grasshopper (Rhino 3D) definitions. \
You get a text extract of one definition: its components in the order data flows, their wiring, group labels, panel notes, \
slider values, scripts and plugins. You don't see the canvas or the geometry.

Explain it in plain, warm language for a designer, not a programmer. Use exactly these sections:

## What it does
Two to four sentences.

## Stages
A numbered list of the main stages in the order data flows. Name each stage by its group label if it has one, \
otherwise by its key components (quote their names exactly as listed), and say what that stage produces.

## Palm and phyllotaxis
What in the definition relates to palms, phyllotaxis, stalks, fronds, spirals, the golden angle or growth patterns, \
citing the exact group, panel or component names as evidence. If nothing does, say so plainly.

## What to change
The sliders and values that drive the result, with their current values.

## Needs
Plugins it requires and files it reads, if any.

## Unsure
What can't be told from the extract.

Never invent components that aren't listed. Never claim the method is scientifically valid. Stay under 450 words.";

/// The extract sent as the user message, and also shown to her before sending.
pub fn extract(d: &Digest, file_name: &str, evidence: &[Evidence]) -> String {
    let mut s = String::new();
    let push = |s: &mut String, line: String| {
        s.push_str(&line);
        s.push('\n');
    };
    push(&mut s, format!("File: {file_name}"));
    if let Some(v) = &d.gh_version {
        push(&mut s, format!("Saved by Grasshopper {v}"));
    }
    if let Some(n) = &d.name {
        push(&mut s, format!("Name saved inside: {n}"));
    }
    if let Some(desc) = &d.description {
        push(&mut s, format!("Description: {desc}"));
    }
    if !d.plugins.is_empty() {
        let names: Vec<String> = d.plugins.iter().map(plugin_label).collect();
        push(&mut s, format!("Plugins: {}", names.join("; ")));
    }
    let labelled: Vec<_> = d.groups.iter().filter(|g| !g.name.trim().is_empty()).collect();
    if !labelled.is_empty() {
        push(&mut s, "\nGroups (component numbers inside):".into());
        for g in labelled {
            let members: Vec<String> = g.members.iter().map(|m| (m + 1).to_string()).collect();
            push(&mut s, format!("- \"{}\": {}", g.name, members.join(", ")));
        }
    }

    push(
        &mut s,
        format!(
            "\nComponents in dataflow order ({} total). Format: number. Type \"name\" = value <- fed by",
            d.objects.len()
        ),
    );
    let mut feeds: Vec<Vec<u32>> = vec![Vec::new(); d.objects.len()];
    for &(a, b) in &d.edges {
        if let Some(f) = feeds.get_mut(b as usize) {
            f.push(a + 1);
        }
    }
    for i in dataflow_order(d) {
        let o = &d.objects[i as usize];
        if o.kind.eq_ignore_ascii_case("group") {
            continue;
        }
        let mut line = format!("{}. {}", i + 1, o.kind);
        if let Some(n) = o.nick.as_ref().filter(|n| !n.eq_ignore_ascii_case(&o.kind)) {
            line.push_str(&format!(" \"{n}\""));
        }
        if let Some(detail) = &o.detail {
            line.push_str(&format!(" = {detail}"));
        }
        if let Some(p) = o.plugin.and_then(|p| d.plugins.get(p as usize)) {
            line.push_str(&format!(" [{}]", p.name));
        }
        if o.in_cluster {
            line.push_str(" (inside a cluster)");
        }
        let from = &feeds[i as usize];
        if !from.is_empty() {
            let list: Vec<String> = from.iter().map(|n| n.to_string()).collect();
            line.push_str(&format!(" <- {}", list.join(", ")));
        }
        push(&mut s, line);
        if s.len() > MAX_EXTRACT_CHARS / 2 {
            push(&mut s, "… (more components not shown)".into());
            break;
        }
    }

    let notes: Vec<_> = d.texts.iter().filter(|t| matches!(t.source, TextSource::Panel | TextSource::Note)).collect();
    if !notes.is_empty() {
        push(&mut s, "\nPanels and notes:".into());
        for t in notes.iter().take(60) {
            let at = t.obj.map(|o| format!("({}) ", o + 1)).unwrap_or_default();
            push(&mut s, format!("- {at}{}", crate::gh::digest::excerpt(&t.text, 400)));
        }
    }

    for script in &d.scripts {
        if s.len() > MAX_EXTRACT_CHARS {
            break;
        }
        push(&mut s, format!("\nScript in component {} ({}):", script.obj + 1, script.language));
        let code: String = script.code.chars().take(MAX_SCRIPT_CHARS).collect();
        push(&mut s, code);
    }
    if !d.references.is_empty() {
        push(&mut s, format!("\nFiles it refers to: {}", d.references.join("; ")));
    }
    if !evidence.is_empty() {
        push(&mut s, "\nPalm search matches already found:".into());
        for e in evidence.iter().take(15) {
            push(&mut s, format!("- \"{}\" in the {}: {}", e.term, e.source.label(), e.text));
        }
    }
    if s.len() > MAX_EXTRACT_CHARS {
        s = s.chars().take(MAX_EXTRACT_CHARS).collect();
        s.push_str("\n… (extract cut short)");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gh::write::{build::Definition, to_gh};

    #[test]
    fn extract_reads_like_the_definition() {
        let mut def = Definition::new("palm.gh");
        def.slider(10, "Count", 144.0)
            .component("Series", 20, "Ser", &[10])
            .group(30, "Phyllotaxis spiral", &[20])
            .panel(40, "golden angle 137.5");
        let d = crate::gh::read_digest(&to_gh(&def.archive()), "gh").unwrap();
        let text = extract(&d, "palm.gh", &[]);
        assert!(text.contains("1. Number Slider \"Count\" = 144 (range 0 to 500)"));
        assert!(text.contains("2. Series \"Ser\" <- 1"));
        assert!(text.contains("- \"Phyllotaxis spiral\": 2"));
        assert!(text.contains("golden angle 137.5"));
        assert!(!text.contains("3. Group"));
    }
}
