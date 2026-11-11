// AUTHR_OS — first-run product tour (F-14 completion). A non-blocking card
// pinned to the bottom-right: the author can keep working while it is up.
// Shows once after the first bootstrap (tourStore.maybeStart), replays from
// the maintenance menu, and never returns once dismissed — the flag wins.

import { HelpCircle } from 'lucide-react';
import { useTourStore, TOUR_STEPS } from '../../stores/tourStore';
import { cn } from '../../lib/utils';

export function FirstRunTour() {
  const step = useTourStore((s) => s.step);
  const next = useTourStore((s) => s.next);
  const end = useTourStore((s) => s.end);

  if (step == null) return null;
  const current = TOUR_STEPS[step];
  const last = step === TOUR_STEPS.length - 1;

  return (
    <div
      className="fixed bottom-4 right-4 z-[70] w-[340px] max-w-[calc(100vw-2rem)] rounded-xl border border-accent/40 bg-panel shadow-2xl anim-modal"
      role="dialog"
      aria-label="Product tour"
      data-testid="first-run-tour"
    >
      <div className="flex items-center gap-2 px-4 pt-3 pb-2">
        <HelpCircle size={13} className="text-accent shrink-0" />
        <span className="text-2xs font-semibold tracking-wider text-mut uppercase">
          Welcome tour · step {step + 1} of {TOUR_STEPS.length}
        </span>
        <div className="ml-auto flex items-center gap-1" aria-hidden="true">
          {TOUR_STEPS.map((s, i) => (
            <span
              key={s.id}
              className={cn(
                'w-1.5 h-1.5 rounded-full',
                i <= step ? 'bg-accent' : 'bg-line-2',
              )}
            />
          ))}
        </div>
      </div>
      <div className="px-4 pb-3 grid gap-1.5">
        <h3 className="text-[13px] font-semibold text-ink">{current.title}</h3>
        <p className="text-2xs text-mut leading-relaxed">{current.body}</p>
      </div>
      <div className="flex items-center gap-2 px-4 py-2.5 border-t border-line bg-panel2/50 rounded-b-xl">
        <button
          type="button"
          onClick={end}
          className="text-2xs text-mut hover:text-ink"
          data-testid="tour-skip"
        >
          Skip — don't show again
        </button>
        <button
          type="button"
          onClick={next}
          className="ml-auto text-2xs font-medium px-3 py-1.5 rounded-md bg-accent text-on-accent"
          data-testid="tour-next"
        >
          {last ? 'Done' : 'Next'}
        </button>
      </div>
    </div>
  );
}
