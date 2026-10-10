# Axial Memory

Finds every Grasshopper definition on a computer, reads inside each one without
Rhino, and shows which ones belong to a palm research project (Palmscapes,
phyllotaxis, stalks, palm trees), with the evidence for each. Works on Mac and
Windows, combines the two laptops' results, and writes a report folder of CSVs
and a SQLite database. Read-only: nothing is changed, moved or uploaded.

**Download:** https://ahmedalamawi.github.io/axial-memory/
([Mac](https://github.com/AhmedAlAmawi/axial-memory/releases/latest/download/Axial-Memory-Mac.dmg) ·
[Windows](https://github.com/AhmedAlAmawi/axial-memory/releases/latest/download/Axial-Memory-Windows-setup.exe))

Install steps: [Mac](docs/INSTALL-MAC.md) · [Windows](docs/INSTALL-WINDOWS.md).
What the research brief asks for and where each part stands: [docs/SPEC.md](docs/SPEC.md).

## Development

Needs Rust (stable), Node 24 and pnpm.

```bash
pnpm install
pnpm dev      # the app
pnpm ui       # the interface in a browser on mock data
make check    # tests, lints and typecheck, as CI runs them
```

`AGENTS.md` explains the architecture, the rules and the decisions behind it.
Releases: [docs/RELEASING.md](docs/RELEASING.md).

MIT licensed.
