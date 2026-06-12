import { untrack } from 'svelte';
import { SvelteMap, SvelteSet } from 'svelte/reactivity';

import type { WorkspaceDto } from '$lib/host/types';
import type { AgentId } from '$lib/terminal/login';
import {
	nextSessionOrder,
	nextSessionTitle,
	pruneOpen,
	type SessionRef,
	type SessionStatus,
	sortedSessions
} from './model';
import type { Shell } from './store.svelte';

/** Output quieter than this marks a working session idle again. */
const WORKING_QUIET_MS = 2000;

export interface SessionManager {
	/** Mounted terminals, in mount order (one per spawned session). */
	readonly open: readonly SessionRef[];
	readonly statuses: Readonly<Record<string, SessionStatus>>;
	/** Workspace slug an undismissed concurrent-session advisory points at. */
	readonly advisorySlug: string | null;
	activeOf(slug: string): string | null;
	/** Restored sessions (not created this app run) spawn with the agent's resume. */
	isRestore(sessionId: string): boolean;
	/** Workspace became active: create its first session or activate its current tab. */
	ensureActive(workspace: WorkspaceDto): Promise<void>;
	/** ⊕ / picker: create + persist + activate. agent=null → the workspace default. */
	create(workspace: WorkspaceDto, agent: AgentId | null): Promise<void>;
	activate(workspace: WorkspaceDto, sessionId: string): void;
	/** Close a tab: unmount (kills its PTY), persist removal, keep a live tab. */
	close(workspace: WorkspaceDto, sessionId: string): Promise<void>;
	/** Drop refs whose session/workspace no longer exists. */
	prune(workspaces: readonly WorkspaceDto[]): void;
	noteSpawn(sessionId: string): void;
	noteActivity(sessionId: string, event: 'output' | 'exit'): void;
	dismissAdvisory(): void;
}

export function createSessionManager(shell: Shell): SessionManager {
	let open = $state<SessionRef[]>([]);
	const statuses = $state<Record<string, SessionStatus>>({});
	let advisorySlug = $state<string | null>(null);
	const activeBySlug = $state<Record<string, string>>({});
	/** Sessions created in THIS app run (their first spawn is not a resume). */
	const fresh = new SvelteSet<string>();
	const timers = new SvelteMap<string, ReturnType<typeof setTimeout>>();

	function clearTimer(sessionId: string): void {
		const timer = timers.get(sessionId);
		if (timer !== undefined) clearTimeout(timer);
		timers.delete(sessionId);
	}

	function activate(workspace: WorkspaceDto, sessionId: string): void {
		activeBySlug[workspace.slug] = sessionId;
		statuses[sessionId] ??= 'cold';
		if (!open.some((ref) => ref.sessionId === sessionId)) {
			open.push({ slug: workspace.slug, sessionId });
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
		activate(persisted, id);
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
		isRestore(sessionId) {
			return !fresh.has(sessionId);
		},
		async ensureActive(workspace) {
			if (workspace.sessions.length === 0) {
				await create(workspace, null);
				return;
			}
			const current = activeBySlug[workspace.slug];
			const target = workspace.sessions.some((session) => session.id === current)
				? current
				: sortedSessions(workspace.sessions)[0].id;
			activate(workspace, target);
		},
		create,
		activate,
		async close(workspace, sessionId) {
			open = open.filter((ref) => ref.sessionId !== sessionId);
			fresh.delete(sessionId);
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
				activate(persisted, sortedSessions(persisted.sessions)[0].id);
			}
		},
		prune(workspaces) {
			// untrack: prune is called from an effect tracking `workspaces`; reading
			// `open` tracked would re-trigger on our own write and loop.
			open = pruneOpen(
				untrack(() => open),
				workspaces
			);
		},
		noteSpawn(sessionId) {
			statuses[sessionId] = 'idle';
		},
		noteActivity(sessionId, event) {
			if (event === 'exit') {
				clearTimer(sessionId);
				statuses[sessionId] = 'done';
				return;
			}
			statuses[sessionId] = 'working';
			clearTimer(sessionId);
			timers.set(
				sessionId,
				setTimeout(() => {
					timers.delete(sessionId);
					if (statuses[sessionId] === 'working') statuses[sessionId] = 'idle';
				}, WORKING_QUIET_MS)
			);
		},
		dismissAdvisory() {
			advisorySlug = null;
		}
	};
}
