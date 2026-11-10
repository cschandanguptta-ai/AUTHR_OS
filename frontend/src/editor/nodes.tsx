/**
 * Custom Lexical AST nodes (Sprint 7).
 *
 * - `SegmentNode`  — paragraph subclass carrying the `chapter_segments`
 *   metadata (segment UUID, speaker, type, emotion, prosody, cues, row id).
 *   One top-level SegmentNode == one SQLite segment row == one shadow
 *   Markdown paragraph.
 * - `SpeakerBadgeNode` — inline decorator rendering the speaker gutter
 *   badge (Script & Cast view).
 * - `EmotionChipNode` — inline decorator rendering the emotion chip.
 * - `FoleyTriggerNode` — inline decorator rendering ambient / SFX Foley
 *   triggers (Audio Studio cue IDs).
 *
 * Decorators render pure DOM from `createDOM` (decorate() returns null) so
 * the metadata chrome never leaks into `getTextContent()` and therefore
 * never reaches `text_content` in SQLite.
 */

import {
  $applyNodeReplacement,
  type EditorConfig,
  type LexicalEditor,
  type LexicalNode,
  type NodeKey,
  type SerializedLexicalNode,
  type SerializedParagraphNode,
  DecoratorNode,
  ParagraphNode,
} from 'lexical';
import type { SegmentRow } from '../types';

export type SegmentTypeValue = SegmentRow['segment_type'];

export interface SerializedSegmentNode extends SerializedParagraphNode {
  segmentId: string;
  speakerId: string;
  segmentType: SegmentTypeValue;
  emotion: string;
  prosodyRate: number;
  prosodyPitch: number;
  ambientCue: string | null;
  sfxCue: string | null;
  rowId: number | null;
  unrendered: boolean;
}

export class SegmentNode extends ParagraphNode {
  __segmentId: string;
  __speakerId: string;
  __segmentType: SegmentTypeValue;
  __emotion: string;
  __prosodyRate: number;
  __prosodyPitch: number;
  __ambientCue: string | null;
  __sfxCue: string | null;
  /** Numeric `chapter_segments.id` once persisted (null = created in editor). */
  __rowId: number | null;
  /** Audio-cache invalidation flag ("Unrendered ▶" badge in the gutter). */
  __unrendered: boolean;

  constructor(
    segmentId: string,
    speakerId = 'NARRATOR',
    segmentType: SegmentTypeValue = 'NARRATION',
    emotion = 'neutral',
    prosodyRate = 1.0,
    prosodyPitch = 1.0,
    ambientCue: string | null = null,
    sfxCue: string | null = null,
    rowId: number | null = null,
    key?: NodeKey,
  ) {
    super(key);
    this.__segmentId = segmentId;
    this.__speakerId = speakerId;
    __typeCheck(segmentType);
    this.__segmentType = segmentType;
    this.__emotion = emotion;
    this.__prosodyRate = prosodyRate;
    this.__prosodyPitch = prosodyPitch;
    this.__ambientCue = ambientCue;
    this.__sfxCue = sfxCue;
    this.__rowId = rowId;
    this.__unrendered = false;
  }

  static getType(): string {
    return 'authr-segment';
  }

  static clone(node: SegmentNode): SegmentNode {
    return new SegmentNode(
      node.__segmentId,
      node.__speakerId,
      node.__segmentType,
      node.__emotion,
      node.__prosodyRate,
      node.__prosodyPitch,
      node.__ambientCue,
      node.__sfxCue,
      node.__rowId,
      node.__key,
    );
  }

  static importJSON(serialized: any): SegmentNode {
    const s = serialized as SerializedSegmentNode;
    return $applyNodeReplacement(
      new SegmentNode(
        s.segmentId,
        s.speakerId,
        s.segmentType,
        s.emotion,
        s.prosodyRate,
        s.prosodyPitch,
        s.ambientCue,
        s.sfxCue,
        s.rowId,
      ),
    ).updateFromJSON(serialized);
  }

  exportJSON(): SerializedSegmentNode {
    return {
      ...super.exportJSON(),
      type: SegmentNode.getType(),
      version: 1,
      segmentId: this.__segmentId,
      speakerId: this.__speakerId,
      segmentType: this.__segmentType,
      emotion: this.__emotion,
      prosodyRate: this.__prosodyRate,
      prosodyPitch: this.__prosodyPitch,
      ambientCue: this.__ambientCue,
      sfxCue: this.__sfxCue,
      rowId: this.__rowId,
      unrendered: this.__unrendered,
    };
  }

  createDOM(config: EditorConfig): HTMLElement {
    const dom = document.createElement('p');
    const classes = ['authr-seg'];
    if (this.__segmentType === 'DIALOGUE') classes.push('authr-seg-dialogue');
    if (this.__segmentType === 'INTERNAL_MONOLOGUE') classes.push('authr-seg-mono');
    if (this.__unrendered) classes.push('authr-seg-unrendered');
    dom.className = classes.join(' ');
    dom.dataset.segmentId = this.__segmentId;
    dom.dataset.speaker = this.__speakerId;
    dom.dataset.emotion = this.__emotion;
    return dom;
  }

  updateDOM(prevNode: ParagraphNode, dom: HTMLElement, _config: EditorConfig): boolean {
    void prevNode;
    dom.dataset.segmentId = this.__segmentId;
    dom.dataset.speaker = this.__speakerId;
    dom.dataset.emotion = this.__emotion;
    dom.classList.toggle('authr-seg-dialogue', this.__segmentType === 'DIALOGUE');
    dom.classList.toggle('authr-seg-mono', this.__segmentType === 'INTERNAL_MONOLOGUE');
    dom.classList.toggle('authr-seg-unrendered', this.__unrendered);
    return false;
  }

  // ---- metadata accessors (bridge + inspector) ----

  getSegmentId(): string {
    return this.__segmentId;
  }
  /** Called by the bridge after `segment_create` returns the DB UUID. */
  setSegmentId(segmentId: string): void {
    this.getWritable().__segmentId = segmentId;
  }
  getSpeakerId(): string {
    return this.__speakerId;
  }
  setSpeaker(speakerId: string): void {
    const writable = this.getWritable();
    writable.__speakerId = speakerId;
  }
  getSegmentType(): SegmentTypeValue {
    return this.__segmentType;
  }
  setSegmentType(t: SegmentTypeValue): void {
    this.getWritable().__segmentType = t;
  }
  getEmotion(): string {
    return this.__emotion;
  }
  setEmotion(e: string): void {
    this.getWritable().__emotion = e;
  }
  getProsody(): { rate: number; pitch: number } {
    return { rate: this.__prosodyRate, pitch: this.__prosodyPitch };
  }
  setProsody(rate: number, pitch: number): void {
    const w = this.getWritable();
    w.__prosodyRate = rate;
    w.__prosodyPitch = pitch;
  }
  getCues(): { ambient: string | null; sfx: string | null } {
    return { ambient: this.__ambientCue, sfx: this.__sfxCue };
  }
  setCues(ambient: string | null, sfx: string | null): void {
    const w = this.getWritable();
    w.__ambientCue = ambient;
    w.__sfxCue = sfx;
  }
  getRowId(): number | null {
    return this.__rowId;
  }
  setRowId(rowId: number | null): void {
    this.getWritable().__rowId = rowId;
  }
  markUnrendered(): void {
    this.getWritable().__unrendered = true;
  }
}

function __typeCheck(t: SegmentTypeValue): void {
  if (!['NARRATION', 'DIALOGUE', 'INTERNAL_MONOLOGUE'].includes(t)) {
    throw new Error(`invalid segment type: ${t}`);
  }
}

// ---------------- inline decorators ----------------

export interface SerializedBadgeNode extends SerializedLexicalNode {
  speakerId: string;
  display: string;
}

export class SpeakerBadgeNode extends DecoratorNode<null> {
  __speakerId: string;
  __display: string;

  constructor(speakerId: string, display: string, key?: NodeKey) {
    super(key);
    this.__speakerId = speakerId;
    this.__display = display;
  }

  static getType(): string {
    return 'authr-speaker-badge';
  }
  static clone(node: SpeakerBadgeNode): SpeakerBadgeNode {
    return new SpeakerBadgeNode(node.__speakerId, node.__display, node.__key);
  }
  static importJSON(json: any): SpeakerBadgeNode {
    return $applyNodeReplacement(new SpeakerBadgeNode(json.speakerId, json.display));
  }
  exportJSON(): SerializedBadgeNode {
    return { type: SpeakerBadgeNode.getType(), version: 1, speakerId: this.__speakerId, display: this.__display };
  }
  createDOM(_config: EditorConfig, _editor: LexicalEditor): HTMLElement {
    const span = document.createElement('span');
    span.className = 'authr-badge deco';
    span.dataset.speaker = this.__speakerId;
    span.textContent = this.__display;
    return span;
  }
  updateDOM(_prev: SpeakerBadgeNode, dom: HTMLElement, _config: EditorConfig): boolean {
    dom.textContent = this.__display;
    dom.dataset.speaker = this.__speakerId;
    return false;
  }
  decorate(_editor: LexicalEditor, _config: EditorConfig): null {
    return null;
  }
  isInline(): boolean {
    return true;
  }
  isIsolated(): boolean {
    return true;
  }
}

export interface SerializedChipNode extends SerializedLexicalNode {
  emotion: string;
}

export class EmotionChipNode extends DecoratorNode<null> {
  __emotion: string;

  constructor(emotion: string, key?: NodeKey) {
    super(key);
    this.__emotion = emotion;
  }

  static getType(): string {
    return 'authr-emotion-chip';
  }
  static clone(node: EmotionChipNode): EmotionChipNode {
    return new EmotionChipNode(node.__emotion, node.__key);
  }
  static importJSON(json: any): EmotionChipNode {
    return $applyNodeReplacement(new EmotionChipNode(json.emotion));
  }
  exportJSON(): SerializedChipNode {
    return { type: EmotionChipNode.getType(), version: 1, emotion: this.__emotion };
  }
  createDOM(_config: EditorConfig, _editor: LexicalEditor): HTMLElement {
    const span = document.createElement('span');
    span.className = 'authr-chip deco';
    span.dataset.emotion = this.__emotion;
    span.textContent = this.__emotion;
    return span;
  }
  updateDOM(_prev: EmotionChipNode, dom: HTMLElement, _config: EditorConfig): boolean {
    dom.textContent = this.__emotion;
    return false;
  }
  decorate(_editor: LexicalEditor, _config: EditorConfig): null {
    return null;
  }
  isInline(): boolean {
    return true;
  }
  isIsolated(): boolean {
    return true;
  }
}

export interface SerializedFoleyNode extends SerializedLexicalNode {
  cue: string;
  kind: 'sfx' | 'ambient';
}

export class FoleyTriggerNode extends DecoratorNode<null> {
  __cue: string;
  __kind: 'sfx' | 'ambient';

  constructor(cue: string, kind: 'sfx' | 'ambient', key?: NodeKey) {
    super(key);
    this.__cue = cue;
    this.__kind = kind;
  }

  static getType(): string {
    return 'authr-foley-trigger';
  }
  static clone(node: FoleyTriggerNode): FoleyTriggerNode {
    return new FoleyTriggerNode(node.__cue, node.__kind, node.__key);
  }
  static importJSON(json: any): FoleyTriggerNode {
    return $applyNodeReplacement(new FoleyTriggerNode(json.cue, json.kind));
  }
  exportJSON(): SerializedFoleyNode {
    return { type: FoleyTriggerNode.getType(), version: 1, cue: this.__cue, kind: this.__kind };
  }
  createDOM(_config: EditorConfig, _editor: LexicalEditor): HTMLElement {
    const span = document.createElement('span');
    span.className = `authr-foley deco foley-${this.__kind}`;
    span.dataset.cue = this.__cue;
    span.textContent = this.__kind === 'sfx' ? `⚡ ${this.__cue}` : `∞ ${this.__cue}`;
    return span;
  }
  updateDOM(_prev: FoleyTriggerNode, dom: HTMLElement, _config: EditorConfig): boolean {
    dom.dataset.cue = this.__cue;
    dom.textContent = this.__kind === 'sfx' ? `⚡ ${this.__cue}` : `∞ ${this.__cue}`;
    return false;
  }
  decorate(_editor: LexicalEditor, _config: EditorConfig): null {
    return null;
  }
  isInline(): boolean {
    return true;
  }
  isIsolated(): boolean {
    return true;
  }
}

/** Node classes registered with the editor (KlassConstructor shape). */
export const AUTHR_NODES = [SegmentNode, SpeakerBadgeNode, EmotionChipNode, FoleyTriggerNode];

export function $isSegmentNode(node: LexicalNode | null | undefined): node is SegmentNode {
  return node instanceof SegmentNode;
}
