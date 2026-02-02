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

<div class="usage-bar-container">
  <div class="usage-header">
    <span class="label">{label}</span>
    <span class="percent {colorClass}">{percent.toFixed(1)}%</span>
  </div>

  <div class="bar-background">
    <div
      class="bar-fill {colorClass}"
      style="width: {clampedPercent}%"
    ></div>
  </div>

  {#if resetTime}
    <div class="reset-time">
      Resets in: {resetTime}
    </div>
  {/if}
</div>

<style>
  .usage-bar-container {
    margin-bottom: 0.5rem;
  }

  .usage-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.25rem;
  }

  .label {
    color: #d1d5db;
    font-size: 0.875rem;
  }

  .percent {
    font-weight: 500;
    font-size: 0.875rem;
  }

  .percent.normal {
    color: #34d399;
  }

  .percent.warning {
    color: #fbbf24;
  }

  .percent.critical {
    color: #f87171;
  }

  .bar-background {
    width: 100%;
    height: 0.5rem;
    background-color: #374151;
    border-radius: 9999px;
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    border-radius: 9999px;
    transition: width 0.3s ease-out;
  }

  .bar-fill.normal {
    background: linear-gradient(90deg, #10b981, #34d399);
  }

  .bar-fill.warning {
    background: linear-gradient(90deg, #f59e0b, #fbbf24);
  }

  .bar-fill.critical {
    background: linear-gradient(90deg, #ef4444, #f87171);
  }

  .reset-time {
    color: #6b7280;
    font-size: 0.75rem;
    margin-top: 0.25rem;
  }
</style>
