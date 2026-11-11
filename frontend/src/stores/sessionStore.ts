/**
 * Session store — the long-lived desktop bundle session (Phase 2 model)
 * plus shell event pumping, expressed as a Zustand store so every Phase 3
 * workstation observes the same mount/lease state.
 */

import { create } from 'zustand';
import { getTransport } from '../transport';
import { EVT_COMMAND, EVT_GIT, EVT_LEASE, type AppEvent, type Envelope } from '../api';

interface SessionState {
  transportKind: 'tauri' | 'mock';
  mountedRoot: string | null;
  sessionId: string | null;
  keychain: string | null;
  platform: string | null;
  version: string | null;
  leaseExpiresAt: string | null;
  busy: boolean;
  lastError: string | null;
  /** monotonically bumped after any mutating command lands (authr://git). */
  revision: number;

  bootstrap(): Promise<void>;
  mount(root: string, leaseSecs?: number): Promise<boolean>;
  unmount(): Promise<void>;
  /** Subscribe the shell event channels; safe to call more than once. */
  wireEvents(): () => void;
  /** Raw invoke passthrough for panels that bypass domain stores. */
  invoke(cmd: string, payload?: unknown): Promise<Envelope>;
}

let wired = false;

export const useSession = create<SessionState>((set, get) => ({
  transportKind: getTransport().kind,
  mountedRoot: null,
  sessionId: null,
  keychain: null,
  platform: null,
  version: null,
  leaseExpiresAt: null,
  busy: false,
  lastError: null,
  revision: 0,

  async bootstrap() {
    const t = getTransport();
    const res = await t.invoke('desktop_info');
    if (res.ok) {
      set({ keychain: res.data.keychain, platform: res.data.platform, version: res.data.version });
    }
    const status = await t.invoke('desktop_status');
    if (status.ok && status.data.mounted) {
      set({
        mountedRoot: status.data.root,
        sessionId: status.data.session_id,
        leaseExpiresAt: status.data.lease?.expires_at ?? null,
      });
    }
  },

  async mount(root, leaseSecs = 300) {
    set({ busy: true, lastError: null });
    const res = await getTransport().invoke('desktop_mount', { root, lease_secs: leaseSecs });
    set({ busy: false });
    if (res.ok) {
      set({ mountedRoot: res.data.root, sessionId: res.data.session_id, lastError: null });
      return true;
    }
    set({ lastError: res.error.message });
    return false;
  },

  async unmount() {
    set({ busy: true });
    const res = await getTransport().invoke('desktop_unmount');
    set({ busy: false });
    if (res.ok) set({ mountedRoot: null, sessionId: null, leaseExpiresAt: null });
  },

  wireEvents() {
    if (wired) return () => undefined;
    wired = true;
    const off = getTransport().onEvent((evt: AppEvent) => {
      if (evt.event === EVT_LEASE) {
        const p = evt.payload ?? {};
        if (p.state === 'unmounted') set({ mountedRoot: null, sessionId: null, leaseExpiresAt: null });
        else if (p.session?.lease?.expires_at) set({ leaseExpiresAt: p.session.lease.expires_at });
      }
      if (evt.event === EVT_GIT) set((s) => ({ revision: s.revision + 1 }));
    });
    return off;
  },

  async invoke(cmd, payload = {}) {
    return getTransport().invoke(cmd, payload);
  },
}));

export { EVT_COMMAND, EVT_GIT, EVT_LEASE };
