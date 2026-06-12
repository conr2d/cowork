import { describe, expect, it } from 'vitest';

import type { WorkspaceDto } from '$lib/host/types';
import { agentBinary, brand, initialSlug, nextOpenSlugs, pinnedOf, recentOf } from './model';

function workspace(
	partial: Partial<WorkspaceDto> & Pick<WorkspaceDto, 'name' | 'slug'>
): WorkspaceDto {
	return {
		name: partial.name,
		slug: partial.slug,
		createdMs: partial.createdMs ?? 0,
		pinned: partial.pinned ?? false,
		pinOrder: partial.pinOrder,
		lastUsedMs: partial.lastUsedMs ?? 0,
		defaultAgent: partial.defaultAgent ?? 'claude',
		preset: partial.preset ?? 'blank',
		sessions: partial.sessions ?? []
	};
}

describe('brand', () => {
	it('maps internal ids to product brands', () => {
		expect(brand('claude')).toBe('Claude');
		expect(brand('codex')).toBe('ChatGPT');
		expect(brand('antigravity')).toBe('Gemini');
	});
});

describe('agentBinary', () => {
	it('maps internal ids to guest binaries', () => {
		expect(agentBinary('claude')).toBe('claude');
		expect(agentBinary('codex')).toBe('codex');
		expect(agentBinary('antigravity')).toBe('agy');
	});
});

describe('pinnedOf', () => {
	it('orders pinned workspaces by pinOrder with nulls last and ties by name', () => {
		const list = [
			workspace({ name: 'Zulu', slug: 'zulu', pinned: true, pinOrder: null }),
			workspace({ name: 'Alpha', slug: 'alpha', pinned: true, pinOrder: 2 }),
			workspace({ name: 'Recent', slug: 'recent', pinned: false }),
			workspace({ name: 'Beta', slug: 'beta', pinned: true, pinOrder: 2 }),
			workspace({ name: 'First', slug: 'first', pinned: true, pinOrder: 0 })
		];

		expect(pinnedOf(list).map((item) => item.slug)).toEqual(['first', 'alpha', 'beta', 'zulu']);
	});
});

describe('recentOf', () => {
	it('orders unpinned workspaces by last used time with ties by name', () => {
		const list = [
			workspace({ name: 'Pinned', slug: 'pinned', pinned: true, lastUsedMs: 9 }),
			workspace({ name: 'Zulu', slug: 'zulu', lastUsedMs: 10 }),
			workspace({ name: 'Alpha', slug: 'alpha', lastUsedMs: 10 }),
			workspace({ name: 'Old', slug: 'old', lastUsedMs: 1 })
		];

		expect(recentOf(list).map((item) => item.slug)).toEqual(['alpha', 'zulu', 'old']);
	});
});

describe('initialSlug', () => {
	it('returns null for an empty list', () => {
		expect(initialSlug([])).toBeNull();
	});

	it('returns the slug with the highest lastUsedMs', () => {
		const list = [
			workspace({ name: 'Old', slug: 'old', lastUsedMs: 1 }),
			workspace({ name: 'Pinned new', slug: 'pinned-new', pinned: true, lastUsedMs: 9 }),
			workspace({ name: 'Recent', slug: 'recent', lastUsedMs: 5 })
		];

		expect(initialSlug(list)).toBe('pinned-new');
	});
});

describe('nextOpenSlugs', () => {
	it('appends the active slug on first visit', () => {
		expect(nextOpenSlugs([], 'a', ['a'])).toEqual(['a']);
	});

	it('does not duplicate an already-open slug and preserves order', () => {
		expect(nextOpenSlugs(['a', 'b'], 'a', ['a', 'b'])).toEqual(['a', 'b']);
	});

	it("prunes a deleted workspace's slug", () => {
		expect(nextOpenSlugs(['a', 'b'], 'a', ['a'])).toEqual(['a']);
	});

	it('only prunes when active is null', () => {
		expect(nextOpenSlugs(['a'], null, [])).toEqual([]);
	});

	it('does not append an active slug missing from existing workspaces', () => {
		expect(nextOpenSlugs([], 'ghost', ['a'])).toEqual([]);
	});
});
