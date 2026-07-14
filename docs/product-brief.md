# Cowork — product brief

*A self-contained snapshot of what Cowork is and why. Written to be pasted into a fresh
session with no other context. Last revised 2026-07-13.*

---

## The product in one line

Cowork gives a non-developer a **safe, disposable computer** in which AI coding agents can
actually do work — and makes that computer appear, behave, and recover like an app.

## Who it is for

People who pay for an AI coding subscription (Claude Code, OpenAI Codex CLI, Google
Antigravity CLI) but are **not developers**. They want the agent to translate a PDF, draft a
proposal, reorganize their files — and they cannot be asked to install WSL, understand a
terminal, or reason about what an agent is allowed to touch.

Today the agents ship as CLIs. That is a wall these users never get over, and the ones who
climb it hand a capable agent full access to their real machine.

## The core bet

**The environment is the product.** Everyone else is building a nicer chat window around the
agent. The hard, unglamorous part is the box the agent runs in: a Linux environment that is
reproducible, isolated from the user's real files, observable, and undoable. Cowork ships
that box and hides it completely.

The agent's own interface is left alone — we host each vendor's real TUI in a styled
terminal rather than rebuilding it. Rebuilding it would break vendor neutrality the moment a
vendor ships a feature we have not cloned.

## Non-negotiables

1. **Vendor-neutral.** Whatever agent subscription you already have. Never our own model,
   never our own API key, no extra cost. A vendor lock-in shortcut is not worth taking.
2. **The user never learns Git, WSL, Linux, or a package manager.** If a concept leaks into
   the UI, that is a bug. Undo is "undo", not `git checkout`.
3. **Foundations before features.** One goal per version. Isolation, recovery, and
   observability are sequenced *before* the feature pile, not bolted on after.
4. **Global-first.** English is the base language; Japanese and Korean ship alongside it from
   day one, not as an afterthought.
5. **Never destroy the user's work.** Undo restores; it never deletes. Anything the user put
   in their files stays.

## How it works (the shape, not the details)

- A **desktop app** (Tauri) — not a browser tab, not a CLI. The whole point is that a
  non-developer can double-click it.
- The app provisions a **Linux environment** the user never sees: WSL2 on Windows, a VM on
  macOS. Same image, same guest, different transport.
- Work happens in a **workspace** — a fresh, named context, created as casually as a new
  chat. The agent runs there with access to that workspace and nothing else.
- Files move in and out through a **folder the user opens in Explorer/Finder**. No host
  filesystem is mounted into the environment.
- Every workspace is **versioned invisibly**, so "undo" means undo — restore the state from
  before you asked for that thing.

## What it is not

- Not an IDE, not a code editor, not a chat client.
- Not a model, not an API reseller, not a proxy.
- Not a devtool. If the answer to a UX problem is "the user should learn X", it is the wrong
  answer.

## Status (2026-07)

Windows-first. Setup (v0.1) and Workspace (v0.2) are built and in final hardware validation;
Isolation is next. Nothing has shipped publicly yet — there is no user, no migration, no
backward compatibility to preserve.

Roadmap, one goal per version: **Setup → Workspace → Isolation → Recovery → Community →
Observability.**

## Strategic context

The founder is a solo operator targeting the global market, using this product as the
vehicle for a Japan Startup Visa. Positioning, naming, and copy should read as
internationally native — not as a localized Korean or Japanese product.

---

## The name

"Cowork" is the working title, not a decision. Whatever the name is, it has to carry: an
environment where you and an agent work together, safety by construction, and approachability
for someone who is not a programmer. It must work in English first and be pronounceable for
Japanese and Korean speakers.
