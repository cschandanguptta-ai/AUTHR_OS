// AUTHR_OS — Provider settings modal (v1.6, ADR-0012): the settings surface
// for the live AI endpoints. Answers "is image generation configured in
// settings?" — YES: the image endpoint (OpenAI-compatible /images/generations)
// and the local LLM server both resolve through one ladder
// (per-call args > project settings > env > fail-closed), and this dialog
// edits the middle layer, stores the API key in the OS keychain (never in the
// settings table — bundles travel), and runs one real generation probe
// (provider_test) with latency feedback. Zero new dependencies.

import { useCallback, useEffect, useState } from 'react';
import {
  BrainCircuit,
  Check,
  CircleAlert,
  Image as ImageIcon,
  KeyRound,
  Loader2,
  PlugZap,
} from 'lucide-react';
import { Modal, Field, inputClass, btnPrimary, btnGhost } from '../Modal';
import {
  providerGet,
  providerSet,
  providerTest,
  keychainStore,
  keychainDelete,
  type ProviderGetResult,
  type ProviderTestResult,
} from '../../../ipc/api';
import { toast, toastError } from '../../../stores/uiStore';
import { cn } from '../../../lib/utils';

const IMG_KEYCHAIN_SERVICE = 'authr_img';
const IMG_KEYCHAIN_KEY = 'api_key';

function SourceBadge({ label, source }: { label: string; source: string }) {
  const tone =
    source === 'none'
      ? 'border-rose-500/40 bg-rose-500/10 text-bad'
      : source === 'env'
        ? 'border-amber-500/40 bg-amber-500/10 text-warn'
        : 'border-emerald-500/40 bg-emerald-500/10 text-ok';
  return (
    <span className={cn('text-2xs font-mono px-1.5 py-0.5 rounded border', tone)}>
      {label}: {source}
    </span>
  );
}

function TestLine({ result }: { result: ProviderTestResult }) {
  return (
    <p
      className={cn('flex items-start gap-1.5 text-2xs', result.ok ? 'text-ok' : 'text-bad')}
      role="status"
    >
      {result.ok ? (
        <Check size={12} className="mt-0.5 shrink-0" />
      ) : (
        <CircleAlert size={12} className="mt-0.5 shrink-0" />
      )}
      <span>
        {result.ok
          ? `Probe generated one real image · ${result.bytes} bytes (${result.ext}) · ${result.latency_ms} ms`
          : 'Probe failed — see the error toast for the endpoint detail'}
        <span className="block text-mut">
          endpoint: {result.endpoint_source} · key: {result.key_source} · retries {result.retries}
        </span>
      </span>
    </p>
  );
}

export function ProviderSettingsModal({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const [cfg, setCfg] = useState<ProviderGetResult | null>(null);
  const [endpointUrl, setEndpointUrl] = useState('');
  const [model, setModel] = useState('');
  const [timeoutS, setTimeoutS] = useState(120);
  const [retries, setRetries] = useState(1);
  const [apiKey, setApiKey] = useState('');
  const [llmUrl, setLlmUrl] = useState('');
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [test, setTest] = useState<ProviderTestResult | null>(null);

  const load = useCallback(async () => {
    try {
      const res = await providerGet();
      setCfg(res);
      setEndpointUrl(res.image.settings.endpoint_url ?? '');
      setModel(res.image.settings.model ?? '');
      setTimeoutS(res.image.settings.timeout_s ?? 120);
      setRetries(res.image.settings.retries ?? 1);
      setLlmUrl(res.llm.settings.server_url ?? '');
    } catch (err) {
      toastError('provider_get failed', err);
    }
  }, []);

  useEffect(() => {
    if (open) {
      setTest(null);
      void load();
    }
  }, [open, load]);

  const save = async () => {
    setSaving(true);
    try {
      await providerSet({
        endpoint_url: endpointUrl.trim() || null,
        model: model.trim() || null,
        timeout_s: timeoutS,
        retries,
        server_url: llmUrl.trim() || null,
      });
      if (apiKey.trim()) {
        await keychainStore({
          service: IMG_KEYCHAIN_SERVICE,
          key: IMG_KEYCHAIN_KEY,
          value: apiKey.trim(),
        });
        setApiKey('');
      }
      await load();
      toast('success', 'Provider settings saved', 'Resolution ladder updated — settings now outrank env.');
    } catch (err) {
      toastError('provider_set failed', err);
    } finally {
      setSaving(false);
    }
  };

  const clearKey = async () => {
    try {
      await keychainDelete({ service: IMG_KEYCHAIN_SERVICE, key: IMG_KEYCHAIN_KEY });
      await load();
      toast('success', 'API key removed', 'Keychain entry authr_img/api_key deleted.');
    } catch (err) {
      toastError('keychain_delete failed', err);
    }
  };

  const runTest = async () => {
    setTesting(true);
    setTest(null);
    try {
      // Test the UNSAVED form values first (novelcast parity: test-before-save).
      const overrides: Parameters<typeof providerTest>[0] = {};
      if (endpointUrl.trim()) overrides.endpoint_url = endpointUrl.trim();
      if (model.trim()) overrides.model = model.trim();
      overrides.timeout_s = timeoutS;
      overrides.retries = retries;
      const res = await providerTest(overrides);
      setTest(res);
    } catch (err) {
      setTest(null);
      toastError('provider_test failed', err);
    } finally {
      setTesting(false);
    }
  };

  const imgResolved = cfg?.image.resolved;
  const offline = !imgResolved || imgResolved.endpoint_source === 'none';

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="AI providers"
      subtitle="Ladder: per-call args > project settings > env > fail-closed — keys live in the OS keychain, never in the bundle"
      width="max-w-2xl"
    >
      <div className="grid gap-5">
        {/* ── Image (R2I) provider ─────────────────────────────────────── */}
        <section className="grid gap-3">
          <div className="flex items-center gap-2 flex-wrap">
            <ImageIcon size={14} className="text-accent" />
            <h3 className="text-xs font-semibold tracking-wider text-mut uppercase">
              Scene artwork (Image · R2I tier)
            </h3>
            {imgResolved && (
              <span className="ml-auto flex gap-1.5">
                <SourceBadge label="endpoint" source={imgResolved.endpoint_source} />
                <SourceBadge label="key" source={imgResolved.key_source} />
              </span>
            )}
          </div>
          <p className="text-2xs text-mut leading-relaxed">
            OpenAI-compatible <code>POST /images/generations</code> returning{' '}
            <code>b64_json</code>. Used by the Scene Studio repaint tier; without a
            resolved endpoint the runtime fails closed with{' '}
            <code>AI_ENGINE_UNAVAILABLE</code> and the offline SCRIPTED engine stays
            the default — never fakes output.
          </p>
          <Field label="Endpoint URL">
            <input
              className={inputClass}
              value={endpointUrl}
              onChange={(e) => setEndpointUrl(e.target.value)}
              placeholder="https://api.example.com/v1/images/generations"
              autoComplete="off"
              spellCheck={false}
            />
          </Field>
          <div className="grid grid-cols-3 gap-3">
            <Field label="Model">
              <input
                className={inputClass}
                value={model}
                onChange={(e) => setModel(e.target.value)}
                placeholder="flux · dall-e-3 · sd3"
                autoComplete="off"
                spellCheck={false}
              />
            </Field>
            <Field label="Timeout (s, 5–300)">
              <input
                type="number"
                min={5}
                max={300}
                className={cn(inputClass, 'font-mono')}
                value={timeoutS}
                onChange={(e) => setTimeoutS(Number(e.target.value) || 120)}
              />
            </Field>
            <Field label="Retries (0–3)">
              <input
                type="number"
                min={0}
                max={3}
                className={cn(inputClass, 'font-mono')}
                value={retries}
                onChange={(e) => setRetries(Number(e.target.value) || 0)}
              />
            </Field>
          </div>
          <Field label="API key → OS keychain (authr_img)">
            <div className="flex items-center gap-2">
              <span className="relative flex-1">
                <KeyRound
                  size={12}
                  className="absolute left-2.5 top-1/2 -translate-y-1/2 text-mut"
                />
                <input
                  type="password"
                  className={cn(inputClass, 'pl-7 font-mono')}
                  value={apiKey}
                  onChange={(e) => setApiKey(e.target.value)}
                  placeholder={
                    imgResolved?.key_configured
                      ? `configured (${imgResolved.key_source}) — type to replace`
                      : 'sk-…'
                  }
                  autoComplete="new-password"
                />
              </span>
              {imgResolved?.key_source === 'keychain' && (
                <button type="button" onClick={() => void clearKey()} className={btnGhost}>
                  Remove
                </button>
              )}
            </div>
          </Field>
        </section>

        <div className="border-t border-line" />

        {/* ── Local LLM server ─────────────────────────────────────────── */}
        <section className="grid gap-3">
          <div className="flex items-center gap-2">
            <BrainCircuit size={14} className="text-accent" />
            <h3 className="text-xs font-semibold tracking-wider text-mut uppercase">
              Text analysis (local LLM server)
            </h3>
            {cfg && <span className="ml-auto"><SourceBadge label="source" source={cfg.llm.resolved.source} /></span>}
          </div>
          <p className="text-2xs text-mut leading-relaxed">
            llama.cpp server URL (<code>AUTHR_LLAMA_SERVER</code> env is the fallback
            tier). Left empty, the offline grammar engine keeps running — semantics
            unchanged.
          </p>
          <Field label="Server URL">
            <input
              className={inputClass}
              value={llmUrl}
              onChange={(e) => setLlmUrl(e.target.value)}
              placeholder="http://127.0.0.1:8080"
              autoComplete="off"
              spellCheck={false}
            />
          </Field>
        </section>

        {/* ── Probe + actions ──────────────────────────────────────────── */}
        <section className="rounded-lg border border-line bg-panel2 px-3.5 py-3 grid gap-2">
          <div className="flex items-center gap-2 flex-wrap">
            <button
              type="button"
              onClick={() => void runTest()}
              disabled={testing}
              className={cn(btnGhost, 'flex items-center gap-1.5')}
            >
              {testing ? (
                <Loader2 size={12} className="animate-spin" />
              ) : (
                <PlugZap size={12} />
              )}
              Test image provider (one real generation)
            </button>
            {offline && (
              <span className="text-2xs text-warn flex items-center gap-1">
                <CircleAlert size={11} /> no endpoint resolved — probe will fail closed
              </span>
            )}
          </div>
          {test && <TestLine result={test} />}
        </section>
      </div>

      <footer className="mt-6 flex items-center justify-end gap-2">
        <button type="button" onClick={onClose} className={btnGhost}>
          Close
        </button>
        <button
          type="button"
          onClick={() => void save()}
          disabled={saving}
          className={cn(btnPrimary, 'flex items-center gap-1.5')}
        >
          {saving && <Loader2 size={12} className="animate-spin" />}
          Save settings
        </button>
      </footer>
    </Modal>
  );
}
