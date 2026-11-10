// AUTHR_OS — runtime environment flags.

/** True inside the Tauri desktop shell (the same seam the IPC bridge uses). */
export const IS_DESKTOP =
  typeof window !== 'undefined' &&
  (window as unknown as { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ != null;
