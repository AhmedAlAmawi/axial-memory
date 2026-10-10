# Axial Memory

A desktop app (Mac and Windows) that finds every Grasshopper definition on a
computer, reads inside each one without Rhino, and ranks the ones linked to a
palm research project (Palmscapes, phyllotaxis, stalks, palm trees). It is the
"Agent 01: Grasshopper Archivist" of the research brief in `docs/SPEC.md`, plus
an optional AI explanation of a single definition.

The person using it is an architect and researcher, not technical, with one Mac
and one Windows laptop. She installs from a link, presses Search, and reads the
results. Every screen and message is written for her: plain words, no jargon,
always what to do next. Keep it that way.

The repo is public. Keep the researcher's name and personal details out of it.

## Hard rules

- **Read-only.** Never write, move, rename, delete or execute anything in a
  scanned location. Files are opened for reading only; Grasshopper definitions
  are parsed, never run. The acceptance test (`core/tests/seeded.rs`) checks
  bytes and modification times before and after a scan.
- **No uploads.** `core/` has no network code. The only network calls are the
  updater and `src-tauri/src/ai.rs`, which sends one definition's text extract
  (never the file) to Claude or ChatGPT with her own key, after she has seen
  exactly what will be sent and clicked Send.
- **Cloud placeholders are never opened** unless she turns on "Download
  cloud-only files". Opening one makes iCloud/OneDrive/Dropbox/Google Drive
  download it. Detection: macOS `SF_DATALESS`, old `.name.icloud` stubs,
  Windows `RECALL_ON_DATA_ACCESS | RECALL_ON_OPEN | OFFLINE`.
- **Never claim completeness you didn't check.** Every place the scan skipped
  or couldn't read is recorded as an issue with a reason and advice.
- **A file whose inside can't be read is "inconclusive", never "not related".**
- **The Rhino script leaves no trace.** It only toggles previews, and puts back
  every preview, the preview mode and filter, the selection and the modified
  flag in a `finally`. It never saves the definition. Keep it IronPython 2.7
  compatible (Rhino 7): no f-strings, no `nonlocal`; check every new Rhino or
  Grasshopper call against the 7.x XML docs in the `Grasshopper` and
  `RhinoCommon` NuGet packages, since nobody here can run Rhino.

## Layout

| Path | What |
| --- | --- |
| `core/` | The engine (Rust, no UI, no network). Also builds `axial-scan`, a headless CLI. |
| `core/src/gh/` | Grasshopper archives: `binary.rs` (.gh/.ghuser: raw deflate + GH_IO chunk tree), `xml.rs` (.ghx), `digest.rs` (components, wires, groups, panels, scripts, plugins, references), `write.rs` (test-only writer). |
| `core/src/scan.rs` | Walks drives (walkdir, links never followed), classifies, queues files to worker threads, reads zips in memory with limits, hashes (SHA-256), collects. |
| `core/src/classify.rs` | File kinds by name, location tags (autosave, trash, cloud folder…), skipped system folders. |
| `core/src/terms.rs` | Search words, matching (case/separator-insensitive; short words must be whole words), evidence, confidence. |
| `core/src/results.rs` | Merges laptop scans: candidates (one per distinct content), duplicate clusters, version families, totals, comparison. |
| `core/src/store.rs` | The `.axialscan` file: one SQLite database per laptop scan. |
| `core/src/export.rs` | The report folder: the spec's CSVs, merged SQLite, `REVIEW_SET.json`. |
| `core/src/explain.rs` | The text extract and system prompt for AI explanations. |
| `core/src/map.rs` | The map of one definition, drawn only from what the file saved: components at their canvas positions, wires port to port, groups in their own colours, search-word matches marked, plus Grasshopper's own preview image. Rendered by `src/components/DefinitionMap.tsx`. |
| `rhino/axial-capture.py` | Runs inside Rhino 7/8 (IronPython 2.7): one picture per component in dataflow order, each showing only that component's preview, plus `index.html`, `manifest.json` and `log.txt`. Attached to every release. See `docs/RHINO.md`. |
| `src-tauri/` | The Tauri 2 app: commands, settings, AI calls, updater. |
| `src/` | The React interface. `mock.ts` stands in for the backend in a plain browser. |
| `docs/` | Spec status, install guides, releasing, and the download page (GitHub Pages). |

## How it decides

- **Found:** every `.gh`, `.ghx`, `.ghcluster` and backup variant (`.gh.bak`, `.ghx~`…), plus `.bak/.tmp/.old…` files whose first bytes are a Grasshopper archive. Also, by module: `.ghuser`, `.3dm/.3dmbak`, `.py/.cs` only next to a definition or named after the research, `.zip` opened for definitions inside (nested one level), documents only when their name has a strong or palm word.
- **Skipped on purpose (and reported):** OS folders at the system root (`/System`, `/usr`, `C:\Windows`…), Spotlight/fseventsd indexes, `/Volumes` (searched as separate drives). Network and backup drives are listed but off by default.
- **Evidence:** search words found in the file name, folder names, the path inside a zip, and inside the file (group labels, panels, scribbles, renamed components, scripts, typed values, component types, plugins, the name saved in the file).
- **Confidence:** strong = a strong word in the name or inside, or a palm word in the name backed by something inside; possible = strong word only in a folder or plugin, or a palm word; weak = only related words; inconclusive = nothing matched and the inside couldn't be read.

## Commands

```bash
pnpm install
pnpm dev                 # the app, with hot reload
pnpm ui                  # the interface alone in a browser, on mock data (add ?empty for first run)
cargo test -p axial-core # engine tests, including the spec's acceptance test
AXIAL_SAMPLES_DIR=/path/to/gh/files cargo test -p axial-core --test real_samples -- --nocapture
cargo run -p axial-core --release --bin axial-scan -- --out /tmp/out ~/Documents   # headless scan + report
cargo run -p axial-core --bin axial-scan -- --map some.gh > map-some.json            # then pnpm ui, open /?map=/map-some.json
make check               # everything CI runs
make release VERSION=0.2.0
```

`CARGO_TARGET_DIR` may be set by the environment; built bundles land under it.

## Releasing and updates

`make release VERSION=x.y.z` bumps `package.json` (the app reads its version
from there), commits, tags and pushes. `.github/workflows/release.yml` builds a
universal Mac DMG and a Windows installer, publishes them with fixed names
(`Axial-Memory-Mac.dmg`, `Axial-Memory-Windows-setup.exe`) and updates
`latest.json`; installed copies show "A new version is ready". Updates are
signed with the key in `~/.tauri/axial-memory.key` on Ahmed's Mac (GitHub
secrets `TAURI_SIGNING_PRIVATE_KEY` and `_PASSWORD`). Losing that key means
installed copies can't update and must be reinstalled. See `docs/RELEASING.md`.

The Mac app is ad-hoc signed (no paid Apple account), so the first open needs
"Open Anyway" (`docs/INSTALL-MAC.md`). macOS ties folder permissions and Full
Disk Access to the signature, which changes every build: after an update she may
be asked to Allow folders again, and Full Disk Access may need turning off and
on. A paid Developer ID (or a stable self-signed identity) would fix both.

## Not built yet (see docs/SPEC.md)

- Agent 02 (the interpreter): the step-by-step capture exists as a Rhino
  script. Still to do: showing the captures inside the app beside the map,
  capturing the Grasshopper canvas per step, charts for number steps,
  parameter sweeps, and batch runs over `REVIEW_SET.json`.
- Resumable scans, `.rar/.7z` contents, Time Machine backups, text inside `.3dm`.
- A shared folder that merges the two laptops without moving a file by hand.
