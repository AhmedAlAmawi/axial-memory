# The brief, and where each part stands

The research brief describes two agents. Axial Memory 0.1 is Agent 01 in full
plus an optional AI explanation; Agent 02 is the next stage. The brief's own
release gate applies: Agent 01's inventory is verified before bulk
interpretation, and Agent 02 proves capture accuracy on one definition first.

## Agent 01: Grasshopper Archivist

**Mission.** Find every accessible Grasshopper definition on both laptops,
whatever its name or project; rank possible links to Palmscapes, phyllotaxis,
stalks, palm trees and related words; keep provenance; report every gap.

| Requirement | Status | Where |
| --- | --- | --- |
| Whole-disk enumeration, hidden folders, old projects, temp/autosave, removable and mounted drives, synced cloud folders | Done | `scan.rs`, `roots.rs` |
| Not offline cloud files or disconnected media | Done: cloud placeholders listed by name, never downloaded (opt-in setting) | `cloud.rs` |
| Read-only: no renaming, moving, deleting, executing or uploading | Done, checked by the acceptance test | `tests/seeded.rs` |
| Recursive traversal without symlink loops | Done: links are never followed | `scan.rs` |
| Logs of permission and corruption errors | Done: every issue recorded with a reason and advice | `ACCESS_ERRORS.csv` |
| Resumable scanning | Not yet (a full laptop scan takes minutes; it can be stopped and re-run) | V2 |
| Master inventory of all .gh/.ghx | Done, plus backup variants and renamed files detected by content | `ALL_GRASSHOPPER_FILES.csv` |
| Archive members with safety limits | Done for .zip (one nested level, size and count limits); .rar/.7z listed by name | V2 for .rar/.7z |
| SHA-256 duplicate groups, path/time/size, cross-laptop comparison | Done | `DUPLICATE_CLUSTERS.csv`, Laptops tab |
| Case-insensitive name/path matching with spelling variants | Done, words editable in Settings | `terms.rs` |
| Text from .ghx XML and .gh binaries; high-confidence vs inconclusive | Done: both formats are fully parsed, so this exceeds "detectable text" | `gh/` |
| Native parsing: names, group labels, panels, scripts, references | Done (no Rhino needed), including clusters and user objects | `gh/digest.rs` |
| A binary non-match is never proof of irrelevance | Done: unreadable files are "inconclusive" and listed | `results.rs` |
| Secondary: .ghuser, .3dm, .3dmbak, .py, .cs, .zip, documents | Done as switchable modules; .3dm is listed and hashed but not read inside | V2 for .3dm text |

**Outputs.** In the report folder: `ALL_GRASSHOPPER_FILES.csv` and
`axial_memory.sqlite` (machine, full path, type, size, dates, hash, status);
`PALM_CANDIDATES.csv` (matched terms, evidence source, confidence, reason) and
`PALM_EVIDENCE.csv` (every match with its text); `ACCESS_ERRORS.csv`;
`COVERAGE.csv` (per drive and per laptop totals); `DUPLICATE_CLUSTERS.csv`;
`REVIEW_SET.json` (project families, distinct versions, companion Rhino models,
plugins and references, ordered by palm relevance: the handoff to Agent 02).

**Acceptance criteria**, all covered by `core/tests/seeded.rs`:

- every .gh/.ghx in a seeded folder is listed (17 files: nested, hidden, Unicode, Arabic, uppercase, backups, renamed, inside zips, damaged);
- no originals altered (bytes and modification times compared);
- every skipped or unreadable path reported (locked folder, damaged files);
- duplicates identified (across names and backup copies, and across laptops);
- every palm match traceable to concrete evidence.

Whole-machine completeness is only claimed for places actually searched; the
Gaps tab and `COVERAGE.csv` say which those were.

## Optional AI explanation

For any readable definition: a text extract (components in dataflow order,
wires, group labels, panel text, slider values, scripts, plugins, references)
is shown to her, and only on Send goes to Claude or ChatGPT with her own key.
The answer follows fixed sections: what it does, stages, palm and phyllotaxis
relevance with cited evidence, what to change, needs, unsure. It never claims
scientific validity.

## Agent 02: Grasshopper Interpreter (next)

Turn selected definitions into a traceable computational atlas: dependency
graph, stage sequence by dataflow, staged solving in Rhino, paired canvas and
viewport captures with fixed camera settings, a stage manifest tied to file
hashes, and a publication kit.

Already in place: the dependency graph, dataflow order, groups, plugin list
and references for every parsed definition (`gh/digest.rs`), and the
prioritized review set. Still needed: a Rhino 8 plugin (Mac and Windows) that
opens a copy of a definition, solves it, isolates stages, and captures the
canvas and viewport; then batch runs, Karamba3D-specific analysis, parameter
sweeps and layouts. MVP per the brief: one known definition, three to six
approved stages, paired captures.
