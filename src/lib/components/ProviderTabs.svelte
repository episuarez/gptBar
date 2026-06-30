<script lang="ts">
  interface Props {
    providers: string[];
    providerNames: Record<string, string>;
    activeProvider: string;
    onSelect: (providerId: string) => void;
  }

  let { providers, providerNames, activeProvider, onSelect }: Props = $props();

  const providerColors: Record<string, string> = {
    claude: '#D97706',
    openai: '#10a37f',
    gemini: '#4285f4',
    codex: '#8B5CF6',
    xai: '#94A3B8',
  };

  function getColor(providerId: string) {
    return providerColors[providerId] || '#94A3B8';
  }

  // Detect if there are hidden tabs (scroll indicator)
  let scrollContainer: HTMLElement;
  let showScrollHint = $state(false);

  function checkScroll() {
    if (scrollContainer) {
      showScrollHint = scrollContainer.scrollWidth > scrollContainer.clientWidth + 4;
    }
  }

  // Check scroll on mount and when providers change
  $effect(() => {
    void providers.length;
    requestAnimationFrame(checkScroll);
  });

  // Convert vertical wheel to horizontal scroll
  function handleWheel(event: WheelEvent) {
    if (scrollContainer && Math.abs(event.deltaY) > Math.abs(event.deltaX)) {
      event.preventDefault();
      scrollContainer.scrollLeft += event.deltaY;
      checkScroll();
    }
  }

  // Arrow-key navigation between tabs
  let tabEls = $state<HTMLButtonElement[]>([]);

  function handleTabKeydown(event: KeyboardEvent, index: number) {
    if (event.key !== 'ArrowRight' && event.key !== 'ArrowLeft') return;
    event.preventDefault();
    const dir = event.key === 'ArrowRight' ? 1 : -1;
    const next = (index + dir + providers.length) % providers.length;
    onSelect(providers[next]);
    tabEls[next]?.focus();
  }
</script>

<div class="tab-bar">
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="tab-scroll"
    role="tablist"
    aria-label="Providers"
    bind:this={scrollContainer}
    onscroll={checkScroll}
    onwheel={handleWheel}
  >
    {#each providers as providerId, i}
      {@const isActive = activeProvider === providerId}
      <button
        class="tab"
        class:active={isActive}
        role="tab"
        aria-selected={isActive}
        tabindex={isActive ? 0 : -1}
        bind:this={tabEls[i]}
        onclick={() => onSelect(providerId)}
        onkeydown={(e) => handleTabKeydown(e, i)}
      >
        <span
          class="tab-dot"
          class:active-dot={isActive}
          style="background: {isActive ? `linear-gradient(135deg, ${getColor(providerId)}, ${getColor(providerId)}cc)` : getColor(providerId)}"
        ></span>
        <span class="tab-label" class:active-label={isActive}>
          {providerNames[providerId] || providerId}
        </span>
      </button>
    {/each}
  </div>

  {#if showScrollHint}
    <div class="fade-right"></div>
    <div class="more-indicator">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="#22D3EE" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-label="More tabs" role="img">
        <polyline points="9 18 15 12 9 6"></polyline>
      </svg>
    </div>
  {/if}
</div>

<svelte:window onresize={checkScroll} />

<style>
  .tab-bar {
    display: flex;
    align-items: center;
    height: 52px;
    background-color: #0A0F1C;
    position: relative;
    overflow: hidden;
  }

  .tab-scroll {
    display: flex;
    gap: 4px;
    align-items: center;
    padding: 0 12px;
    overflow-x: auto;
    flex: 1;
    height: 100%;
    scrollbar-width: none;
  }

  .tab-scroll::-webkit-scrollbar {
    display: none;
  }

  .tab {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 6px 12px;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    flex-shrink: 0;
    min-width: 80px;
    height: 38px;
    background: transparent;
    transition: background-color 0.15s;
  }

  .tab:hover {
    background-color: #1E293B80;
  }

  .tab.active {
    background-color: #1E293B;
  }

  .tab-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .tab-dot.active-dot {
    width: 8px;
    height: 8px;
  }

  .tab-label {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 500;
    font-size: 11px;
    color: #94A3B8;
    white-space: nowrap;
  }

  .tab-label.active-label {
    color: #FFFFFF;
    font-weight: 600;
  }

  .fade-right {
    position: absolute;
    right: 28px;
    top: 0;
    width: 48px;
    height: 100%;
    background: linear-gradient(90deg, transparent, #0A0F1C);
    pointer-events: none;
  }

  .more-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 100%;
    flex-shrink: 0;
  }
</style>
