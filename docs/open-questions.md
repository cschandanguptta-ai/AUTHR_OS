# open-questions.md — UNKNOWNs for a human to answer

Checklist format: **Q-id | question | where the gap shows in code/docs | what
resolves it**. When you answer one, delete the entry and fold the answer into
the linked doc (workflow rule in `AGENTS.md`).

- [ ] **Q-1 — Where does CI actually run?** No `.github/`, no pipeline config
  in the repo (verified 2026-11-08). `test_results.json` records a run, and
  `docs/PACKAGING.md` refers to CI checks (`SECURITY_AUDIT.md` marks
  `cargo audit`/soak as "release-machine"). Which system executes the suite
  per commit, and can `docs/testing.md §6` be wired there? → resolves
  `tech-debt.md T-14`.

- [ ] **Q-2 — Is the desktop Tauri transport mismatch intentional?**
  `shell/mod.rs` registers only the generic `authr_command` while
  `src/ipc/bridge.ts:52-54` invokes contract names directly
  (`tech-debt.md T-1`). Was the 56-command Tauri surface planned for a later
  phase, or is the bridge expected to map through `authr_command`? → decides
  the fix location.

- [ ] **Q-3 — Updater public key + release signing identity.**
  `tauri.conf.json:66` `pubkey` is a placeholder string and
  `certificateThumbprint`/`signingIdentity` are `null` (`tauri.conf.json:43,50`).
  Who owns generating the production ed25519 keypair, pasting the pubkey, and
  the code-signing certs? (`docs/PACKAGING.md` assumes a release machine.)

- [ ] **Q-4 — Version string relationship.** Cargo crate version is `3.0.0`
  (`Cargo.toml:3`), manifest stamps `authr_version: "3.0.0-PROD"`
  (`bundle/manifest.rs:7`), frontend `package.json` is `3.0.0`, tags speak of
  `v3.0.0-PROD`. Is `-PROD` the release channel marker, and should they be
  locked together by a test?

- [ ] **Q-5 — Prebuilt binary policy.** `src-tauri/target/release/authr` is
  tracked in git so tests run without cargo (`tech-debt.md T-13`). What is
  the process guaranteeing it is rebuilt from current sources before a
  release/test cycle (Golden Master step 1 says build, but nothing enforces
  freshness)?

- [ ] **Q-6 — Frontend runtime test strategy.** There are no UI/runtime
  tests for `src/` (only `tsc --noEmit` + manual dev-bridge smoke,
  `docs/testing.md §1`). Is a Playwright/WebDriver pass planned, or is the
  CLI-contract suite deemed sufficient coverage?

- [ ] **Q-7 — First core-table migration precedent.** `docs/data-model.md §5`
  rule 3 needs a human-approved pattern before anyone adds a column to a
  frozen §6.1 table (ALTER TABLE vs view layer vs contract bump). No
  precedent exists in-repo.

- [ ] **Q-8 — Cache-hit acoustic continuity semantics.** The cache key
  excludes `previous_request_id` (`audio/cache.rs:37`), so repeated lines
  reuse audio synthesized in a different context. Is best-effort continuity
  acceptable for production voices, or should continuity-sensitive engines
  (ELEVEN_LABS) bypass the cache on chained turns?

- [ ] **Q-9 — Manual edit policy for `manuscript/`.** Shadow files are
  re-rendered on every write path (`ADR-0003`); hand edits by an author are
  overwritten at next sync (INFERRED — no import-from-markdown command
  exists). Is "files are read-only output" the documented user contract?

- [ ] **Q-10 — Release-machine checklist owner.** `docs/PACKAGING.md §Golden
  Master` and `docs/SECURITY_AUDIT.md` ⏳ items (cargo audit, 12 h soak,
  Authenticode/notarization) name tasks that cannot run in CI. Who signs off
  each, and where are the records kept?

- [ ] **Q-11 — `test_results.json` freshness policy.** The repo ships a
  recorded `test_results.json` (suite metadata, counts, benchmark numbers)
  whose `suite`/`test_modules` fields are already stale vs contract v1.2
  (`tech-debt.md T-7`). Who regenerates it and when — per phase, per release,
  per CI run — and should it stay in git at all?
