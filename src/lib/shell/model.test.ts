import { describe, expect, it, vi } from 'vitest';

import type { SessionDto, WorkspaceDto } from '$lib/host/types';
import { createMockHost } from '$lib/host/mock';
import {
	agentBinary,
	brand,
	initialSlug,
	nextSessionOrder,
	nextSessionTitle,
	pinnedOf,
	pruneOpen,
	resolveSessionLaunch,
	recentOf,
	sessionLaunch,
	sortedSessions
} from './model';

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

function session(partial: Partial<SessionDto> & Pick<SessionDto, 'id'>): SessionDto {
	return { agent: 'claude', agentSessionUuid: undefined, title: 't', order: 0, ...partial };
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

describe('sortedSessions', () => {
	it('returns sessions ordered by order ascending', () => {
		const sessions = [
			session({ id: 'third', order: 3 }),
			session({ id: 'first', order: 1 }),
			session({ id: 'second', order: 2 })
		];

		expect(sortedSessions(sessions).map((item) => item.id)).toEqual(['first', 'second', 'third']);
	});
});

describe('nextSessionOrder', () => {
	it('returns 0 for an empty list', () => {
		expect(nextSessionOrder([])).toBe(0);
	});

	it('returns max order plus one', () => {
		expect(nextSessionOrder([session({ id: 'a', order: 0 }), session({ id: 'b', order: 2 })])).toBe(
			3
		);
	});
});

describe('nextSessionTitle', () => {
	it('returns Claude 1 when there are no sessions', () => {
		expect(nextSessionTitle([], 'claude')).toBe('Claude 1');
	});

	it('counts existing sessions for the same agent', () => {
		expect(nextSessionTitle([session({ id: 'c1', agent: 'codex' })], 'codex')).toBe('ChatGPT 2');
	});

	it('ignores other agents when numbering', () => {
		expect(nextSessionTitle([session({ id: 's1', agent: 'claude' })], 'antigravity')).toBe(
			'Gemini 1'
		);
	});
});

describe('sessionLaunch', () => {
	it('launches fresh claude sessions with a fixed session id', () => {
		expect(sessionLaunch('claude', 'u1', false)).toBe('claude --session-id u1');
	});

	it('resumes restored claude sessions', () => {
		expect(sessionLaunch('claude', 'u1', true)).toBe('claude --resume u1');
	});

	it('launches bare claude when no uuid exists', () => {
		expect(sessionLaunch('claude', null, true)).toBe('claude');
	});

	it('resumes restored codex sessions', () => {
		expect(sessionLaunch('codex', 'u1', true)).toBe('codex resume u1');
	});

	it('launches fresh codex sessions bare', () => {
		expect(sessionLaunch('codex', 'u1', false)).toBe('codex');
	});

	it('launches codex bare when no uuid exists', () => {
		expect(sessionLaunch('codex', null, true)).toBe('codex');
	});

	it('resumes restored antigravity sessions', () => {
		expect(sessionLaunch('antigravity', 'u1', true)).toBe('agy --conversation u1');
	});

	it('launches fresh antigravity sessions bare when no uuid exists', () => {
		expect(sessionLaunch('antigravity', null, false)).toBe('agy');
	});
});

describe('resolveSessionLaunch', () => {
	it('resumes a restored claude session when the conversation exists', async () => {
		const host = createMockHost({ sessionChecks: { claude: true } });
		const restored = session({
			id: 's1',
			agentSessionUuid: '550e8400-e29b-41d4-a716-446655440000'
		});
		let persisted: string | null = null;

		await expect(
			resolveSessionLaunch(host, 'demo', restored, true, async (uuid) => {
				persisted = uuid;
			})
		).resolves.toEqual({
			uuid: '550e8400-e29b-41d4-a716-446655440000',
			resume: true
		});
		expect(persisted).toBeNull();
	});

	it('re-pins a restored claude session id when the conversation is missing', async () => {
		const host = createMockHost({ sessionChecks: { claude: false } });
		const restored = session({
			id: 's1',
			agentSessionUuid: '550e8400-e29b-41d4-a716-446655440000'
		});
		let persisted: string | null = null;

		await expect(
			resolveSessionLaunch(host, 'demo', restored, true, async (uuid) => {
				persisted = uuid;
			})
		).resolves.toEqual({
			uuid: '550e8400-e29b-41d4-a716-446655440000',
			resume: false
		});
		expect(persisted).toBeNull();
	});

	it('mints and persists a claude session id when the stored uuid is absent', async () => {
		const host = createMockHost();
		const broken = session({ id: 's1' });
		let persisted: string | null = null;
		const randomUuid = vi
			.spyOn(globalThis.crypto, 'randomUUID')
			.mockReturnValue('550e8400-e29b-41d4-a716-446655440001');

		try {
			await expect(
				resolveSessionLaunch(host, 'demo', broken, true, async (uuid) => {
					persisted = uuid;
				})
			).resolves.toEqual({
				uuid: '550e8400-e29b-41d4-a716-446655440001',
				resume: false
			});
			expect(persisted).toBe('550e8400-e29b-41d4-a716-446655440001');
		} finally {
			randomUuid.mockRestore();
		}
	});

	it('falls back to the stored claude uuid when the probe fails', async () => {
		const envelope = { code: 'session.uuid_capture_failed', kind: 'Internal', stage: 'workspace' };
		const host = createMockHost({ failWith: { sessionCheck: envelope } });
		const restored = session({
			id: 's1',
			agentSessionUuid: '550e8400-e29b-41d4-a716-446655440000'
		});
		let persisted: string | null = null;

		await expect(
			resolveSessionLaunch(host, 'demo', restored, true, async (uuid) => {
				persisted = uuid;
			})
		).resolves.toEqual({
			uuid: '550e8400-e29b-41d4-a716-446655440000',
			resume: true
		});
		expect(persisted).toBeNull();
	});

	it('heals a restored antigravity session from agy index when the stored uuid is absent', async () => {
		const host = createMockHost({ sessionUuids: { antigravity: 'u1' } });
		const restored = session({ id: 's1', agent: 'antigravity', agentSessionUuid: null });
		let persisted: string | null = null;

		await expect(
			resolveSessionLaunch(host, 'demo', restored, true, async (uuid) => {
				persisted = uuid;
			})
		).resolves.toEqual({
			uuid: 'u1',
			resume: true
		});
		expect(persisted).toBe('u1');
	});

	it('falls back to a bare restored antigravity launch when the index has no uuid', async () => {
		const host = createMockHost({ sessionUuids: { antigravity: null } });
		const restored = session({ id: 's1', agent: 'antigravity', agentSessionUuid: null });
		let persisted: string | null = null;

		await expect(
			resolveSessionLaunch(host, 'demo', restored, true, async (uuid) => {
				persisted = uuid;
			})
		).resolves.toEqual({
			uuid: null,
			resume: false
		});
		expect(persisted).toBeNull();
	});
});

describe('pruneOpen', () => {
	it('keeps a ref whose workspace and session exist', () => {
		const workspaces = [workspace({ name: 'A', slug: 'a', sessions: [session({ id: 's1' })] })];

		expect(pruneOpen([{ slug: 'a', sessionId: 's1' }], workspaces)).toEqual([
			{ slug: 'a', sessionId: 's1' }
		]);
	});

	it('drops a ref whose session was removed from its workspace', () => {
		const workspaces = [workspace({ name: 'A', slug: 'a', sessions: [session({ id: 's2' })] })];

		expect(pruneOpen([{ slug: 'a', sessionId: 's1' }], workspaces)).toEqual([]);
	});

	it('drops a ref whose workspace is gone', () => {
		const workspaces = [workspace({ name: 'A', slug: 'a', sessions: [session({ id: 's1' })] })];

		expect(pruneOpen([{ slug: 'b', sessionId: 's1' }], workspaces)).toEqual([]);
	});
});
