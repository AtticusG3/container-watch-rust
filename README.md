# Script Reloader (Rust)

Docker container management over SSH — Windows Slint GUI. Functionally equivalent to the [reference WPF app](https://github.com/AtticusG3/docker-script-reloader), plus interactive **Exec** (popout terminal via `docker exec -it`).

**Authored by Kevyn Watkins** (shown in the status bar).

## Features

- SSH password auth to a Linux host; all Docker commands run remotely
- Container table: Name, State, Status, Image, ID (read-only, Ctrl/Shift multi-select)
- Batch stop / start / restart (one SSH exec per action, POSIX single-quoted args)
- Restart all running (two-step: `docker ps -q`, then batched `docker restart --time 10`)
- **Exec**: open one selected container in a popout terminal (`docker exec -it ... bash`)
- Remember connection (DPAPI-encrypted password in `%LocalAppData%\ContainerWatch\session.json`; separate from the .NET Script Reloader app)
- Layered config: embedded defaults, sidecar `appsettings.json`, env overrides

## Prerequisites (Windows)

| Tool | Purpose |
|------|---------|
| Rust stable (MSVC) | Build |
| Visual Studio Build Tools | MSVC linker |
| OpenSSH client (`ssh.exe`) | All SSH (list/lifecycle and Exec; preinstalled on Windows 10/11) |
| Windows Terminal (`wt.exe`) optional | Preferred exec popout host |

Remote Linux host: user must be in the `docker` group (or use root).

## Build

```powershell
cargo build --release --target x86_64-pc-windows-msvc
```

Output: `target\x86_64-pc-windows-msvc\release\script-reloader.exe`

Run:

```powershell
cargo run --release
```

## Portable publish

Copy `script-reloader.exe` anywhere. Optional sidecar `appsettings.json` beside the exe overrides embedded defaults.

Measured release size (this tree, `opt-level = "z"`, `lto = "fat"`): **~4.6 MB** (~4,862,464 bytes).

Reference .NET 8 portable WPF build is typically **~70–100+ MB** (framework-dependent or self-contained). This Rust build avoids .NET/Electron/WebView runtimes.

## Configuration

Precedence (lowest to highest):

1. Embedded `appsettings.json` (via `include_str!`)
2. Sidecar `appsettings.json` next to the exe
3. Environment: `Ssh__Host`, `Ssh__Port`, `Ssh__Username`, `Ssh__Password`
4. Saved session when loaded (UI fields)

```json
{
  "Ssh": {
    "Host": "",
    "Port": 22,
    "Username": "",
    "CommandTimeoutSeconds": 120
  }
}
```

Empty UI password falls back to config/env default. Timeout clamped to 5–600 s.

## Exec popout

When exactly one container is selected, **Exec** opens a separate terminal:

1. `wt.exe` if on PATH, else `cmd /C start` + `cmd /K`
2. Runs: `ssh -t -p <port> <user>@<host> "docker exec -it '<target>' bash"`
3. Password via short-lived `SSH_ASKPASS` helper (temp `.cmd`, deleted after spawn)

**Tradeoff:** Exec uses system `ssh.exe` (same as list/lifecycle SSH). Requires OpenSSH Client (preinstalled on Windows 10/11).

**Security:** Exec grants full interactive shell inside the container. Password is never written to disk in plain text; askpass temp file is removed immediately after launch. Do not use on untrusted hosts.

If `bash` is missing in the container, the session may fail; use `/bin/sh` manually or rebuild the image with bash.

## Dependencies (justification)

| Crate | Role |
|-------|------|
| `slint` | Native GUI (`backend-winit`, `renderer-femtovg` only) |
| `serde` / `serde_json` | Config, docker JSON lines, session file |
| `windows` | DPAPI protect/unprotect (targeted features only) |
| `base64` | Session ciphertext encoding |

All SSH (list/lifecycle and Exec) uses the **Windows OpenSSH client** (`ssh.exe`) with a short-lived `SSH_ASKPASS` helper. No bundled libssh2/OpenSSL.

## Release profile

`Cargo.toml` uses `opt-level = "z"` (size over speed), `lto = "fat"`, `codegen-units = 1`, `strip = true`, `panic = "abort"`.

Slint features: `compat-1-2`, `backend-winit`, `renderer-femtovg` (minimal Windows set).

## Tests

```powershell
cargo test
```

Covers JSON/Names parsing, shell quoting, confirm messages, session JSON, DPAPI roundtrip (Windows).

## License

MIT — Copyright (c) 2026 Kevyn Watkins
