<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { exit } from "@tauri-apps/plugin-process";
  import { open } from "@tauri-apps/plugin-shell";
  import { getVersion } from "@tauri-apps/api/app";
  import type { UsageSnapshot, AppConfig } from '$lib/types';
  import UsageBar from './UsageBar.svelte';
  import { updateState, checkForUpdate, installUpdate } from '$lib/updateStore.svelte';

  // Label for the manual "check for updates" control in About.
  let updateButtonLabel = $derived(
    updateState.phase === 'checking' ? 'Checking…'
    : updateState.phase === 'available' ? `Install v${updateState.newVersion}`
    : updateState.phase === 'downloading' ? `Downloading ${updateState.progress}%`
    : updateState.phase === 'uptodate' ? 'Up to date'
    : updateState.phase === 'error' ? 'Check failed — retry'
    : 'Check for updates'
  );

  function handleUpdateButton() {
    if (updateState.phase === 'available') {
      installUpdate();
    } else if (updateState.phase !== 'downloading' && updateState.phase !== 'ready') {
      checkForUpdate(true);
    }
  }

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
    { id: 'claude', name: 'Claude', color: '#D97706' },
    { id: 'openai', name: 'OpenAI', color: '#10a37f' },
    { id: 'gemini', name: 'Gemini', color: '#4285f4' },
    { id: 'codex', name: 'Codex', color: '#8B5CF6' },
    { id: 'xai', name: 'xAI (Grok)', color: '#94A3B8' },
  ];

  // Provider styles
  const providerStyles: Record<string, { gradient: string; letter: string }> = {
    claude: { gradient: 'linear-gradient(135deg, #D97706, #F59E0B)', letter: 'C' },
    openai: { gradient: 'linear-gradient(135deg, #10a37f, #1a7f64)', letter: 'O' },
    gemini: { gradient: 'linear-gradient(135deg, #4285f4, #34a853)', letter: 'G' },
    codex: { gradient: 'linear-gradient(135deg, #6366f1, #8b5cf6)', letter: 'X' },
    xai: { gradient: 'linear-gradient(135deg, #64748B, #94A3B8)', letter: 'X' },
  };

  // Provider-specific URLs
  const providerUrls: Record<string, { dashboard: string; status: string; loginSteps: string[] }> = {
    claude: {
      dashboard: 'https://claude.ai/settings/usage',
      status: 'https://status.anthropic.com',
      loginSteps: ['Run: claude login', 'Authorize in your browser', 'Click Refresh Now below'],
    },
    openai: {
      dashboard: 'https://platform.openai.com/usage',
      status: 'https://status.openai.com',
      loginSteps: ['Open Settings → API Keys', 'Paste your OpenAI API key', 'Click Refresh Now below'],
    },
    gemini: {
      dashboard: 'https://aistudio.google.com',
      status: 'https://status.cloud.google.com',
      loginSteps: ['Open Settings → API Keys', 'Paste your Google API key', 'Click Refresh Now below'],
    },
    codex: {
      dashboard: 'https://platform.openai.com/usage',
      status: 'https://status.openai.com',
      loginSteps: ['Open Settings → API Keys', 'Paste your OpenAI/Codex API key', 'Click Refresh Now below'],
    },
    xai: {
      dashboard: 'https://console.x.ai/',
      status: 'https://status.x.ai',
      loginSteps: ['Open Settings → API Keys', 'Paste your xAI API key', 'Click Refresh Now below'],
    },
  };

  function getStyle() {
    return providerStyles[providerId] || { gradient: '#6b7280', letter: '?' };
  }

  function getUrls() {
    return providerUrls[providerId] || providerUrls.claude;
  }

  // Close all modals
  function closeAllModals() {
    const wasOpen = showAbout || showSettings;
    showAbout = false;
    showSettings = false;
    showExportMenu = false;
    if (wasOpen) restoreFocus();
  }

  $effect(() => {
    closeModals = closeAllModals;
  });

  function formatTime(dateStr: string): string {
    const date = new Date(dateStr);
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    if (diff < 60000) return 'just now';
    if (diff < 3600000) return `${Math.floor(diff / 60000)} min ago`;
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
    if (days > 0) return `${days}d ${hours}h`;
    if (hours > 0) return `${hours}h ${minutes}m`;
    return `${minutes}m`;
  }

  async function openUsageDashboard() {
    await open(getUrls().dashboard);
  }

  async function handleQuit() {
    await exit(0);
  }

  let showAbout = $state(false);
  let showSettings = $state(false);
  let showExportMenu = $state(false);

  // Dynamic version
  let appVersion = $state('...');
  $effect(() => {
    getVersion().then((v: string) => { appVersion = v; }).catch(() => { appVersion = '0.2.2'; });
  });

  // Transient save-error indicator
  let saveError = $state('');
  let saveErrorTimer: ReturnType<typeof setTimeout> | null = null;
  function flashError(msg: string) {
    saveError = msg;
    if (saveErrorTimer) clearTimeout(saveErrorTimer);
    saveErrorTimer = setTimeout(() => { saveError = ''; saveErrorTimer = null; }, 3000);
  }

  // Modal focus management
  let settingsModalEl = $state<HTMLDivElement | null>(null);
  let aboutModalEl = $state<HTMLDivElement | null>(null);
  let lastFocused: HTMLElement | null = null;
  function captureFocus() {
    lastFocused = document.activeElement as HTMLElement | null;
  }
  function restoreFocus() {
    if (lastFocused && typeof lastFocused.focus === 'function') lastFocused.focus();
    lastFocused = null;
  }
  $effect(() => {
    if (showSettings) settingsModalEl?.focus();
  });
  $effect(() => {
    if (showAbout) aboutModalEl?.focus();
  });

  // Settings state
  let refreshInterval = $state(10);
  let startOnLogin = $state(false);
  let settingsLoading = $state(false);
  let localEnabledProviders = $state<string[]>([]);
  let notificationCooldown = $state(30);
  let warningThreshold = $state(85);
  let criticalThreshold = $state(95);
  let apiKeys = $state<Record<string, string>>({});

  // History / sparkline
  let historyPoints = $state<number[]>([]);

  function toggleAbout() {
    if (!showAbout) {
      captureFocus();
      showAbout = true;
      showSettings = false;
    } else {
      showAbout = false;
      restoreFocus();
    }
  }

  async function toggleSettings() {
    if (!showSettings) {
      captureFocus();
      try {
        const config = await invoke<AppConfig>('get_config');
        refreshInterval = config.refresh_interval;
        startOnLogin = config.start_on_login;
        localEnabledProviders = [...config.enabled_providers];
        notificationCooldown = config.notification_cooldown_minutes ?? 30;
        warningThreshold = config.warning_threshold ?? 85;
        criticalThreshold = config.critical_threshold ?? 95;
        apiKeys = Object.fromEntries(
          allProviders
            .filter(p => p.id !== 'claude')
            .map(p => [p.id, config.provider_settings?.[p.id]?.api_key ?? ''])
        );
      } catch {
        localEnabledProviders = [...enabledProviders];
      }
      showSettings = true;
      showAbout = false;
    } else {
      showSettings = false;
      restoreFocus();
    }
  }

  // Settings handlers
  async function setRefreshInterval(value: number) {
    refreshInterval = value;
    try { await invoke('set_refresh_interval', { minutes: value }); }
    catch (e) { console.error('Failed to set refresh interval:', e); flashError('Could not save refresh interval'); }
  }

  async function handleStartOnLoginChange(event: Event) {
    const target = event.target as HTMLInputElement;
    const value = target.checked;
    settingsLoading = true;
    try { await invoke('set_start_on_login', { enabled: value }); startOnLogin = value; } catch { target.checked = !value; }
    settingsLoading = false;
  }

  async function adjustThreshold(type: 'warning' | 'critical', delta: number) {
    if (type === 'warning') {
      const next = Math.min(99, Math.max(50, warningThreshold + delta));
      warningThreshold = Math.min(next, criticalThreshold - 1);
    } else {
      const next = Math.min(100, Math.max(51, criticalThreshold + delta));
      criticalThreshold = Math.max(next, warningThreshold + 1);
    }
    try {
      await invoke('set_notification_thresholds', { warning: warningThreshold, critical: criticalThreshold });
    } catch (e) { console.error('Failed to set thresholds:', e); flashError('Could not save thresholds'); }
  }

  async function setCooldown(value: number) {
    notificationCooldown = value;
    try { await invoke('set_notification_cooldown', { minutes: value }); }
    catch (e) { console.error('Failed to set cooldown:', e); flashError('Could not save cooldown'); }
  }

  async function adjustCooldown(delta: number) {
    const presets = [15, 30, 60, 120];
    const idx = presets.indexOf(notificationCooldown);
    const newIdx = Math.min(presets.length - 1, Math.max(0, idx + delta));
    await setCooldown(presets[newIdx]);
  }

  async function handleApiKeyChange(providerId: string, value: string) {
    apiKeys[providerId] = value;
    try {
      await invoke('set_provider_api_key', { providerId, apiKey: value });
      if (value) await invoke('trigger_refresh');
    } catch (e) { console.error('Failed to save API key:', e); flashError('Could not save API key'); }
  }

  async function handleProviderToggle(provId: string) {
    const isEnabled = localEnabledProviders.includes(provId);
    if (isEnabled) {
      if (localEnabledProviders.length <= 1) return;
      localEnabledProviders = localEnabledProviders.filter(p => p !== provId);
    } else {
      localEnabledProviders = [...localEnabledProviders, provId];
    }
    try {
      await invoke('set_provider_enabled', { providerId: provId, enabled: !isEnabled });
      await invoke('set_provider_order', { order: localEnabledProviders });
      if (onEnabledProvidersChange) onEnabledProvidersChange(localEnabledProviders);
    } catch {
      if (isEnabled) localEnabledProviders = [...localEnabledProviders, provId];
      else localEnabledProviders = localEnabledProviders.filter(p => p !== provId);
    }
  }

  async function handleExportJson() {
    showExportMenu = false;
    try {
      const json = await invoke<string>('export_history_json');
      const blob = new Blob([json], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a'); a.href = url; a.download = 'gptbar-history.json'; a.click();
      URL.revokeObjectURL(url);
    } catch (e) { console.error('Failed to export JSON:', e); flashError('Export failed'); }
  }

  async function handleExportCsv() {
    showExportMenu = false;
    try {
      const csv = await invoke<string>('export_history_csv');
      const blob = new Blob([csv], { type: 'text/csv' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a'); a.href = url; a.download = 'gptbar-history.csv'; a.click();
      URL.revokeObjectURL(url);
    } catch (e) { console.error('Failed to export CSV:', e); flashError('Export failed'); }
  }

  // Load sparkline history
  $effect(() => {
    if (snapshot) {
      invoke<Array<{primary_percent: number | null}>>('get_provider_history', {
        providerId, limit: 20,
      }).then((entries) => {
        historyPoints = entries.map((e) => e.primary_percent ?? 0).reverse();
      }).catch(() => { historyPoints = []; });
    }
  });

  const style = $derived(getStyle());
  const urls = $derived(getUrls());

  // Sparkline trend calculation
  let sparkTrend = $derived(() => {
    if (historyPoints.length < 2) return '';
    const last = historyPoints[historyPoints.length - 1];
    const prev = historyPoints[historyPoints.length - 2];
    const diff = last - prev;
    if (diff > 0) return `↑ +${Math.round(diff)}%`;
    if (diff < 0) return `↓ ${Math.round(diff)}%`;
    return '→ 0%';
  });

  const refreshPresets = [5, 10, 15, 30];
  const cooldownPresets = [15, 30, 60, 120];

  function cooldownLabel(m: number) {
    if (m < 60) return `${m}m`;
    return `${m / 60}h`;
  }
</script>

<div class="card">
  <div class="content-scroll">
  {#if !isAvailable}
    <!-- ========== LOGIN / NOT AUTHENTICATED ========== -->
    <div class="provider-header-bar">
      <div class="provider-row">
        <div class="provider-icon" style="background: {style.gradient}">
          <span class="icon-letter">{style.letter}</span>
        </div>
        <div class="provider-info">
          <span class="provider-name">{providerName}</span>
          <span class="provider-sub">Not connected</span>
        </div>
      </div>
    </div>

    <div class="auth-content">
      <div class="lock-icon-bg">
        <svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="#64748B" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <rect width="18" height="11" x="3" y="11" rx="2" ry="2"></rect>
          <path d="M7 11V7a5 5 0 0 1 10 0v4"></path>
        </svg>
      </div>
      <h2 class="auth-title">Not Authenticated</h2>
      <p class="auth-desc">Connect your {providerName} account to monitor API usage limits in real-time.</p>
      <div class="auth-steps">
        {#each urls.loginSteps as step, i}
          <div class="step-row">
            <div class="step-num"><span>{i + 1}</span></div>
            <span class="step-text">{step}</span>
          </div>
        {/each}
      </div>
    </div>

    <div class="auth-buttons">
      <button class="btn-primary" onclick={onLogin}>
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#0A0F1C" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4"/><polyline points="10 17 15 12 10 7"/><line x1="15" y1="12" x2="3" y2="12"/></svg>
        Connect {providerName} Account
      </button>
    </div>

  {:else}
    <!-- ========== MAIN VIEW (AUTHENTICATED) ========== -->
    <!-- Provider card -->
    <div class="provider-section">
      <div class="provider-card">
        <div class="provider-icon" style="background: {style.gradient}">
          <span class="icon-letter">{style.letter}</span>
        </div>
        <div class="provider-info">
          <span class="provider-name">{providerName}</span>
          <span class="provider-email">{snapshot?.identity?.email || 'Connected'}</span>
          <div class="provider-meta">
            {#if snapshot?.identity?.plan}
              <span class="plan-badge">{snapshot.identity.plan.toUpperCase()}</span>
            {/if}
            <span class="updated-time">
              {#if snapshot}
                {formatTime(snapshot.updated_at)}
              {:else if loading}
                Loading...
              {:else}
                --
              {/if}
            </span>
          </div>
        </div>
      </div>
    </div>

    <!-- Usage Bars — always show 3 regardless of data -->
    <div class="usage-section">
      <span class="section-label">USAGE LIMITS</span>
      {#if loading && !snapshot}
        <UsageBar label="Session" percent={0} />
        <UsageBar label="Daily" percent={0} />
        <UsageBar label="Monthly" percent={0} />
        <span class="loading-hint">Loading usage data...</span>
      {:else if snapshot}
        <UsageBar
          label="{snapshot.primary?.reset_description || 'Session'}{snapshot.primary?.window_minutes ? '  (' + Math.round(snapshot.primary.window_minutes / 60) + 'h)' : ''}"
          percent={snapshot.primary?.used_percent ?? 0}
          resetTime={snapshot.primary ? formatResetTime(snapshot.primary.resets_at) : ''}
        />
        <UsageBar
          label="{snapshot.secondary?.reset_description || 'Daily'}{snapshot.secondary?.window_minutes ? '  (' + Math.round(snapshot.secondary.window_minutes / 60 / 24) + 'd)' : ''}"
          percent={snapshot.secondary?.used_percent ?? 0}
          resetTime={snapshot.secondary ? formatResetTime(snapshot.secondary.resets_at) : ''}
        />
        <UsageBar
          label="{snapshot.tertiary?.reset_description || 'Monthly'}{snapshot.tertiary?.window_minutes ? '  (' + Math.round(snapshot.tertiary.window_minutes / 60 / 24) + 'd)' : ''}"
          percent={snapshot.tertiary?.used_percent ?? 0}
          resetTime={snapshot.tertiary ? formatResetTime(snapshot.tertiary.resets_at) : ''}
        />
      {:else}
        <UsageBar label="Session" percent={0} />
        <UsageBar label="Daily" percent={0} />
        <UsageBar label="Monthly" percent={0} />
      {/if}
    </div>

    <!-- Sparkline -->
    {#if historyPoints.length > 1}
      <div class="sparkline-section">
        <div class="sparkline-header">
          <span class="section-label-small">TREND  ({historyPoints.length} sessions)</span>
          <span class="trend-value">{sparkTrend()}</span>
        </div>
        <div class="sparkline-canvas">
          <svg class="sparkline-svg" viewBox="0 0 280 30" preserveAspectRatio="none" role="img" aria-label="Usage trend">
            <polyline
              points={historyPoints.map((v, i) =>
                `${(i / (historyPoints.length - 1)) * 280},${30 - (v / 100) * 28}`
              ).join(' ')}
              fill="none"
              stroke="#22D3EE"
              stroke-width="2"
              stroke-linecap="round"
              stroke-linejoin="round"
            />
          </svg>
        </div>
      </div>
    {/if}

    {#if error}
      <div class="error-banner">
        <div class="error-banner-left">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#EF4444" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" role="img" aria-label="Error"><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
          <div class="error-banner-text">
            <span class="error-banner-title">Request failed</span>
            <span class="error-banner-desc">{error}</span>
          </div>
        </div>
        <button class="btn-retry" onclick={onRefresh} disabled={loading}>Retry</button>
      </div>
    {/if}

    <!-- Action Buttons -->
    <div class="actions-row">
      <button class="btn-primary btn-action" onclick={onRefresh} disabled={loading}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#0A0F1C" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="23 4 23 10 17 10"/><polyline points="1 20 1 14 7 14"/><path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"/></svg>
        Refresh Now
      </button>
      <button class="btn-secondary btn-action" onclick={openUsageDashboard}>
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>
        Dashboard
      </button>
      <div class="export-wrapper">
        <button class="btn-icon" onclick={() => showExportMenu = !showExportMenu} aria-label="Export history">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
        </button>
        {#if showExportMenu}
          <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions a11y_interactive_supports_focus -->
          <div class="export-menu" role="menu" tabindex="-1" onclick={(e) => e.stopPropagation()}>
            <button class="export-item" onclick={handleExportJson}>Export JSON</button>
            <button class="export-item" onclick={handleExportCsv}>Export CSV</button>
          </div>
        {/if}
      </div>
    </div>
    {#if saveError}
      <div class="save-error" role="alert">{saveError}</div>
    {/if}
  {/if}
  </div>

  <!-- Divider + Footer -->
  <div class="divider"></div>
  <div class="footer">
    <button class="footer-link" onclick={toggleSettings}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#64748B" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>
      Settings
    </button>
    <div class="footer-right">
      {#if isAvailable}
        <button class="footer-link-subtle" onclick={onLogout}>Disconnect</button>
      {/if}
      <button class="footer-link-subtle" onclick={toggleAbout}>About</button>
      <button class="footer-link-subtle" onclick={handleQuit}>Quit</button>
    </div>
  </div>

  <!-- ========== SETTINGS MODAL ========== -->
  {#if showSettings}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modal-overlay" onclick={toggleSettings} role="presentation">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="modal" bind:this={settingsModalEl} onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') toggleSettings(); }} role="dialog" aria-modal="true" aria-label="Settings" tabindex="-1">
        <div class="modal-header">
          <div class="modal-header-left">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#22D3EE" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M12 1v2M12 21v2M4.22 4.22l1.42 1.42M18.36 18.36l1.42 1.42M1 12h2M21 12h2M4.22 19.78l1.42-1.42M18.36 5.64l1.42-1.42"/></svg>
            <span class="modal-title">Settings</span>
          </div>
          <button class="btn-close" onclick={toggleSettings} aria-label="Close">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="modal-divider"></div>

        <div class="settings-scroll">
          <!-- PROVIDERS -->
          <div class="settings-section-label">PROVIDERS</div>
          <div class="settings-list">
            {#each allProviders as provider}
              {@const isEnabled = localEnabledProviders.includes(provider.id)}
              <div class="settings-row" class:active-row={isEnabled}>
                <div class="settings-row-left">
                  <span class="provider-dot" style="background: {provider.color}"></span>
                  <span class="settings-row-text" class:dimmed={!isEnabled}>{provider.name}</span>
                </div>
                <label class="toggle">
                  <input type="checkbox" aria-label="Enable {provider.name} provider" checked={isEnabled} onchange={() => handleProviderToggle(provider.id)} disabled={isEnabled && localEnabledProviders.length <= 1} />
                  <span class="toggle-slider"></span>
                </label>
              </div>
            {/each}
          </div>

          <div class="section-divider"></div>

          <!-- API KEYS -->
          <div class="settings-section-label">API KEYS</div>
          <div class="settings-list">
            {#each allProviders.filter(p => p.id !== 'claude') as provider}
              <div class="settings-row-vertical">
                <div class="settings-row">
                  <div class="settings-row-left">
                    <span class="provider-dot" style="background: {provider.color}"></span>
                    <span class="settings-row-text">{provider.name}</span>
                  </div>
                </div>
                <input
                  type="password"
                  class="api-key-input"
                  aria-label="{provider.name} API key"
                  placeholder="Enter API key…"
                  value={apiKeys[provider.id] ?? ''}
                  onblur={(e) => handleApiKeyChange(provider.id, (e.target as HTMLInputElement).value)}
                />
              </div>
            {/each}
          </div>

          <div class="section-divider"></div>

          <!-- GENERAL -->
          <div class="settings-section-label">GENERAL</div>
          <div class="settings-list">
            <div class="settings-row-vertical">
              <div class="settings-row">
                <div class="settings-row-detail">
                  <span class="settings-row-text">Auto-refresh interval</span>
                  <span class="settings-row-hint">How often to fetch new data</span>
                </div>
                <div class="stepper cyan">
                  <button class="stepper-btn" onclick={() => { const i = refreshPresets.indexOf(refreshInterval); if (i > 0) setRefreshInterval(refreshPresets[i-1]); }} aria-label="Decrease">−</button>
                  <span class="stepper-val">{refreshInterval}m</span>
                  <button class="stepper-btn" onclick={() => { const i = refreshPresets.indexOf(refreshInterval); if (i < refreshPresets.length - 1) setRefreshInterval(refreshPresets[i+1]); }} aria-label="Increase">+</button>
                </div>
              </div>
              <div class="presets-row">
                {#each refreshPresets as preset}
                  <button class="preset-chip" class:active-preset={refreshInterval === preset} onclick={() => setRefreshInterval(preset)}>
                    {preset}m
                  </button>
                {/each}
              </div>
            </div>
            <div class="settings-row">
              <div class="settings-row-detail">
                <span class="settings-row-text">Start on login</span>
                <span class="settings-row-hint">Launch with system startup</span>
              </div>
              <label class="toggle">
                <input type="checkbox" aria-label="Start on login" checked={startOnLogin} onchange={handleStartOnLoginChange} disabled={settingsLoading} />
                <span class="toggle-slider"></span>
              </label>
            </div>
          </div>

          <div class="section-divider"></div>

          <!-- NOTIFICATIONS -->
          <div class="settings-section-label">NOTIFICATIONS</div>
          <div class="settings-list notif-list">
            <!-- Warning Threshold -->
            <div class="notif-card">
              <div class="notif-header">
                <div class="notif-header-left">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#F59E0B" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>
                  <span class="notif-title">Warning threshold</span>
                </div>
                <div class="stepper amber">
                  <button class="stepper-btn" onclick={() => adjustThreshold('warning', -5)} aria-label="Decrease">−</button>
                  <span class="stepper-val">{warningThreshold}%</span>
                  <button class="stepper-btn" onclick={() => adjustThreshold('warning', 5)} aria-label="Increase">+</button>
                </div>
              </div>
              <div class="notif-slider-track">
                <div class="notif-slider-fill amber-fill" style="width: {warningThreshold}%"></div>
              </div>
              <span class="notif-hint">Alert before hitting the limit</span>
            </div>

            <!-- Critical Threshold -->
            <div class="notif-card">
              <div class="notif-header">
                <div class="notif-header-left">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#EF4444" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polygon points="7.86 2 16.14 2 22 7.86 22 16.14 16.14 22 7.86 22 2 16.14 2 7.86 7.86 2"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
                  <span class="notif-title">Critical threshold</span>
                </div>
                <div class="stepper red">
                  <button class="stepper-btn" onclick={() => adjustThreshold('critical', -5)} aria-label="Decrease">−</button>
                  <span class="stepper-val">{criticalThreshold}%</span>
                  <button class="stepper-btn" onclick={() => adjustThreshold('critical', 5)} aria-label="Increase">+</button>
                </div>
              </div>
              <div class="notif-slider-track">
                <div class="notif-slider-fill red-fill" style="width: {criticalThreshold}%"></div>
              </div>
              <span class="notif-hint">Send critical OS notification</span>
            </div>

            <!-- Cooldown Period -->
            <div class="notif-card">
              <div class="notif-header">
                <div class="notif-header-left">
                  <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="#22D3EE" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
                  <span class="notif-title">Cooldown period</span>
                </div>
                <div class="stepper cyan">
                  <button class="stepper-btn" onclick={() => adjustCooldown(-1)} aria-label="Decrease">−</button>
                  <span class="stepper-val">{cooldownLabel(notificationCooldown)}</span>
                  <button class="stepper-btn" onclick={() => adjustCooldown(1)} aria-label="Increase">+</button>
                </div>
              </div>
              <div class="presets-row">
                {#each cooldownPresets as preset}
                  <button class="preset-chip" class:active-preset={notificationCooldown === preset} onclick={() => setCooldown(preset)}>
                    {cooldownLabel(preset)}
                  </button>
                {/each}
              </div>
              <span class="notif-hint">Min. time between repeat alerts</span>
            </div>
          </div>

          {#if saveError}
            <div class="save-error" role="alert">{saveError}</div>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  <!-- ========== ABOUT MODAL ========== -->
  {#if showAbout}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="modal-overlay" onclick={toggleAbout} role="presentation">
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="modal" bind:this={aboutModalEl} onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === 'Escape') toggleAbout(); }} role="dialog" aria-modal="true" aria-label="About GPTBar" tabindex="-1">
        <div class="modal-header">
          <div class="modal-header-left">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="#22D3EE" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>
            <span class="modal-title">About GPTBar</span>
          </div>
          <button class="btn-close" onclick={toggleAbout} aria-label="Close">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#94A3B8" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
        <div class="modal-divider"></div>

        <div class="about-content">
          <div class="about-logo">
            <svg width="40" height="40" viewBox="0 0 32 32" fill="none" aria-hidden="true">
              <rect x="3" y="19" width="5" height="8" rx="1.5" fill="#0A0F1C"/>
              <rect x="10" y="13" width="5" height="14" rx="1.5" fill="#0A0F1C"/>
              <rect x="17" y="16" width="5" height="11" rx="1.5" fill="#0A0F1C"/>
              <rect x="24" y="9" width="5" height="18" rx="1.5" fill="#0A0F1C"/>
            </svg>
          </div>
          <div class="about-info">
            <h2 class="about-name">GPTBar</h2>
            <span class="about-version-pill">v{appVersion}</span>
            <p class="about-desc">Real-time AI usage monitor for your system tray</p>
          </div>

          <div class="about-stats">
            <div class="stat-card">
              <span class="stat-val">5</span>
              <span class="stat-label">Providers</span>
            </div>
            <div class="stat-card">
              <span class="stat-val">Win</span>
              <span class="stat-label">Platform</span>
            </div>
            <div class="stat-card">
              <span class="stat-val">MIT</span>
              <span class="stat-label">License</span>
            </div>
          </div>

          <button
            class="about-update-btn"
            onclick={handleUpdateButton}
            disabled={updateState.phase === 'downloading' || updateState.phase === 'ready' || updateState.phase === 'checking'}
          >
            {updateButtonLabel}
          </button>

          <span class="about-footer-text">Built with Rust + Tauri + Svelte</span>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  /* ========== BASE ========== */
  .card {
    background-color: #0A0F1C;
    display: flex;
    flex-direction: column;
    font-family: 'Inter', sans-serif;
    flex: 1;
    min-height: 0;
    overflow: hidden;
  }

  .content-scroll {
    flex: 1;
    overflow-y: auto;
    overflow-x: hidden;
    display: flex;
    flex-direction: column;
  }

  /* ========== PROVIDER SECTION ========== */
  .provider-section { padding: 16px 16px 0; }

  .provider-card {
    display: flex;
    gap: 12px;
    align-items: center;
    padding: 12px 16px;
    border-radius: 12px;
    background: #1E293B;
  }

  .provider-header-bar {
    padding: 12px 16px;
  }

  .provider-row {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .provider-icon {
    width: 40px;
    height: 40px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .icon-letter {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 700;
    font-size: 18px;
    color: #FFFFFF;
  }

  .provider-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  .provider-name {
    font-weight: 700;
    font-size: 15px;
    color: #FFFFFF;
  }

  .provider-email {
    font-size: 11px;
    color: #CBD5E1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .provider-sub {
    font-size: 11px;
    color: #CBD5E1;
  }

  .provider-meta {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .plan-badge {
    background: rgba(34, 211, 238, 0.13);
    color: #22D3EE;
    font-family: 'JetBrains Mono', monospace;
    font-weight: 700;
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 4px;
    letter-spacing: 1.5px;
  }

  .updated-time {
    font-size: 10px;
    color: #CBD5E1;
  }

  /* ========== USAGE BARS ========== */
  .usage-section {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .section-label {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 600;
    font-size: 10px;
    color: #94A3B8;
    letter-spacing: 2px;
  }

  .loading-hint {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    color: #94A3B8;
    text-align: center;
  }

  /* ========== SPARKLINE ========== */
  .sparkline-section {
    padding: 0 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .sparkline-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .section-label-small {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 500;
    font-size: 10px;
    color: #94A3B8;
    letter-spacing: 1.5px;
  }

  .trend-value {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 600;
    font-size: 10px;
    color: #22D3EE;
  }

  .sparkline-canvas {
    height: 36px;
    background: #0F172A;
    border-radius: 6px;
    padding: 4px;
  }

  .sparkline-svg {
    width: 100%;
    height: 100%;
    display: block;
  }

  /* ========== ERROR BANNER ========== */
  .error-banner {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    margin: 0 16px 8px;
    padding: 10px 12px;
    border-radius: 8px;
    background: linear-gradient(135deg, rgba(239, 68, 68, 0.15), rgba(239, 68, 68, 0.05));
    border: 1px solid rgba(239, 68, 68, 0.3);
  }

  .error-banner-left {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    min-width: 0;
    flex: 1;
  }

  .error-banner-left svg {
    flex-shrink: 0;
    margin-top: 1px;
  }

  .error-banner-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .error-banner-title {
    font-weight: 600;
    font-size: 12px;
    color: #EF4444;
  }

  .error-banner-desc {
    font-family: 'JetBrains Mono', monospace;
    font-size: 10px;
    color: #94A3B8;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }

  .btn-retry {
    padding: 5px 12px;
    border: none;
    border-radius: 6px;
    background: rgba(239, 68, 68, 0.2);
    color: #EF4444;
    font-weight: 700;
    font-size: 11px;
    cursor: pointer;
    white-space: nowrap;
    flex-shrink: 0;
    transition: background-color 0.15s;
  }

  .btn-retry:hover:not(:disabled) { background: rgba(239, 68, 68, 0.3); }
  .btn-retry:disabled { opacity: 0.5; cursor: not-allowed; }

  .save-error {
    margin: 0 16px 8px;
    padding: 6px 12px;
    border-radius: 6px;
    background: rgba(239, 68, 68, 0.12);
    color: #FCA5A5;
    font-size: 11px;
    text-align: center;
  }

  /* ========== ACTION BUTTONS ========== */
  .actions-row {
    display: flex;
    gap: 8px;
    padding: 10px 16px;
  }

  .btn-primary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 14px;
    border: none;
    border-radius: 8px;
    background: #22D3EE;
    color: #0A0F1C;
    font-family: 'Inter', sans-serif;
    font-weight: 600;
    font-size: 12px;
    cursor: pointer;
    transition: opacity 0.15s;
    white-space: nowrap;
  }

  .btn-primary:hover:not(:disabled) { opacity: 0.9; }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }

  .btn-secondary {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 8px 12px;
    border: none;
    border-radius: 8px;
    background: #1E293B;
    color: #94A3B8;
    font-family: 'Inter', sans-serif;
    font-weight: 500;
    font-size: 12px;
    cursor: pointer;
    transition: background-color 0.15s;
    white-space: nowrap;
  }

  .btn-secondary:hover { background: #334155; }

  .btn-action { flex: 1; }

  .btn-icon {
    width: 36px;
    height: 36px;
    border: none;
    border-radius: 8px;
    background: #1E293B;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background-color 0.15s;
    flex-shrink: 0;
  }

  .btn-icon:hover { background: #334155; }

  .export-wrapper { position: relative; }

  .export-menu {
    position: absolute;
    bottom: 100%;
    right: 0;
    background: #1E293B;
    border: 1px solid #334155;
    border-radius: 8px;
    overflow: hidden;
    z-index: 10;
    margin-bottom: 4px;
  }

  .export-item {
    display: block;
    width: 100%;
    padding: 8px 16px;
    border: none;
    background: transparent;
    color: #94A3B8;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    cursor: pointer;
    text-align: left;
    white-space: nowrap;
  }

  .export-item:hover { background: #334155; color: #FFFFFF; }

  /* ========== DIVIDER & FOOTER ========== */
  .divider {
    height: 1px;
    background: #1E293B;
  }

  .footer {
    display: flex;
    justify-content: space-between;
    align-items: center;
    height: 48px;
    padding: 0 16px;
  }

  .footer-link {
    display: flex;
    align-items: center;
    gap: 6px;
    border: none;
    background: none;
    color: #94A3B8;
    font-family: 'Inter', sans-serif;
    font-size: 11px;
    cursor: pointer;
    padding: 4px 0;
  }

  .footer-link:hover { color: #CBD5E1; }

  .footer-right {
    display: flex;
    gap: 12px;
    align-items: center;
  }

  .footer-link-subtle {
    border: none;
    background: none;
    color: #94A3B8;
    font-family: 'Inter', sans-serif;
    font-size: 11px;
    cursor: pointer;
    padding: 4px 0;
  }

  .footer-link-subtle:hover { color: #CBD5E1; }

  /* ========== AUTH (NOT AUTHENTICATED) ========== */
  .auth-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
    padding: 32px 24px;
  }

  .lock-icon-bg {
    width: 72px;
    height: 72px;
    border-radius: 16px;
    background: #1E293B;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .auth-title {
    font-weight: 700;
    font-size: 18px;
    color: #FFFFFF;
    margin: 0;
  }

  .auth-desc {
    font-size: 13px;
    color: #94A3B8;
    text-align: center;
    line-height: 1.5;
    margin: 0;
  }

  .auth-steps {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    padding: 14px 16px;
    border-radius: 10px;
    background: #0F172A;
  }

  .step-row {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .step-num {
    width: 22px;
    height: 22px;
    border-radius: 11px;
    background: #1E293B;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .step-num span {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 600;
    font-size: 11px;
    color: #22D3EE;
  }

  .step-text {
    font-family: 'JetBrains Mono', monospace;
    font-size: 12px;
    color: #94A3B8;
  }

  .auth-buttons {
    padding: 0 16px 16px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  /* ========== MODAL (shared) ========== */
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.7);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: overlay-fade 0.18s ease-out both;
  }

  .modal {
    background: #0A0F1C;
    border: 1px solid #1E293B;
    border-radius: 12px;
    width: 92%;
    max-width: 300px;
    max-height: calc(100vh - 24px);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: modal-pop 0.22s cubic-bezier(0.22, 1, 0.36, 1) both;
  }

  @keyframes overlay-fade {
    from { opacity: 0; }
    to { opacity: 1; }
  }

  @keyframes modal-pop {
    from { opacity: 0; transform: translateY(8px) scale(0.97); }
    to { opacity: 1; transform: translateY(0) scale(1); }
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 16px;
    height: 56px;
    background: #0F172A;
    flex-shrink: 0;
  }

  .modal-header-left {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .modal-title {
    font-weight: 700;
    font-size: 15px;
    color: #FFFFFF;
  }

  .btn-close {
    width: 28px;
    height: 28px;
    border: none;
    border-radius: 6px;
    background: #1E293B;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background-color 0.15s;
  }

  .btn-close:hover { background: #334155; }

  .modal-divider {
    height: 1px;
    background: #1E293B;
  }

  /* ========== SETTINGS MODAL ========== */
  .settings-scroll {
    overflow-y: auto;
    padding-bottom: 16px;
  }

  .settings-section-label {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 600;
    font-size: 10px;
    color: #94A3B8;
    letter-spacing: 2px;
    padding: 14px 16px 8px;
  }

  .settings-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 12px;
  }

  .notif-list {
    gap: 8px;
    padding: 0 12px 4px;
  }

  .settings-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0 8px;
    height: 44px;
    border-radius: 8px;
  }

  .settings-row-vertical {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-bottom: 4px;
  }

  .active-row { background: #1E293B; }

  .settings-row-left {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .provider-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }

  .settings-row-text {
    font-size: 13px;
    font-weight: 500;
    color: #FFFFFF;
  }

  .settings-row-text.dimmed { color: #94A3B8; }

  .settings-row-detail {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .settings-row-hint {
    font-size: 11px;
    color: #94A3B8;
  }

  .section-divider {
    height: 1px;
    background: #0F172A;
    margin: 4px 0;
  }

  /* Toggle */
  .toggle {
    position: relative;
    display: inline-block;
    width: 34px;
    height: 20px;
  }

  .toggle input { opacity: 0; width: 0; height: 0; }

  .toggle-slider {
    position: absolute;
    cursor: pointer;
    inset: 0;
    background-color: #1E293B;
    transition: 0.3s;
    border-radius: 10px;
  }

  .toggle-slider:before {
    content: "";
    position: absolute;
    height: 16px;
    width: 16px;
    left: 2px;
    bottom: 2px;
    background-color: #475569;
    transition: 0.3s;
    border-radius: 50%;
  }

  .toggle input:checked + .toggle-slider {
    background-color: #22D3EE;
  }

  .toggle input:checked + .toggle-slider:before {
    background-color: #0A0F1C;
    transform: translateX(14px);
  }

  .toggle input:disabled + .toggle-slider {
    opacity: 0.4;
    cursor: not-allowed;
  }

  /* Stepper */
  .stepper {
    display: flex;
    align-items: center;
    border-radius: 6px;
    padding: 3px 0;
  }

  .stepper.cyan { background: rgba(34, 211, 238, 0.13); }
  .stepper.amber { background: rgba(245, 158, 11, 0.13); }
  .stepper.red { background: rgba(239, 68, 68, 0.13); }

  .stepper-btn {
    width: 28px;
    height: 26px;
    border: none;
    background: transparent;
    font-family: 'JetBrains Mono', monospace;
    font-weight: 700;
    font-size: 14px;
    cursor: pointer;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .stepper.cyan .stepper-btn { color: #22D3EE; }
  .stepper.amber .stepper-btn { color: #F59E0B; }
  .stepper.red .stepper-btn { color: #EF4444; }

  .stepper-btn:hover { background: rgba(255,255,255,0.05); }

  .stepper-val {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 700;
    font-size: 13px;
    min-width: 36px;
    text-align: center;
  }

  .stepper.cyan .stepper-val { color: #22D3EE; }
  .stepper.amber .stepper-val { color: #F59E0B; }
  .stepper.red .stepper-val { color: #EF4444; }

  /* Preset chips */
  .presets-row {
    display: flex;
    gap: 6px;
    padding: 0 8px;
  }

  .preset-chip {
    flex: 1;
    padding: 4px 0;
    border: 1px solid transparent;
    border-radius: 5px;
    background: #1E293B;
    font-family: 'JetBrains Mono', monospace;
    font-weight: 500;
    font-size: 10px;
    color: #64748B;
    cursor: pointer;
    text-align: center;
    transition: all 0.15s;
  }

  .preset-chip:hover { background: #334155; }

  .preset-chip.active-preset {
    background: rgba(34, 211, 238, 0.2);
    border-color: rgba(34, 211, 238, 0.53);
    color: #22D3EE;
    font-weight: 700;
  }

  /* API key inputs */
  .api-key-input {
    width: 100%;
    box-sizing: border-box;
    background: #0F172A;
    border: 1px solid #1E293B;
    border-radius: 6px;
    color: #E2E8F0;
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    padding: 6px 10px;
    outline: none;
    transition: border-color 0.15s;
  }

  .api-key-input:focus {
    border-color: #22D3EE;
  }

  .api-key-input::placeholder {
    color: #475569;
  }

  /* Notification cards */
  .notif-card {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px;
    border-radius: 8px;
    background: #0F172A;
  }

  .notif-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .notif-header-left {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .notif-title {
    font-size: 12px;
    font-weight: 500;
    color: #FFFFFF;
  }

  .notif-slider-track {
    height: 6px;
    border-radius: 3px;
    background: #1E293B;
    overflow: hidden;
  }

  .notif-slider-fill {
    height: 100%;
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .amber-fill { background: linear-gradient(90deg, #F59E0B, #D97706); }
  .red-fill { background: linear-gradient(90deg, #EF4444, #DC2626); }

  .notif-hint {
    font-size: 10px;
    color: #94A3B8;
  }

  /* ========== ABOUT MODAL ========== */
  .about-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 20px;
    padding: 28px 24px 24px;
  }

  .about-logo {
    width: 80px;
    height: 80px;
    border-radius: 20px;
    background: linear-gradient(135deg, #22D3EE, #0891B2);
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .about-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .about-name {
    font-weight: 800;
    font-size: 22px;
    color: #FFFFFF;
    margin: 0;
  }

  .about-version-pill {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 600;
    font-size: 11px;
    color: #22D3EE;
    background: #1E293B;
    padding: 3px 10px;
    border-radius: 100px;
  }

  .about-desc {
    font-size: 13px;
    color: #94A3B8;
    text-align: center;
    line-height: 1.5;
    margin: 0;
  }

  .about-stats {
    display: flex;
    gap: 8px;
    width: 100%;
  }

  .stat-card {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 14px 12px;
    border-radius: 10px;
    background: #0F172A;
  }

  .stat-val {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 700;
    font-size: 20px;
    color: #22D3EE;
  }

  .stat-label {
    font-size: 11px;
    color: #94A3B8;
  }

  .about-footer-text {
    font-family: 'JetBrains Mono', monospace;
    font-size: 11px;
    color: #94A3B8;
  }

  .about-update-btn {
    font-size: 11px;
    font-weight: 600;
    color: #22D3EE;
    background: rgba(34, 211, 238, 0.08);
    border: 1px solid rgba(34, 211, 238, 0.2);
    border-radius: 6px;
    padding: 6px 14px;
    margin-bottom: 10px;
    cursor: pointer;
    transition: background 0.15s ease, border-color 0.15s ease;
  }

  .about-update-btn:hover:not(:disabled) {
    background: rgba(34, 211, 238, 0.14);
    border-color: rgba(34, 211, 238, 0.35);
  }

  .about-update-btn:disabled {
    cursor: default;
    opacity: 0.6;
  }

  .provider-card {
    animation: card-rise 0.32s ease-out both;
  }

  @keyframes card-rise {
    from { opacity: 0; transform: translateY(6px); }
    to { opacity: 1; transform: translateY(0); }
  }

  @media (prefers-reduced-motion: reduce) {
    .modal-overlay,
    .modal,
    .provider-card,
    .about-update-btn {
      animation: none;
    }
  }
</style>
