/**
 * Transport layer: routes envelope calls to the Tauri IPC bridge when
 * running inside the desktop shell, or to an in-memory mock bundle when
 * previewed in a plain browser (`npm run dev` / CI screenshots).
 *
 * Both transports emit the same shell events (`authr://command`,
 * `authr://git`, `authr://lease`) so the Inspector behaves identically.
 */

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { AppEvent, Envelope } from './api';
import { EVT_COMMAND, EVT_GIT, EVT_LEASE } from './api';
import { MockTransport } from './mock';

export interface Transport {
  readonly kind: 'tauri' | 'mock';
  invoke(cmd: string, payload?: unknown): Promise<Envelope>;
  onEvent(cb: (evt: AppEvent) => void): () => void;
}

/** Running inside the Tauri v2 webview? */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

const EVENT_CHANNELS = [EVT_COMMAND, EVT_GIT, EVT_LEASE];

export function createTransport(): Transport {
  if (isTauri()) {
    return new TauriTransport();
  }
  return new MockTransport();
}

/**
 * Process-wide transport singleton. The Phase 3 Cockpit, the Phase 2
 * console and every Zustand store must observe the SAME session (one
 * mounted bundle, one mock fixture), so the instance is memoized.
 */
let singleton: Transport | null = null;

export function getTransport(): Transport {
  if (!singleton) singleton = createTransport();
  return singleton;
}

class TauriTransport implements Transport {
  readonly kind = 'tauri' as const;
  private listeners = new Set<(evt: AppEvent) => void>();
  private unlisteners: UnlistenFn[] = [];

  constructor() {
    for (const channel of EVENT_CHANNELS) {
      listen(channel, (evt) => {
        const appEvent: AppEvent = { event: channel, payload: evt.payload };
        for (const cb of this.listeners) cb(appEvent);
      }).then((un) => this.unlisteners.push(un));
    }
  }

  async invoke(cmd: string, payload: unknown = {}): Promise<Envelope> {
    // Every AUTHR_OS command takes a single `payload` argument and returns
    // the frozen envelope. Session commands pass flat args.
    if (cmd === 'desktop_mount') {
      const p = payload as { root: string; lease_secs?: number };
      return (await tauriInvoke(cmd, { root: p.root, leaseSecs: p.lease_secs })) as Envelope;
    }
    return (await tauriInvoke(cmd, { payload })) as Envelope;
  }

  onEvent(cb: (evt: AppEvent) => void): () => void {
    this.listeners.add(cb);
    return () => this.listeners.delete(cb);
  }
}
