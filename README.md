<div align="center">
  <img src=".github/screenshots/dashboard.png" width="260" alt="Usage dashboard">
  &nbsp;&nbsp;
  <img src=".github/screenshots/tabs.png" width="260" alt="Multi-provider tabs">
  &nbsp;&nbsp;
  <img src=".github/screenshots/settings.png" width="260" alt="Settings">

  <h1>GPTBar</h1>

  <p><b>All your AI usage, one glance away.</b><br>
  A tiny, native Windows tray app that tracks your Claude, OpenAI, Gemini, Codex and xAI limits in real time — so you never hit a rate cap by surprise.</p>

  <p>
    <a href="https://github.com/episuarez/gptBar/releases"><img src="https://img.shields.io/badge/⬇_Download-Windows-0078d4?style=for-the-badge&logo=windows&logoColor=white" alt="Download for Windows"></a>
  </p>

  <p>
    <a href="https://github.com/episuarez/gptBar/actions"><img src="https://img.shields.io/github/actions/workflow/status/episuarez/gptBar/ci.yml?style=flat-square&label=CI" alt="CI"></a>
    <a href="https://github.com/episuarez/gptBar/releases"><img src="https://img.shields.io/github/v/release/episuarez/gptBar?style=flat-square&label=release" alt="Release"></a>
    <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="License"></a>
    <a href="https://github.com/episuarez/gptBar/releases"><img src="https://img.shields.io/badge/platform-Windows-0078d4?style=flat-square&logo=windows&logoColor=white" alt="Platform"></a>
    <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/built_with-Rust-orange?style=flat-square&logo=rust&logoColor=white" alt="Rust"></a>
    <a href="https://tauri.app"><img src="https://img.shields.io/badge/Tauri-2-24c8db?style=flat-square&logo=tauri&logoColor=white" alt="Tauri"></a>
  </p>
</div>

---

## Why GPTBar?

If you live in Claude, ChatGPT, Gemini or Grok, you already know the feeling: you're mid-flow and **slam into a rate limit** with no warning. GPTBar puts every provider's usage in your system tray, color-coded at a glance, and **pings you before you run out** — not after.

- 🎯 **No surprises** — a tray icon that turns amber, then red, as you approach your limits.
- 👁️ **One pane of glass** — Claude, OpenAI, Gemini, Codex and xAI side by side. Stop juggling five dashboards.
- 🔒 **Private by design** — keys live in the Windows Credential Manager, never in a config file. No accounts, no telemetry, no cloud.
- 🪶 **Featherweight & native** — built with Rust + Tauri. A few MB, near-zero idle CPU, lives quietly in your tray.
- 🆓 **Free & open source** — MIT licensed. Audit it, fork it, ship it.

## Install

<a href="https://github.com/episuarez/gptBar/releases"><b>⬇ Download the latest installer</b></a> — grab the `.exe` (NSIS) or `.msi` from the [Releases](https://github.com/episuarez/gptBar/releases) page, run it, done.

Once installed, GPTBar **updates itself** — new signed releases land in-app with a one-click "Install" banner.

> Requires Windows 10 (1803+) or Windows 11. WebView2 ships with the OS.

## Features

| | |
|---|---|
| 🧩 **5 providers** | Claude (OAuth), OpenAI, Gemini, Codex and xAI — toggle each on/off |
| 🚦 **Live tray icon** | Color-coded at a glance: cyan (ok) → amber (warning) → red (critical) |
| 🔔 **Smart alerts** | Desktop notifications at your own thresholds, with per-provider mute & cooldown |
| 📈 **Usage history** | Sparkline trends with one-click JSON / CSV export |
| 🔑 **Secure keys** | Paste keys in Settings — sealed in the Windows Credential Manager |
| 🔄 **Auto-refresh** | Polls every provider on an interval, with built-in client-side rate limiting |
| ⬆️ **Auto-update** | Cryptographically signed in-app updates, or check manually anytime |

### What each provider tracks

| Provider | Auth | Tracks |
|----------|------|--------|
| **Claude** (Anthropic) | OAuth via Claude Code CLI | Session, weekly and model limits |
| **OpenAI** | API key | Billing usage and limits |
| **Gemini** (Google) | API key | Quota usage |
| **Codex** | API key | Token usage (OpenAI endpoints) |
| **xAI** (Grok) | API key | Token and balance usage |

## Build from source

**Prerequisites:** [Rust stable](https://rustup.rs), Node.js 18+, [VS Build Tools](https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022) with the C++ workload and the WebView2 runtime (ships with Windows 10 1803+).

```bat
git clone https://github.com/episuarez/gptBar.git
cd gptBar
npm install
scripts\dev.bat      :: hot-reload dev mode
scripts\build.bat    :: production build → src-tauri/target/release/bundle/
```

## Tech stack

Rust · [Tauri 2](https://tauri.app) · [Svelte 5](https://svelte.dev) · TypeScript — a native shell with a web UI, no Electron bloat.

## Changelog

See [CHANGELOG.md](CHANGELOG.md) for release notes. Latest: **v0.2.2** — auto-update, desktop notifications, in-app update checks and a UI polish pass.

## Contributing

PRs and issues welcome — see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

[MIT](LICENSE) — Copyright (c) 2026 episuarez

Inspired by [CodexBar](https://github.com/steipete/CodexBar) by steipete.
