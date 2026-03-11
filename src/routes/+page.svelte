<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from "@tauri-apps/api/core";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import ProviderCard from '$lib/components/ProviderCard.svelte';
  import ProviderTabs from '$lib/components/ProviderTabs.svelte';
  import type { UsageSnapshot, ProviderState, AppConfig } from '$lib/types';

  // All available providers with their display names
  const providerNames: Record<string, string> = {
    claude: 'Claude',
    openai: 'OpenAI',
    gemini: 'Gemini',
    codex: 'Codex',
    xai: 'xAI (Grok)',
  };

  // Provider states
  let providerStates = $state<Record<string, ProviderState>>({});
  let enabledProviders = $state<string[]>(['claude']);
  let activeProvider = $state('claude');

  // Reference to close modals
  let closeModals: (() => void) | null = $state(null);

  // Compute overall usage status for header dot and tray icon
  type StatusLevel = 'normal' | 'warning' | 'critical';
  let usageStatus = $derived.by<StatusLevel>(() => {
    let maxPercent = 0;
    for (const pid of enabledProviders) {
      const s = providerStates[pid]?.snapshot;
      if (s?.primary?.used_percent != null && s.primary.used_percent > maxPercent) {
        maxPercent = s.primary.used_percent;
      }
    }
    if (maxPercent >= 95) return 'critical';
    if (maxPercent >= 85) return 'warning';
    return 'normal';
  });

  // Header subtitle
  let headerSubtitle = $derived(
    enabledProviders.length > 1
      ? `${enabledProviders.length} providers active`
      : 'AI Usage Monitor'
  );

  // Update tray icon when status changes
  $effect(() => {
    invoke('update_tray_status', { status: usageStatus }).catch(() => {});
  });

  // Refresh from header
  let refreshing = $state(false);
  async function handleHeaderRefresh() {
    if (refreshing) return;
    refreshing = true;
    try {
      await handleRefresh(activeProvider);
    } finally {
      refreshing = false;
    }
  }

  // Get current provider state
  let currentProvider = $derived(providerStates[activeProvider] || {
    id: activeProvider,
    name: providerNames[activeProvider] || activeProvider,
    snapshot: null,
    loading: false,
    error: null,
    isAvailable: false,
  });

  async function loadConfig() {
    try {
      const config = await invoke<AppConfig>('get_config');
      enabledProviders = config.enabled_providers.length > 0
        ? config.enabled_providers
        : ['claude'];

      // Set active to first enabled provider
      if (!enabledProviders.includes(activeProvider)) {
        activeProvider = enabledProviders[0];
      }
    } catch (e) {
      console.error('Failed to load config:', e);
    }
  }

  async function checkProviderAvailability(providerId: string) {
    try {
      const available = await invoke<boolean>('is_provider_available', { providerId });
      updateProviderState(providerId, { isAvailable: available });
      return available;
    } catch (e) {
      console.error(`Failed to check availability for ${providerId}:`, e);
      return false;
    }
  }

  async function fetchProviderUsage(providerId: string) {
    updateProviderState(providerId, { loading: true, error: null });
    try {
      const snapshot = await invoke<UsageSnapshot>('fetch_provider_usage', { providerId });
      updateProviderState(providerId, { snapshot, loading: false });
    } catch (e) {
      updateProviderState(providerId, { error: String(e), loading: false });
      console.error(`Failed to fetch usage for ${providerId}:`, e);
    }
  }

  function updateProviderState(providerId: string, updates: Partial<ProviderState>) {
    const existing = providerStates[providerId] || {
      id: providerId,
      name: providerNames[providerId] || providerId,
      snapshot: null,
      loading: false,
      error: null,
      isAvailable: false,
    };

    providerStates = {
      ...providerStates,
      [providerId]: {
        ...existing,
        ...updates,
      },
    };
  }

  async function handleLogin(providerId: string) {
    try {
      await invoke('login_provider', { providerId });
      // For Claude, try reload token
      if (providerId === 'claude') {
        await handleReloadToken();
      } else {
        await checkProviderAvailability(providerId);
      }
    } catch (e) {
      console.error(`Login failed for ${providerId}:`, e);
    }
  }

  async function handleReloadToken() {
    try {
      const found = await invoke<boolean>('reload_token');
      const available = await checkProviderAvailability('claude');
      if (found && available) {
        await fetchProviderUsage('claude');
      }
    } catch (e) {
      console.error('Failed to reload token:', e);
    }
  }

  async function handleLogout(providerId: string) {
    try {
      await invoke('logout_provider', { providerId });
      updateProviderState(providerId, { snapshot: null });
      await checkProviderAvailability(providerId);
    } catch (e) {
      console.error(`Logout failed for ${providerId}:`, e);
    }
  }

  async function handleRefresh(providerId: string) {
    const available = await checkProviderAvailability(providerId);
    if (available) {
      await fetchProviderUsage(providerId);
    }
  }

  function handleTabSelect(providerId: string) {
    activeProvider = providerId;
    // Fetch data if not already loaded
    if (!providerStates[providerId]?.snapshot && !providerStates[providerId]?.loading) {
      handleRefresh(providerId);
    }
  }

  // Called when enabled providers change from Settings
  async function onEnabledProvidersChange(newProviders: string[]) {
    enabledProviders = newProviders;
    if (!enabledProviders.includes(activeProvider) && enabledProviders.length > 0) {
      activeProvider = enabledProviders[0];
    }
  }

  onMount(() => {
    let unlistenFocus: (() => void) | null = null;
    let interval: ReturnType<typeof setInterval> | null = null;

    // Initialize everything
    (async () => {
      // Load config first
      await loadConfig();

      // Initialize all enabled providers
      for (const providerId of enabledProviders) {
        updateProviderState(providerId, {
          id: providerId,
          name: providerNames[providerId] || providerId,
        });

        const available = await checkProviderAvailability(providerId);
        if (available) {
          await fetchProviderUsage(providerId);
        }
      }

      // Listen for focus changes to close modals (hide is handled by Rust)
      const appWindow = getCurrentWindow();
      unlistenFocus = await appWindow.onFocusChanged(({ payload: focused }: { payload: boolean }) => {
        if (!focused && closeModals) {
          closeModals();
        }
      });

      // Refresh active provider every 5 minutes
      interval = setInterval(async () => {
        const state = providerStates[activeProvider];
        if (state?.isAvailable) {
          await fetchProviderUsage(activeProvider);
        }
      }, 5 * 60 * 1000);
    })();

    return () => {
      if (interval) clearInterval(interval);
      if (unlistenFocus) unlistenFocus();
    };
  });
</script>

<main class="container">
  <header class="app-header">
    <div class="header-left">
      <div class="header-icon">
        <svg viewBox="0 0 32 32" aria-hidden="true">
          <defs>
            <linearGradient id="hdr-bg" x1="0%" y1="0%" x2="100%" y2="100%">
              <stop offset="0%" style="stop-color:#1A2332"/>
              <stop offset="100%" style="stop-color:#0B1120"/>
            </linearGradient>
            <linearGradient id="hdr-bar" x1="0%" y1="100%" x2="0%" y2="0%">
              <stop offset="0%" style="stop-color:#0891B2"/>
              <stop offset="100%" style="stop-color:#22D3EE"/>
            </linearGradient>
            <linearGradient id="hdr-bar-hi" x1="0%" y1="100%" x2="0%" y2="0%">
              <stop offset="0%" style="stop-color:#D97706"/>
              <stop offset="100%" style="stop-color:#FBBF24"/>
            </linearGradient>
          </defs>
          <rect width="32" height="32" rx="7" fill="url(#hdr-bg)"/>
          <rect x="5" y="20" width="4" height="5" rx="1" fill="url(#hdr-bar)" opacity="0.85"/>
          <rect x="11" y="15" width="4" height="10" rx="1" fill="url(#hdr-bar)" opacity="0.9"/>
          <rect x="17" y="11" width="4" height="14" rx="1" fill="url(#hdr-bar)"/>
          <rect x="23" y="8" width="4" height="17" rx="1" fill="url(#hdr-bar-hi)"/>
        </svg>
      </div>
      <div class="header-text">
        <span class="header-title">GPTBar</span>
        <span class="header-subtitle">{headerSubtitle}</span>
      </div>
    </div>
    <div class="header-right">
      <button
        class="header-refresh"
        onclick={handleHeaderRefresh}
        disabled={refreshing}
        title="Refresh"
        aria-label="Refresh usage data"
      >
        <svg viewBox="0 0 16 16" fill="none" aria-hidden="true" class:spinning={refreshing}>
          <path d="M13.65 2.35A7.96 7.96 0 0 0 8 0a8 8 0 1 0 8 8h-2a6 6 0 1 1-1.76-4.24L9 7h7V0l-2.35 2.35z" fill="currentColor"/>
        </svg>
      </button>
      <div class="header-dot status-{usageStatus}"></div>
    </div>
  </header>

  {#if enabledProviders.length > 1}
    <ProviderTabs
      providers={enabledProviders}
      {providerNames}
      {activeProvider}
      onSelect={handleTabSelect}
    />
  {/if}

  <ProviderCard
    providerId={currentProvider.id}
    providerName={currentProvider.name}
    snapshot={currentProvider.snapshot}
    loading={currentProvider.loading}
    error={currentProvider.error}
    isAvailable={currentProvider.isAvailable}
    onRefresh={() => handleRefresh(currentProvider.id)}
    onLogin={() => handleLogin(currentProvider.id)}
    onLogout={() => handleLogout(currentProvider.id)}
    {enabledProviders}
    {onEnabledProvidersChange}
    bind:closeModals={closeModals}
  />
</main>

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  :global(html, body) {
    width: 100%;
    height: 100%;
    background-color: #0A0F1C;
    font-family: 'Inter', -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    overflow: hidden;
  }

  :global(::-webkit-scrollbar) {
    width: 4px;
  }
  :global(::-webkit-scrollbar-track) {
    background: transparent;
  }
  :global(::-webkit-scrollbar-thumb) {
    background: #334155;
    border-radius: 2px;
  }
  :global(::-webkit-scrollbar-thumb:hover) {
    background: #475569;
  }

  .container {
    width: 100%;
    height: 100vh;
    background-color: #0A0F1C;
    color: white;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .app-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    flex-shrink: 0;
    border-bottom: 1px solid rgba(34, 211, 238, 0.08);
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .header-icon {
    width: 32px;
    height: 32px;
    flex-shrink: 0;
  }

  .header-icon svg {
    width: 100%;
    height: 100%;
  }

  .header-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .header-title {
    font-family: 'Inter', sans-serif;
    font-size: 14px;
    font-weight: 700;
    color: #F1F5F9;
    line-height: 1.2;
    letter-spacing: -0.01em;
  }

  .header-subtitle {
    font-family: 'Inter', sans-serif;
    font-size: 11px;
    font-weight: 400;
    color: #64748B;
    line-height: 1.2;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .header-refresh {
    width: 28px;
    height: 28px;
    border-radius: 6px;
    border: 1px solid #1E293B;
    background: #0F172A;
    color: #64748B;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all 0.15s ease;
    padding: 0;
  }

  .header-refresh:hover {
    color: #22D3EE;
    border-color: rgba(34, 211, 238, 0.2);
    background: rgba(34, 211, 238, 0.05);
  }

  .header-refresh:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .header-refresh svg {
    width: 14px;
    height: 14px;
  }

  .header-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .header-dot.status-normal {
    background: #22D3EE;
    box-shadow: 0 0 6px rgba(34, 211, 238, 0.4);
  }

  .header-dot.status-warning {
    background: #F59E0B;
    box-shadow: 0 0 6px rgba(245, 158, 11, 0.4);
  }

  .header-dot.status-critical {
    background: #EF4444;
    box-shadow: 0 0 6px rgba(239, 68, 68, 0.4);
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  :global(.spinning) {
    animation: spin 0.8s linear infinite;
  }
</style>
