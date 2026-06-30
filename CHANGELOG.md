# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.2] — 2026-07-01

### Added
- Auto-update via `tauri-plugin-updater` — signed NSIS updates served from GitHub
  Releases (`latest.json`), with an in-app "Update available" banner
- "Check for updates" button in the About panel for manual update checks
- Desktop notifications when a provider crosses the warning/critical threshold,
  respecting per-provider mutes and the notification cooldown
- Disconnect button to clear a provider's stored API key from the UI
- Subtle UI animations (banner, cards, usage bars, modals, status dot) with
  `prefers-reduced-motion` support

### Changed
- Threshold steppers clamp warning below critical (and critical above warning)
- Dimmed UI text lightened to meet WCAG AA contrast on the dark theme
- Auto-refresh now polls every enabled provider so the tray and notifications
  stay current for all of them
- Release workflow signs updater artifacts and publishes `latest.json`

### Fixed
- Gemini API key no longer sent in the request URL (moved to header)
- API keys are stored only in the Windows Credential Manager, never written to
  `config.json` in plaintext
- Claude OAuth token prefix is no longer written to logs
- `escape_for_log` no longer double-escapes ampersands
- OpenAI usage request no longer builds invalid month-end dates
- Refresh agent can be stopped and restarted (cancellation token is reset)
- Modals, tabs and form controls have proper ARIA roles, labels and keyboard support
- Settings save failures now surface an inline error instead of failing silently

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

[Unreleased]: https://github.com/episuarez/gptBar/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/episuarez/gptBar/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/episuarez/gptBar/compare/v0.1.0...v0.2.1
[0.1.0]: https://github.com/episuarez/gptBar/releases/tag/v0.1.0
