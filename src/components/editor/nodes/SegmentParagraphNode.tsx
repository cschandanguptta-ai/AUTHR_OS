// SegmentParagraphNode — one paragraph per chapter_segments row (contract §4.8).
// Carries the segment identity on the DOM element (`data-segment-id`) so the
// view-mode plugin, read-along cursor and click-to-select can resolve segments
// without walking Lexical internals.

import {
  $applyNodeReplacement,
  ParagraphNode,
  type EditorConfig,
  type NodeKey,
  type SerializedParagraphNode,
  type Spread,
} from 'lexical';

export type SerializedSegmentParagraphNode = Spread<
  { type: 'segment-paragraph'; version: 1 },
  SerializedParagraphNode
>;

export class SegmentParagraphNode extends ParagraphNode {
  __segmentId: string;

  static override getType(): string {
    return 'segment-paragraph';
  }

  static override clone(node: SegmentParagraphNode): SegmentParagraphNode {
    return new SegmentParagraphNode(node.__segmentId, node.__key);
  }

  constructor(segmentId: string, key?: NodeKey) {
    super(key);
    this.__segmentId = segmentId;
  }

  override createDOM(config: EditorConfig): HTMLElement {
    const el = super.createDOM(config);
    el.classList.add('canvas-paragraph');
    if (this.__segmentId) {
      el.setAttribute('data-segment-id', this.__segmentId);
    }
    return el;
  }

  override updateDOM(prevNode: this, dom: HTMLElement, config: EditorConfig): boolean {
    const updated = super.updateDOM(prevNode, dom, config);
    const id = this.__segmentId;
    if (id) {
      if (dom.getAttribute('data-segment-id') !== id) {
        dom.setAttribute('data-segment-id', id);
      }
    } else {
      dom.removeAttribute('data-segment-id');
    }
    return updated;
  }

  getSegmentId(): string {
    return this.getLatest().__segmentId;
  }

  setSegmentId(segmentId: string): void {
    const writable = this.getWritable();
    writable.__segmentId = segmentId;
  }

  override exportJSON(): SerializedSegmentParagraphNode {
    return { ...super.exportJSON(), type: 'segment-paragraph', version: 1 };
  }

  static override importJSON(): SegmentParagraphNode {
    return $createSegmentParagraphNode('');
  }
}

export function $createSegmentParagraphNode(segmentId: string): SegmentParagraphNode {
  return $applyNodeReplacement(new SegmentParagraphNode(segmentId));
}

export function $isSegmentParagraph(
  node: unknown,
): node is SegmentParagraphNode {
  return node instanceof SegmentParagraphNode;
}
