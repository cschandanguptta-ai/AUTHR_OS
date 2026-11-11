// AUTHR_OS — canonical error codes → human copy + pure mapper.
// Kept import-free so the toast bus (uiStore) can use it without a cycle.

export interface ErrorCopy {
  title: string;
  body: string;
  action?: string;
}

export const ERROR_COPY: Record<string, ErrorCopy> = {
  ENTITY_NOT_FOUND: {
    title: 'Not found',
    body: 'That item no longer exists in this bundle — it may have been deleted in another view.',
    action: 'Refresh the binder and try again.',
  },
  FOREIGN_KEY_VIOLATION: {
    title: 'Blocked — still referenced',
    body: 'This entry is referenced by other records (for example, a speaker still used by dialogue segments).',
    action: 'Remove or reassign the references first.',
  },
  LEASE_LOCK_ACTIVE: {
    title: 'Bundle in use elsewhere',
    body: 'A live 120 s lease is held by another session on this or another host.',
    action: 'Wait for the lease to expire, or close the other session.',
  },
  READ_ONLY_PROJECT: {
    title: 'Read-only bundle',
    body: 'A fresh foreign lease is active — every write is frozen while another session works here.',
    action: 'Open the bundle again once the other session releases it.',
  },
  BUNDLE_ERROR: {
    title: 'Bundle problem',
    body: 'The bundle layout, manifest or a required tool for this step failed validation.',
    action: 'Check the detail line; missing tools (ffmpeg, typst) report install guidance.',
  },
  PATH_TRAVERSAL_VIOLATION: {
    title: 'Path outside the bundle',
    body: 'The path must stay inside the bundle sandbox — `..`, absolute paths and encodings are rejected.',
    action: 'Use a bundle-relative path.',
  },
  ZIP_SLIP_VIOLATION: {
    title: 'Unsafe archive entry',
    body: 'The archive contains an entry that would escape the destination directory.',
    action: 'Use an archive produced by this app’s export.',
  },
  SECURITY_ERROR: {
    title: 'Keychain unavailable',
    body: 'The OS keychain refused the operation, so the secret could not be stored or read.',
    action: 'Check the system keychain settings, or set the matching environment variable.',
  },
  AUDIO_ERROR: {
    title: 'Audio step rejected',
    body: 'The audio arguments were invalid (engine name, layer kind, sample rate, wav decode or output name).',
    action: 'Adjust the mastering settings and retry.',
  },
  AUDIO_ENGINE_UNAVAILABLE: {
    title: 'Voice engine unavailable',
    body: 'That voice engine needs network access or an API key that is not configured.',
    action: 'Choose the offline SCRIPTED engine, or configure a provider in Settings.',
  },
  AI_ERROR: {
    title: 'AI step rejected',
    body: 'The request was invalid (unknown source, mode, embedder, empty query…).',
    action: 'Adjust the request and retry.',
  },
  AI_ENGINE_UNAVAILABLE: {
    title: 'AI engine unavailable',
    body: 'The local model runtime or server for this feature is not installed.',
    action: 'Run the offline fallback instead, or configure a server in Settings.',
  },
  CRAFT_ERROR: {
    title: 'Craft settings rejected',
    body: 'A craft/KDP/retention argument was out of range (format id, keyword count, price bounds, QC scores…).',
    action: 'Check the highlighted field and retry.',
  },
  STORAGE_ERROR: {
    title: 'Storage error',
    body: 'The bundle database reported a failure (disk, corruption or internal error).',
    action: 'Check the detail line; a bundle ZIP export may recover the manuscript.',
  },
  IPC_TRANSPORT_ERROR: {
    title: 'Could not reach the core',
    body: 'The runtime bridge failed before a contract command could answer.',
    action: 'Restart the app; if it persists, check the core binary path.',
  },
};

const GENERIC: ErrorCopy = {
  title: 'That did not go through',
  body: 'The core rejected the request with an unrecognized condition.',
  action: 'Retry; if it repeats, note the detail line.',
};

export interface MappedError {
  code: string;
  copy: ErrorCopy;
  detail: string;
}

/** Extract {code, message} from an IpcError-shaped throw (bridge contract). */
export function mapError(err: unknown): MappedError {
  const e = err as { code?: string; message?: string } | null;
  const code = typeof e?.code === 'string' && e.code ? e.code : 'IPC_TRANSPORT_ERROR';
  const message = typeof e?.message === 'string' ? e.message : String(err);
  return { code, copy: ERROR_COPY[code] ?? GENERIC, detail: message };
}

/** One-liner for inline chips: human copy, raw message kept short. */
export function errorInline(err: unknown): string {
  const { copy, detail } = mapError(err);
  return `${copy.body} ${detail}`.replace(/\s+/g, ' ').trim();
}
