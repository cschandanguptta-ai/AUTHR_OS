import { useEffect, useState } from 'react';
import {
  BookOpen,
  Clapperboard,
  Crosshair,
  Feather,
  FolderOpen,
  FolderPlus,
  Gauge,
  Loader2,
  Lock,
  Plus,
} from 'lucide-react';
import { useProjectStore, lastBundlePath } from '../../stores/projectStore';
import { pruneStaleDrafts } from '../../stores/localizationStore';
import { IS_DESKTOP } from '../../lib/env';
import { APP_VERSION } from '../../lib/constants';
import { btnGhost, btnPrimary, inputClass } from './Modal';

const FEATURES = [
  {
    icon: Feather,
    title: 'Manuscript AST Canvas',
    hint: 'Segments, speakers and prosody stored as a queryable AST — with BM25 full-text search across everything you have ever written.',
    tag: 'CANVAS',
  },
  {
    icon: BookOpen,
    title: 'Voice Cast Rack',
    hint: 'Assign synthesis engines, IPA phonetics and per-character prosody baselines across five engines.',
    tag: 'AUDIO',
  },
  {
    icon: Clapperboard,
    title: 'Video-Novel Studio',
    hint: 'Character reference sheets, a canonical pacing timeline and the offline ffmpeg MP4 pipeline — narration included.',
    tag: 'VIDEO',
  },
  {
    icon: Crosshair,
    title: "Chekhov's Ledger",
    hint: 'Every introduced thread tracked from its intro chapter to the payoff that must resolve it.',
    tag: 'CRAFT',
  },
  {
    icon: Gauge,
    title: 'Craft Library & Retention QC',
    hint: '25 formats × 37 personas × 18 arc frameworks, Serialized Fiction Blueprint QC, and KDP compliance scoring.',
    tag: 'CRAFT',
  },
  {
    icon: Lock,
    title: 'Lease & Bundle Safety',
    hint: 'Crash-safe 120s leases, WAL checkpoints and automatic git snapshots of your manuscript.',
    tag: 'CORE',
  },
];

export function WelcomeScreen() {
  const status = useProjectStore((s) => s.status);
  const initProject = useProjectStore((s) => s.initProject);
  const openProject = useProjectStore((s) => s.openProject);

  // Draft hygiene (R-02): on Welcome open, drop persisted localization drafts
  // that are neither from the last opened bundle nor touched for 14 days.
  useEffect(() => {
    pruneStaleDrafts(lastBundlePath());
  }, []);

  const [mode, setMode] = useState<'create' | 'open'>('create');
  const [title, setTitle] = useState('');
  const [author, setAuthor] = useState('');
  const [newPath, setNewPath] = useState('sandbox.authr');
  const [openPath, setOpenPath] = useState(lastBundlePath());
  // R-10: on desktop the native dialog is the primary path; the typed input
  // collapses into an 'Advanced' disclosure. Browser/dev mode keeps typing.
  const [advancedCreate, setAdvancedCreate] = useState(!IS_DESKTOP);
  const [advancedOpen, setAdvancedOpen] = useState(!IS_DESKTOP);

  const busy = status === 'opening';

  /** Native save dialog → the bundle DIRECTORY path (an .authr directory). */
  const pickNewBundle = async () => {
    const { save } = await import('@tauri-apps/plugin-dialog');
    const suggested = title.trim()
      ? `${title.trim().replace(/[^\w -]/g, '').trim() || 'New Book'}.authr`
      : 'NewBundle.authr';
    const picked = await save({
      title: 'Create the bundle directory',
      defaultPath: suggested,
      filters: [{ name: 'AUTHR_OS bundle', extensions: ['authr'] }],
    });
    if (typeof picked === 'string' && picked) setNewPath(picked);
  };

  /** Native directory picker for an existing .authr bundle. */
  const pickExistingBundle = async () => {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const picked = await open({
      directory: true,
      multiple: false,
      title: 'Open an .authr bundle directory',
    });
    if (typeof picked === 'string' && picked) setOpenPath(picked);
  };

  return (
    <div className="flex-1 min-h-0 overflow-y-auto">
      <div className="mx-auto w-full max-w-4xl px-6 py-14 anim-fade">
        {/* Wordmark */}
        <div className="flex flex-col items-center text-center gap-3 mb-12">
          <div className="inline-flex items-baseline gap-1 rounded-xl border border-line2 bg-panel px-6 py-3 shadow-lg">
            <span className="text-2xl font-semibold tracking-[0.18em] text-ink">AUTHR</span>
            <span className="text-2xl font-mono font-semibold text-accent">_OS</span>
          </div>
          <p className="text-sm text-mut max-w-[52ch] leading-relaxed">
            The Author's Cockpit — a professional authoring workstation with an AST manuscript
            canvas, a voice cast rack, and a Chekhov ledger that never forgets a promise.
          </p>
        </div>

        {/* Forms */}
        <div className="rounded-xl border border-line2 bg-panel shadow-xl overflow-hidden mb-12">
          <div className="flex border-b border-line" role="tablist" aria-label="Project actions">
            <button
              type="button"
              role="tab"
              aria-selected={mode === 'create'}
              onClick={() => setMode('create')}
              className={`flex items-center gap-2 px-5 py-3 text-[13px] font-medium transition-colors ${
                mode === 'create'
                  ? 'text-accent border-b-2 border-accent -mb-px'
                  : 'text-mut hover:text-ink'
              }`}
            >
              <Plus size={14} /> Create Project
            </button>
            <button
              type="button"
              role="tab"
              aria-selected={mode === 'open'}
              onClick={() => setMode('open')}
              className={`flex items-center gap-2 px-5 py-3 text-[13px] font-medium transition-colors ${
                mode === 'open'
                  ? 'text-accent border-b-2 border-accent -mb-px'
                  : 'text-mut hover:text-ink'
              }`}
            >
              <FolderOpen size={14} /> Open Project
            </button>
          </div>

          {mode === 'create' ? (
            <form
              className="p-6 grid gap-4"
              onSubmit={async (e) => {
                e.preventDefault();
                if (!title.trim() || busy) return;
                await initProject(newPath.trim() || 'sandbox.authr', title.trim(), author.trim());
              }}
            >
              <div className="grid gap-4 sm:grid-cols-2">
                <label className="block">
                  <span className="block text-2xs font-medium text-mut mb-1 uppercase tracking-wide">
                    Working title
                  </span>
                  <input
                    className={inputClass}
                    value={title}
                    onChange={(e) => setTitle(e.target.value)}
                    placeholder="The Ninefold Vault"
                    autoFocus
                  />
                </label>
                <label className="block">
                  <span className="block text-2xs font-medium text-mut mb-1 uppercase tracking-wide">
                    Author
                  </span>
                  <input
                    className={inputClass}
                    value={author}
                    onChange={(e) => setAuthor(e.target.value)}
                    placeholder="A. Merchant"
                  />
                </label>
              </div>
              <label className="block">
                <span className="flex items-center justify-between text-2xs font-medium text-mut mb-1 uppercase tracking-wide">
                  Bundle location
                  {IS_DESKTOP && (
                    <button
                      type="button"
                      onClick={() => setAdvancedCreate((a) => !a)}
                      className="text-2xs normal-case text-mut hover:text-ink underline decoration-dotted"
                    >
                      {advancedCreate ? 'Hide typed path' : 'Advanced: type path'}
                    </button>
                  )}
                </span>
                {IS_DESKTOP && !advancedCreate ? (
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => void pickNewBundle()}
                      className={`${btnGhost} flex-1 justify-start font-mono text-xs truncate`}
                      title="Opens the native save dialog — the bundle is a directory"
                      data-testid="pick-new-bundle"
                    >
                      <FolderPlus size={13} className="text-accent shrink-0" />
                      <span className="truncate">{newPath || 'Choose a location…'}</span>
                    </button>
                  </div>
                ) : (
                  <input
                    className={`${inputClass} font-mono`}
                    value={newPath}
                    onChange={(e) => setNewPath(e.target.value)}
                    placeholder="sandbox.authr"
                  />
                )}
                <span className="block text-2xs text-mut mt-1.5">
                  A <span className="font-mono">.authr</span> bundle directory{IS_DESKTOP ? ' — the native dialog picks the spot' : ''}. Leave as{' '}
                  <span className="font-mono">sandbox.authr</span> to work in the dev sandbox.
                </span>
              </label>
              <div className="flex items-center justify-between pt-1">
                <p className="text-2xs text-mut">
                  Creates the manifest, SQLite core, git scaffold and seeds the{' '}
                  <span className="font-mono">NARRATOR</span> voice profile.
                </p>
                <button type="submit" className={btnPrimary} disabled={busy || !title.trim()}>
                  {busy ? (
                    <Loader2 size={14} className="animate-spin" />
                  ) : (
                    <Plus size={14} />
                  )}
                  Initialize bundle
                </button>
              </div>
            </form>
          ) : (
            <form
              className="p-6 grid gap-4"
              onSubmit={async (e) => {
                e.preventDefault();
                if (!openPath.trim() || busy) return;
                await openProject(openPath.trim());
              }}
            >
              <label className="block">
                <span className="flex items-center justify-between text-2xs font-medium text-mut mb-1 uppercase tracking-wide">
                  Bundle location
                  {IS_DESKTOP && (
                    <button
                      type="button"
                      onClick={() => setAdvancedOpen((a) => !a)}
                      className="text-2xs normal-case text-mut hover:text-ink underline decoration-dotted"
                    >
                      {advancedOpen ? 'Hide typed path' : 'Advanced: type path'}
                    </button>
                  )}
                </span>
                {IS_DESKTOP && !advancedOpen ? (
                  <div className="flex items-center gap-2">
                    <button
                      type="button"
                      onClick={() => void pickExistingBundle()}
                      className={`${btnGhost} flex-1 justify-start font-mono text-xs truncate`}
                      title="Opens the native folder picker — choose the .authr directory"
                      data-testid="pick-open-bundle"
                    >
                      <FolderOpen size={13} className="text-accent shrink-0" />
                      <span className="truncate">{openPath || 'Choose a bundle directory…'}</span>
                    </button>
                  </div>
                ) : (
                  <input
                    className={`${inputClass} font-mono`}
                    value={openPath}
                    onChange={(e) => setOpenPath(e.target.value)}
                    placeholder="sandbox.authr"
                    autoFocus
                  />
                )}
                <span className="block text-2xs text-mut mt-1.5">
                  Opening acquires the 120s lease. A fresh foreign lease opens the bundle
                  read-only — every write returns <span className="font-mono">READ_ONLY_PROJECT</span>.
                </span>
              </label>
              <div className="flex items-center justify-between pt-1">
                <p className="text-2xs text-mut">
                  Last opened:{' '}
                  <span className="font-mono">{lastBundlePath() || '—'}</span>
                </p>
                <button type="submit" className={btnPrimary} disabled={busy || !openPath.trim()}>
                  {busy ? <Loader2 size={14} className="animate-spin" /> : <FolderOpen size={14} />}
                  Open bundle
                </button>
              </div>
            </form>
          )}
        </div>

        {/* Feature strip */}
        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
          {FEATURES.map((f) => (
            <div
              key={f.title}
              className="rounded-lg border border-line bg-panel p-4 hover:border-line2 transition-colors"
            >
              <div className="flex items-center justify-between mb-2.5">
                <div className="w-8 h-8 rounded-md border border-line bg-panel2 flex items-center justify-center text-accent">
                  <f.icon size={15} strokeWidth={1.7} />
                </div>
                <span className="text-2xs font-mono text-mut border border-line rounded px-1.5 py-0.5">
                  {f.tag}
                </span>
              </div>
              <div className="text-[13px] font-semibold mb-1">{f.title}</div>
              <p className="text-2xs text-mut leading-relaxed">{f.hint}</p>
            </div>
          ))}
        </div>

        <footer className="mt-12 flex items-center justify-center gap-3 text-2xs text-mut">
          <span className="font-mono">AUTHR_OS {APP_VERSION}</span>
          <span aria-hidden="true">·</span>
          <span>The Author's Cockpit</span>
        </footer>

        <p className="mt-6 text-center text-2xs text-mut/70 max-w-[62ch] mx-auto leading-relaxed">
          In browser mode the bridge spawns the <span className="font-mono">authr</span> CLI for every
          call — set <span className="font-mono">AUTHR_CLI</span> /{' '}
          <span className="font-mono">AUTHR_BUNDLE</span> to override the binary and bundle paths.
        </p>
      </div>
    </div>
  );
}
