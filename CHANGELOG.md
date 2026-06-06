# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.1.0] - 2026-06-06

### Added

- Full Slint GUI: SSH connect, container table, refresh, batch stop/start/restart, restart all running.
- **Exec** (new vs reference WPF 1.1.0): popout terminal with interactive `docker exec -it ... bash`.
- Session remember (`%LocalAppData%\ContainerWatch\session.json`, DPAPI; separate from .NET Script Reloader).
- Layered config: embedded + sidecar `appsettings.json` + env overrides.
- Unit tests for parsing, quoting, session JSON, DPAPI.
- CI workflow (Windows: `cargo test` + release build).

### Changed

- Pure Rust implementation (Slint + system `ssh.exe`) replacing .NET/WPF reference.

## [1.0.0] - 2026-06-06

- Project bootstrap (Slint shell only).

[Unreleased]: https://github.com/AtticusG3/container-watch-rust/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/AtticusG3/container-watch-rust/releases/tag/v1.1.0
[1.0.0]: https://github.com/AtticusG3/container-watch-rust/releases/tag/v1.0.0
