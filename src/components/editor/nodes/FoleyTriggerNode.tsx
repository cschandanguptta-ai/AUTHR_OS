// FoleyTriggerNode — inline DecoratorNode rendering the cue pill
// (SFX one-shot or ambient bed) from the live manuscript store.
// Visible in 'script' and 'readalong' modes; vanishes when the row has no cue.

import {
  $applyNodeReplacement,
  DecoratorNode,
  type EditorConfig,
  type LexicalNode,
  type NodeKey,
  type SerializedLexicalNode,
  type Spread,
} from 'lexical';
import { AudioLines } from 'lucide-react';
import { useManuscriptStore } from '../../../stores/manuscriptStore';

export interface FoleyTriggerData {
  segment_id: string;
  /** Which cue to display: 'sfx' (one-shot Foley) or 'ambient' (looping bed). */
  kind: 'sfx' | 'ambient';
}

export type SerializedFoleyTriggerNode = Spread<
  { data: FoleyTriggerData; type: 'foley-trigger'; version: 1 },
  SerializedLexicalNode
>;

function FoleyPill({ data }: { data: FoleyTriggerData }) {
  const row = useManuscriptStore((s) =>
    s.segments.find((seg) => seg.segment_id === data.segment_id),
  );
  const cue = data.kind === 'sfx' ? row?.sfx_cue : row?.ambient_cue;
  if (!cue) return null;
  const isSfx = data.kind === 'sfx';
  return (
    <span
      className="foley-pill"
      title={isSfx ? 'One-shot Foley' : 'Ambient bed'}
    >
      <AudioLines size={10} className={isSfx ? 'text-warn' : 'text-accent2'} />
      {cue}
    </span>
  );
}

export class FoleyTriggerNode extends DecoratorNode<React.ReactNode> {
  __data: FoleyTriggerData;

  static override getType(): string {
    return 'foley-trigger';
  }

  static override clone(node: FoleyTriggerNode): FoleyTriggerNode {
    return new FoleyTriggerNode(node.__data, node.__key);
  }

  constructor(data: FoleyTriggerData, key?: NodeKey) {
    super(key);
    this.__data = data;
  }

  override createDOM(_config: EditorConfig): HTMLElement {
    const span = document.createElement('span');
    span.className = 'foley-anchor';
    span.setAttribute('data-lexical-decorator', 'true');
    span.contentEditable = 'false';
    return span;
  }

  override updateDOM(): false {
    return false;
  }

  setData(data: FoleyTriggerData): void {
    const writable = this.getWritable();
    writable.__data = data;
  }

  override decorate(): React.ReactNode {
    return <FoleyPill data={this.__data} />;
  }

  override exportJSON(): SerializedFoleyTriggerNode {
    return {
      ...super.exportJSON(),
      type: 'foley-trigger',
      version: 1,
      data: this.__data,
    };
  }

  static override importJSON(serialized: SerializedFoleyTriggerNode): FoleyTriggerNode {
    const data: FoleyTriggerData = serialized.data ?? { segment_id: '', kind: 'sfx' };
    return $createFoleyTriggerNode(data);
  }
}

export function $createFoleyTriggerNode(data: FoleyTriggerData): FoleyTriggerNode {
  return $applyNodeReplacement(new FoleyTriggerNode(data));
}

export function $isFoleyTriggerNode(
  node: LexicalNode | null | undefined,
): node is FoleyTriggerNode {
  return node instanceof FoleyTriggerNode;
}
