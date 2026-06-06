# Agent entry point

## Build

On Windows:

```powershell
cargo build --release
```

Target: `x86_64-pc-windows-msvc`. Release profile is tuned for a small portable exe.

## Conventions

- **Slint + minimal deps:** only add crates when a feature needs them.
- **Layout:** core logic under `src/` modules; Slint UI in `ui/`; thin `src/ui/` callbacks/view model.
- **SemVer:** version lives in `Cargo.toml`; update `CHANGELOG.md` on release.
- **Strings:** ASCII-only in UI and operator-facing logs.
- **Secrets:** never commit passwords, keys, or DPAPI session data.

## Out of scope until feature prompts

SSH, Docker commands, DPAPI session persistence, container table binding, exec popout terminal.
