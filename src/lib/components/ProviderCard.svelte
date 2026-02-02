<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { exit } from "@tauri-apps/plugin-process";
  import { open } from "@tauri-apps/plugin-shell";
  import type { UsageSnapshot, AppConfig } from '$lib/types';
  import UsageBar from './UsageBar.svelte';

  interface Props {
    providerId: string;
    providerName: string;
    snapshot: UsageSnapshot | null;
    loading: boolean;
    error: string | null;
    isAvailable: boolean;
    onRefresh: () => void;
    onLogin: () => void;
    onLogout: () => void;
    enabledProviders?: string[];
    onEnabledProvidersChange?: (providers: string[]) => void;
    closeModals?: (() => void) | null;
  }

  let {
    providerId,
    providerName,
    snapshot,
    loading,
    error,
    isAvailable,
    onRefresh,
    onLogin,
    onLogout,
    enabledProviders = ['claude'],
    onEnabledProvidersChange,
    closeModals = $bindable(null)
  }: Props = $props();

  // All available providers
  const allProviders = [
    { id: 'claude', name: 'Claude', color: '#d97706' },
    { id: 'openai', name: 'OpenAI', color: '#10a37f' },
    { id: 'gemini', name: 'Gemini', color: '#4285f4' },
    { id: 'codex', name: 'Codex', color: '#6366f1' },
  ];

  // Provider icons/colors
  const providerStyles: Record<string, { bg: string; letter: string }> = {
    claude: { bg: 'linear-gradient(135deg, #d97706, #f59e0b)', letter: 'C' },
    openai: { bg: 'linear-gradient(135deg, #10a37f, #1a7f64)', letter: 'O' },
    gemini: { bg: 'linear-gradient(135deg, #4285f4, #34a853)', letter: 'G' },
    codex: { bg: 'linear-gradient(135deg, #6366f1, #8b5cf6)', letter: 'X' },
  };

  // Provider-specific URLs
  const providerUrls: Record<string, { dashboard: string; status: string; loginHint: string }> = {
    claude: {
      dashboard: 'https://claude.ai/settings/usage',
      status: 'https://status.anthropic.com',
      loginHint: 'Run <code>claude login</code> in your terminal',
    },
    openai: {
      dashboard: 'https://platform.openai.com/usage',
      status: 'https://status.openai.com',
      loginHint: 'Set <code>OPENAI_API_KEY</code> environment variable',
    },
    gemini: {
      dashboard: 'https://aistudio.google.com',
      status: 'https://status.cloud.google.com',
      loginHint: 'Set <code>GOOGLE_API_KEY</code> environment variable',
    },
    codex: {
      dashboard: 'https://platform.openai.com/usage',
      status: 'https://status.openai.com',
      loginHint: 'Set <code>CODEX_API_KEY</code> or <code>OPENAI_API_KEY</code>',
    },
  };

  function getStyle() {
    return providerStyles[providerId] || { bg: '#6b7280', letter: '?' };
  }

  function getUrls() {
    return providerUrls[providerId] || providerUrls.claude;
  }

  // Function to close all modals - exposed to parent
  function closeAllModals() {
    showAbout = false;
    showSettings = false;
  }

  // Expose the function to parent
  $effect(() => {
    closeModals = closeAllModals;
  });

  function formatTime(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = now.getTime() - date.getTime();

    if (diff < 60000) return 'just now';
    if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`;
    return date.toLocaleTimeString();
  }

  function formatResetTime(dateStr: string | null): string {
    if (!dateStr) return '';
    const date = new Date(dateStr);
    const now = new Date();
    const diff = date.getTime() - now.getTime();

    if (diff < 0) return 'Now';

    const days = Math.floor(diff / (1000 * 60 * 60 * 24));
    const hours = Math.floor((diff % (1000 * 60 * 60 * 24)) / (1000 * 60 * 60));
    const minutes = Math.floor((diff % (1000 * 60 * 60)) / (1000 * 60));

    if (days > 0) {
      return `${days}d ${hours}h`;
    }
    if (hours > 0) {
      return `${hours}h ${minutes}m`;
    }
    return `${minutes}m`;
  }

  async function openUsageDashboard() {
    await open(getUrls().dashboard);
  }

  async function openStatusPage() {
    await open(getUrls().status);
  }

  async function handleQuit() {
    await exit(0);
  }

  let showAbout = $state(false);
  let showSettings = $state(false);

  // Settings state
  let refreshInterval = $state(5);
  let startOnLogin = $state(false);
  let settingsLoading = $state(false);
  let localEnabledProviders = $state<string[]>([]);

  function toggleAbout() {
    showAbout = !showAbout;
    showSettings = false;
  }

  async function toggleSettings() {
    if (!showSettings) {
      // Load current settings when opening
      try {
        const config = await invoke<AppConfig>('get_config');
        refreshInterval = config.refresh_interval;
        startOnLogin = config.start_on_login;
        localEnabledProviders = [...config.enabled_providers];
      } catch (e) {
        console.error('Failed to load config:', e);
        localEnabledProviders = [...enabledProviders];
      }
    }
    showSettings = !showSettings;
    showAbout = false;
  }

  async function handleRefreshIntervalChange(event: Event) {
    const target = event.target as HTMLSelectElement;
    const value = parseInt(target.value);
    refreshInterval = value;
    try {
      await invoke('set_refresh_interval', { minutes: value });
    } catch (e) {
      console.error('Failed to save refresh interval:', e);
    }
  }

  async function handleStartOnLoginChange(event: Event) {
    const target = event.target as HTMLInputElement;
    const value = target.checked;
    settingsLoading = true;
    try {
      await invoke('set_start_on_login', { enabled: value });
      startOnLogin = value;
    } catch (e) {
      console.error('Failed to save start on login:', e);
      target.checked = !value;
    }
    settingsLoading = false;
  }

  async function handleProviderToggle(provId: string) {
    const isEnabled = localEnabledProviders.includes(provId);

    if (isEnabled) {
      // Don't allow disabling the last provider
      if (localEnabledProviders.length <= 1) return;
      localEnabledProviders = localEnabledProviders.filter(p => p !== provId);
    } else {
      localEnabledProviders = [...localEnabledProviders, provId];
    }

    // Save to backend
    try {
      await invoke('set_provider_enabled', { providerId: provId, enabled: !isEnabled });
      await invoke('set_provider_order', { order: localEnabledProviders });

      // Notify parent
      if (onEnabledProvidersChange) {
        onEnabledProvidersChange(localEnabledProviders);
      }
    } catch (e) {
      console.error('Failed to save provider settings:', e);
      // Revert
      if (isEnabled) {
        localEnabledProviders = [...localEnabledProviders, provId];
      } else {
        localEnabledProviders = localEnabledProviders.filter(p => p !== provId);
      }
    }
  }

  const style = $derived(getStyle());
  const urls = $derived(getUrls());
</script>

<div class="card">
  <!-- Header -->
  <div class="card-header">
    <div class="provider-info">
      <div class="provider-icon" style="background: {style.bg}">
        <span class="icon-letter">{style.letter}</span>
      </div>
      <div class="provider-details">
        <span class="provider-name">{providerName}</span>
        {#if snapshot}
          <span class="updated-time">Updated {formatTime(snapshot.updated_at)}</span>
        {/if}
      </div>
    </div>
    <div class="header-right">
      {#if snapshot?.identity?.email}
        <span class="user-email">{snapshot.identity.email}</span>
      {/if}
      {#if snapshot?.identity?.plan}
        <span class="plan-badge">{snapshot.identity.plan}</span>
      {/if}
    </div>
  </div>

  {#if !isAvailable}
    <!-- Login prompt -->
    <div class="login-prompt">
      <p class="login-title">Not authenticated</p>
      <p class="login-hint">{@html urls.loginHint},<br/>then click Refresh Now below.</p>
      <button class="login-button" onclick={onLogin}>
        Get API Key / Login
      </button>
    </div>
  {:else if snapshot}
    <!-- Usage bars -->
    <div class="usage-section">
      {#if snapshot.primary}
        <UsageBar
          label={snapshot.primary.reset_description || "Session"}
          percent={snapshot.primary.used_percent}
          resetTime={formatResetTime(snapshot.primary.resets_at)}
        />
      {/if}

      {#if snapshot.secondary}
        <UsageBar
          label={snapshot.secondary.reset_description || "Weekly"}
          percent={snapshot.secondary.used_percent}
          resetTime={formatResetTime(snapshot.secondary.resets_at)}
        />
      {/if}

      {#if snapshot.tertiary}
        <UsageBar
          label={snapshot.tertiary.reset_description || "Model"}
          percent={snapshot.tertiary.used_percent}
          resetTime={formatResetTime(snapshot.tertiary.resets_at)}
        />
      {/if}
    </div>
  {:else if loading}
    <div class="loading-state">
      <p>Loading...</p>
    </div>
  {:else}
    <div class="empty-state">
      <p>Click Refresh Now to load usage data</p>
    </div>
  {/if}

  {#if error}
    <div class="error-box">
      <p>{error}</p>
    </div>
  {/if}

  <!-- Divider -->
  <div class="divider"></div>

  <!-- Action buttons -->
  <div class="actions-section">
    <button class="action-button" onclick={onRefresh} disabled={loading}>
      <span class="action-icon">{loading ? '⟳' : '↻'}</span>
      <span>Refresh Now</span>
    </button>
    <button class="action-button" onclick={openUsageDashboard}>
      <span class="action-icon">📊</span>
      <span>Usage Dashboard</span>
    </button>
    <button class="action-button" onclick={openStatusPage}>
      <span class="action-icon">⚡</span>
      <span>Status Page</span>
    </button>
  </div>

  <!-- Divider -->
  <div class="divider"></div>

  <!-- Footer -->
  <div class="footer-section">
    <button class="footer-button" onclick={toggleSettings}>
      Settings...
    </button>
    <button class="footer-button" onclick={toggleAbout}>
      About GPTBar
    </button>
    <button class="footer-button danger" onclick={handleQuit}>
      Quit
    </button>
  </div>

  <!-- About Modal -->
  {#if showAbout}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modal-overlay" onclick={toggleAbout} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_interactive_supports_focus -->
      <div class="modal about-modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-labelledby="about-title" tabindex="-1">
        <div class="modal-header">
          <h3 id="about-title">About GPTBar</h3>
          <button class="modal-close-x" onclick={toggleAbout}>✕</button>
        </div>
        <div class="about-content">
          <div class="about-icon">G</div>
          <p class="version">Version 0.2.0</p>
          <p class="description">Monitor AI provider usage from the system tray.</p>
          <p class="platforms">Windows • macOS • Linux</p>
          <p class="credits">Inspired by CodexBar for macOS</p>
        </div>
      </div>
    </div>
  {/if}

  <!-- Settings Modal -->
  {#if showSettings}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modal-overlay" onclick={toggleSettings} role="presentation">
      <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_interactive_supports_focus -->
      <div class="modal settings-modal" onclick={(e) => e.stopPropagation()} role="dialog" aria-labelledby="settings-title" tabindex="-1">
        <div class="modal-header">
          <h3 id="settings-title">Settings</h3>
          <button class="modal-close-x" onclick={toggleSettings}>✕</button>
        </div>

        <div class="settings-content">
          <!-- Providers Section -->
          <div class="settings-section">
            <h4 class="settings-section-title">Providers</h4>
            <div class="provider-list">
              {#each allProviders as provider}
                {@const isEnabled = localEnabledProviders.includes(provider.id)}
                {@const pStyle = providerStyles[provider.id]}
                <div class="provider-row">
                  <label for="provider-{provider.id}" class="provider-row-info">
                    <span class="provider-row-icon" style="background: {pStyle.bg}">{pStyle.letter}</span>
                    <span class="provider-row-name">{provider.name}</span>
                  </label>
                  <label class="toggle">
                    <input
                      type="checkbox"
                      id="provider-{provider.id}"
                      checked={isEnabled}
                      onchange={() => handleProviderToggle(provider.id)}
                      disabled={isEnabled && localEnabledProviders.length <= 1}
                      aria-label="Enable {provider.name}"
                    />
                    <span class="toggle-slider"></span>
                  </label>
                </div>
              {/each}
            </div>
          </div>

          <div class="divider-thin"></div>

          <!-- General Section -->
          <div class="settings-section">
            <h4 class="settings-section-title">General</h4>

            <div class="settings-item">
              <label for="refresh-interval">Auto-refresh</label>
              <select
                id="refresh-interval"
                class="settings-select"
                value={refreshInterval}
                onchange={handleRefreshIntervalChange}
              >
                <option value={1}>1 min</option>
                <option value={2}>2 min</option>
                <option value={5}>5 min</option>
                <option value={10}>10 min</option>
                <option value={15}>15 min</option>
                <option value={30}>30 min</option>
              </select>
            </div>

            <div class="settings-item no-border">
              <label for="start-login">Start on login</label>
              <label class="toggle">
                <input
                  type="checkbox"
                  id="start-login"
                  checked={startOnLogin}
                  onchange={handleStartOnLoginChange}
                  disabled={settingsLoading}
                />
                <span class="toggle-slider"></span>
              </label>
            </div>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .card {
    background-color: #1a1f2e;
    display: flex;
    flex-direction: column;
    flex: 1;
    overflow: hidden;
  }

  /* Header */
  .card-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 0.75rem;
    border-bottom: 1px solid #2d3548;
  }

  .provider-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .provider-icon {
    width: 2rem;
    height: 2rem;
    border-radius: 0.375rem;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
  }

  .icon-letter {
    font-weight: bold;
    font-size: 1rem;
  }

  .provider-details {
    display: flex;
    flex-direction: column;
  }

  .provider-name {
    font-weight: 600;
    font-size: 0.9rem;
    color: white;
  }

  .updated-time {
    font-size: 0.7rem;
    color: #6b7280;
  }

  .header-right {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 0.25rem;
  }

  .user-email {
    font-size: 0.7rem;
    color: #9ca3af;
  }

  .plan-badge {
    background-color: #3b82f6;
    color: white;
    font-size: 0.6rem;
    padding: 0.125rem 0.5rem;
    border-radius: 9999px;
    text-transform: capitalize;
    font-weight: 600;
  }

  /* Login prompt */
  .login-prompt {
    text-align: center;
    padding: 1.5rem 1rem;
    flex: 1;
  }

  .login-prompt .login-title {
    color: #9ca3af;
    margin: 0 0 0.5rem 0;
    font-weight: 500;
  }

  .login-prompt .login-hint {
    color: #6b7280;
    margin: 0 0 1rem 0;
    font-size: 0.75rem;
    line-height: 1.4;
  }

  .login-prompt :global(code) {
    background-color: #374151;
    padding: 0.125rem 0.375rem;
    border-radius: 0.25rem;
    font-family: monospace;
    font-size: 0.75rem;
  }

  .login-button {
    background: linear-gradient(135deg, #6366f1, #8b5cf6);
    border: none;
    color: white;
    padding: 0.5rem 1rem;
    border-radius: 0.5rem;
    cursor: pointer;
    font-weight: 500;
    transition: opacity 0.2s;
  }

  .login-button:hover {
    opacity: 0.9;
  }

  /* Usage section */
  .usage-section {
    padding: 0.75rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  /* Loading state */
  .loading-state {
    text-align: center;
    padding: 2rem 1rem;
    color: #6b7280;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .loading-state p {
    margin: 0;
    font-size: 0.85rem;
  }

  /* Divider */
  .divider {
    height: 1px;
    background-color: #2d3548;
    margin: 0;
  }

  .divider-thin {
    height: 1px;
    background-color: #2d3548;
    margin: 0.5rem 0;
  }

  /* Error box */
  .error-box {
    background-color: rgba(127, 29, 29, 0.5);
    border: 1px solid #b91c1c;
    padding: 0.5rem;
    margin: 0 0.75rem 0.5rem;
    border-radius: 0.25rem;
  }

  .error-box p {
    color: #fca5a5;
    font-size: 0.7rem;
    margin: 0;
  }

  /* Actions section */
  .actions-section {
    padding: 0.5rem;
    display: flex;
    flex-direction: column;
  }

  .action-button {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    background: transparent;
    border: none;
    color: #d1d5db;
    padding: 0.5rem 0.75rem;
    cursor: pointer;
    font-size: 0.8rem;
    border-radius: 0.375rem;
    transition: background-color 0.15s;
    text-align: left;
  }

  .action-button:hover:not(:disabled) {
    background-color: #2d3548;
  }

  .action-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .action-icon {
    width: 1.25rem;
    text-align: center;
  }

  /* Footer section */
  .footer-section {
    padding: 0.5rem;
    display: flex;
    flex-direction: column;
  }

  .footer-button {
    background: transparent;
    border: none;
    color: #9ca3af;
    padding: 0.5rem 0.75rem;
    cursor: pointer;
    font-size: 0.8rem;
    border-radius: 0.375rem;
    transition: background-color 0.15s;
    text-align: left;
  }

  .footer-button:hover {
    background-color: #2d3548;
    color: #d1d5db;
  }

  .footer-button.danger:hover {
    color: #f87171;
  }

  /* Empty state */
  .empty-state {
    text-align: center;
    padding: 2rem 1rem;
    color: #6b7280;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .empty-state p {
    margin: 0;
    font-size: 0.85rem;
  }

  /* Modal */
  .modal-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background-color: #1a1f2e;
    border: 1px solid #2d3548;
    border-radius: 0.5rem;
    padding: 1.25rem;
    width: 90%;
    max-width: 280px;
    text-align: center;
    max-height: 80vh;
    overflow-y: auto;
  }

  .modal h3 {
    color: white;
    margin: 0 0 0.5rem 0;
    font-size: 1rem;
  }

  .about-modal {
    padding: 0;
    overflow: hidden;
  }

  .about-content {
    padding: 1rem;
    text-align: center;
  }

  .about-icon {
    width: 3rem;
    height: 3rem;
    background: linear-gradient(135deg, #3b82f6, #8b5cf6);
    border-radius: 0.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-weight: bold;
    font-size: 1.5rem;
    margin: 0 auto 0.75rem;
  }

  .about-content .version {
    color: #6b7280;
    font-size: 0.75rem;
    margin: 0 0 0.75rem 0;
  }

  .about-content .description {
    color: #9ca3af;
    font-size: 0.8rem;
    margin: 0 0 0.25rem 0;
  }

  .about-content .platforms {
    color: #6b7280;
    font-size: 0.7rem;
    margin: 0 0 0.5rem 0;
  }

  .about-content .credits {
    color: #6b7280;
    font-size: 0.7rem;
    margin: 0;
  }

  /* Settings */
  .settings-section {
    margin-bottom: 0.5rem;
  }

  .settings-section-title {
    color: #6b7280;
    font-size: 0.7rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin: 0 0 0.5rem 0;
    text-align: left;
  }

  .provider-list {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .provider-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.4rem 0;
  }

  .provider-row-info {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .provider-row-icon {
    width: 1.25rem;
    height: 1.25rem;
    border-radius: 0.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-weight: bold;
    font-size: 0.6rem;
  }

  .provider-row-name {
    color: #d1d5db;
    font-size: 0.8rem;
  }

  .settings-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.5rem 0;
    border-bottom: 1px solid #2d3548;
    font-size: 0.8rem;
  }

  .settings-item label:first-child {
    color: #d1d5db;
  }

  .settings-item.no-border {
    border-bottom: none;
  }

  .settings-modal {
    text-align: left;
    padding: 0;
    overflow: hidden;
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #2d3548;
  }

  .modal-header h3 {
    margin: 0;
    font-size: 0.9rem;
  }

  .modal-close-x {
    background: none;
    border: none;
    color: #6b7280;
    font-size: 1rem;
    cursor: pointer;
    padding: 0.25rem;
    line-height: 1;
    transition: color 0.15s;
  }

  .modal-close-x:hover {
    color: #d1d5db;
  }

  .settings-content {
    padding: 0.75rem 1rem;
  }

  .settings-select {
    background-color: #2d3548;
    border: 1px solid #3d4558;
    color: white;
    padding: 0.3rem 0.5rem;
    border-radius: 0.25rem;
    font-size: 0.75rem;
    cursor: pointer;
  }

  .settings-select:focus {
    outline: none;
    border-color: #3b82f6;
  }

  /* Toggle switch */
  .toggle {
    position: relative;
    display: inline-block;
    width: 36px;
    height: 20px;
  }

  .toggle input {
    opacity: 0;
    width: 0;
    height: 0;
  }

  .toggle-slider {
    position: absolute;
    cursor: pointer;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: #374151;
    transition: 0.3s;
    border-radius: 20px;
  }

  .toggle-slider:before {
    position: absolute;
    content: "";
    height: 14px;
    width: 14px;
    left: 3px;
    bottom: 3px;
    background-color: white;
    transition: 0.3s;
    border-radius: 50%;
  }

  .toggle input:checked + .toggle-slider {
    background-color: #3b82f6;
  }

  .toggle input:checked + .toggle-slider:before {
    transform: translateX(16px);
  }

  .toggle input:disabled + .toggle-slider {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
