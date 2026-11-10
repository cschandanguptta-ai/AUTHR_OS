import type { JSX } from 'react';
/**
 * The Triple-View Lexical AST Canvas (Sprint 7).
 *
 * One editor instance, three instant views (CSS-class switch + editability):
 *   - `clean`      Clean Literary  — prose only, all metadata chrome hidden
 *   - `script`     Script & Cast   — speaker gutter badges + emotion chips
 *                                    + Foley triggers visible
 *   - `readalong`  Read-Along      — karaoke playback, editor read-only
 *
 * Editing semantics (build plan §6.4):
 *   - Enter      splits a segment; the leading half KEEPS its UUID.
 *   - Backspace  at the start of a segment merges into the LEADING segment
 *                (leading UUID survives); the trailing segment is deleted
 *                through the bridge.
 *   - Any edit   queues a debounced flush that clears `audio_hash` on
 *                changed rows (audio invalidation).
 * All persistence flows through the 400 ms debounced AST bridge over the
 * frozen `segment_*` commands + the additive `segment_reorder`.
 */

import { useCallback, useEffect, useMemo, useRef } from 'react';
import { LexicalComposer } from '@lexical/react/LexicalComposer';
import { PlainTextPlugin } from '@lexical/react/LexicalPlainTextPlugin';
import { ContentEditable } from '@lexical/react/LexicalContentEditable';
import { LexicalErrorBoundary } from '@lexical/react/LexicalErrorBoundary';
import { HistoryPlugin } from '@lexical/react/LexicalHistoryPlugin';
import { OnChangePlugin } from '@lexical/react/LexicalOnChangePlugin';
import { useLexicalComposerContext } from '@lexical/react/LexicalComposerContext';
import {
  $getRoot,
  $getSelection,
  $isRangeSelection,
  COMMAND_PRIORITY_HIGH,
  KEY_BACKSPACE_COMMAND,
  KEY_ENTER_COMMAND,
  type EditorState,
  type LexicalEditor,
} from 'lexical';
import { AUTHR_NODES, $isSegmentNode } from './nodes';
import { $exportAst, $importSegments, type SpeakerDirectory } from './segmentsAdapter';
import { $splitSegmentAtSelection, $mergeSegmentBackward, $enclosingSegment as $enclosingSegmentOf } from './segmentOps';
import { AstSyncScheduler, computeTimings } from './astBridge';
import { useManuscript } from '../stores/manuscriptStore';
import { useUi } from '../stores/uiStore';
import { useVoiceCast } from '../stores/voiceCastStore';
import { useSession } from '../stores/sessionStore';

// ---------------- key semantics plugins ----------------

/**
 * Enter: split the current segment. The leading paragraph keeps its
 * SegmentNode instance (and therefore its UUID + audio cache identity);
 * the new paragraph receives a fresh UUID v4.
 */
function SplitPlugin(): null {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    const handler = (event: KeyboardEvent | null): boolean => {
      if (event) event.preventDefault();
      let handled = false;
      editor.update(() => {
        handled = $splitSegmentAtSelection();
      });
      return handled;
    };
    return editor.registerCommand(KEY_ENTER_COMMAND, handler, COMMAND_PRIORITY_HIGH);
  }, [editor]);
  return null;
}

/**
 * Backspace at offset 0 of a segment: merge into the LEADING segment.
 * Only the prose text moves; the trailing segment's decorators (badge,
 * chips, Foley) are discarded, and the trailing UUID disappears from the
 * export so the bridge deletes its row.
 */
function MergePlugin(): null {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    const handler = (event: KeyboardEvent | null): boolean => {
      let handled = false;
      editor.update(() => {
        if (event) event.preventDefault();
        handled = $mergeSegmentBackward();
      });
      return handled;
    };
    return editor.registerCommand(KEY_BACKSPACE_COMMAND, handler, COMMAND_PRIORITY_HIGH);
  }, [editor]);
  return null;
}

/** Editability follows the view mode (Read-Along is read-only). */
function EditableModePlugin({ editable }: { editable: boolean }): null {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    editor.setEditable(editable);
  }, [editor, editable]);
  return null;
}

/** Track the selected segment for the Dynamic Inspector. */
function SelectionTrackerPlugin(): null {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    return editor.registerUpdateListener(({ editorState }) => {
      editorState.read(() => {
        const selection = $getSelection();
        if (!$isRangeSelection(selection)) return;
        const seg = $enclosingSegmentOf(selection.anchor.getNode());
        const id = seg ? seg.getSegmentId() : null;
        if (id !== useUi.getState().selection.selectedSegmentId) {
          useUi.getState().select({ selectedSegmentId: id });
        }
      });
    });
  }, [editor]);
  return null;
}

/** Capture the editor instance for the bridge flusher (node re-keying). */
function EditorRefPlugin({ editorRef }: { editorRef: React.MutableRefObject<LexicalEditor | null> }): null {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    editorRef.current = editor;
    return () => {
      if (editorRef.current === editor) editorRef.current = null;
    };
  }, [editor, editorRef]);
  return null;
}

/** Initial (and chapter-switch) hydration from the manuscript store. */
function LoadSegmentsPlugin({ reloadKey }: { reloadKey: number }): null {
  const [editor] = useLexicalComposerContext();
  useEffect(() => {
    const { segments } = useManuscript.getState();
    const speakers: SpeakerDirectory = new Map(
      useVoiceCast.getState().characters.map((c) => [c.character_key, c.display_name]),
    );
    editor.update(() => {
      $importSegments(segments, speakers);
    });
  }, [editor, reloadKey]);
  return null;
}

// ---------------- the canvas ----------------

export function LexicalCanvas(): JSX.Element {
  const viewMode = useUi((s) => s.viewMode);
  const chapterId = useManuscript((s) => s.chapterId);
  const segments = useManuscript((s) => s.segments);
  const loading = useManuscript((s) => s.loading);
  const bridge = useManuscript((s) => s.bridge);
  const playback = useManuscript((s) => s.playback);
  const play = useManuscript((s) => s.play);
  const stop = useManuscript((s) => s.stop);
  const setActiveSegment = useManuscript((s) => s.setActiveSegment);
  const mounted = useSession((s) => s.mountedRoot !== null);

  const initialConfig = useMemo(
    () => ({
      namespace: 'authr-canvas',
      nodes: AUTHR_NODES,
      onError: (err: Error) => {
        console.error('[lexical]', err);
      },
      editable: true,
    }),
    [],
  );

  // One scheduler per mounted chapter; flushNow on chapter switch/unmount.
  const schedulerRef = useRef<AstSyncScheduler | null>(null);
  const chapterRef = useRef<number | null>(null);
  const editorRef = useRef<LexicalEditor | null>(null);

  useEffect(() => {
    if (chapterId != null && (chapterRef.current !== chapterId || !schedulerRef.current)) {
      const rows = useManuscript.getState().segments;
      const targetChapter = chapterId;
      schedulerRef.current = new AstSyncScheduler(
        rows,
        async (payload) => {
          if (payload.empty) return { remap: [] };
          useManuscript.getState().setBridge({ queued: 0 });
          const result = await useManuscript.getState().applyBridgePayload(payload, targetChapter);
          // Re-key created nodes: the DB generates its own segment_id.
          if (result.remap.length > 0 && editorRef.current) {
            const editor = editorRef.current;
            editor.update(() => {
              for (const r of result.remap) {
                for (const child of $getRoot().getChildren()) {
                  if ($isSegmentNode(child) && child.getSegmentId() === r.editorId) {
                    child.setSegmentId(r.segmentId);
                    child.setRowId(r.rowId);
                  }
                }
              }
            });
          }
          return result;
        },
        400,
        (err) => {
          console.error('[ast-bridge] flush failed', err);
        },
      );
      chapterRef.current = chapterId;
    }
    if (chapterId == null) {
      schedulerRef.current = null;
      chapterRef.current = null;
    }
  }, [chapterId]);

  useEffect(() => {
    const scheduler = schedulerRef.current;
    return () => {
      void scheduler?.flushNow().catch(() => undefined);
    };
  }, []);

  const onEditorChange = useCallback((state: EditorState, _editor: LexicalEditor) => {
    const scheduler = schedulerRef.current;
    if (!scheduler) return;
    state.read(() => {
      const ast = $exportAst();
      scheduler.push(ast);
      useManuscript.getState().setBridge({ queued: scheduler.pendingCount() });
    });
  }, []);

  // Read-Along playback ticker (estimated at 150 wpm when no cached audio).
  useEffect(() => {
    if (!playback.playing || segments.length === 0) return;
    const timings = computeTimings(segments);
    let cancelled = false;
    let idx = Math.max(0, timings.findIndex((t) => t.segmentId === playback.activeSegmentId));
    const step = (): void => {
      if (cancelled) return;
      if (idx >= timings.length) {
        stop();
        return;
      }
      const t = timings[idx];
      setActiveSegment(t.segmentId);
      document.querySelectorAll('.authr-seg.karaoke-active').forEach((el) => {
        el.classList.remove('karaoke-active');
      });
      const el = document.querySelector(`[data-segment-id="${t.segmentId}"]`);
      el?.classList.add('karaoke-active');
      el?.scrollIntoView({ block: 'center', behavior: 'smooth' });
      window.setTimeout(() => {
        idx += 1;
        step();
      }, Math.max(600, t.durationMs * 0.6));
    };
    step();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [playback.playing]);

  if (!mounted) {
    return (
      <div className="canvas-empty">
        <div>
          <h3>No bundle mounted</h3>
          <p>Mount an .authr bundle in the top bar to open the AST canvas.</p>
        </div>
      </div>
    );
  }
  if (chapterId == null) {
    return (
      <div className="canvas-empty">
        <div>
          <h3>No chapter open</h3>
          <p>Pick a chapter in the Project Binder to start writing.</p>
        </div>
      </div>
    );
  }

  return (
    <div className={`lexical-canvas view-${viewMode}`}>
      <div className="canvas-toolbar">
        <div className="view-toggle" role="tablist" aria-label="Canvas view mode">
          {(
            [
              ['clean', 'Clean Literary'],
              ['script', 'Script & Cast'],
              ['readalong', 'Read-Along'],
            ] as const
          ).map(([mode, label]) => (
            <button
              key={mode}
              className={viewMode === mode ? 'active' : ''}
              onClick={() => useUi.getState().setViewMode(mode)}
            >
              {label}
            </button>
          ))}
        </div>
        <div className="canvas-status mono">
          {bridge.syncing && <span className="pill ok">syncing…</span>}
          {!bridge.syncing && bridge.queued > 0 && <span className="pill warn">{bridge.queued} pending</span>}
          {!bridge.syncing && bridge.queued === 0 && bridge.lastSyncAt != null && <span className="pill ok">saved</span>}
          {bridge.syncError && <span className="pill bad">sync error</span>}
          <span>{segments.length} segments</span>
        </div>
      </div>
      <LexicalComposer initialConfig={initialConfig}>
        <PlainTextPlugin
          contentEditable={
            <ContentEditable
              className={`canvas-editable ${viewMode === 'readalong' ? 'readonly' : ''}`}
              aria-placeholder="Write the scene…"
              placeholder={<div className="canvas-placeholder">Write the scene… (Enter splits segments)</div>}
            />
          }
          ErrorBoundary={LexicalErrorBoundary}
        />
        <HistoryPlugin />
        <SplitPlugin />
        <MergePlugin />
        <EditableModePlugin editable={viewMode !== 'readalong'} />
        <SelectionTrackerPlugin />
        <EditorRefPlugin editorRef={editorRef} />
        <LoadSegmentsPlugin reloadKey={chapterId} />
        <OnChangePlugin onChange={onEditorChange} />
      </LexicalComposer>
      {viewMode === 'readalong' && (
        <div className="karaoke-bar">
          {!playback.playing ? (
            <button className="btn primary" onClick={play}>▶ Play read-along</button>
          ) : (
            <button className="btn" onClick={stop}>■ Stop</button>
          )}
          <span className="mono">
            {Math.round(computeTimings(segments).reduce((acc, t) => acc + t.durationMs, 0) / 1000)} s estimated
          </span>
        </div>
      )}
      {loading && <div className="canvas-loading mono">loading segments…</div>}
    </div>
  );
}
