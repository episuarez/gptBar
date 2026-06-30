<script lang="ts">
  interface Props {
    label: string;
    percent: number;
    resetTime?: string;
  }

  let { label, percent, resetTime = '' }: Props = $props();

  function getColorClass(p: number): string {
    if (p >= 95) return 'critical';
    if (p >= 80) return 'warning';
    return 'normal';
  }

  let colorClass = $derived(getColorClass(percent));
  let clampedPercent = $derived(Math.min(Math.max(percent, 0), 100));
</script>

<div class="usage-bar">
  <div class="usage-header">
    <span class="label">{label}</span>
    <span class="percent {colorClass}">{Math.round(percent)}%</span>
  </div>

  <div class="bar-track">
    <div
      class="bar-fill {colorClass}"
      style="width: {clampedPercent}%"
    ></div>
  </div>

  {#if resetTime}
    <div class="reset-time">↺  Resets in {resetTime}</div>
  {/if}
</div>

<style>
  .usage-bar {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .usage-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .label {
    color: #94A3B8;
    font-family: 'Inter', sans-serif;
    font-weight: 500;
    font-size: 12px;
  }

  .percent {
    font-family: 'JetBrains Mono', monospace;
    font-weight: 700;
    font-size: 13px;
  }

  .percent.normal { color: #22D3EE; }
  .percent.warning { color: #F59E0B; }
  .percent.critical { color: #EF4444; }

  .bar-track {
    width: 100%;
    height: 6px;
    background-color: #0F172A;
    border-radius: 3px;
    overflow: hidden;
  }

  .bar-fill {
    position: relative;
    height: 100%;
    border-radius: 3px;
    transition: width 0.5s cubic-bezier(0.22, 1, 0.36, 1);
    overflow: hidden;
  }

  /* Soft light sweep across the filled portion */
  .bar-fill::after {
    content: '';
    position: absolute;
    inset: 0;
    background: linear-gradient(
      90deg,
      transparent,
      rgba(255, 255, 255, 0.25),
      transparent
    );
    transform: translateX(-100%);
    animation: bar-shimmer 2.8s ease-in-out infinite;
  }

  @keyframes bar-shimmer {
    0% { transform: translateX(-100%); }
    60%, 100% { transform: translateX(100%); }
  }

  .bar-fill.normal {
    background: linear-gradient(90deg, #22D3EE, #0891B2);
  }

  .bar-fill.warning {
    background: linear-gradient(90deg, #F59E0B, #D97706);
  }

  .bar-fill.critical {
    background: linear-gradient(90deg, #EF4444, #DC2626);
  }

  .reset-time {
    color: #CBD5E1;
    font-family: 'JetBrains Mono', monospace;
    font-weight: 400;
    font-size: 10px;
  }

  @media (prefers-reduced-motion: reduce) {
    .bar-fill { transition: none; }
    .bar-fill::after { animation: none; display: none; }
  }
</style>
