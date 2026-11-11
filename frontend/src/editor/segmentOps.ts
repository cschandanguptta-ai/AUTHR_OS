/**
 * Segment editing operations (Sprint 7) — the Enter-split and
 * Backspace-merge semantics of the AST canvas, expressed as pure
 * editor-context functions so the headless unit suite can pin the §6.4
 * UUID rules exactly as the React plugins apply them.
 *
 *   split  : leading half KEEPS its UUID/rowId; the new paragraph gets a
 *            fresh UUID v4 and inherits speaker/type/emotion.
 *   merge  : the LEADING segment survives (keeps its UUID); only prose
 *            text moves; trailing decorators are discarded.
 */

import { $createTextNode, $getSelection, $isRangeSelection } from 'lexical';
import { SegmentNode, $isSegmentNode } from './nodes';
import { newSegmentUuid } from './segmentsAdapter';

/** Closest enclosing SegmentNode (walks up from any child node). */
export function $enclosingSegment(node: any): SegmentNode | null {
  let cur = node;
  while (cur) {
    if ($isSegmentNode(cur)) return cur as SegmentNode;
    cur = typeof cur.getParent === 'function' ? cur.getParent() : null;
  }
  return null;
}

/** Selection offset relative to the segment's pure prose text. */
export function relativeTextOffset(seg: SegmentNode, anchor: any): number {
  let offset = 0;
  const anchorKey = anchor.getNode().getKey();
  for (const child of seg.getChildren()) {
    if (child.getKey() === anchorKey) {
      return offset + anchor.offset;
    }
    offset += child.getTextContent().length;
  }
  return offset;
}

/** Replace the segment's prose text, preserving decorator chrome. */
export function $setSegmentText(seg: SegmentNode, text: string): void {
  for (const child of [...seg.getChildren()]) {
    if (child.getType() === 'text') child.remove();
  }
  seg.append($createTextNode(text));
}

export function $appendText(seg: SegmentNode, text: string): void {
  const children = seg.getChildren();
  const last = children[children.length - 1];
  if (last && last.getType() === 'text') {
    const merged = last.getTextContent() + text;
    last.remove();
    seg.append($createTextNode(merged));
  } else if (text.length > 0) {
    seg.append($createTextNode(text));
  }
}

/**
 * Enter-key handler body. Must run inside `editor.update()`.
 * Returns false when the caret is not inside a SegmentNode (fall through).
 */
export function $splitSegmentAtSelection(): boolean {
  const selection = $getSelection();
  if (!$isRangeSelection(selection)) return false;
  const anchor = selection.anchor;
  const seg = $enclosingSegment(anchor.getNode());
  if (!seg) return false;
  const text = seg.getTextContent();
  const offset = relativeTextOffset(seg, anchor);
  const before = text.slice(0, offset);
  const after = text.slice(offset);
  $setSegmentText(seg, before);
  const next = new SegmentNode(
    newSegmentUuid(),
    seg.getSpeakerId(),
    seg.getSegmentType(),
    seg.getEmotion(),
  );
  next.append($createTextNode(after));
  seg.insertAfter(next);
  next.select(0, 0);
  return true;
}

/**
 * Backspace-at-start handler body. Must run inside `editor.update()`.
 * Returns false when the merge preconditions do not hold (fall through).
 */
export function $mergeSegmentBackward(): boolean {
  const selection = $getSelection();
  if (!$isRangeSelection(selection) || !selection.isCollapsed()) return false;
  const anchor = selection.anchor;
  if (anchor.offset !== 0) return false;
  const seg = $enclosingSegment(anchor.getNode());
  if (!seg) return false;
  const prev = seg.getPreviousSibling();
  if (!$isSegmentNode(prev)) return false;
  const leading = prev as SegmentNode;
  $appendText(leading, seg.getTextContent());
  seg.remove();
  leading.selectEnd();
  return true;
}
