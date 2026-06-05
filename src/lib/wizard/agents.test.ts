import { describe, expect, it } from 'vitest';

import { AGENT_CHOICES, isValidSelection, parseAgentIds, toggleAgent } from './agents';

describe('toggleAgent', () => {
	it('adds an unselected agent to the end', () => {
		expect(toggleAgent(['claude'], 'codex')).toEqual(['claude', 'codex']);
	});

	it('removes an already-selected agent', () => {
		expect(toggleAgent(['claude', 'codex'], 'claude')).toEqual(['codex']);
	});

	it('does not mutate the input', () => {
		const input = ['claude'] as const;
		toggleAgent(input, 'codex');
		expect(input).toEqual(['claude']);
	});
});

describe('isValidSelection', () => {
	it('requires at least one agent', () => {
		expect(isValidSelection([])).toBe(false);
		expect(isValidSelection(['codex'])).toBe(true);
	});
});

describe('parseAgentIds', () => {
	it('keeps known ids in order and drops unknown values', () => {
		expect(parseAgentIds(['codex', 'bogus', 'claude'])).toEqual(['codex', 'claude']);
	});
});

describe('AGENT_CHOICES', () => {
	it('lists the three installable agents in order', () => {
		expect(AGENT_CHOICES.map((c) => c.id)).toEqual(['claude', 'codex', 'antigravity']);
	});
});
