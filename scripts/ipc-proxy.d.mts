import type { Plugin } from 'vite';

/**
 * Vite dev-server plugin implementing the AUTHR_OS browser dev bridge
 * (POST /api/ipc → spawns the `authr` CLI binary, docs/CLI_CONTRACT.md §1).
 */
export declare function authrIpcProxy(): Plugin;
