// AUTHR_OS — toast-level error helpers (R-08). The pure mapping lives in
// mapError.ts; this module adds the toast push so components never hand-write
// failure copy. Command names never reach the user through this path.

import { toast } from '../stores/uiStore';

export {
  ERROR_COPY,
  mapError,
  errorInline,
  type ErrorCopy,
  type MappedError,
} from './mapError';
import { mapError } from './mapError';

/** Push a mapped toast — the ONLY way component code should report failures. */
export function toastIpcError(err: unknown, hint?: string): void {
  const { copy, detail } = mapError(err);
  const body = hint ? `${copy.body} ${hint}` : copy.body;
  toast('error', copy.title, copy.action ? `${body}\n${copy.action}` : body || detail);
}
