import { untrack } from 'svelte';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';

import type { HostClient } from '$lib/host/client';
import type { WorkspaceDto } from '$lib/host/types';
import type { AgentId } from '$lib/terminal/agent';
import {
	nextSessionOrder,
	nextSessionTitle,
	pruneOpen,
	resolveSessionLaunch,
	type SessionLaunchPlan,
	type SessionRef,
	type SessionStatus,
	sortedSessions
} from './model';
import type { Shell } from './store.svelte';

/** Output quieter than this marks a working session idle again. */
const WORKING_QUIET_MS = 2000;
/** Guards host↔guest clock skew when comparing spawn time against guest file mtimes. */
const CAPTURE_SKEW_MS = 5000;

export interface SessionManager {
	/** Mounted terminals, in mount order (one per spawned session). */
	readonly open: readonly SessionRef[];
	readonly statuses: Readonly<Record<string, SessionStatus>>;
	/** Workspace slug an undismissed concurrent-session advisory points at. */
	readonly advisorySlug: string | null;
	activeOf(slug: string): string | null;
	launchPlan(sessionId: string, fallbackUuid: string | null): SessionLaunchPlan;
	/** Workspace became active: create its first session or activate its current tab. */
	ensureActive(workspace: WorkspaceDto): Promise<void>;
	/** ⊕ / picker: create + persist + activate. agent=null → the workspace default. */
	create(workspace: WorkspaceDto, agent: AgentId | null): Promise<void>;
	activate(workspace: WorkspaceDto, sessionId: string): Promise<void>;
	/** Close a tab: unmount (kills its PTY), persist removal, keep a live tab. */
	close(workspace: WorkspaceDto, sessionId: string): Promise<void>;
	/** Drop refs whose session/workspace no longer exists. */
	prune(workspaces: readonly WorkspaceDto[]): void;
	noteSpawn(sessionId: string): void;
	noteActivity(sessionId: string, event: 'output' | 'exit'): void;
	dismissAdvisory(): void;
}

export function createSessionManager(
	shell: Shell,
	host: HostClient,
	theme: () => 'light' | 'dark'
): SessionManager {
	let open = $state<SessionRef[]>([]);
	const statuses = $state<Record<string, SessionStatus>>({});
	let advisorySlug = $state<string | null>(null);
	const activeBySlug = $state<Record<string, string>>({});
	/** Sessions created in THIS app run (their first spawn is not a resume). */
	const fresh = new SvelteSet<string>();
	const timers = new SvelteMap<string, ReturnType<typeof setTimeout>>();
	const spawnedAt = new SvelteMap<string, number>();
	const capturing = new SvelteSet<string>();
	const launchPlans = new SvelteMap<string, SessionLaunchPlan>();
	/**
	 * Sessions whose `activate` is in flight. `activate` awaits (the claude
	 * session-existence probe, the agy theme sync) between deciding to mount and
	 * pushing to `open`, and the shell's effect can call `ensureActive` again
	 * inside that window — both calls would pass the membership check and push,
	 * giving `{#each ... (ref.sessionId)}` a duplicate key. This guard is set
	 * SYNCHRONOUSLY, before the first await, so the second caller bails.
	 */
	const mounting = new SvelteSet<string>();

	function clearTimer(sessionId: string): void {
		const timer = timers.get(sessionId);
		if (timer !== undefined) clearTimeout(timer);
		timers.delete(sessionId);
	}

	function resolveSession(slug: string, sessionId: string) {
		const workspace = shell.workspaces.find((item) => item.slug === slug);
		const session = workspace?.sessions.find((item) => item.id === sessionId);
		return { workspace, session };
	}

	async function maybeCapture(sessionId: string): Promise<void> {
		const ref = open.find((item) => item.sessionId === sessionId);
		if (!ref) return;
		const workspace = shell.workspaces.find((item) => item.slug === ref.slug);
		const session = workspace?.sessions.find((item) => item.id === sessionId);
		if (!workspace || !session) return;
		if (session.agent === 'claude' || session.agentSessionUuid || capturing.has(sessionId)) {
			return;
		}
		capturing.add(sessionId);
		try {
			const since = Math.max(0, (spawnedAt.get(sessionId) ?? 0) - CAPTURE_SKEW_MS);
			const uuid = await host.captureSessionUuid(session.agent, workspace.slug, since);
			if (uuid === null) return;
			// Re-resolve after the await: the workspace object may have been replaced.
			const current = shell.workspaces.find((item) => item.slug === ref.slug);
			if (!current?.sessions.some((item) => item.id === sessionId)) return;
			await shell.updateSessions(
				current.slug,
				current.sessions.map((item) =>
					item.id === sessionId ? { ...item, agentSessionUuid: uuid } : item
				)
			);
		} catch {
			// Best-effort: a failed capture retries on the next idle/exit transition.
		} finally {
			capturing.delete(sessionId);
		}
	}

	async function activate(workspace: WorkspaceDto, sessionId: string): Promise<void> {
		activeBySlug[workspace.slug] = sessionId;
		if (workspace.activeSessionId !== sessionId) {
			void shell.setActiveSession(workspace.slug, sessionId);
		}
		statuses[sessionId] ??= 'cold';
		if (open.some((ref) => ref.sessionId === sessionId) || mounting.has(sessionId)) return;
		mounting.add(sessionId);
		try {
			let session = workspace.sessions.find((item) => item.id === sessionId);
			const launchPlan = session
				? await resolveSessionLaunch(
						host,
						workspace.slug,
						session,
						!fresh.has(sessionId),
						async (uuid) => {
							const current = resolveSession(workspace.slug, sessionId);
							if (!current.workspace || !current.session) return;
							await shell.updateSessions(
								current.workspace.slug,
								current.workspace.sessions.map((item) =>
									item.id === sessionId ? { ...item, agentSessionUuid: uuid } : item
								)
							);
							session = resolveSession(workspace.slug, sessionId).session ?? session;
						}
					)
				: { uuid: null, resume: false };
			launchPlans.set(sessionId, launchPlan);
			if (session?.agent === 'antigravity') {
				try {
					await host.agentThemeSync(theme());
				} catch {
					// Best-effort: a failed sync still mounts; the scheme may be stale.
				}
			}
			// Re-check after the awaits: the session may have been deleted meanwhile,
			// and a concurrent activate may already have mounted it.
			const still = resolveSession(workspace.slug, sessionId).session;
			if (still && !open.some((ref) => ref.sessionId === sessionId)) {
				open.push({ slug: workspace.slug, sessionId });
			}
		} finally {
			mounting.delete(sessionId);
		}
	}

	async function create(workspace: WorkspaceDto, agent: AgentId | null): Promise<void> {
		const chosen = agent ?? workspace.defaultAgent;
		const live = open.some(
			(ref) =>
				ref.slug === workspace.slug &&
				(statuses[ref.sessionId] === 'working' || statuses[ref.sessionId] === 'idle')
		);
		if (live) advisorySlug = workspace.slug;
		const id = crypto.randomUUID();
		await shell.updateSessions(workspace.slug, [
			...workspace.sessions,
			{
				id,
				agent: chosen,
				agentSessionUuid: chosen === 'claude' ? crypto.randomUUID() : null,
				title: nextSessionTitle(workspace.sessions, chosen),
				order: nextSessionOrder(workspace.sessions)
			}
		]);
		if (agent !== null && agent !== workspace.defaultAgent) {
			await shell.setDefaultAgent(workspace.slug, agent);
		}
		// Mount only what actually persisted (updateSessions reports failure via
		// shell.error; the tab must not exist locally if the host rejected it).
		const persisted = shell.workspaces.find((item) => item.slug === workspace.slug);
		if (!persisted || !persisted.sessions.some((session) => session.id === id)) return;
		fresh.add(id);
		await activate(persisted, id);
	}

	return {
		get open() {
			return open;
		},
		get statuses() {
			return statuses;
		},
		get advisorySlug() {
			return advisorySlug;
		},
		activeOf(slug) {
			return activeBySlug[slug] ?? null;
		},
		launchPlan(sessionId, fallbackUuid) {
			// `activate` sets the plan before pushing to `open`, so a render should
			// always find one. If it somehow does not, resume rather than guess a new
			// session: with a stored uuid, `resume: false` makes codex and agy silently
			// open a NEW conversation (they only pass the uuid when resuming) and makes
			// claude spawn `--session-id`, which is fatal when the conversation exists.
			// Resuming a conversation that is not there merely prints the agent's own
			// error. Fail loudly, never lose history quietly.
			return launchPlans.get(sessionId) ?? { uuid: fallbackUuid, resume: fallbackUuid !== null };
		},
		async ensureActive(workspace) {
			if (workspace.sessions.length === 0) {
				await create(workspace, null);
				return;
			}
			const current = activeBySlug[workspace.slug] ?? workspace.activeSessionId ?? undefined;
			const target = workspace.sessions.some((session) => session.id === current)
				? current
				: sortedSessions(workspace.sessions)[0].id;
			await activate(workspace, target);
		},
		create,
		activate,
		async close(workspace, sessionId) {
			open = open.filter((ref) => ref.sessionId !== sessionId);
			fresh.delete(sessionId);
			spawnedAt.delete(sessionId);
			capturing.delete(sessionId);
			launchPlans.delete(sessionId);
			clearTimer(sessionId);
			delete statuses[sessionId];
			await shell.updateSessions(
				workspace.slug,
				workspace.sessions.filter((session) => session.id !== sessionId)
			);
			const persisted = shell.workspaces.find((item) => item.slug === workspace.slug);
			if (!persisted) return;
			if (persisted.sessions.length === 0) {
				// The terminal area never goes empty while a workspace is active.
				await create(persisted, null);
				return;
			}
			if (activeBySlug[workspace.slug] === sessionId) {
				await activate(persisted, sortedSessions(persisted.sessions)[0].id);
			}
		},
		prune(workspaces) {
			// prune runs inside an effect that tracks `workspaces`. Every read of
			// `open` here must be untracked, including the one below: pruneOpen
			// returns a NEW array each call, so a tracked read of what we just wrote
			// would re-trigger this effect forever (effect_update_depth_exceeded).
			// Hence `next`, the plain array — reading it is not reading the signal.
			const next = pruneOpen(
				untrack(() => open),
				workspaces
			);
			open = next;
			for (const sessionId of launchPlans.keys()) {
				if (!next.some((ref) => ref.sessionId === sessionId)) launchPlans.delete(sessionId);
			}
		},
		noteSpawn(sessionId) {
			spawnedAt.set(sessionId, Date.now());
			statuses[sessionId] = 'idle';
		},
		noteActivity(sessionId, event) {
			if (event === 'exit') {
				clearTimer(sessionId);
				statuses[sessionId] = 'done';
				void maybeCapture(sessionId);
				return;
			}
			// Burst start (was not already 'working') = the workspace is being worked in.
			// Bump recency ONCE per burst — never per output chunk (this fires on every
			// PTY chunk), and never on mere selection (that path no longer touches
			// lastUsedMs). This is issue #32: Recent tracks activity, not clicks.
			if (statuses[sessionId] !== 'working') {
				const slug = open.find((ref) => ref.sessionId === sessionId)?.slug;
				if (slug !== undefined) void shell.bumpLastUsed(slug);
			}
			statuses[sessionId] = 'working';
			clearTimer(sessionId);
			timers.set(
				sessionId,
				setTimeout(() => {
					timers.delete(sessionId);
					if (statuses[sessionId] === 'working') {
						statuses[sessionId] = 'idle';
						void maybeCapture(sessionId);
					}
				}, WORKING_QUIET_MS)
			);
		},
		dismissAdvisory() {
			advisorySlug = null;
		}
	};
}
