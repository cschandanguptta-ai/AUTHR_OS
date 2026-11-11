// LexicalCanvas — Sprint 7: the Triple-View Lexical AST Canvas.
//
// One Lexical paragraph ↔ one `chapter_segments` row (contract §4.8). Segment
// identity lives in the paragraph's SpeakerGutterNode payload; text and order
// live in the DOM; prosody/emotion/cue metadata lives in the manuscript store.
// A 400 ms debounced autosave diff (adds / deletes / mutations) is serialized
// into one `sync_lexical_ast_mutation` call (§6.4 Debounced Transactional Bridge).

import { useCallback, useEffect, useRef, useState } from 'react';
import {
  $createTextNode,
  $getRoot,
  $getSelection,
  $isRangeSelection,
  EditorState,
  ParagraphNode,
} from 'lexical';
import { LexicalComposer } from '@lexical/react/LexicalComposer';
import { ContentEditable } from '@lexical/react/LexicalContentEditable';
import { LexicalErrorBoundary } from '@lexical/react/LexicalErrorBoundary';
import { HistoryPlugin } from '@lexical/react/LexicalHistoryPlugin';
import { OnChangePlugin } from '@lexical/react/LexicalOnChangePlugin';
import { RichTextPlugin } from '@lexical/react/LexicalRichTextPlugin';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  CircleAlert, FileText, Layers, Loader2, PlugZap, Radio, RefreshCw, Sparkles, Wand2, Waves, X,
} from 'lucide-react';
import { useManuscriptStore } from '../../stores/manuscriptStore';
import { useProjectStore } from '../../stores/projectStore';
import { useWorkspaceStore, type ViewMode } from '../../stores/workspaceStore';
import { useAiAssistStore, AI_ACTION_LABEL, seedFromText, type AiAction } from '../../stores/aiAssistStore';
import { localLlmGenerate, providerGet } from '../../ipc/api';
import { mapError } from '../../ipc/errorCopy';
import { useAutosave } from '../../hooks/useAutosave';
import { useChapterStats } from '../../hooks/useChapterStats';
import { cn } from '../../lib/utils';
import { CHAPTER_STATUSES } from '../../lib/constants';
import type { ChapterStatus, NarrativeSegmentAst, SegmentType } from '../../types/ast';
import { SpeakerGutterNode, $createSpeakerGutterNode } from './nodes/SpeakerGutterNode';
import { SegmentParagraphNode, $createSegmentParagraphNode } from './nodes/SegmentParagraphNode';
import { EmotionChipNode, $createEmotionChipNode } from './nodes/EmotionChipNode';
import { FoleyTriggerNode, $createFoleyTriggerNode } from './nodes/FoleyTriggerNode';
import { useViewModeSync } from './nodes/viewModePlugin';

const VIEW_MODES: Array<{ id: ViewMode; label: string; hint: string }> = [
  { id: 'clean', label: 'Clean Literary', hint: 'Prose only — no chrome' },
  { id: 'script', label: 'Script & Cast', hint: 'Gutter badges, emotion chips, Foley cues' },
  { id: 'readalong', label: 'Read-Along', hint: 'Karaoke preview — click a paragraph to follow' },
];

/** One fully-chromed paragraph per segment: gutter badge + text + chip + cues. */
function $makeSegmentParagraph(seg: NarrativeSegmentAst): SegmentParagraphNode {
  const paragraph = $createSegmentParagraphNode(seg.segment_id);
  paragraph.append($createSpeakerGutterNode({
    segment_id: seg.segment_id,
    segment_type: seg.segment_type,
    speaker_id: seg.speaker_id,
  }));
  if (seg.text_content) paragraph.append($createTextNode(seg.text_content));
  paragraph.append($createEmotionChipNode({ segment_id: seg.segment_id, emotion: seg.emotion }));
  if (seg.sfx_cue) {
    paragraph.append($createFoleyTriggerNode({ segment_id: seg.segment_id, kind: 'sfx' }));
  }
  if (seg.ambient_cue) {
    paragraph.append($createFoleyTriggerNode({ segment_id: seg.segment_id, kind: 'ambient' }));
  }
  return paragraph;
}

interface ParagraphInfo {
  segmentId: string;
  segmentType: SegmentType;
  speakerId: string;
  text: string;
}

function $readParagraphs(): ParagraphInfo[] {
  const out: ParagraphInfo[] = [];
  for (const child of $getRoot().getChildren()) {
    if (!(child instanceof ParagraphNode)) continue;
    // Identity rides the paragraph node itself (created as SegmentParagraph);
    // gutter badges remain the script-view chrome + type/speaker source.
    let segmentId = child instanceof SegmentParagraphNode ? child.getSegmentId() : '';
    let segmentType: SegmentType = 'NARRATION';
    let speakerId = 'NARRATOR';
    for (const node of child.getChildren()) {
      if (node instanceof SpeakerGutterNode) {
        if (!segmentId) segmentId = node.getData().segment_id;
        segmentType = node.getData().segment_type;
        speakerId = node.getData().speaker_id;
      }
    }
    out.push({ segmentId, segmentType, speakerId, text: child.getTextContent() });
  }
  return out;
}

/**
 * AI assist (R-03): one deterministic offline generation from the selection
 * (or the whole chapter for summarize), surfaced as an accept/discard
 * proposal — never auto-committed. Accept applies a single undoable Lexical
 * transaction.
 */
function useAiAssist(editor: ReturnType<typeof useLexicalComposerContext>[0], chapterId: number | null, readonly: boolean) {
  const busy = useAiAssistStore((s) => s.busy);
  const proposal = useAiAssistStore((s) => s.proposal);
  const error = useAiAssistStore((s) => s.error);

  const run = useCallback(
    async (action: AiAction) => {
      if (readonly || chapterId == null) return;
      const assist = useAiAssistStore.getState();
      if (assist.busy) return;
      assist.setBusy(action);

      // Read the working text: selection for continue/rewrite, chapter otherwise.
      let workText = '';
      let hasSelection = false;
      editor.getEditorState().read(() => {
        const sel = $getSelection();
        if ($isRangeSelection(sel)) {
          workText = sel.getTextContent();
          hasSelection = workText.trim().length > 0;
        }
      });
      if (action !== 'summarize' && !hasSelection) {
        // No selection: continue works from the tail of the chapter text.
        const segs = useManuscriptStore.getState().segments;
        workText = segs.slice(-3).map((s) => s.text_content).join(' ').slice(-400);
      }
      if (action === 'summarize') {
        const segs = useManuscriptStore.getState().segments;
        workText = segs.map((s) => s.text_content).join('\n').slice(0, 4000);
      }

      const seed = seedFromText(`${action}:${workText}`);
      const prompts: Record<AiAction, string> = {
        continue: `Continue the following passage in the same voice, one paragraph, no preamble:\n\n${workText}`,
        rewrite: `Rewrite the following passage tighter while keeping its meaning, no preamble:\n\n${workText}`,
        summarize: `Summarize this chapter in 2-3 sentences, no preamble:\n\n${workText}`,
      };

      try {
        // server_url comes from the provider ladder when configured (R-03).
        let serverUrl: string | undefined;
        try {
          const p = await providerGet();
          serverUrl = p.llm.resolved.server_url ?? undefined;
        } catch { /* offline default */ }
        const res = await localLlmGenerate({ prompt: prompts[action], max_tokens: 128, seed, server_url: serverUrl });
        const text = res.text.trim();
        if (!text) {
          useAiAssistStore.getState().fail('The engine returned an empty draft — nothing to propose.');
          return;
        }
        useAiAssistStore.getState().propose({
          action,
          text,
          backend: res.backend === 'LLAMA_SERVER' && res.server ? `llama-server · ${res.server}` : res.backend,
          seed: res.seed ?? seed,
          effect:
            action === 'continue'
              ? 'Insert at the caret'
              : action === 'rewrite'
                ? 'Replace the selection'
                : 'Insert at the caret',
        });
      } catch (err) {
        const mapped = mapError(err);
        useAiAssistStore.getState().fail(`${mapped.copy.body} ${mapped.detail}`.trim());
      }
    },
    [editor, chapterId, readonly],
  );

  /** Accept: one undoable transaction. Continue/summarize insert at the caret
   *  (after the selection, when one exists); rewrite replaces the selection. */
  const accept = useCallback(() => {
    const p = useAiAssistStore.getState().proposal;
    if (!p) return;
    editor.update(() => {
      const sel = $getSelection();
      if ($isRangeSelection(sel)) {
        if (p.action === 'continue' && !sel.isCollapsed()) {
          // Keep the original text; append the continuation right after it.
          const original = sel.getTextContent();
          sel.insertText(original + p.text);
        } else {
          sel.insertText(p.text);
        }
      } else {
        // No caret (blur): append to the last paragraph.
        const root = $getRoot();
        const last = root.getLastChild();
        if (last instanceof ParagraphNode) last.append($createTextNode(p.text));
      }
    });
    useAiAssistStore.getState().resolve();
  }, [editor]);

  const discard = useCallback(() => useAiAssistStore.getState().resolve(), []);

  return { busy, proposal, error, run, accept, discard };
}

function CanvasInner() {
  const [editor] = useLexicalComposerContext();
  const viewMode = useWorkspaceStore((s) => s.viewMode);
  const setViewMode = useWorkspaceStore((s) => s.setViewMode);
  const readonly = useProjectStore((s) => s.readonly);
  const status = useProjectStore((s) => s.status);

  const segments = useManuscriptStore((s) => s.segments);
  const dirty = useManuscriptStore((s) => s.dirty);
  const saving = useManuscriptStore((s) => s.saving);
  const saveMutation = useManuscriptStore((s) => s.saveMutation);
  const chapters = useManuscriptStore((s) => s.chapters);
  const activeChapterId = useManuscriptStore((s) => s.activeChapterId);
  const loadingSegments = useManuscriptStore((s) => s.loadingSegments);
  const loadSegments = useManuscriptStore((s) => s.loadSegments);
  const updateChapter = useManuscriptStore((s) => s.updateChapter);

  useViewModeSync(viewMode); // plugin-style hook: root classes + read-along cursor

  const applyingRef = useRef(true);
  const loadedChapterRef = useRef<number | null>(null);
  const chapter = chapters.find((c) => c.id === activeChapterId) ?? null;

  const [titleDraft, setTitleDraft] = useState('');
  useEffect(() => {
    setTitleDraft(chapter?.title ?? '');
  }, [chapter?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  // ── Load a chapter into the editor ────────────────────────────────────────
  const rebuild = useCallback(
    async (chapterId: number) => {
      applyingRef.current = true;
      const rows = await loadSegments(chapterId);
      editor.update(() => {
        const root = $getRoot();
        root.clear();
        if (rows.length === 0) {
          root.append($createSegmentParagraphNode(''));
        } else {
          for (const row of rows) root.append($makeSegmentParagraph(row));
        }
      });
      window.setTimeout(() => {
        applyingRef.current = false;
      }, 80);
    },
    [editor, loadSegments],
  );

  useEffect(() => {
    if (activeChapterId != null && activeChapterId !== loadedChapterRef.current) {
      loadedChapterRef.current = activeChapterId;
      void rebuild(activeChapterId);
    }
  }, [activeChapterId, rebuild]);

  useEffect(() => {
    editor.setEditable(status === 'open' && !readonly);
  }, [editor, readonly, status]);

  // ── Debounced bridge (§6.4): DOM diff → sync_lexical_ast_mutation ────────
  const flush = useCallback(async () => {
    if (applyingRef.current || activeChapterId == null || readonly) return;
    const dom = editor.getEditorState().read($readParagraphs);
    const storeRows = useManuscriptStore.getState().segments;
    const rowById = new Map(storeRows.map((r) => [r.segment_id, r]));

    const upserts: NarrativeSegmentAst[] = [];
    const seen = new Set<string>();
    let createdAny = false;

    dom.forEach((p, index) => {
      let segId = p.segmentId;
      let row = segId ? rowById.get(segId) : undefined;
      if (!row) {
        // Brand-new paragraph (Enter split): fresh UUID v4 per §6.4 split rule.
        segId = crypto.randomUUID();
        createdAny = true;
        row = {
          segment_id: segId,
          chapter_id: activeChapterId,
          sequence_order: index,
          segment_type: p.segmentType,
          speaker_id: p.speakerId,
          emotion: 'neutral',
          prosody_rate: 1.0,
          prosody_pitch: 1.0,
          text_content: p.text,
          ambient_cue: null,
          sfx_cue: null,
          prev_request_id: null,
          audio_hash: null,
          audio_start_ms: 0,
          audio_duration_ms: 0,
        };
      }
      seen.add(segId);
      upserts.push({ ...row, text_content: p.text, sequence_order: index });
    });

    // Paragraph merges: trailing ids vanish from the DOM → deletes (§6.4).
    const deletes = storeRows
      .filter((r) => !seen.has(r.segment_id))
      .map((r) => r.segment_id);

    const changed = upserts.some((u) => {
      const prev = rowById.get(u.segment_id);
      return !prev
        || prev.text_content !== u.text_content
        || prev.sequence_order !== u.sequence_order;
    });
    if (!changed && deletes.length === 0 && !createdAny) return;

    const res = await saveMutation({ chapter_id: activeChapterId, upserts, deletes });
    // Structural changes get a rebuild so new UUIDs receive their gutter/chrome.
    if (res.ok && (createdAny || deletes.length > 0)) {
      await rebuild(activeChapterId);
    }
  }, [activeChapterId, editor, readonly, rebuild, saveMutation]);

  const autosave = useAutosave(flush, 400);

  const onChange = useCallback(
    (_editorState: EditorState, _editor: unknown, tags: Set<string>) => {
      if (applyingRef.current || tags.has('history-merge')) return;
      autosave.schedule();
    },
    [autosave],
  );

  // Inspector-driven metadata edits (dirty flag) also schedule a save.
  useEffect(() => {
    return useManuscriptStore.subscribe((s, prev) => {
      if (s.dirty && !prev.dirty && !applyingRef.current) autosave.schedule();
    });
  }, [autosave]);

  const commitTitle = () => {
    if (chapter && titleDraft !== chapter.title && titleDraft.trim()) {
      void updateChapter(chapter.id, { title: titleDraft.trim() });
    }
  };

  // ── AI assist (R-03) ──────────────────────────────────────────────────
  const ai = useAiAssist(editor, activeChapterId, readonly);
  const pendingRequest = useAiAssistStore((s) => s.pendingRequest);

  // Command palette / other surfaces request actions; the canvas consumes
  // them here (the editor owns the transaction).
  useEffect(() => {
    if (pendingRequest) {
      useAiAssistStore.getState().consumeRequest();
      void ai.run(pendingRequest);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pendingRequest, ai.run]);

  // Wire the debounce to Lexical's change stream (after hooks above).
  const onChangePlugin = useCallback(
    (editorState: EditorState, _e: unknown, tags: Set<string>) => {
      onChange(editorState, _e, tags);
    },
    [onChange],
  );

  // Off-main-thread chapter stats (workerisation, ADR-0008) — the live word
  // counter no longer re-walks every segment's text on every render.
  const stats = useChapterStats(segments);

  return (
    <div className="flex-1 min-h-0 flex flex-col min-w-0">
      {/* Chapter header */}
      <div className="shrink-0 flex items-center gap-3 px-5 py-2.5 border-b border-line bg-panel">
        <FileText size={15} className="text-accent shrink-0" />
        {chapter ? (
          <input
            value={titleDraft}
            onChange={(e) => setTitleDraft(e.target.value)}
            onBlur={commitTitle}
            onKeyDown={(e) => {
              if (e.key === 'Enter') commitTitle();
            }}
            disabled={readonly}
            aria-label="Chapter title"
            className="bg-transparent border-none outline-none text-sm font-semibold text-ink min-w-0 flex-1 focus:bg-panel2 rounded px-1.5 py-0.5"
          />
        ) : (
          <span className="text-sm text-mut">No chapter selected — pick one in the binder</span>
        )}

        {chapter && (
          <select
            value={chapter.status}
            disabled={readonly}
            onChange={(e) => void updateChapter(chapter.id, { status: e.target.value as ChapterStatus })}
            aria-label="Chapter status"
            className="text-2xs font-mono bg-panel2 border border-line rounded px-1.5 py-1 text-mut"
          >
            {CHAPTER_STATUSES.map((s) => (
              <option key={s} value={s}>{s}</option>
            ))}
          </select>
        )}

        <span className="font-mono text-2xs text-mut tabular-nums whitespace-nowrap">
          {stats.words.toLocaleString('en-US')} words
        </span>

        {/* Triple-view switcher */}
        <div
          className="ml-auto flex items-center rounded-lg border border-line bg-panel2 p-0.5"
          role="tablist"
          aria-label="Canvas view mode"
        >
          {VIEW_MODES.map((m, i) => (
            <span key={m.id} className="flex items-center">
              {i > 0 && <span className="w-px h-4 bg-line" aria-hidden />}
              <button
                type="button"
                role="tab"
                aria-selected={viewMode === m.id}
                title={m.hint}
                onClick={() => setViewMode(m.id)}
                className={cn(
                  'px-2.5 py-1 text-2xs rounded-md transition-colors whitespace-nowrap',
                  viewMode === m.id
                    ? 'bg-accent text-on-accent font-semibold'
                    : 'text-mut hover:text-ink',
                )}
              >
                {m.id === 'clean' && <FileText size={11} className="inline mr-1 -mt-px" />}
                {m.id === 'script' && <Layers size={11} className="inline mr-1 -mt-px" />}
                {m.id === 'readalong' && <Radio size={11} className="inline mr-1 -mt-px" />}
                {m.label}
              </button>
            </span>
          ))}
        </div>

        <span
          className="flex items-center gap-1.5 text-2xs text-mut whitespace-nowrap"
          aria-live="polite"
        >
          {saving ? (
            <><Loader2 size={12} className="animate-spin text-accent" /> syncing…</>
          ) : dirty ? (
            <><RefreshCw size={12} className="text-warn" /> pending</>
          ) : (
            <><Waves size={12} className="text-ok" /> synced</>
          )}
        </span>

        {/* AI assist — selection actions (R-03) */}
        {!readonly && chapter && (
          <div className="flex items-center gap-1 shrink-0" role="group" aria-label="AI assist">
            {(['continue', 'rewrite', 'summarize'] as AiAction[]).map((a) => (
              <button
                key={a}
                type="button"
                disabled={ai.busy !== null}
                onClick={() => void ai.run(a)}
                title={`${AI_ACTION_LABEL[a]} — offline deterministic engine, proposal with accept/discard`}
                data-testid={`ai-${a}`}
                className="flex items-center gap-1 text-2xs px-1.5 py-1 rounded border border-line bg-panel2 hover:border-accent/50 hover:text-ink text-mut disabled:opacity-50"
              >
                {ai.busy === a ? <Loader2 size={10} className="animate-spin text-accent" /> : <Sparkles size={10} className="text-accent" />}
                {a === 'continue' ? 'Continue' : a === 'rewrite' ? 'Rewrite' : 'Summarize'}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* Editor surface */}
      <div className="flex-1 min-h-0 overflow-y-auto relative bg-app">
        {loadingSegments && (
          <div className="absolute inset-0 flex items-center justify-center bg-app/70 z-10">
            <Loader2 className="animate-spin text-accent" size={22} />
          </div>
        )}
        {/* Proposal bar — never auto-commits; Accept is one undoable transaction */}
        {(ai.proposal || ai.error) && (
          <div className="sticky top-0 z-20 mx-auto max-w-[900px] px-2 pt-2" data-testid="ai-proposal-bar">
            <div className="rounded-lg border border-accent/40 bg-panel shadow-xl px-3 py-2.5 grid gap-2">
              {ai.error ? (
                <div className="flex items-start gap-2">
                  <Wand2 size={13} className="text-bad mt-0.5 shrink-0" />
                  <p className="text-2xs text-bad leading-relaxed flex-1">{ai.error}</p>
                  <button type="button" onClick={ai.discard} aria-label="Dismiss" className="text-mut hover:text-ink">
                    <X size={12} />
                  </button>
                </div>
              ) : ai.proposal && (
                <>
                  <div className="flex items-center gap-2 flex-wrap text-2xs">
                    <span className="font-semibold text-ink">{AI_ACTION_LABEL[ai.proposal.action]}</span>
                    <span className="text-mut">{ai.proposal.effect}</span>
                    <span className="ml-auto font-mono px-1.5 py-0.5 rounded border border-line text-mut" data-testid="ai-provenance">
                      {ai.proposal.backend} · seed {ai.proposal.seed}
                    </span>
                  </div>
                  <p className="text-xs leading-relaxed text-ink whitespace-pre-wrap max-h-24 overflow-y-auto">
                    {ai.proposal.text}
                  </p>
                  {ai.proposal.backend === 'OFFLINE_GRAMMAR' && (
                    <div
                      className="flex items-center gap-2 flex-wrap rounded-md border border-warn/30 bg-warn/5 px-2.5 py-1.5"
                      data-testid="ai-fallback-note"
                    >
                      <CircleAlert size={11} className="text-warn shrink-0" />
                      <span className="text-2xs text-mut leading-relaxed flex-1 min-w-[24ch]">
                        Offline grammar engine — deterministic filler, not model prose. Connect an
                        LLM server for real continuations; output stays marked until then.
                      </span>
                      <button
                        type="button"
                        onClick={() => useWorkspaceStore.getState().openProviderModal('llm')}
                        className="flex items-center gap-1 text-2xs font-medium px-2 py-1 rounded border border-accent/50 bg-accent/10 hover:bg-accent/20 text-ink"
                        data-testid="ai-connect-provider"
                      >
                        <PlugZap size={10} className="text-accent" /> Connect an LLM server
                      </button>
                    </div>
                  )}
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={ai.accept}
                      className="text-2xs font-medium px-2.5 py-1 rounded bg-accent text-on-accent"
                      data-testid="ai-accept"
                    >
                      Accept & insert
                    </button>
                    <button
                      type="button"
                      onClick={ai.discard}
                      className="text-2xs px-2.5 py-1 rounded border border-line text-mut hover:text-ink"
                      data-testid="ai-discard"
                    >
                      Discard
                    </button>
                    <span className="text-2xs text-mut/70">undo works normally after accept</span>
                  </div>
                </>
              )}
            </div>
          </div>
        )}
        <div className="mx-auto max-w-[900px]">
          <RichTextPlugin
            contentEditable={
              <ContentEditable
                className={cn('canvas-editor', readonly && 'opacity-70 cursor-not-allowed')}
                aria-placeholder={chapter ? 'Begin writing…' : 'Select a chapter in the Project Binder'}
                placeholder={
                  <span className="editor-placeholder">
                    {chapter
                      ? 'Begin writing… (Enter splits segments — the gutter assigns cast & type)'
                      : 'Select a chapter in the Project Binder'}
                  </span>
                }
                readOnly={readonly}
              />
            }
            ErrorBoundary={LexicalErrorBoundary}
          />
        </div>
        {readonly && (
          <div className="absolute top-3 left-1/2 -translate-x-1/2 px-3 py-1.5 rounded-lg border border-warn/40 bg-warn/10 text-warn text-2xs">
            Read-only — a foreign lease is active on this bundle
          </div>
        )}
      </div>

      <OnChangePlugin onChange={onChangePlugin} />

      {/* Status bar */}
      <div className="shrink-0 flex items-center gap-4 px-5 py-1.5 border-t border-line bg-panel text-2xs text-mut font-mono">
        <span>NARRATION · DIALOGUE · INTERNAL_MONOLOGUE — one paragraph = one chapter_segments row</span>
        <span className="ml-auto">{segments.length} segments · view: {viewMode}</span>
      </div>
    </div>
  );
}

export function LexicalCanvas() {
  return (
    <LexicalComposer
      initialConfig={{
        namespace: 'authr-canvas',
        editable: false,
        theme: {},
        nodes: [SegmentParagraphNode, SpeakerGutterNode, EmotionChipNode, FoleyTriggerNode],
        onError: (error: Error) => {
          console.error('[LexicalCanvas]', error);
        },
      }}
    >
      <CanvasInner />
      <HistoryPlugin />
    </LexicalComposer>
  );
}
