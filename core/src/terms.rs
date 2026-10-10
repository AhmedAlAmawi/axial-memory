//! The search words and how a match becomes evidence. Matching ignores case,
//! spaces, `_`, `-` and `.` between words, so "Palm_Tree", "palm-tree" and
//! "PalmTree" all match "palm tree". Short words like "palm" must stand alone
//! (or start a compound such as "palmleaf"), so "napalm" and "Palmer" don't count.

use crate::gh::TextSource;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Strength {
    /// Words that almost certainly mean her palm research.
    Strong,
    /// Palm words that are also used in other senses.
    Palm,
    /// Neighbouring vocabulary: weak on its own, supporting next to the others.
    Related,
}

impl Strength {
    pub fn weight(self) -> f64 {
        match self {
            Strength::Strong => 1.0,
            Strength::Palm => 0.6,
            Strength::Related => 0.25,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Strength::Strong => "strong",
            Strength::Palm => "palm",
            Strength::Related => "related",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Term {
    pub word: String,
    pub strength: Strength,
    /// Other spellings that count as this word.
    #[serde(default)]
    pub variants: Vec<String>,
}

fn term(word: &str, strength: Strength, variants: &[&str]) -> Term {
    Term { word: word.into(), strength, variants: variants.iter().map(|v| v.to_string()).collect() }
}

pub fn default_terms() -> Vec<Term> {
    use Strength::*;
    vec![
        term("Palmscapes", Strong, &["palmscape", "palm scape"]),
        term(
            "phyllotaxis",
            Strong,
            &["phyllotaxy", "phyllotactic", "phylotaxis", "phillotaxis", "filotaxis", "fillotaxis", "phyllotaxie"],
        ),
        term("palm tree", Strong, &["palmtree", "palm trees"]),
        term("axial memory", Strong, &[]),
        term("palm", Palm, &["palms"]),
        term("stalk", Palm, &["stalks"]),
        term("نخيل", Palm, &["نخل", "نخلة", "سعف", "جريد"]),
        term("frond", Related, &["fronds"]),
        term("fibonacci", Related, &["fibonaci", "fibonnaci"]),
        term("golden angle", Related, &["137.5", "137,5"]),
        term("parastichy", Related, &["parastichies", "parastich"]),
        term("vogel", Related, &[]),
        term("petiole", Related, &[]),
        term("rachis", Related, &[]),
        term("date palm", Related, &[]),
        term("coconut", Related, &[]),
        term("arecaceae", Related, &[]),
        term("sunflower", Related, &[]),
    ]
}

/// Where in or about a file a word was found.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    FileName,
    Folder,
    InsideArchivePath,
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

impl From<TextSource> for Source {
    fn from(t: TextSource) -> Self {
        match t {
            TextSource::Definition => Source::Definition,
            TextSource::Group => Source::Group,
            TextSource::Panel => Source::Panel,
            TextSource::Note => Source::Note,
            TextSource::Nickname => Source::Nickname,
            TextSource::Script => Source::Script,
            TextSource::Value => Source::Value,
            TextSource::Component => Source::Component,
            TextSource::Plugin => Source::Plugin,
            TextSource::Port => Source::Port,
            TextSource::UserObject => Source::UserObject,
        }
    }
}

impl Source {
    pub fn weight(self) -> f64 {
        match self {
            Source::FileName => 1.0,
            Source::InsideArchivePath => 0.95,
            Source::Definition | Source::Group => 0.9,
            Source::Panel | Source::Note => 0.85,
            Source::Nickname | Source::UserObject => 0.8,
            Source::Script => 0.75,
            Source::Component => 0.7,
            Source::Port | Source::Value => 0.6,
            Source::Folder => 0.55,
            Source::Plugin => 0.4,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Source::FileName => "file name",
            Source::Folder => "folder name",
            Source::InsideArchivePath => "path inside the zip",
            Source::Definition => "name saved inside the file",
            Source::Group => "group label",
            Source::Panel => "panel text",
            Source::Note => "scribble note",
            Source::Nickname => "component she renamed",
            Source::Script => "script code",
            Source::Value => "typed value",
            Source::Component => "component type",
            Source::Plugin => "plugin",
            Source::Port => "script input/output",
            Source::UserObject => "user object details",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Source::FileName => "file_name",
            Source::Folder => "folder",
            Source::InsideArchivePath => "inside_archive_path",
            Source::Definition => "definition",
            Source::Group => "group",
            Source::Panel => "panel",
            Source::Note => "note",
            Source::Nickname => "nickname",
            Source::Script => "script",
            Source::Value => "value",
            Source::Component => "component",
            Source::Plugin => "plugin",
            Source::Port => "port",
            Source::UserObject => "user_object",
        }
    }

    /// Text the person wrote themselves, inside the file.
    pub fn is_authored_content(self) -> bool {
        matches!(
            self,
            Source::Definition
                | Source::Group
                | Source::Panel
                | Source::Note
                | Source::Nickname
                | Source::Script
                | Source::Value
                | Source::Port
                | Source::UserObject
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Evidence {
    pub term: String,
    pub strength: Strength,
    pub source: Source,
    /// The words around the match, as they appear.
    pub text: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub in_cluster: bool,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Strong,
    Possible,
    Weak,
    /// Nothing matched, but the inside couldn't be checked.
    Inconclusive,
}

impl Confidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Confidence::Strong => "strong",
            Confidence::Possible => "possible",
            Confidence::Weak => "weak",
            Confidence::Inconclusive => "inconclusive",
        }
    }
}

enum Mode {
    /// Must be a whole word, or begin a compound like "palmleaf".
    Word,
    /// Anywhere, after squashing case and separators.
    Squashed,
    /// Anywhere in the lowercase text as typed (numbers like 137.5).
    Literal,
}

struct Pattern {
    needle: String,
    mode: Mode,
}

pub struct Matcher {
    terms: Vec<(Term, Vec<Pattern>)>,
}

const COMPOUND_TAILS: &[&str] = &[
    "tree", "trees", "leaf", "leaves", "frond", "fronds", "scape", "scapes", "trunk", "trunks", "stalk", "stalks",
    "stem", "stems", "seed", "seeds", "form", "forms", "grove", "oil", "s", "es",
];

fn squash(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).flat_map(char::to_lowercase).collect()
}

impl Matcher {
    pub fn new(terms: &[Term]) -> Matcher {
        let terms = terms
            .iter()
            .filter(|t| !t.word.trim().is_empty())
            .map(|t| {
                let mut patterns: Vec<Pattern> = Vec::new();
                for raw in std::iter::once(&t.word).chain(t.variants.iter()) {
                    let raw = raw.trim().to_lowercase();
                    if raw.is_empty() {
                        continue;
                    }
                    let squashed = squash(&raw);
                    let ascii_word = squashed.chars().all(|c| c.is_ascii_alphabetic());
                    let pattern = if raw.chars().any(|c| c.is_ascii_digit()) {
                        Pattern { needle: raw.clone(), mode: Mode::Literal }
                    } else if ascii_word && squashed.len() <= 5 && !raw.contains(' ') {
                        Pattern { needle: squashed, mode: Mode::Word }
                    } else {
                        Pattern { needle: squashed, mode: Mode::Squashed }
                    };
                    if !patterns.iter().any(|p| p.needle == pattern.needle) {
                        patterns.push(pattern);
                    }
                }
                (t.clone(), patterns)
            })
            .collect();
        Matcher { terms }
    }

    /// Every search word found in `text`, each once, with the words around it.
    pub fn find(&self, text: &str) -> Vec<(Term, String)> {
        if text.trim().is_empty() {
            return Vec::new();
        }
        let lower = text.to_lowercase();
        let squashed = squash(text);
        let tokens = tokens(text);
        let mut out = Vec::new();
        for (term, patterns) in &self.terms {
            let hit = patterns.iter().find_map(|p| match p.mode {
                Mode::Literal => lower.find(&p.needle).map(|i| {
                    // Lowercasing can change byte lengths outside ASCII; then quote the whole text.
                    if lower.len() == text.len() && text.is_char_boundary(i) {
                        context(text, i, p.needle.len())
                    } else {
                        crate::gh::digest::excerpt(text, 90)
                    }
                }),
                Mode::Squashed => squashed.contains(&p.needle).then(|| locate(text, &p.needle)),
                Mode::Word => tokens
                    .iter()
                    .find(|tok| {
                        tok.as_str() == p.needle
                            || tok.strip_prefix(p.needle.as_str()).is_some_and(|rest| {
                                COMPOUND_TAILS.contains(&rest) || rest.chars().all(|c| c.is_ascii_digit())
                            })
                    })
                    .map(|tok| locate(text, tok)),
            });
            if let Some(excerpt) = hit {
                out.push((term.clone(), excerpt));
            }
        }
        out
    }
}

/// Words, split at separators, case changes (PalmLeaf) and letter/digit changes (palm01).
fn tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut prev: Option<char> = None;
    for c in text.chars() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev = None;
            continue;
        }
        if let Some(p) = prev {
            let camel = p.is_lowercase() && c.is_uppercase();
            let digit_edge = p.is_ascii_digit() != c.is_ascii_digit();
            if (camel || digit_edge) && !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        }
        cur.extend(c.to_lowercase());
        prev = Some(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// The words around the first place `needle` (squashed) occurs in `text`.
fn locate(text: &str, needle: &str) -> String {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut squashed = String::new();
    let mut origin = Vec::new();
    for (i, (byte, c)) in chars.iter().enumerate() {
        if c.is_alphanumeric() {
            for lc in c.to_lowercase() {
                squashed.push(lc);
                origin.push((i, *byte));
            }
        }
    }
    match squashed.find(needle) {
        Some(pos) => {
            let start_char = squashed[..pos].chars().count();
            let end_char = start_char + needle.chars().count() - 1;
            let (_, start_byte) = origin[start_char];
            let (last_i, _) = origin[end_char.min(origin.len() - 1)];
            let end_byte = chars.get(last_i + 1).map(|(b, _)| *b).unwrap_or(text.len());
            context(text, start_byte, end_byte - start_byte)
        }
        None => crate::gh::digest::excerpt(text, 90),
    }
}

/// Up to 40 characters either side of `text[start..start + len]`, on one line.
fn context(text: &str, start: usize, len: usize) -> String {
    let end = (start + len).min(text.len());
    let end = (end..=text.len()).find(|&i| text.is_char_boundary(i)).unwrap_or(text.len());
    let before: Vec<char> = text[..start].chars().rev().take(41).collect();
    let after: Vec<char> = text[end..].chars().take(41).collect();
    let lead = if before.len() > 40 { "…" } else { "" };
    let tail = if after.len() > 40 { "…" } else { "" };
    let before: String = before.iter().take(40).rev().collect();
    let after: String = after.iter().take(40).collect();
    let snippet = format!("{before}{}{after}", &text[start..end]);
    let flat = snippet.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("{lead}{flat}{tail}")
}

/// How much a set of evidence says this file belongs to the palm research.
pub fn judge(evidence: &[Evidence]) -> (Confidence, f64) {
    if evidence.is_empty() {
        return (Confidence::Weak, 0.0);
    }
    let mut seen = std::collections::HashSet::new();
    let mut score = 0.0;
    let mut distinct_terms = std::collections::HashSet::new();
    for e in evidence {
        if seen.insert((e.term.as_str(), e.source)) {
            score += e.strength.weight() * e.source.weight();
        }
        if e.strength != Strength::Related {
            distinct_terms.insert(e.term.as_str());
        }
    }
    score += 0.25 * distinct_terms.len().saturating_sub(1) as f64;
    let strong_direct =
        evidence.iter().any(|e| e.strength == Strength::Strong && !matches!(e.source, Source::Folder | Source::Plugin));
    let palm_named = evidence.iter().any(|e| {
        e.strength == Strength::Palm
            && matches!(e.source, Source::FileName | Source::Definition | Source::InsideArchivePath)
    });
    let backed_inside = evidence.iter().any(|e| e.source.is_authored_content() || e.source == Source::Component);
    let any_strong = evidence.iter().any(|e| e.strength == Strength::Strong);
    let palm_beyond_folder = evidence.iter().any(|e| e.strength == Strength::Palm && e.source != Source::Folder);
    let confidence = if strong_direct || (palm_named && backed_inside) || score >= 1.6 {
        Confidence::Strong
    } else if any_strong || palm_beyond_folder || score >= 0.8 {
        Confidence::Possible
    } else {
        Confidence::Weak
    };
    (confidence, (score * 100.0).round() / 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        Matcher::new(&default_terms()).find(text).into_iter().map(|(t, _)| t.word).collect()
    }

    #[test]
    fn matches_spellings_and_separators() {
        assert_eq!(words("Palm_Tree_v3.gh"), vec!["palm tree", "palm"]);
        assert_eq!(words("PALMSCAPES final"), vec!["Palmscapes", "palm"]);
        assert!(words("phylotaxis-study.ghx").contains(&"phyllotaxis".to_string()));
        assert!(words("Phyllotactic spiral").contains(&"phyllotaxis".to_string()));
        assert!(words("PalmLeaf01").contains(&"palm".to_string()));
        assert!(words("palmleaf").contains(&"palm".to_string()));
        assert!(words("angle = 137.5").contains(&"golden angle".to_string()));
        assert!(words("مشروع النخيل").contains(&"نخيل".to_string()));
    }

    #[test]
    fn ignores_lookalikes() {
        assert!(words("napalm.gh").is_empty());
        assert!(words("Palmer house.gh").is_empty());
        assert!(words("stalker").is_empty());
        assert!(words("13750 points").is_empty());
    }

    #[test]
    fn excerpts_show_the_match_in_place() {
        let hits = Matcher::new(&default_terms()).find("Group: Phyllotaxis spiral of 144 seeds");
        assert_eq!(hits[0].1, "Group: Phyllotaxis spiral of 144 seeds");
        let long = format!("{} palm tree {}", "x ".repeat(60), "y ".repeat(60));
        let hit = &Matcher::new(&default_terms()).find(&long)[0].1;
        assert!(hit.starts_with('…') && hit.ends_with('…') && hit.contains("palm tree"));
    }

    #[test]
    fn judges_confidence() {
        let ev = |term: &str, strength, source| Evidence {
            term: term.into(),
            strength,
            source,
            text: String::new(),
            in_cluster: false,
        };
        assert_eq!(judge(&[ev("phyllotaxis", Strength::Strong, Source::Group)]).0, Confidence::Strong);
        assert_eq!(judge(&[ev("Palmscapes", Strength::Strong, Source::Folder)]).0, Confidence::Possible);
        assert_eq!(judge(&[ev("palm", Strength::Palm, Source::FileName)]).0, Confidence::Possible);
        assert_eq!(
            judge(&[ev("palm", Strength::Palm, Source::FileName), ev("frond", Strength::Related, Source::Panel)]).0,
            Confidence::Strong
        );
        assert_eq!(judge(&[ev("fibonacci", Strength::Related, Source::Script)]).0, Confidence::Weak);
        assert_eq!(judge(&[ev("palm", Strength::Palm, Source::Folder)]).0, Confidence::Weak);
    }
}
