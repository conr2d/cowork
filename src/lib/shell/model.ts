import type { WorkspaceDto } from '$lib/host/types';
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

/** Preset catalog for the create dialog. */
export const PRESETS: readonly { id: 'blank' | 'pdf' | 'proposal' }[] = [
	{ id: 'blank' },
	{ id: 'pdf' },
	{ id: 'proposal' }
];
