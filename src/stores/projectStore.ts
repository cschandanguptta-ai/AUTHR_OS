// AUTHR_OS — bundle lifecycle: open / init / close, lease + lock awareness.

import { create } from 'zustand';
import * as api from '../ipc/api';
import type { BundleManifest, LockStatus } from '../types/ast';
import { toast, toastError } from './uiStore';
import { useManuscriptStore } from './manuscriptStore';
import { useVoiceCastStore } from './voiceCastStore';
import { useLoreStore } from './loreStore';
import { useThreadStore } from './threadStore';
import { useWorkspaceStore } from './workspaceStore';
import { useSettingsStore } from './settingsStore';
import { useBookContextStore } from './bookContextStore';
import { useTourStore } from './tourStore';

export type ProjectStatus = 'closed' | 'opening' | 'open';

const LAST_BUNDLE_KEY = 'authr_os.last_bundle';
// RA-03: recent bundles — (path, title, last-opened), max 5, localStorage
// (same persistence tier as last_bundle; survives relaunch in the Tauri
// WebView profile and in browser dev). Lease semantics unchanged: recents
// open through the very same openProject path.
const RECENTS_KEY = 'authr_os.recent_bundles';
const RECENTS_MAX = 5;

export function lastBundlePath(): string {
  try {
    return window.localStorage.getItem(LAST_BUNDLE_KEY) ?? '';
  } catch {
    return '';
  }
}

export interface RecentBundle {
  path: string;
  title: string;
  lastOpened: number;
}

export function listRecents(): RecentBundle[] {
  try {
    const raw = window.localStorage.getItem(RECENTS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as RecentBundle[];
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((r) => r && typeof r.path === 'string')
      .sort((a, b) => (b.lastOpened ?? 0) - (a.lastOpened ?? 0))
      .slice(0, RECENTS_MAX);
  } catch {
    return [];
  }
}

function rememberRecent(path: string, title: string): void {
  try {
    const next = [
      { path, title, lastOpened: Date.now() },
      ...listRecents().filter((r) => r.path !== path),
    ].slice(0, RECENTS_MAX);
    window.localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
  } catch {
    /* private mode — ignore */
  }
}

export function removeRecent(path: string): void {
  try {
    const next = listRecents().filter((r) => r.path !== path);
    window.localStorage.setItem(RECENTS_KEY, JSON.stringify(next));
  } catch {
    /* private mode — ignore */
  }
}

interface ProjectState {
  status: ProjectStatus;
  bundlePath: string | null;
  manifest: BundleManifest | null;
  readonly: boolean;
  cloudSyncGuard: boolean;
  lockInfo: LockStatus | null;
  defaultBookId: number | null;
  sessionNonce: number;
  /** Wall-clock time the current bundle was opened (RA-05 lease chip). */
  openedAt: number;
  openProject: (path: string) => Promise<boolean>;
  initProject: (path: string, title: string, author: string) => Promise<boolean>;
  closeProject: () => Promise<void>;
  refreshLockStatus: () => Promise<void>;
}

export const useProjectStore = create<ProjectState>((set, get) => ({
  status: 'closed',
  bundlePath: null,
  manifest: null,
  readonly: false,
  cloudSyncGuard: false,
  lockInfo: null,
  defaultBookId: null,
  sessionNonce: 0,
  openedAt: 0,

  openProject: async (path) => {
    set({ status: 'opening' });
    try {
      const res = await api.projectOpen({ path });
      try {
        window.localStorage.setItem(LAST_BUNDLE_KEY, path);
      } catch {
        /* private mode — ignore */
      }
      rememberRecent(path, res.manifest.title);
      set({
        status: 'open',
        bundlePath: path,
        manifest: res.manifest,
        readonly: res.readonly,
        cloudSyncGuard: res.cloud_sync_guard,
        defaultBookId: null,
        sessionNonce: get().sessionNonce + 1,
        openedAt: Date.now(),
      });
      toast('success', 'Bundle open', path);
      void get().refreshLockStatus();
      return true;
    } catch (err) {
      set({ status: 'closed' });
      toastError('Could not open bundle', err);
      return false;
    }
  },

  initProject: async (path, title, author) => {
    set({ status: 'opening' });
    try {
      const res = await api.projectInit({ path, title, author: author || undefined });
      try {
        window.localStorage.setItem(LAST_BUNDLE_KEY, res.path);
      } catch {
        /* private mode — ignore */
      }
      rememberRecent(res.path, res.manifest.title);
      set({
        status: 'open',
        bundlePath: res.path,
        manifest: res.manifest,
        readonly: res.readonly ?? false,
        cloudSyncGuard: false,
        defaultBookId: res.book_id,
        sessionNonce: get().sessionNonce + 1,
        openedAt: Date.now(),
      });
      toast('success', 'Project initialized', res.path);
      // R-12/F-14: a fresh bootstrap is never silent — the dashboard opens
      // stating exactly what was created and what to do next.
      useWorkspaceStore.getState().openDashboard({ fresh: true });
      // F-14 completion: the three-step tour rides only on a FIRST bootstrap —
      // the done-flag wins, returning users never see it.
      useTourStore.getState().maybeStart();
      void get().refreshLockStatus();
      return true;
    } catch (err) {
      set({ status: 'closed' });
      toastError('Could not initialize project', err);
      return false;
    }
  },

  closeProject: async () => {
    try {
      await api.projectClose();
      toast('info', 'Bundle closed', 'Lease released, WAL checkpointed.');
    } catch (err) {
      toastError('Close failed (releasing locally anyway)', err);
    }
    useManuscriptStore.getState().reset();
    useVoiceCastStore.getState().reset();
    useLoreStore.getState().reset();
    useThreadStore.getState().reset();
    useBookContextStore.getState().reset();
    useWorkspaceStore.getState().setActiveStudio('novel');
    useSettingsStore.getState().setTypewriterMode(false);
    set({
      status: 'closed',
      bundlePath: null,
      manifest: null,
      readonly: false,
      cloudSyncGuard: false,
      lockInfo: null,
      defaultBookId: null,
      openedAt: 0,
    });
  },

  refreshLockStatus: async () => {
    if (get().status !== 'open') return;
    try {
      const info = await api.projectLockStatus();
      set({ lockInfo: info, readonly: info.readonly });
    } catch {
      /* lock probe failures are non-fatal */
    }
  },
}));
