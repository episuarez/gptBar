# Contributing

Windows project. You need a Windows machine (or VM) with:

- **Rust stable** — `rustup update stable`
- **Node.js >= 18**
- **VS Build Tools** with C++ workload
- **cargo-tauri** — `cargo install tauri-cli`
- **WebView2 runtime** — ships with Windows 10 1803+

## Setup

```bat
git clone https://github.com/episuarez/gptBar
cd gptBar
npm install
cargo build --manifest-path src-tauri/Cargo.toml
```

## Dev loop

```bat
scripts\dev.bat                         :: Tauri hot-reload (Rust + Svelte)
cargo test --manifest-path src-tauri/Cargo.toml   :: all tests
```

## Code style

```bat
cargo fmt --manifest-path src-tauri/Cargo.toml --all
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run check
```

No warnings allowed on CI. Fix clippy before pushing.

## Commit format

[Conventional Commits](https://www.conventionalcommits.org/). Subject <= 50 chars. Body explains *why*, not what.

```
feat: add Mistral provider
fix: xai balance parsing on zero credits
docs: add provider setup guide
chore: bump tokio to 1.38
```

No references to AI tools in commit messages.

## Branch model

- `main` — stable, always green
- Feature branches → PR → squash merge to main

## Pull requests

- Describe *what* and *why* in the PR body
- All CI checks must pass (fmt → clippy → test → svelte-check)
- One logical change per PR

## Releases

Releases are tag-driven. When a commit on `main` is ready to ship:

1. Update `version` in `src-tauri/tauri.conf.json`
2. Commit: `chore: bump version to X.Y.Z`
3. Update `CHANGELOG.md` — move `[Unreleased]` items under the new version
4. Push the commit
5. Tag: `git tag vX.Y.Z && git push origin vX.Y.Z`

The `release` workflow validates that the tag matches `tauri.conf.json`, builds NSIS + MSI installers, and creates a draft GitHub Release. Review and publish from the Releases page.

## Scope rules

- Provider logic stays in `src-tauri/src/providers/` — no UI imports
- IPC commands in `src-tauri/src/commands.rs` are thin wrappers; no business logic
- New dependencies need justification in the PR description
