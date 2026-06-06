# Agent entry point

## Build

On Windows:

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

Target: `x86_64-pc-windows-msvc`. Release profile is tuned for a small portable exe.

## Conventions

- **Slint + minimal deps:** only add crates when a feature needs them.
- **Layout:** core logic under `src/` modules; Slint UI in `ui/`; thin `src/ui/` callbacks/view model.
- **SemVer:** version lives in `Cargo.toml`; update `CHANGELOG.md` on release.
- **Strings:** ASCII-only in UI and operator-facing logs.
- **Secrets:** never commit passwords, keys, or DPAPI session data.

## Architecture

| Module | Role |
|--------|------|
| `config/` | Embedded + sidecar + env settings |
| `session/` | DPAPI session file (`%LocalAppData%\ContainerWatch\`, not .NET path) |
| `ssh_docker/` | Remote docker commands via system `ssh.exe` |
| `ssh_util/` | SSH_ASKPASS helper + non-interactive ssh runner |
| `exec/` | Popout terminal launcher |
| `ui/` | Slint wiring, worker threads, dialogs |

## Out of scope

SSH key auth UI, Linux/macOS GUI, local Docker, embedded terminal emulator, installer/auto-update.
