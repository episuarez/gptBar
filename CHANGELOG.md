# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] — 2026-06-11

### Added
- xAI (Grok) provider with token and balance tracking
- Usage history with sparkline charts and JSON/CSV export
- Per-provider notification cooldowns and mute controls
- API key management in Settings — stored in Windows Credential Manager
- All 5 providers pre-registered in default config (disabled by default except Claude)
- CI workflow: fmt + clippy + tests + svelte-check on every push/PR
- NSIS and MSI Windows installers via release workflow
- CONTRIBUTING.md, CHANGELOG.md, issue templates and PR template
- `scripts/dev.bat` and `scripts/build.bat` helpers

### Changed
- Windows-only builds — removed macOS and Linux from release matrix
- License changed from GPL-3.0 to MIT
- Login instructions for non-Claude providers now point to Settings → API Keys

### Removed
- Auto-updater plugin — users download new releases from GitHub Releases
- Dead cookie-extraction module and its dependencies (`aes-gcm`)

## [0.1.0] — 2025-12-01

### Added
- Initial release
- Claude, OpenAI, Gemini and Codex providers
- System tray icon with usage color indicator
- Desktop notifications with configurable thresholds
- Settings modal with provider toggles and refresh interval
- Secure credential storage via Windows Credential Manager

[Unreleased]: https://github.com/episuarez/gptBar/compare/v0.2.1...HEAD
[0.2.1]: https://github.com/episuarez/gptBar/compare/v0.1.0...v0.2.1
[0.1.0]: https://github.com/episuarez/gptBar/releases/tag/v0.1.0
