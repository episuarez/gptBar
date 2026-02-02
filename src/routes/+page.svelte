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
  };

  // Provider states
  let providerStates = $state<Record<string, ProviderState>>({});
  let enabledProviders = $state<string[]>(['claude']);
  let activeProvider = $state('claude');

  // Reference to close modals
  let closeModals: (() => void) | null = $state(null);

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
      unlistenFocus = await appWindow.onFocusChanged(({ payload: focused }) => {
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
    background-color: #1a1f2e;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
    overflow: hidden;
  }

  .container {
    width: 100%;
    height: 100vh;
    background-color: #1a1f2e;
    color: white;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
</style>
