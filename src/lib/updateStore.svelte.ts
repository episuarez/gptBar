// Shared updater state so both the banner (auto) and the About modal button
// (manual) drive the same flow.

import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

type Phase =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'ready'
  | 'uptodate'
  | 'error';

export const updateState = $state<{
  phase: Phase;
  newVersion: string;
  progress: number;
  dismissed: boolean;
}>({
  phase: 'idle',
  newVersion: '',
  progress: 0,
  dismissed: false,
});

// Pending Update handle (not reactive; not rendered directly).
let pending: Update | null = null;

/**
 * Checks for an update. When `manual` is true, surfaces "checking"/"uptodate"
 * states (for the Settings button); otherwise stays quiet when up to date.
 */
export async function checkForUpdate(manual = false): Promise<void> {
  if (updateState.phase === 'downloading' || updateState.phase === 'ready') return;
  if (manual) updateState.phase = 'checking';
  try {
    const update = await check();
    if (update) {
      pending = update;
      if (update.version !== updateState.newVersion) updateState.dismissed = false;
      updateState.newVersion = update.version;
      updateState.phase = 'available';
    } else {
      updateState.phase = manual ? 'uptodate' : 'idle';
    }
  } catch (e) {
    console.error('Update check failed:', e);
    if (manual) updateState.phase = 'error';
  }
}

export async function installUpdate(): Promise<void> {
  if (!pending) return;
  updateState.phase = 'downloading';
  updateState.progress = 0;
  let downloaded = 0;
  let contentLength = 0;
  try {
    await pending.downloadAndInstall((event) => {
      switch (event.event) {
        case 'Started':
          contentLength = event.data.contentLength ?? 0;
          break;
        case 'Progress':
          downloaded += event.data.chunkLength;
          updateState.progress =
            contentLength > 0
              ? Math.min(100, Math.round((downloaded / contentLength) * 100))
              : 0;
          break;
        case 'Finished':
          updateState.progress = 100;
          break;
      }
    });
    updateState.phase = 'ready';
    await relaunch();
  } catch (e) {
    console.error('Update install failed:', e);
    updateState.phase = 'error';
  }
}

export function dismissUpdate(): void {
  updateState.dismissed = true;
}
