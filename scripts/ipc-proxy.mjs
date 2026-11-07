// AUTHR_OS — Browser dev bridge middleware.
//
// Implements the transport described in docs/CLI_CONTRACT.md §1:
//   Browser dev bridge | POST /api/ipc with {command, args, bundle} (vite middleware spawns the CLI)
//
// The plugin spawns the `authr` CLI binary:
//   authr <command> [--bundle <BUNDLE_DIR>] --args '<json>'
//
// - `--bundle` is omitted for project_init / project_import / keychain_* (contract §1).
// - project_init gets `path` = <repo>/sandbox.authr when the client does not provide one.
// - After a successful project_init / project_import the resolved bundle becomes the
//   module-level "current bundle" for all subsequent bundle-scoped commands.
// - The CLI answers with exactly one line of JSON on stdout (contract §2); we relay it
//   as HTTP 200 {ok:true,data} or HTTP 400 {ok:false,error}.
// - A missing CLI binary yields a clear HTTP 500 JSON instead of a silent failure.

import { spawn } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const REPO_ROOT = path.resolve(__dirname, '..');

/** Commands that must NOT receive --bundle (contract §1). */
const NO_BUNDLE_COMMANDS = new Set([
  'project_init',
  'project_import',
  'keychain_store',
  'keychain_get',
  'keychain_delete',
]);

/** Module-level current bundle — updated after successful project_init / project_import. */
let currentBundle = null;

function defaultBundle() {
  return process.env.AUTHR_BUNDLE || path.join(REPO_ROOT, 'sandbox.authr');
}

function resolveBinary() {
  return process.env.AUTHR_CLI || path.join(REPO_ROOT, 'src-tauri', 'target', 'release', 'authr');
}

function sendJson(res, status, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(status, {
    'Content-Type': 'application/json; charset=utf-8',
    'Cache-Control': 'no-store',
  });
  res.end(body);
}

/**
 * Spawn the CLI and collect stdout/stderr. Resolves {code, out, err}.
 */
function runCli(binary, cliArgs, timeoutMs = 120_000) {
  return new Promise((resolve, reject) => {
    let child;
    try {
      child = spawn(binary, cliArgs, { windowsHide: true });
    } catch (err) {
      reject({
        code: 'BINARY_SPAWN_FAILED',
        message: `Failed to spawn "${binary}": ${err?.message ?? String(err)}`,
        category: 'bundle',
      });
      return;
    }

    let out = '';
    let err = '';
    let settled = false;

    const timer = setTimeout(() => {
      if (settled) return;
      settled = true;
      child.kill('SIGKILL');
      reject({
        code: 'CLI_TIMEOUT',
        message: `authr ${cliArgs[0]} did not answer within ${timeoutMs / 1000}s.`,
        category: 'bundle',
      });
    }, timeoutMs);

    child.stdout.on('data', (chunk) => {
      out += chunk;
    });
    child.stderr.on('data', (chunk) => {
      err += chunk;
    });
    child.on('error', (spawnErr) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      reject({
        code: 'BINARY_SPAWN_FAILED',
        message: `Failed to spawn "${binary}": ${spawnErr?.message ?? String(spawnErr)}`,
        category: 'bundle',
      });
    });
    child.on('close', (code) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve({ code, out, err });
    });
  });
}

/**
 * The CLI prints exactly one line of JSON (contract §2). Tolerate stray noise by
 * scanning from the last line backwards for a parseable envelope.
 */
function parseEnvelope(stdout) {
  const lines = String(stdout ?? '').split('\n').map((l) => l.trim()).filter(Boolean);
  for (let i = lines.length - 1; i >= 0; i -= 1) {
    try {
      const parsed = JSON.parse(lines[i]);
      if (parsed && typeof parsed.ok === 'boolean') return parsed;
    } catch {
      // not the envelope line — keep scanning
    }
  }
  return null;
}

export function authrIpcProxy() {
  return {
    name: 'authr-ipc-proxy',
    configureServer(server) {
      server.middlewares.use('/api/ipc', (req, res) => {
        if (req.method !== 'POST') {
          sendJson(res, 405, {
            ok: false,
            error: {
              code: 'METHOD_NOT_ALLOWED',
              message: 'POST {command, args} only.',
              category: 'storage',
            },
          });
          return;
        }

        let raw = '';
        req.setEncoding('utf8');
        req.on('data', (chunk) => {
          raw += chunk;
        });
        req.on('end', async () => {
          let body;
          try {
            body = raw ? JSON.parse(raw) : {};
          } catch {
            sendJson(res, 400, {
              ok: false,
              error: {
                code: 'BAD_REQUEST',
                message: 'Request body must be JSON: {command, args, bundle?}.',
                category: 'storage',
              },
            });
            return;
          }

          const command = typeof body.command === 'string' ? body.command : null;
          if (!command) {
            sendJson(res, 400, {
              ok: false,
              error: {
                code: 'BAD_REQUEST',
                message: 'Missing "command" field.',
                category: 'storage',
              },
            });
            return;
          }

          let args = body.args && typeof body.args === 'object' ? { ...body.args } : {};

          // project_init: force a sane default path inside the repo sandbox (contract §1).
          if (command === 'project_init' && !args.path) {
            args = { ...args, path: defaultBundle() };
          }

          // Bundle resolution: explicit body.bundle > current bundle (set by
          // project_init/project_import) > env/default sandbox.
          const bundle =
            typeof body.bundle === 'string' && body.bundle.length > 0
              ? body.bundle
              : currentBundle || defaultBundle();

          const binary = resolveBinary();
          if (!fs.existsSync(binary)) {
            sendJson(res, 500, {
              ok: false,
              error: {
                code: 'CLI_BINARY_MISSING',
                message:
                  `The authr CLI binary was not found at "${binary}". ` +
                  'Build the Rust core (cargo build --release in src-tauri/) or point the ' +
                  'AUTHR_CLI environment variable at a built binary. The browser dev bridge ' +
                  'spawns the CLI for every /api/ipc call.',
                category: 'bundle',
              },
            });
            return;
          }

          const cliArgs = [command];
          if (!NO_BUNDLE_COMMANDS.has(command)) {
            cliArgs.push('--bundle', bundle);
          }
          cliArgs.push('--args', JSON.stringify(args ?? {}));

          try {
            const { code, out, err } = await runCli(binary, cliArgs);
            const envelope = parseEnvelope(out);

            if (!envelope) {
              sendJson(res, code === 0 ? 500 : 400, {
                ok: false,
                error: {
                  code: 'INVALID_ENVELOPE',
                  message:
                    `authr ${command} did not emit a valid result envelope. ` +
                    (err ? `stderr: ${err.slice(0, 400)}` : `stdout: ${out.slice(0, 400)}`),
                  category: 'bundle',
                },
              });
              return;
            }

            if (envelope.ok) {
              const data = envelope.data ?? {};
              if (
                (command === 'project_init' || command === 'project_import') &&
                data && typeof data.path === 'string' && data.path.length > 0
              ) {
                currentBundle = data.path;
              }
              sendJson(res, 200, { ok: true, data });
            } else {
              const error = envelope.error ?? {
                code: 'STORAGE_ERROR',
                message: 'Unknown CLI failure.',
                category: 'storage',
              };
              sendJson(res, 400, { ok: false, error });
            }
          } catch (e) {
            sendJson(res, 500, { ok: false, error: e });
          }
        });
      });
    },
  };
}
