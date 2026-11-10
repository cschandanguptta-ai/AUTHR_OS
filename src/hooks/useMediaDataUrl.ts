// useMediaDataUrl — one shared loader for bundle media (D7 §8, R-07).
// Replaces the three duplicated thumbnail effects in Scene Studio; failures
// carry the mapped reason and a Retry instead of an empty catch.

import { useCallback, useEffect, useState } from 'react';
import { mediaDataUrl } from '../ipc/api';
import { mapError } from '../ipc/errorCopy';

export interface MediaUrlState {
  url: string | null;
  loading: boolean;
  error: string | null;
  retry: () => void;
}

export function useMediaDataUrl(
  path: string | null | undefined,
  bust: string | number = '',
  enabled = true,
): MediaUrlState {
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let alive = true;
    setUrl(null);
    setError(null);
    if (!path || !enabled) {
      setLoading(false);
      return () => {
        alive = false;
      };
    }
    setLoading(true);
    mediaDataUrl(path, bust)
      .then((u) => {
        if (!alive) return;
        setUrl(u);
        setLoading(false);
      })
      .catch((err) => {
        if (!alive) return;
        const mapped = mapError(err);
        setError(`${mapped.copy.body} ${mapped.detail}`.replace(/\s+/g, ' ').trim());
        setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, [path, bust, attempt, enabled]);

  const retry = useCallback(() => setAttempt((a) => a + 1), []);
  return { url, loading, error, retry };
}
