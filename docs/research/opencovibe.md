# OpenCovibe — Reuse Analysis for Cowork

**Date:** 2026-06-04
**Analyst pass:** evidence-based, code-read (repo cloned to `/tmp/opencovibe`, depth 1)
**Lens:** what is reusable for **Cowork** (Tauri v2 + SvelteKit/Svelte 5 runes + Tailwind v4 + xterm.js, Windows-first, WSL2 bootstrap, CLI/TUI agent over a **real PTY**, Paraglide-JS v2 i18n, Rust `errors.json` codegen).

> Evidence convention: **[V]** = verified by reading the actual file/line; **[I]** = inferred from surrounding code/structure but not exhaustively traced.

---

## 0. Repo verification (Step 0)

**The prior claim was substantially CORRECT on identity, WRONG on two stack details and on the core philosophy.**

| Field | Value | Source |
|---|---|---|
| Repo URL | `https://github.com/AnyiWang/OpenCovibe` | clone succeeded [V] |
| Stars | **194** | `gh repo view` [V] |
| Last commit | **2026-06-04** (`2e857638`, "docs: update README architecture for Codex integration") | `git log` [V] |
| Last push | 2026-06-04T01:49:37Z | `gh` [V] |
| License | **Apache-2.0** | `LICENSE`, `package.json`, `Cargo.toml` [V] |
| Version | 0.2.0 | `package.json` [V] |
| Description | "Local-first desktop app for AI coding agents (Claude Code, Codex). Built with Tauri v2 + Svelte 5." | `gh` [V] |

### Corrections to the prior research pass
1. **Tailwind is v3, NOT v4.** `package.json` pins `"tailwindcss": "^3.4.17"` with `tailwind.config.ts` + `postcss.config.js` (classic v3 setup). We are on **Tailwind v4** (CSS-first config, no `tailwind.config.ts`). [V]
2. **They do NOT use Paraglide.** There is zero Paraglide dependency. They built a **self-rolled i18n runtime** (`src/lib/i18n/`) and explicitly *migrated off* Paraglide (legacy localStorage key `PARAGLIDE_LOCALE` is read for migration). The stale `src/paraglide/messages/**` glob in `tailwind.config.ts` is a leftover. [V]
3. **MOST IMPORTANT — the "API-key / direct-LLM-API tool" framing in the task brief is FALSE for OpenCovibe.** OpenCovibe wraps **CLI agents (Claude Code, Codex) over stdio pipes**, exactly like Cowork's core thesis. It is *not* a chat-bubble LLM-API client. The API keys it manages are **passed through to the CLI agents** (e.g. Codex `model_providers.*.env_key`), not used to call an LLM directly. So the "avoid the API/chat parts" instruction mostly **does not apply** — there is very little direct-API code to avoid.

**Verdict: real repo, correct identity, and architecturally MUCH closer to Cowork than the brief assumed.** This raises the reuse value but also sharpens the one real divergence (pipes vs PTY — see §3c).

---

## 1. Exact stack & versions [V]

### Frontend (`package.json`)
- **SvelteKit** `@sveltejs/kit ^2.16`, **Svelte `^5.53`** (runes — confirmed `$state`/`$props`/`$derived`/`$effect` throughout), `@sveltejs/adapter-static ^3` with `fallback: "index.html"` (SPA mode).
- **Vite `^6.1`**, **Vitest `^4`**.
- **Tailwind `^3.4`** + `@tailwindcss/typography`, `autoprefixer`, `postcss`.
- **xterm**: `@xterm/xterm ^6`, `@xterm/addon-fit ^0.11`, `@xterm/addon-web-links ^0.12`.
- Markdown/sanitize: `marked`, `highlight.js`, `dompurify`, `turndown`.
- Editor: **CodeMirror 6** (many `@codemirror/lang-*`) — they have an in-app code/file viewer.
- Doc parsing: `exceljs`, `mammoth` (xlsx/docx ingestion for attachments).

### Host (`src-tauri/Cargo.toml`)
- **Tauri v2** (`tauri = { version = "2", features = ["tray-icon","image-png"] }`).
- Plugins: `tauri-plugin-notification`, `-dialog`, `-shell`, `-global-shortcut` (all v2).
- Async: **tokio (full)**, `tokio-util`, `futures-util`.
- **`axum 0.7` + `tower-http` + ws** — they run an **embedded HTTP/WebSocket server** (remote/browser access to the same backend; see §3 transport).
- `reqwest` (json+stream), `notify 7` (fs watch), `uuid`, `chrono`, `serde`/`serde_json`/`serde_yaml`, `toml`/`toml_edit` (Codex config editing), `strip-ansi-escapes`, `sha2`, `rayon`, `if-addrs`, `include_dir`, `rand`.
- Windows-specific: `windows 0.61` (DataExchange/Ole/Shell/Foundation) — clipboard + shell integration.
- **NO `portable-pty`. NO `conpty`. NO `nix`/`openpty`.** Process I/O is plain `Stdio::piped()`. [V]
- Release profile: `panic="abort"`, `lto=true`, `opt-level="s"`, `strip=true`, `codegen-units=1`.

### Tauri config (`src-tauri/tauri.conf.json`) [V]
- Window 1280×800, `maximized: true`, `dragDropEnabled: true`.
- Bundle `targets: "all"`, macOS + Windows (NSIS `downloadBootstrapper` for WebView2), Linux deb deps.
- CSP allows `connect-src 'self' https://api.anthropic.com https://*.anthropic.com` (used for usage/pricing fetches, not chat).
- Capabilities (`capabilities/default.json`): `core:default`, `shell:allow-open`, `dialog:default`, `notification:default`, `global-shortcut:default`, `core:webview:allow-set-webview-zoom`. Minimal, clean. [V]

---

## 2. Repo structure [V]

```
/ (root)
├─ src/                      # SvelteKit frontend
│  ├─ lib/
│  │  ├─ components/         # ~40 Svelte components (XTerminal, TerminalPane, ChatMessage, InlineToolCard, ...)
│  │  ├─ stores/             # Svelte-5 rune stores (session-store.svelte.ts is 4284 lines)
│  │  ├─ i18n/               # SELF-BUILT i18n runtime (index.svelte.ts, registry.ts, format.ts, types.ts)
│  │  ├─ transport/          # Transport abstraction: tauri.ts (IPC) vs websocket.ts (remote)
│  │  └─ utils/
│  └─ routes/                # chat, config/{claude,codex}, explorer, history, memory, plugins, settings, teams, usage
├─ messages/                 # en.json (85KB), zh-CN.json — flat key→string maps
├─ src-tauri/
│  ├─ src/
│  │  ├─ agent/              # session_actor.rs (3172 lines!), spawn.rs, adapter.rs, claude_*, codex_*, turn_engine.rs, control.rs, stream.rs, ssh.rs
│  │  ├─ commands/           # ~35 Tauri #[command] modules (session, chat, git, files, settings, onboarding, ...)
│  │  ├─ storage/            # JSONL/file persistence (runs, events, sessions, settings, usage, teams)
│  │  ├─ web_server/         # axum router/ws/auth/broadcaster — remote-access backend
│  │  ├─ hooks/              # CLI hook setup + team watcher
│  │  ├─ models.rs, pricing.rs, process_ext.rs (Windows CREATE_NO_WINDOW)
│  └─ capabilities/default.json
├─ scripts/                  # release.mjs, i18n-check.mjs, i18n-add-locale.mjs, check-serde-sync.sh, setup.sh
└─ .github/workflows/        # ci.yml, release.yml
```

### Host ↔ frontend contract [V]
- **Transport abstraction** (`src/lib/transport/index.ts`): a `Transport` interface with `invoke` / `listen` / `subscribeRun`. At runtime it picks `TauriTransport` (wraps `@tauri-apps/api` `invoke` + `event.listen`) when `window.__TAURI_INTERNALS__` exists, else `WsTransport` (talks to the embedded axum server over WebSocket). **This is a clean seam** that lets the exact same frontend run as desktop or remote browser client. [V]
- Backend pushes domain events as a **`BusEvent` enum** persisted to JSONL and emitted both to the Tauri webview and broadcast to WS clients (`persist_and_emit`). [V]

---

## 3. Per-area reuse verdict

### Summary table

| Area | Verdict | One-phrase why |
|---|---|---|
| Desktop shell, window/layout, theming tokens | **REFERENCE** | clean Tailwind HSL-var token system + sidebar layout, but Tailwind v3→v4 forces a rewrite |
| Session/workspace model & lifecycle | **REFERENCE** | excellent actor-per-session ownership model; copy the *shape*, not the Claude/Codex-specific code |
| **PTY / terminal handling** | **REFERENCE (partial) / IGNORE for the core** | they use **stdio pipes + stream-JSON**, NOT a PTY; their xterm is an output mirror. Cowork needs a real PTY they don't have |
| Rust host architecture (actor, mailbox, BusEvent) | **REFERENCE** | the single-owner tokio-actor + mpsc mailbox pattern is the crown jewel; re-implement, don't lift |
| Svelte components / stores / runes | **REFERENCE** | solid runes patterns + transport abstraction; components are domain-coupled, lift selectively |
| Paraglide i18n setup | **IGNORE (they don't use it) / REFERENCE the i18n-check gate** | they hand-rolled i18n; we keep Paraglide. But their `i18n-check.mjs` gate is directly adaptable |
| Tauri config & capabilities | **LIFT (small) / REFERENCE** | minimal capabilities + Windows NSIS/WebView2 + CREATE_NO_WINDOW are copy-worthy |
| Build / CI / release tooling | **REFERENCE / LIFT (release.yml skeleton)** | `tauri-action` matrix release is a near drop-in; CI conformance gate is good reference |
| Transport abstraction (Tauri vs WS) | **REFERENCE (optional, high-value-later)** | not needed for v0.1 but a proven remote-access design if Cowork ever wants it |
| `process_ext.rs` Windows console hiding | **LIFT** | tiny, Windows-first, directly useful |

---

### 3a. Desktop shell & window/layout — **REFERENCE**
- `tailwind.config.ts` defines a **shadcn-style HSL CSS-variable token system** (`background`, `foreground`, `primary`, `sidebar.*`, `border`, `ring`, `--radius`) with `darkMode: "class"`. This token vocabulary is worth copying conceptually. [V]
- **Trap:** it's Tailwind **v3** config-file based. Cowork is **Tailwind v4** (CSS-first `@theme`, no `tailwind.config.ts`). You cannot lift the config file; re-express the same tokens in v4 `@theme` syntax. **REFERENCE the token names, not the file.**
- Layout is sidebar + main pane with route-based panes (`src/routes/*`). Standard; re-implement.

### 3b. Session/workspace model & lifecycle — **REFERENCE**
- A "run" (session) is owned end-to-end by one **`SessionActor`** (`src-tauri/src/agent/session_actor.rs`). Lifecycle: spawn child → actor `run()` select-loop owns stdin/stdout/stderr + mailbox → `cleanup()` on EOF/stop/cancel. [V]
- `SessionActorHandle` holds `cmd_tx` (mpsc), `run_id`, an `Arc<()>` identity tag (uses `Arc::ptr_eq` so cleanup doesn't clobber a replacement actor — a nice race-safety idiom), `JoinHandle`, and a `shutdown_rx` oneshot so callers can await actor exit before respawning. [V]
- Frontend `session-store.svelte.ts` (4284 lines) binds sidebar items to runs and replays `BusEvent`s. **Domain-coupled to Claude/Codex stream-json — do not lift wholesale; reference the store shape.** [V]
- **Reuse:** copy the *ownership discipline* (one actor = one process = one mailbox, no external locks) — this is exactly what Cowork wants for PTY-backed agent sessions. Re-implement against a PTY instead of pipes.

### 3c. PTY / terminal handling — **REFERENCE (partial), IGNORE for the core terminal** ⚠️ BIGGEST FINDING
- **OpenCovibe has NO real PTY.** [V]
  - `src-tauri/src/agent/stream.rs:158` and `control.rs:62`: `Command::new(...).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()`. Plain OS pipes.
  - No `portable-pty`, `conpty`, `nix::pty`, or `openpty` anywhere in the tree.
- **Their `XTerminal.svelte` is an OUTPUT MIRROR, not an interactive terminal.** [V]
  - In `src/routes/chat/+page.svelte:5248` it is mounted with only `onResize`/`onReady` — **no `onData` handler**, so `disableStdin` is true (see `XTerminal.svelte:45` `const hasInput = !!onDataProp`). Keystrokes are not forwarded; the agent runs in stream-JSON mode and the xterm just renders bytes pushed via `writeData()`.
  - `TerminalPane.svelte` is even further from a terminal: it `JSON.parse`s each stream line and renders **colored, line-numbered labeled rows** ([tool]/[reasoning]/[assistant]/[error]). Pure event renderer.
- **Why this matters for Cowork:** Cowork's thesis is bridging a CLI/TUI agent's **real PTY** to an interactive xterm.js (the agent runs in its native TUI, full ANSI, the user types into it). OpenCovibe deliberately runs agents in **headless stream-JSON mode** and reconstructs a pseudo-UI. These are fundamentally different I/O models.
- **What IS reusable here:**
  - The **`XTerminal.svelte` xterm bootstrap** (FitAddon, WebLinksAddon, ResizeObserver-with-debounce → `onResize(cols,rows)`, theme palette, scrollbar CSS, dynamic `import()` of xterm for code-splitting) is a clean, correct xterm wiring you can **REFERENCE** for the rendering half. Note it already exposes `onData` plumbing (`XTerminal.svelte:87`) — they just don't use it. Cowork would pass `onData` to forward keystrokes to the PTY. [V]
  - The **resize→cols/rows propagation** pattern is exactly what you need to send `SIGWINCH`/PTY resize.
- **What to IGNORE:** the stream-JSON parsing pipeline (`claude_stream.rs`, `codex_parser.rs`, `turn_engine.rs`, `TerminalPane.svelte` line rendering) — irrelevant to a raw-PTY passthrough.

### 3d. Rust host architecture — **REFERENCE (crown jewel)**
- **Actor pattern** (`session_actor.rs`): one tokio task per `run_id`; all mutations go through a bounded `mpsc` mailbox (`ActorCommand` enum with `oneshot` reply channels); a single `tokio::select!` loop multiplexes {mailbox, stdout lines, stderr lines, tick timer, external `CancellationToken`}. No external mutexes around session state. [V]
- **Two-phase control** (`SendControl`): the actor writes stdin + registers a waiter, then returns `(request_id, response_rx)` so the *caller* awaits the reply **outside** the select loop — avoids deadlocking the actor. Excellent idiom worth copying. [V]
- **Independent timeout clock + quarantine state machine** (`on_tick_timeout`): soft/hard deadlines, draining, quarantine→interrupt→kill escalation. Heavy but instructive for robust process supervision. [V]
- Command modules under `src-tauri/src/commands/` are thin `#[tauri::command]` wrappers that talk to the actor map + storage. Clean separation. [V]
- **Reuse:** REFERENCE the actor + mailbox + select-loop + two-phase-control shape for Cowork's PTY session supervisor. Do **not** lift — the bodies are saturated with Claude/Codex stream-json and Ralph-loop specifics Cowork doesn't need.

### 3e. Svelte components / stores / runes / Tailwind — **REFERENCE**
- Runes usage is idiomatic Svelte 5 (`$state`, `$props`, `$derived.by`, `$effect`) — good examples for Cowork's own runes code. [V]
- **Transport abstraction** (`src/lib/transport/`) is the standout reusable frontend pattern (see §2). REFERENCE.
- Components (`ChatMessage`, `InlineToolCard`, `MarkdownContent`, `CommandPalette`, `SetupWizard`) are mostly domain-coupled to the chat/stream model. `SetupWizard.svelte` and `CommandPalette.svelte` are the most generically reusable; reference them. The chat/tool-card components are IGNORE for Cowork.

### 3f. i18n — Paraglide: **IGNORE** (they don't use it); i18n-check gate: **REFERENCE/LIFT**
- Their `src/lib/i18n/index.svelte.ts` is a **self-built runtime**: flat `messages/<locale>.json`, `{var}` interpolation, fallback chain locale→en→raw-key, reactive `_locale` via `$state`, localStorage persistence (`ocv:locale`), **legacy `PARAGLIDE_LOCALE` migration** (confirming they came *from* Paraglide), navigator-language detection, async per-locale loaders for code-splitting, `applyHtmlAttrs` (sets `lang`/`dir`). [V]
- **Cowork uses Paraglide-JS v2, so the runtime itself is IGNORE.** But several *ideas* transfer:
  - `registry.ts` single-source-of-truth locale list (`code`/`nativeName`/`shortLabel`/`dir`/`status:stable|beta`) → good model for Cowork's locale selector + manifest locale map. **REFERENCE.**
  - `format.ts`: Intl-based `fmtNumber/fmtDate/fmtTime/fmtRelative` with NaN/Invalid-Date guards. **REFERENCE** (Paraglide doesn't give you these).
  - **`scripts/i18n-check.mjs`** is a CI quality gate that diffs every locale against `en.json` for (1) key alignment, (2) `{placeholder}` set consistency, (3) empty/untranslated detection with an allowlist + technical-value heuristic. Exit 1 on error. **This is directly adaptable to Cowork's Paraglide message files as a conformance gate** (Cowork's `ENGINEERING_PRINCIPLES.md` wants automated conformance gates). **LIFT/adapt.** [V]
  - `scripts/i18n-add-locale.mjs` exists as scaffolding for new languages. REFERENCE.

### 3g. Tauri config & capabilities — **LIFT (small parts) / REFERENCE**
- `capabilities/default.json` is a clean minimal permission set — **LIFT** the structure (adjust permissions to Cowork's plugins). [V]
- `tauri.conf.json` Windows section: NSIS installer + `webviewInstallMode: downloadBootstrapper` (WebView2 auto-install) is exactly what a Windows-first app wants. **LIFT** that block. [V]
- `process_ext.rs` — Windows `CREATE_NO_WINDOW` `HideConsole` trait for spawned `Command`s so child CLIs don't flash a console window. **LIFT** (tiny, Windows-first, directly useful when Cowork spawns processes on the host side). [V]

### 3h. Build / CI / release — **REFERENCE / LIFT (release.yml skeleton)**
- `.github/workflows/release.yml`: tag-triggered (`v*`), matrix over macOS-universal + Windows-msvc, uses **`tauri-apps/tauri-action@v0`**, `Swatinem/rust-cache`, zips the NSIS exe, uploads MSI, then a `publish` job aggregates artifacts via `softprops/action-gh-release`. **Near drop-in skeleton for Cowork's Windows release** (Cowork can drop the macOS leg). **LIFT/adapt.** [V]
  - Gap vs Cowork: this targets MS-Store-independent GitHub Releases already (matches Cowork's distribution memory), but does **not** build/ship a WSL rootfs — Cowork must add that.
- `package.json` `verify` script chains lint + format + svelte-check + i18n-check + test + build + rust fmt/clippy — a good **conformance-gate composition** to mirror. [V]
- `.githooks` + `prepare` script wires `core.hooksPath`. REFERENCE for commit-seal gating.

### 3i. Embedded web server / transport — **REFERENCE (optional, future)**
- `src-tauri/src/web_server/` (axum + ws + auth + broadcaster) lets the same backend serve a remote browser client; the frontend's `WsTransport` is the mirror. Not needed for Cowork v0.1 (foundation-first), but a proven design if Cowork ever wants remote/headless access. **REFERENCE, defer.**

---

## 4. License & attribution

- **SPDX: `Apache-2.0`** (root `LICENSE`, `package.json`, `Cargo.toml`). [V]
- **`NOTICE` file present** — Apache-2.0 §4(d) requires preserving NOTICE content in redistributions. [V]
- **No copyleft trap in their *own* code.** The only copyleft in the dependency tree is **MPL-2.0** (`cssparser`, `cssparser-macros`, `dtoa-short`, `selectors`, `option-ext`) — these are **transitive Rust deps pulled in by the WebView/CSS stack, file-level copyleft only**, and Cowork would pull the same crates via Tauri regardless. Not a concern. [V]
- **If Cowork LIFTs any OpenCovibe source** (e.g. `process_ext.rs`, `i18n-check.mjs`, `release.yml`, the `XTerminal.svelte` bootstrap, `capabilities/default.json`):
  - Apache-2.0 is **compatible** with Cowork redistribution.
  - You **must**: (a) retain the Apache-2.0 license + copyright notice on the copied portions, (b) state changes you made, (c) carry forward relevant `NOTICE` content. Practically: add an entry to Cowork's own `NOTICE`/attributions (e.g. "Portions adapted from OpenCovibe, © 2025-2026 OpenCovibe Contributors, Apache-2.0") and keep a license header on lifted files.
  - Most high-value items here are **REFERENCE** (re-implement from pattern), which carries **no attribution obligation** — only verbatim/substantial copying triggers it.

---

## 5. The API-key / LLM parts to explicitly avoid

The brief warned about "LLM-API-client / chat-bubble" code. In OpenCovibe this is **smaller than expected** because it wraps CLIs, but the parts to NOT drag into Cowork's PTY-passthrough model are:

- **Stream-JSON protocol layer** — `agent/claude_protocol.rs`, `claude_stream.rs`, `codex_appserver.rs`, `codex_parser.rs`, `codex_control.rs`, `pipe_parser.rs`, `session_protocol.rs`, `turn_engine.rs`. This is the entire "headless agent + reconstruct a chat UI" philosophy. Cowork runs the agent's **native TUI over a PTY**, so none of this applies. **AVOID.**
- **Chat/tool rendering** — `TerminalPane.svelte` (JSON→line renderer), `ChatMessage.svelte`, `InlineToolCard.svelte`, `tool-rendering.ts`. **AVOID** (Cowork's terminal is the agent's own TUI, not a re-rendered chat).
- **Provider/API-key management UI** — `commands/cli_config.rs`, `config/codex` routes, the Codex `model_providers.*` injection in `spawn.rs`. Cowork's memory says **no API keys** (subscription/native auth). **AVOID** — do not import the provider-credential model.
- **Ralph loop, auto-context, usage/pricing** (`pricing.rs`, `storage/*_usage.rs`, `RalphLoopState`) — product-specific, not Cowork's concern. **AVOID.**
- **`connect-src api.anthropic.com` CSP entry** — they reach Anthropic for usage data; Cowork should NOT inherit this CSP allowance.

---

## 6. Bottom line for Cowork

- **Highest-value reusable thing:** the **single-owner tokio-actor session model** (`session_actor.rs`) — one task owns one process + stdin/stdout + an mpsc mailbox with oneshot replies and a unified `select!` loop, plus the `Arc::ptr_eq` identity-tag cleanup and two-phase-control idiom. REFERENCE this shape for Cowork's PTY-backed agent supervisor; it is exactly the robustness layer Cowork's foundation-first v0.1 needs. (Runner-up, immediately liftable: `release.yml` + `i18n-check.mjs` + `process_ext.rs`.)
- **Biggest trap:** assuming their "terminal" is a PTY you can copy. **It is not** — OpenCovibe runs agents headless in stream-JSON over OS pipes and renders a reconstructed UI; there is **no PTY, no ConPTY, no portable-pty** in the codebase, and their xterm mount runs with `disableStdin`. Cowork's entire PTY↔xterm passthrough (the product's core) must be built from scratch; OpenCovibe gives you the xterm *rendering* bootstrap and the actor *supervision* shape, but **nothing for the actual PTY bridge**.
- **Secondary trap:** stack drift — they are Tailwind **v3** (config-file) and **self-rolled i18n** (not Paraglide). Don't lift `tailwind.config.ts` or the i18n runtime; re-express tokens in Tailwind v4 `@theme` and keep Paraglide.
