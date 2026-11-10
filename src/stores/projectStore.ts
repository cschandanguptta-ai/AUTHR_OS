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

export type ProjectStatus = 'closed' | 'opening' | 'open';

const LAST_BUNDLE_KEY = 'authr_os.last_bundle';

export function lastBundlePath(): string {
  try {
    return window.localStorage.getItem(LAST_BUNDLE_KEY) ?? '';
  } catch {
    return '';
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

  openProject: async (path) => {
    set({ status: 'opening' });
    try {
      const res = await api.projectOpen({ path });
      try {
        window.localStorage.setItem(LAST_BUNDLE_KEY, path);
      } catch {
        /* private mode — ignore */
      }
      set({
        status: 'open',
        bundlePath: path,
        manifest: res.manifest,
        readonly: res.readonly,
        cloudSyncGuard: res.cloud_sync_guard,
        defaultBookId: null,
        sessionNonce: get().sessionNonce + 1,
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
      set({
        status: 'open',
        bundlePath: res.path,
        manifest: res.manifest,
        readonly: res.readonly ?? false,
        cloudSyncGuard: false,
        defaultBookId: res.book_id,
        sessionNonce: get().sessionNonce + 1,
      });
      toast('success', 'Project initialized', res.path);
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
