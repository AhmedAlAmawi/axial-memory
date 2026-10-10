# Releasing

```bash
make check                    # what CI runs
make release VERSION=0.2.0    # bump package.json, commit, tag v0.2.0, push
```

The tag starts `.github/workflows/release.yml`, which builds a universal Mac
DMG and a Windows installer (about 15 minutes), publishes them on the GitHub
release with fixed names, and updates `latest.json`. The download links never
change:

- Mac: https://github.com/AhmedAlAmawi/axial-memory/releases/latest/download/Axial-Memory-Mac.dmg
- Windows: https://github.com/AhmedAlAmawi/axial-memory/releases/latest/download/Axial-Memory-Windows-setup.exe
- Page with both and the install steps: https://ahmedalamawi.github.io/axial-memory/

Installed copies check `latest.json` when they open and show "A new version is
ready"; one click installs it and restarts.

## The update signing key

Updates must be signed with `~/.tauri/axial-memory.key` (password in
`~/.tauri/axial-memory.key.password`). Both are also GitHub secrets
(`TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`). Keep a copy
in a password manager: without the key, installed copies can't be updated and
must be reinstalled by hand. The public half is in `src-tauri/tauri.conf.json`.

To set the secrets again:

```bash
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/axial-memory.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD < ~/.tauri/axial-memory.key.password
```

## Signing the Mac app properly (optional)

The Mac app is ad-hoc signed, which is why the first open needs "Open Anyway"
and why macOS may re-ask for folder access after updates. With an Apple
Developer ID ($99 a year), add the `APPLE_CERTIFICATE`,
`APPLE_CERTIFICATE_PASSWORD`, `APPLE_SIGNING_IDENTITY`, `APPLE_ID`,
`APPLE_PASSWORD` and `APPLE_TEAM_ID` secrets, pass them to tauri-action in
`release.yml`, and remove `"signingIdentity": "-"` from `tauri.conf.json`.
