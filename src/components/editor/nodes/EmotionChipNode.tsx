// EmotionChipNode — inline DecoratorNode rendering the per-segment emotion chip.
// Pure view: reads the live manuscript store so inspector edits update chips
// without touching the editor (visible in 'script' and 'readalong' modes).

import {
  $applyNodeReplacement,
  DecoratorNode,
  type EditorConfig,
  type LexicalNode,
  type NodeKey,
  type SerializedLexicalNode,
  type Spread,
} from 'lexical';
import { useManuscriptStore } from '../../../stores/manuscriptStore';
import { emotionChipClass, emotionLabel } from '../../../lib/constants';
import { cn } from '../../../lib/utils';

export interface EmotionChipData {
  segment_id: string;
  /** Fallback for segments that have never been synced yet. */
  emotion: string;
}

export type SerializedEmotionChipNode = Spread<
  { data: EmotionChipData; type: 'emotion-chip'; version: 1 },
  SerializedLexicalNode
>;

function EmotionChipView({ data }: { data: EmotionChipData }) {
  const live = useManuscriptStore(
    (s) => s.segments.find((seg) => seg.segment_id === data.segment_id)?.emotion,
  );
  const emotion = live ?? data.emotion ?? 'neutral';
  return (
    <span className={cn('emotion-chip', emotionChipClass(emotion))} title={`Emotion: ${emotion}`}>
      {emotionLabel(emotion).toLowerCase()}
    </span>
  );
}

export class EmotionChipNode extends DecoratorNode<React.ReactNode> {
  __data: EmotionChipData;

  static override getType(): string {
    return 'emotion-chip';
  }

  static override clone(node: EmotionChipNode): EmotionChipNode {
    return new EmotionChipNode(node.__data, node.__key);
  }

  constructor(data: EmotionChipData, key?: NodeKey) {
    super(key);
    this.__data = data;
  }

  override createDOM(_config: EditorConfig): HTMLElement {
    const span = document.createElement('span');
    span.className = 'chip-anchor';
    span.setAttribute('data-lexical-decorator', 'true');
    span.contentEditable = 'false';
    return span;
  }

  override updateDOM(): false {
    return false;
  }

  setData(data: EmotionChipData): void {
    const writable = this.getWritable();
    writable.__data = data;
  }

  override decorate(): React.ReactNode {
    return <EmotionChipView data={this.__data} />;
  }

  override exportJSON(): SerializedEmotionChipNode {
    return {
      ...super.exportJSON(),
      type: 'emotion-chip',
      version: 1,
      data: this.__data,
    };
  }

  static override importJSON(serialized: SerializedEmotionChipNode): EmotionChipNode {
    const data: EmotionChipData = serialized.data ?? { segment_id: '', emotion: 'neutral' };
    return $createEmotionChipNode(data);
  }
}

export function $createEmotionChipNode(data: EmotionChipData): EmotionChipNode {
  return $applyNodeReplacement(new EmotionChipNode(data));
}

export function $isEmotionChipNode(
  node: LexicalNode | null | undefined,
): node is EmotionChipNode {
  return node instanceof EmotionChipNode;
}
