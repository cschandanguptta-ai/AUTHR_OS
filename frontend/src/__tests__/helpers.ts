/** Shared test helpers for the unit suite. */

import type { AstSnapshot } from '../editor/astBridge';

export { snapshotFromRow } from '../editor/astBridge';

/** Build an editor-side AST snapshot (not yet persisted). */
export function snapshotOf(
  segmentId: string,
  text: string,
  speakerId = 'NARRATOR',
  extra: Partial<AstSnapshot> = {},
): AstSnapshot {
  return {
    key: 'k-' + segmentId,
    segmentId,
    speakerId,
    segmentType: speakerId === 'NARRATOR' ? 'NARRATION' : 'DIALOGUE',
    emotion: 'neutral',
    prosodyRate: 1,
    prosodyPitch: 1,
    ambientCue: null,
    sfxCue: null,
    text,
    rowId: null,
    ...extra,
  };
}
