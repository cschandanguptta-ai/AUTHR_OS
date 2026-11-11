// SpeakerGutterNode — DecoratorNode anchoring the script-view gutter badge.
// Data payload: {segment_id, segment_type, speaker_id} — the segment's identity
// inside the Lexical canvas (contract §4.8: one paragraph per NarrativeSegmentAst).

import {
  $applyNodeReplacement,
  $getNodeByKey,
  DecoratorNode,
  type EditorConfig,
  type LexicalNode,
  type NodeKey,
  type SerializedLexicalNode,
  type Spread,
} from 'lexical';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import { useEffect, useRef } from 'react';
import { UserPlus } from 'lucide-react';
import { useManuscriptStore } from '../../../stores/manuscriptStore';
import { useVoiceCastStore } from '../../../stores/voiceCastStore';
import { useWorkspaceStore } from '../../../stores/workspaceStore';
import { ROLE_SPEAKER_COLORS, SEGMENT_TYPE_HINTS } from '../../../lib/constants';
import { cn, truncate } from '../../../lib/utils';
import type { SegmentType } from '../../../types/ast';

export interface SpeakerGutterData {
  segment_id: string;
  segment_type: SegmentType;
  speaker_id: string;
}

export type SerializedSpeakerGutterNode = Spread<
  { data: SpeakerGutterData; type: 'speaker-gutter'; version: 1 },
  SerializedLexicalNode
>;

/**
 * Native-propagation stopper: the badge lives inside the contentEditable root,
 * so interactions inside it must never reach Lexical's key/mouse handlers.
 */
function useStopNativePropagation() {
  const ref = useRef<HTMLSpanElement | null>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const stop = (e: Event) => e.stopPropagation();
    const events: Array<keyof HTMLElementEventMap> = [
      'mousedown',
      'mouseup',
      'click',
      'dblclick',
      'keydown',
      'keyup',
      'keypress',
      'wheel',
    ];
    events.forEach((evt) => el.addEventListener(evt, stop));
    return () => events.forEach((evt) => el.removeEventListener(evt, stop));
  }, []);
  return ref;
}

function SpeakerGutterBadge({
  data,
  nodeKey,
}: {
  data: SpeakerGutterData;
  nodeKey: NodeKey;
}) {
  const [editor] = useLexicalComposerContext();
  const ref = useStopNativePropagation();
  const character = useVoiceCastStore((s) =>
    s.characters.find((c) => c.character_key === data.speaker_id),
  );
  const isSelected = useManuscriptStore(
    (s) => s.selectedSegmentId === data.segment_id,
  );
  const updateSegment = useManuscriptStore((s) => s.updateSegment);

  const speakerLabel = character ? character.display_name : data.speaker_id;
  const roleColor = ROLE_SPEAKER_COLORS[character?.role ?? 'NARRATOR'] ?? ROLE_SPEAKER_COLORS.NARRATOR;

  // RA-08 companion: a DIALOGUE segment whose speaker has no cast profile
  // offers an inline "add to cast" affordance — the CharacterModal opens
  // pre-seeded with the segment's raw speaker key so one "Add to rack" makes
  // the segment valid without leaving the cockpit.
  const unknownSpeaker =
    data.segment_type === 'DIALOGUE' &&
    !character &&
    data.speaker_id !== 'NARRATOR' &&
    data.speaker_id.trim() !== '';

  const setType = (next: SegmentType) => {
    // Keep node data and the store row in lockstep; the store drives autosave.
    editor.update(() => {
      const node = $getNodeByKey(nodeKey);
      if (node instanceof SpeakerGutterNode) {
        node.setData({ ...node.__data, segment_type: next });
      }
    });
    const row = useManuscriptStore
      .getState()
      .segments.find((s) => s.segment_id === data.segment_id);
    if (row && row.segment_type !== next) {
      updateSegment(data.segment_id, { segment_type: next });
    }
  };

  return (
    <span ref={ref} className="inline-flex items-center">
      <span
        className={cn('gutter-badge', isSelected && 'seg-active')}
        data-segment-id={data.segment_id}
      >
        <span className={cn('font-medium truncate', roleColor)} title={speakerLabel}>
          {truncate(speakerLabel, 12)}
        </span>
        {unknownSpeaker && (
          <button
            type="button"
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => useWorkspaceStore.getState().openCastModal(data.speaker_id)}
            className="flex items-center gap-0.5 text-[9px] px-1 py-px rounded border border-dashed border-accent/60 text-accent hover:bg-accent/10"
            title={`“${speakerLabel}” is not in the cast — add a voice profile (speaker key preserved, so the segment matches)`}
            aria-label={`Add ${speakerLabel} to the cast`}
            data-testid={`add-cast-${data.segment_id}`}
          >
            <UserPlus size={9} /> cast
          </button>
        )}
        <span className="flex items-center gap-px" role="group" aria-label="Segment type">
          {(
            [
              ['NARRATION', 'N'],
              ['DIALOGUE', 'D'],
              ['INTERNAL_MONOLOGUE', 'M'],
            ] as Array<[SegmentType, string]>
          ).map(([type, letter]) => (
            <button
              key={type}
              type="button"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => setType(type)}
              title={SEGMENT_TYPE_HINTS[type]}
              aria-label={`Set segment type ${SEGMENT_TYPE_HINTS[type]}`}
              className={cn(
                'w-4 h-4 rounded text-[9px] font-mono leading-none transition-colors',
                data.segment_type === type
                  ? 'bg-accent text-on-accent font-bold'
                  : 'text-mut hover:text-ink hover:bg-panel3',
              )}
            >
              {letter}
            </button>
          ))}
        </span>
      </span>
    </span>
  );
}

export class SpeakerGutterNode extends DecoratorNode<React.ReactNode> {
  __data: SpeakerGutterData;

  static override getType(): string {
    return 'speaker-gutter';
  }

  static override clone(node: SpeakerGutterNode): SpeakerGutterNode {
    return new SpeakerGutterNode(node.__data, node.__key);
  }

  constructor(data: SpeakerGutterData, key?: NodeKey) {
    super(key);
    this.__data = data;
  }

  override createDOM(_config: EditorConfig): HTMLElement {
    const span = document.createElement('span');
    span.className = 'gutter-anchor';
    span.setAttribute('data-lexical-decorator', 'true');
    span.contentEditable = 'false';
    return span;
  }

  override updateDOM(): false {
    return false;
  }

  setData(data: SpeakerGutterData): void {
    const writable = this.getWritable();
    writable.__data = data;
  }

  getData(): SpeakerGutterData {
    return this.getLatest().__data;
  }

  override decorate(): React.ReactNode {
    return <SpeakerGutterBadge data={this.__data} nodeKey={this.getKey()} />;
  }

  override exportJSON(): SerializedSpeakerGutterNode {
    return {
      ...super.exportJSON(),
      type: 'speaker-gutter',
      version: 1,
      data: this.__data,
    };
  }

  static override importJSON(serialized: SerializedSpeakerGutterNode): SpeakerGutterNode {
    const data: SpeakerGutterData = serialized.data ?? {
      segment_id: '',
      segment_type: 'NARRATION',
      speaker_id: 'NARRATOR',
    };
    return $createSpeakerGutterNode(data);
  }
}

export function $createSpeakerGutterNode(data: SpeakerGutterData): SpeakerGutterNode {
  return $applyNodeReplacement(new SpeakerGutterNode(data));
}

export function $isSpeakerGutterNode(node: LexicalNode | null | undefined): node is SpeakerGutterNode {
  return node instanceof SpeakerGutterNode;
}
