/**
 * Headless Lexical editor tests (T6.UE) — the custom node round-trip and
 * the §6.4 split / merge UUID rules, executed in a DOM-less editor via
 * @lexical/headless.
 */

/// <reference types="vitest" />
import { describe, expect, it } from 'vitest';
import { createHeadlessEditor } from '@lexical/headless';
import {
  $createRangeSelection,
  $getRoot,
  $setSelection,
} from 'lexical';
import { AUTHR_NODES, SegmentNode, $isSegmentNode } from '../editor/nodes';
import { $exportAst, $importSegments } from '../editor/segmentsAdapter';
import { $splitSegmentAtSelection, $mergeSegmentBackward } from '../editor/segmentOps';
import { diffAst, snapshotFromRow } from '../editor/astBridge';
import type { SegmentRow } from '../types';

function makeEditor() {
  return createHeadlessEditor({
    namespace: 'test-canvas',
    nodes: AUTHR_NODES as any,
    onError: (e: Error) => {
      throw e;
    },
  });
}

const segRow = (id: number, segmentId: string, text: string, order: number, speaker = 'NARRATOR'): SegmentRow => ({
  id,
  segment_id: segmentId,
  chapter_id: 1,
  speaker_id: speaker,
  segment_type: speaker === 'NARRATOR' ? 'NARRATION' : 'DIALOGUE',
  text_content: text,
  emotion: 'neutral',
  prosody_rate: 1,
  prosody_pitch: 1,
  ambient_cue: 'rain_heavy',
  sfx_cue: 'door_creak',
  audio_hash: null,
  sequence_order: order,
});

describe('segments ⇄ Lexical AST round trip', () => {
  it('imports rows, exports the same UUIDs, order and metadata', async () => {
    const editor = makeEditor();
    const rows = [segRow(1, 'uuid-a', 'First paragraph.', 1), segRow(2, 'uuid-b', 'Mira speaks.', 2, 'MIRA')];
    await editor.update(() => {
      $importSegments(rows, new Map([['MIRA', 'Mira Vance']]));
    });
    await editor.read(() => {
      const ast = $exportAst();
      expect(ast.map((s) => s.segmentId)).toEqual(['uuid-a', 'uuid-b']);
      expect(ast[1].speakerId).toBe('MIRA');
      expect(ast[1].segmentType).toBe('DIALOGUE');
      expect(ast[0].text).toBe('First paragraph.');
      expect(ast[0].rowId).toBe(1);
      // Text export excludes decorator content (badges/chips/foley).
      expect(ast[0].text).not.toContain('rain_heavy');
      expect(ast[1].text).not.toContain('Mira Vance');
    });
  });

  it('diff of a fresh import against its own rows is empty', async () => {
    const editor = makeEditor();
    const rows = [segRow(1, 'uuid-a', 'Stable prose.', 1)];
    await editor.update(() => {
      $importSegments(rows, new Map());
    });
    await editor.read(() => {
      const ast = $exportAst();
      const d = diffAst(new Map(rows.map((r) => [r.segment_id, snapshotFromRow(r)])), ast);
      expect(d.empty).toBe(true);
    });
  });

  it('exportJSON / importJSON preserve segment metadata', async () => {
    const editor = makeEditor();
    const rows = [segRow(1, 'uuid-a', 'Meta preserved.', 1, 'FERRYMAN')];
    await editor.update(() => {
      $importSegments(rows, new Map());
      const json = $getRoot().getFirstChild()!.exportJSON();
      // Re-import through the node class itself (parsing path).
      const parsed = SegmentNode.importJSON(json);
      expect(parsed.getSegmentId()).toBe('uuid-a');
      expect(parsed.getSpeakerId()).toBe('FERRYMAN');
      expect(parsed.getCues()).toEqual({ ambient: 'rain_heavy', sfx: 'door_creak' });
      expect(parsed.getRowId()).toBe(1);
    });
  });
});

describe('§6.4 split rule (Enter)', () => {
  it('leading half keeps its UUID; trailing gets a fresh one; text splits at caret', async () => {
    const editor = makeEditor();
    await editor.update(() => {
      $importSegments([segRow(1, 'keep-me', 'The moon rose. The tide followed.', 1)], new Map());
      // Caret inside the prose text after "The moon rose." (offset 14).
      const seg = $getRoot().getFirstChild() as SegmentNode;
      const textChild = seg.getChildren().find((c) => c.getType() === 'text')!;
      const sel = $createRangeSelection();
      sel.anchor.set(textChild.getKey(), 14, 'text');
      sel.focus.set(textChild.getKey(), 14, 'text');
      $setSelection(sel);
      expect($splitSegmentAtSelection()).toBe(true);
    });
    await editor.read(() => {
      const ast = $exportAst();
      expect(ast).toHaveLength(2);
      expect(ast[0].segmentId).toBe('keep-me'); // leading UUID survives
      expect(ast[0].text).toBe('The moon rose.');
      expect(ast[1].segmentId).not.toBe('keep-me');
      expect(ast[1].segmentId).toMatch(/[0-9a-f-]{36}/); // fresh UUID v4
      expect(ast[1].text).toBe(' The tide followed.');
      // The new segment inherits speaker / type / emotion.
      expect(ast[1].speakerId).toBe('NARRATOR');
      expect(ast[1].segmentType).toBe('NARRATION');
    });
  });

  it('diff after a split reports exactly one created segment', async () => {
    const editor = makeEditor();
    const rows = [segRow(1, 'uuid-a', 'Alpha. Bravo.', 1)];
    await editor.update(() => {
      $importSegments(rows, new Map());
      const seg = $getRoot().getFirstChild() as SegmentNode;
      const textChild = seg.getChildren().find((c) => c.getType() === 'text')!;
      const sel = $createRangeSelection();
      sel.anchor.set(textChild.getKey(), 6, 'text');
      sel.focus.set(textChild.getKey(), 6, 'text');
      $setSelection(sel);
      $splitSegmentAtSelection();
    });
    await editor.read(() => {
      const d = diffAst(new Map(rows.map((r) => [r.segment_id, snapshotFromRow(r)])), $exportAst());
      // Exactly one created (the fresh UUID) and exactly one updated (the
      // leading segment whose text shrank to the pre-caret half).
      expect(d.created).toHaveLength(1);
      expect(d.updated.map((s) => s.segmentId)).toEqual(['uuid-a']);
      expect(d.deleted).toHaveLength(0);
      expect(d.sequenceOrder[0]).toBe('uuid-a');
    });
  });
});

describe('§6.4 merge rule (Backspace at start)', () => {
  it('trailing segment disappears; leading UUID and text survive', async () => {
    const editor = makeEditor();
    await editor.update(() => {
      $importSegments(
        [segRow(1, 'lead', 'Leading prose.', 1), segRow(2, 'tail', 'Trailing prose.', 2)],
        new Map(),
      );
      // Caret at offset 0 of the trailing segment's text child.
      const root = $getRoot();
      const segs = root.getChildren().filter($isSegmentNode) as SegmentNode[];
      const tail = segs[1];
      const textChild = tail.getChildren().find((c) => c.getType() === 'text')!;
      const sel = $createRangeSelection();
      sel.anchor.set(textChild.getKey(), 0, 'text');
      sel.focus.set(textChild.getKey(), 0, 'text');
      $setSelection(sel);
      expect($mergeSegmentBackward()).toBe(true);
    });
    await editor.read(() => {
      const ast = $exportAst();
      expect(ast).toHaveLength(1);
      expect(ast[0].segmentId).toBe('lead');
      // Only prose moves; decorators of the tail are discarded.
      expect(ast[0].text).toBe('Leading prose.Trailing prose.');
    });
  });

  it('returns false when caret is not at offset 0 (falls through)', async () => {
    const editor = makeEditor();
    await editor.update(() => {
      $importSegments([segRow(1, 'a', 'one two', 1)], new Map());
      const seg = $getRoot().getFirstChild() as SegmentNode;
      const textChild = seg.getChildren().find((c) => c.getType() === 'text')!;
      const sel = $createRangeSelection();
      sel.anchor.set(textChild.getKey(), 3, 'text');
      sel.focus.set(textChild.getKey(), 3, 'text');
      $setSelection(sel);
      expect($mergeSegmentBackward()).toBe(false);
    });
  });
});
