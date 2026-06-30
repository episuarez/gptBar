<script lang="ts">
  import { onMount } from 'svelte';
  import {
    updateState,
    checkForUpdate,
    installUpdate,
    dismissUpdate,
  } from '$lib/updateStore.svelte';

  onMount(() => {
    checkForUpdate();
    // A tray app stays open for days — re-check every 6 hours.
    const interval = setInterval(() => checkForUpdate(), 6 * 60 * 60 * 1000);
    return () => clearInterval(interval);
  });

  // Only the banner-relevant phases render here.
  let show = $derived(
    !updateState.dismissed &&
      (updateState.phase === 'available' ||
        updateState.phase === 'downloading' ||
        updateState.phase === 'ready' ||
        updateState.phase === 'error'),
  );
</script>

{#if show}
  <div class="update-banner" role="status" aria-live="polite">
    {#if updateState.phase === 'available'}
      <div class="update-text">
        <span class="update-title">Update available</span>
        <span class="update-ver">v{updateState.newVersion}</span>
      </div>
      <div class="update-actions">
        <button class="btn-install" onclick={installUpdate}>Install</button>
        <button
          class="btn-dismiss"
          onclick={dismissUpdate}
          aria-label="Dismiss update notification"
        >
          Later
        </button>
      </div>
    {:else if updateState.phase === 'downloading'}
      <div class="update-text">
        <span class="update-title">Downloading update…</span>
        <span class="update-ver">{updateState.progress}%</span>
      </div>
      <div class="dl-track" aria-hidden="true">
        <div class="dl-fill" style="width: {updateState.progress}%"></div>
      </div>
    {:else if updateState.phase === 'ready'}
      <div class="update-text">
        <span class="update-title">Restarting…</span>
      </div>
    {:else if updateState.phase === 'error'}
      <div class="update-text">
        <span class="update-title update-err">Update failed</span>
      </div>
      <div class="update-actions">
        <button class="btn-install" onclick={installUpdate}>Retry</button>
        <button
          class="btn-dismiss"
          onclick={dismissUpdate}
          aria-label="Dismiss update notification"
        >
          Dismiss
        </button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .update-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    flex-wrap: wrap;
    padding: 8px 16px;
    background: rgba(34, 211, 238, 0.08);
    border-bottom: 1px solid rgba(34, 211, 238, 0.18);
    flex-shrink: 0;
    animation: banner-slide 0.3s ease-out both;
  }

  @keyframes banner-slide {
    from { opacity: 0; transform: translateY(-100%); }
    to { opacity: 1; transform: translateY(0); }
  }

  @media (prefers-reduced-motion: reduce) {
    .update-banner { animation: none; }
    .dl-fill { transition: none; }
  }

  .update-text {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
  }

  .update-title {
    font-size: 12px;
    font-weight: 600;
    color: #E2E8F0;
  }

  .update-title.update-err {
    color: #F87171;
  }

  .update-ver {
    font-size: 11px;
    color: #22D3EE;
    font-variant-numeric: tabular-nums;
  }

  .update-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .btn-install {
    font-size: 11px;
    font-weight: 600;
    color: #0A0F1C;
    background: #22D3EE;
    border: none;
    border-radius: 5px;
    padding: 4px 12px;
    cursor: pointer;
    transition: background 0.15s ease;
  }

  .btn-install:hover {
    background: #67E8F9;
  }

  .btn-dismiss {
    font-size: 11px;
    color: #94A3B8;
    background: transparent;
    border: 1px solid #1E293B;
    border-radius: 5px;
    padding: 4px 10px;
    cursor: pointer;
    transition: color 0.15s ease, border-color 0.15s ease;
  }

  .btn-dismiss:hover {
    color: #CBD5E1;
    border-color: #334155;
  }

  .dl-track {
    flex: 1 1 100%;
    height: 4px;
    background: #1E293B;
    border-radius: 2px;
    overflow: hidden;
  }

  .dl-fill {
    height: 100%;
    background: #22D3EE;
    transition: width 0.2s ease;
  }
</style>
