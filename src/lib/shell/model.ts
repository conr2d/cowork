import type { SessionDto, WorkspaceDto } from '$lib/host/types';
import type { AgentId } from '$lib/terminal/login';

/** Product brand shown in chrome (non-devs know these, not CLI names). */
export function brand(agent: AgentId): string {
	switch (agent) {
		case 'claude':
			return 'Claude';
		case 'codex':
			return 'ChatGPT';
		case 'antigravity':
			return 'Gemini';
	}
}

/** The guest binary an agent session runs. */
export function agentBinary(agent: AgentId): string {
	switch (agent) {
		case 'claude':
			return 'claude';
		case 'codex':
			return 'codex';
		case 'antigravity':
			return 'agy';
	}
}

/** Pinned group: pinned=true sorted by pinOrder asc (null/undefined last, ties by name). */
export function pinnedOf(list: readonly WorkspaceDto[]): WorkspaceDto[] {
	return list
		.filter((workspace) => workspace.pinned)
		.toSorted((a, b) => {
			const left = a.pinOrder ?? Number.POSITIVE_INFINITY;
			const right = b.pinOrder ?? Number.POSITIVE_INFINITY;
			return left - right || a.name.localeCompare(b.name);
		});
}

/** Recent group: pinned=false sorted by lastUsedMs desc (ties by name). */
export function recentOf(list: readonly WorkspaceDto[]): WorkspaceDto[] {
	return list
		.filter((workspace) => !workspace.pinned)
		.toSorted((a, b) => b.lastUsedMs - a.lastUsedMs || a.name.localeCompare(b.name));
}

/** Boot selection: slug of the workspace with the highest lastUsedMs (pinned or not); null if empty. */
export function initialSlug(list: readonly WorkspaceDto[]): string | null {
	return (
		list.toSorted((a, b) => b.lastUsedMs - a.lastUsedMs || a.name.localeCompare(b.name))[0]?.slug ??
		null
	);
}

/** One mounted terminal: a session of a workspace. */
export interface SessionRef {
	slug: string;
	sessionId: string;
}

/** Tab status: cold = not spawned this app run; done = its PTY exited. */
export type SessionStatus = 'cold' | 'working' | 'idle' | 'done';

/** Tabs render in stored order (stable, frontend-owned). */
export function sortedSessions(sessions: readonly SessionDto[]): SessionDto[] {
	return sessions.toSorted((a, b) => a.order - b.order);
}

/** Next order value: max existing + 1 (0 for the first session). */
export function nextSessionOrder(sessions: readonly SessionDto[]): number {
	return Math.max(-1, ...sessions.map((session) => session.order)) + 1;
}

/** Default tab title: brand + per-agent ordinal ("Claude 2", "ChatGPT 1"). */
export function nextSessionTitle(sessions: readonly SessionDto[], agent: AgentId): string {
	const count = sessions.filter((session) => session.agent === agent).length;
	return `${brand(agent)} ${count + 1}`;
}

/**
 * The command autorun at PTY spawn for a session. Fresh claude sessions pin a
 * host-generated UUID (--session-id) so a later restore can resume; codex and
 * antigravity UUIDs are captured lazily after first activity, so a fresh spawn
 * is bare and only a restore resumes. A restored session whose conversation
 * never materialized surfaces the agent's own error — accepted; the user opens
 * a new tab.
 */
export function sessionLaunch(agent: AgentId, uuid: string | null, restore: boolean): string {
	if (uuid === null) return agentBinary(agent);
	switch (agent) {
		case 'claude':
			return restore ? `claude --resume ${uuid}` : `claude --session-id ${uuid}`;
		case 'codex':
			return restore ? `codex resume ${uuid}` : agentBinary(agent);
		case 'antigravity':
			return restore ? `agy --conversation ${uuid}` : agentBinary(agent);
	}
}

/**
 * Mounted terminals survive workspace switches (WP4b); this prunes refs whose
 * session or whole workspace no longer exists (close/delete → unmount → the
 * terminal's own cleanup kills its PTY). Appending is the session manager's
 * job. Order is mount order; it never reshuffles.
 */
export function pruneOpen(
	open: readonly SessionRef[],
	workspaces: readonly WorkspaceDto[]
): SessionRef[] {
	return open.filter((ref) =>
		workspaces.some(
			(workspace) =>
				workspace.slug === ref.slug &&
				workspace.sessions.some((session) => session.id === ref.sessionId)
		)
	);
}

/** Preset catalog for the create dialog. */
export const PRESETS: readonly { id: 'blank' | 'pdf' | 'proposal' }[] = [
	{ id: 'blank' },
	{ id: 'pdf' },
	{ id: 'proposal' }
];
