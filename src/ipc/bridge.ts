// AUTHR_OS — IPC bridge (docs/CLI_CONTRACT.md §1–§3).
//
// Two transports:
//  1. Tauri desktop: dynamic-imports @tauri-apps/api/core and invokes the
//     #[tauri::command] wrapper (never imported statically — keeps the bundle
//     lean in pure-browser dev mode).
//  2. Browser dev bridge: POST /api/ipc {command, args}. The vite middleware
//     strips the contract envelope; success → raw `data` field, failure →
//     HTTP 400 carrying {code, message, category}, surfaced as IpcError.

export class IpcError extends Error {
  readonly code: string;
  readonly category: string;

  constructor(code: string, message: string, category: string) {
    super(message);
    this.name = 'IpcError';
    this.code = code;
    this.category = category;
  }
}

interface EnvelopeErrorShape {
  code?: string;
  message?: string;
  category?: string;
}

declare global {
  interface Window {
    __TAURI_INTERNALS__?: unknown;
  }
}

function toIpcError(payload: unknown, status: number): IpcError {
  const err = (payload ?? {}) as EnvelopeErrorShape;
  // The dev middleware answers 400 with the flat {code,message,category} error
  // object; tolerate a nested envelope shape as well.
  const nested = (err as { error?: EnvelopeErrorShape }).error;
  const src = (nested && typeof nested === 'object' ? nested : err) as EnvelopeErrorShape;
  return new IpcError(
    src.code ?? 'IPC_TRANSPORT_ERROR',
    src.message ?? `IPC request failed with HTTP ${status}.`,
    src.category ?? 'storage',
  );
}

export async function invoke<T>(
  command: string,
  args?: object,
): Promise<T> {
  if (window.__TAURI_INTERNALS__) {
    const core = await import('@tauri-apps/api/core');
    return core.invoke<T>(command, (args ?? {}) as Record<string, unknown> | undefined);
  }

  const res = await fetch('/api/ipc', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ command, args: args ?? {} }),
  });

  if (!res.ok) {
    let payload: unknown = null;
    try {
      payload = await res.json();
    } catch {
      payload = null;
    }
    throw toIpcError(payload, res.status);
  }

  const json = (await res.json()) as { ok?: boolean; data?: T };
  if (json && typeof json === 'object' && 'ok' in json) {
    if (json.ok) return json.data as T;
    throw toIpcError(json, res.status);
  }
  return json as T;
}
