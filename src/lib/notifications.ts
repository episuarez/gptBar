// Desktop notification logic driven from the frontend, where usage snapshots,
// config thresholds and provider state already flow. Uses tauri-plugin-notification.

import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification';
import type { UsageSnapshot, AppConfig } from './types';

type Level = 'normal' | 'warning' | 'critical';

// Per-provider record of the last notification sent (for cooldown + escalation).
const lastNotified: Record<string, { level: Level; at: number }> = {};
let permissionGranted = false;

/** Requests notification permission once. Safe to call repeatedly. */
export async function ensureNotificationPermission(): Promise<void> {
  try {
    permissionGranted = await isPermissionGranted();
    if (!permissionGranted) {
      permissionGranted = (await requestPermission()) === 'granted';
    }
  } catch (e) {
    console.error('Notification permission failed:', e);
  }
}

function levelRank(l: Level): number {
  return l === 'critical' ? 2 : l === 'warning' ? 1 : 0;
}

function maxPercent(s: UsageSnapshot): number {
  let m = 0;
  for (const w of [s.primary, s.secondary, s.tertiary]) {
    if (w?.used_percent != null && w.used_percent > m) m = w.used_percent;
  }
  return m;
}

/**
 * Sends a desktop notification when a provider crosses the warning/critical
 * threshold. Respects muted providers and the per-provider cooldown, and always
 * notifies on escalation (warning -> critical).
 */
export function maybeNotifyUsage(
  providerId: string,
  providerName: string,
  snapshot: UsageSnapshot | null,
  config: AppConfig | null,
): void {
  if (!permissionGranted || !snapshot || !config) return;
  if (config.muted_providers?.includes(providerId)) return;

  const pct = maxPercent(snapshot);
  let level: Level = 'normal';
  if (pct >= config.critical_threshold) level = 'critical';
  else if (pct >= config.warning_threshold) level = 'warning';

  if (level === 'normal') {
    // Back below threshold — reset so a future crossing notifies again.
    delete lastNotified[providerId];
    return;
  }

  const now = Date.now();
  const prev = lastNotified[providerId];
  const cooldownMs = (config.notification_cooldown_minutes ?? 30) * 60 * 1000;
  const escalated = !prev || levelRank(level) > levelRank(prev.level);
  const cooledDown = !prev || now - prev.at >= cooldownMs;
  if (!escalated && !cooledDown) return;

  lastNotified[providerId] = { level, at: now };

  try {
    sendNotification({
      title: `${providerName} usage ${level}`,
      body: `Usage at ${Math.round(pct)}%.`,
    });
  } catch (e) {
    console.error('sendNotification failed:', e);
  }
}
