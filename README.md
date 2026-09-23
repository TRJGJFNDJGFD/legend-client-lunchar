# Legend Client — Launcher

The desktop launcher (Tauri 2 + React + Rust) for Legend Client.

- `core/` — pure Rust logic (Java detection, profiles, launch, mods, auth, Fabric/version resolution). No GUI deps, runs `cargo test` anywhere.
- `src-tauri/` — thin Tauri command layer wrapping `core/`.
- `src/` — React UI.

## Building

```
npm install
npm run tauri build
```

Windows builds are produced automatically by `.github/workflows/build-windows.yml` on every push to `main`, published as a GitHub Release with a real `.exe`/`.msi` installer attached.

## Status

See the sibling `legend-client` repo's `docs/ROADMAP.md` for exactly what's real, tested, and what's still `[~]` unverified due to this dev environment's network restrictions.
