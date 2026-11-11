// AUTHR_OS — small-width guard (RA-04): AUTHR_OS is a desktop-first
// workstation (Tauri 2; vendor targets laptop/desktop). Below 480 px the
// layout is condensed by CSS (single-column stacks, scrollable header,
// wrapping artifact rows) and this notice states the product posture
// honestly instead of pretending to be a mobile app. Desktop viewports are
// unchanged — the media query only reaches ≤479 px.

import { useEffect, useState } from 'react';
import { MonitorX } from 'lucide-react';

const QUERY = '(max-width: 479px)';

export function SmallWidthNotice() {
  const [narrow, setNarrow] = useState(
    () => typeof window !== 'undefined' && window.matchMedia(QUERY).matches,
  );

  useEffect(() => {
    const mq = window.matchMedia(QUERY);
    const onChange = (e: MediaQueryListEvent) => setNarrow(e.matches);
    mq.addEventListener('change', onChange);
    return () => mq.removeEventListener('change', onChange);
  }, []);

  if (!narrow) return null;
  return (
    <div
      role="status"
      data-testid="small-width-notice"
      className="flex items-center justify-center gap-2 px-4 py-2 bg-warn/10 border-b border-warn/30 text-2xs text-warn text-center"
    >
      <MonitorX size={12} className="shrink-0" />
      <span>
        AUTHR_OS is a desktop workstation — the layout is condensed below 480 px.
        Best on a laptop or desktop display.
      </span>
    </div>
  );
}
