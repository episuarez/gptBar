<div align="center">
  <img src=".github/screenshots/dashboard.png" width="260" alt="Usage dashboard">
  &nbsp;&nbsp;
  <img src=".github/screenshots/tabs.png" width="260" alt="Multi-provider tabs">
  &nbsp;&nbsp;
  <img src=".github/screenshots/settings.png" width="260" alt="Settings">
  <h1>GPTBar</h1>
  <p>Monitor AI provider usage from your system tray — Claude, OpenAI, Gemini, Codex and xAI in one place.</p>
  <p>
    <a href="https://github.com/episuarez/gptBar/actions"><img src="https://img.shields.io/github/actions/workflow/status/episuarez/gptBar/ci.yml?style=flat-square&label=CI" alt="CI"></a>
    <a href="https://github.com/episuarez/gptBar/releases"><img src="https://img.shields.io/github/v/release/episuarez/gptBar?style=flat-square&label=release" alt="Release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="License"></a>
    <a href="https://github.com/episuarez/gptBar/releases"><img src="https://img.shields.io/badge/platform-Windows-0078d4?style=flat-square&logo=windows&logoColor=white" alt="Platform"></a>
    <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-stable-orange?style=flat-square&logo=rust&logoColor=white" alt="Rust"></a>
    <a href="https://tauri.app"><img src="https://img.shields.io/badge/tauri-2-24c8db?style=flat-square&logo=tauri&logoColor=white" alt="Tauri"></a>
  </p>
</div>

## Install

Download the latest `.exe` (NSIS) or `.msi` installer from the [Releases](https://github.com/episuarez/gptBar/releases) page.

Requires Windows 10 version 1803 or later.

## What it does

| Feature | Description |
|---------|-------------|
| **5 providers** | Claude (OAuth), OpenAI, Gemini, Codex and xAI — toggle each on/off |
| **Tray icon** | Color-coded: cyan (ok) → amber (warning) → red (critical) |
| **API key management** | Enter keys directly in Settings — stored in Windows Credential Manager |
| **Usage history** | Sparkline charts with JSON/CSV export |
| **Desktop notifications** | Configurable thresholds and per-provider cooldowns |
| **Auto-refresh** | Every 10 minutes (configurable), client-side rate limiting |

### Provider details

| Provider | Auth | What it tracks |
|----------|------|----------------|
| **Claude** (Anthropic) | OAuth via Claude Code CLI | Session, weekly and model limits |
| **OpenAI** | API key | Billing usage and limits |
| **Gemini** (Google) | API key | Quota usage |
| **Codex** | API key | Token usage (uses OpenAI endpoints) |
| **xAI** (Grok) | API key | Token and balance usage |

## Build from source

**Prerequisites:** [Rust stable](https://rustup.rs), Node.js 18+, [VS Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) with the C++ workload and WebView2 runtime (ships with Windows 10 1803+).

```bat
git clone https://github.com/episuarez/gptBar.git
cd gptBar
npm install
scripts\dev.bat      # hot-reload dev mode
scripts\build.bat    # production build → src-tauri/target/release/bundle/
```

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE) — Copyright (c) 2026 episuarez

Inspired by [CodexBar](https://github.com/steipete/CodexBar) by steipete.
