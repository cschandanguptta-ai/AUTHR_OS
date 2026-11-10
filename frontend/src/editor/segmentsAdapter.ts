/**
 * Segments ⇄ Lexical editor-state adapter (Sprint 7).
 *
 * `$importSegments` rebuilds the canvas from `chapter_segments` rows
 * (chapter switch / external reload); `$exportAst` walks the editor AST
 * into `AstSnapshot[]` for the bridge. Both run inside `editor.update()` /
 * `editor.read()` respectively and are exercised headlessly in the unit
 * suite via `@lexical/headless`.
 */

import { $getRoot, $createTextNode } from 'lexical';
import {
  SegmentNode,
  SpeakerBadgeNode,
  EmotionChipNode,
  FoleyTriggerNode,
  $isSegmentNode,
} from './nodes';
import type { AstSnapshot } from './astBridge';
import type { SegmentRow } from '../types';

export type SpeakerDirectory = Map<string, string>;

/** Fresh UUID v4 (crypto when available — Tauri webview & Node ≥ 19). */
export function newSegmentUuid(): string {
  if (typeof crypto !== 'undefined' && 'randomUUID' in crypto) {
    return crypto.randomUUID();
  }
  // RFC 4122 v4 fallback (non-crypto hosts).
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    const v = c === 'x' ? r : (r & 0x3) | 0x8;
    return v.toString(16);
  });
}

/**
 * Rebuild the whole canvas from persisted rows. A split performed by the
 * author (Enter key) keeps the leading UUID by construction: Lexical
 * splits the paragraph and the leading half retains the original
 * SegmentNode instance (and its `__segmentId`).
 */
export function $importSegments(rows: SegmentRow[], speakers: SpeakerDirectory): void {
  const root = $getRoot();
  root.clear();
  const ordered = [...rows].sort((a, b) => a.sequence_order - b.sequence_order);
  for (const row of ordered) {
    const seg = new SegmentNode(
      row.segment_id,
      row.speaker_id,
      row.segment_type,
      row.emotion,
      row.prosody_rate,
      row.prosody_pitch,
      row.ambient_cue,
      row.sfx_cue,
      row.id,
    );
    seg.append($createSpeakerBadge(row.speaker_id, speakers));
    seg.append(new EmotionChipNode(row.emotion));
    if (row.sfx_cue) seg.append(new FoleyTriggerNode(row.sfx_cue, 'sfx'));
    if (row.ambient_cue) seg.append(new FoleyTriggerNode(row.ambient_cue, 'ambient'));
    if (row.text_content.length > 0) seg.append($createTextNode(row.text_content));
    root.append(seg);
  }
}

function $createSpeakerBadge(speakerId: string, speakers: SpeakerDirectory): SpeakerBadgeNode {
  const display = speakers.get(speakerId) ?? (speakerId === 'NARRATOR' ? 'Narrator' : speakerId);
  return new SpeakerBadgeNode(speakerId, display);
}

/** Export every top-level SegmentNode into the bridge snapshot format. */
export function $exportAst(): AstSnapshot[] {
  const root = $getRoot();
  const out: AstSnapshot[] = [];
  for (const child of root.getChildren()) {
    if (!$isSegmentNode(child)) continue;
    // Decorator nodes return '' from getTextContent, so this is pure prose.
    out.push({
      key: child.getKey(),
      segmentId: child.getSegmentId(),
      speakerId: child.getSpeakerId(),
      segmentType: child.getSegmentType(),
      emotion: child.getEmotion(),
      prosodyRate: child.getProsody().rate,
      prosodyPitch: child.getProsody().pitch,
      ambientCue: child.getCues().ambient,
      sfxCue: child.getCues().sfx,
      text: child.getTextContent(),
      rowId: child.getRowId(),
    });
  }
  return out;
}

/** Append one fresh empty segment after the currently selected one. */
export function $appendEmptySegment(afterSegmentId?: string): SegmentNode {
  const root = $getRoot();
  const seg = new SegmentNode(newSegmentUuid());
  seg.append($createTextNode(''));
  if (afterSegmentId) {
    for (const child of root.getChildren()) {
      if ($isSegmentNode(child) && child.getSegmentId() === afterSegmentId) {
        child.insertAfter(seg);
        return seg;
      }
    }
  }
  root.append(seg);
  return seg;
}
