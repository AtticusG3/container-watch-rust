# Script Reloader (Rust)

Docker container management over SSH — Windows Slint GUI.

**Bootstrap status:** scaffold only; features not implemented yet.

## Planned stack

- Rust
- Slint (native GUI)
- SSH (remote host access)
- Remote Docker

Reference WPF app: [docker-script-reloader](https://github.com/AtticusG3/docker-script-reloader)

## Development prerequisites

Verified on Windows 10/11 (win-x64):

| Tool | Version | Purpose |
|------|---------|---------|
| Git | 2.52.0.windows.1 | Version control |
| Rust (stable) | rustc 1.96.0, cargo 1.96.0 | Build system |
| rustup | 1.29.0 | Toolchain manager |
| MSVC target | x86_64-pc-windows-msvc | Native Windows build |
| Visual Studio Build Tools | 2022 (MSVC 14.44) + VS 18 Build Tools (MSVC 14.50) | MSVC linker |
| CMake | 4.3.3 | Native dependency builds |
| OpenSSH client | OpenSSH_for_Windows_9.5p2 | SSH (preinstalled) |

Slint features used for bootstrap: `compat-1-2`, `backend-winit`, `renderer-femtovg` (minimal set that builds on Windows).

## Build

Debug:

```powershell
cargo build
```

Release (portable single exe target):

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

Run:

```powershell
cargo run --release
```

Release binary: `target\x86_64-pc-windows-msvc\release\script-reloader.exe` (or `target\release\script-reloader.exe` when building without an explicit target).

## Security

Never commit passwords, SSH keys, or session tokens. Use environment variables or local-only config files (see `.gitignore`).

## License

MIT — Copyright (c) 2026 Kevyn Watkins
