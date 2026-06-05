// The agents the wizard offers to install, and pure helpers for the selection
// set. No runes here — this module is unit-tested directly.

import type { AgentId } from '$lib/terminal/login';

export interface AgentChoice {
	/** The agent's stable id (matches the guest CLI and `loginCommand`). */
	id: AgentId;
	/** Brand name shown in the UI — a proper noun, intentionally not translated. */
	name: string;
}

/** The installable agents, in display order. */
export const AGENT_CHOICES: readonly AgentChoice[] = [
	{ id: 'claude', name: 'Claude Code' },
	{ id: 'codex', name: 'Codex' },
	{ id: 'antigravity', name: 'Antigravity' }
] as const;

const AGENT_IDS: ReadonlySet<AgentId> = new Set(AGENT_CHOICES.map((c) => c.id));

/** Toggle `id` in `selected`, returning a new array (input is never mutated). */
export function toggleAgent(selected: readonly AgentId[], id: AgentId): AgentId[] {
	return selected.includes(id) ? selected.filter((a) => a !== id) : [...selected, id];
}

/** The wizard requires at least one agent before leaving the selection step. */
export function isValidSelection(selected: readonly AgentId[]): boolean {
	return selected.length >= 1;
}

/** Keep only known agent ids (resume state arrives as plain strings). */
export function parseAgentIds(values: readonly string[]): AgentId[] {
	return values.filter((v): v is AgentId => AGENT_IDS.has(v as AgentId));
}
