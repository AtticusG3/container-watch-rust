# Releasing

## Versioning

Follow [Semantic Versioning](https://semver.org/): `MAJOR.MINOR.PATCH`.

1. Update version in `Cargo.toml`.
2. Move `[Unreleased]` entries in `CHANGELOG.md` to a dated release section.
3. Tag: `vMAJOR.MINOR.PATCH` (e.g. `v1.1.0`).
4. Build release: `cargo build --release --target x86_64-pc-windows-msvc`.

## Artifacts

Release binary: `target\x86_64-pc-windows-msvc\release\script-reloader.exe` (strip + LTO enabled).

Publishing release artifacts to GitHub Releases is not automated yet.
