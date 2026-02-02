<script lang="ts">
  interface Props {
    providers: string[];
    providerNames: Record<string, string>;
    activeProvider: string;
    onSelect: (providerId: string) => void;
  }

  let { providers, providerNames, activeProvider, onSelect }: Props = $props();

  // Provider colors/icons
  const providerStyles: Record<string, { bg: string; letter: string }> = {
    claude: { bg: 'linear-gradient(135deg, #d97706, #f59e0b)', letter: 'C' },
    openai: { bg: 'linear-gradient(135deg, #10a37f, #1a7f64)', letter: 'O' },
    gemini: { bg: 'linear-gradient(135deg, #4285f4, #34a853)', letter: 'G' },
    codex: { bg: 'linear-gradient(135deg, #6366f1, #8b5cf6)', letter: 'X' },
  };

  function getStyle(providerId: string) {
    return providerStyles[providerId] || { bg: '#6b7280', letter: '?' };
  }
</script>

<div class="tabs-container">
  {#each providers as providerId}
    {@const style = getStyle(providerId)}
    <button
      class="tab"
      class:active={activeProvider === providerId}
      onclick={() => onSelect(providerId)}
    >
      <span class="tab-icon" style="background: {style.bg}">
        {style.letter}
      </span>
      <span class="tab-name">{providerNames[providerId] || providerId}</span>
    </button>
  {/each}
</div>

<style>
  .tabs-container {
    display: flex;
    gap: 0.25rem;
    padding: 0.5rem;
    background-color: #151922;
    border-bottom: 1px solid #2d3548;
    overflow-x: auto;
  }

  .tabs-container::-webkit-scrollbar {
    height: 4px;
  }

  .tabs-container::-webkit-scrollbar-track {
    background: transparent;
  }

  .tabs-container::-webkit-scrollbar-thumb {
    background: #3d4558;
    border-radius: 2px;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.4rem 0.6rem;
    background: transparent;
    border: none;
    border-radius: 0.375rem;
    cursor: pointer;
    transition: background-color 0.15s;
    flex-shrink: 0;
  }

  .tab:hover {
    background-color: #2d3548;
  }

  .tab.active {
    background-color: #2d3548;
  }

  .tab-icon {
    width: 1.25rem;
    height: 1.25rem;
    border-radius: 0.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    color: white;
    font-weight: bold;
    font-size: 0.65rem;
  }

  .tab-name {
    color: #9ca3af;
    font-size: 0.75rem;
    font-weight: 500;
  }

  .tab.active .tab-name {
    color: white;
  }
</style>
