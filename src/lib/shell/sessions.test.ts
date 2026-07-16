import { describe, expect, it, vi } from 'vitest';

import { createMockHost } from '$lib/host/mock';
import type { SessionDto, WorkspaceDto } from '$lib/host/types';
import { sortedSessions } from './model';

function session(partial: Partial<SessionDto> & Pick<SessionDto, 'id'>): SessionDto {
	return { agent: 'codex', agentSessionUuid: null, title: 't', order: 0, ...partial };
}

async function flushMicrotasks(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

async function loadShellModules() {
	const runtime = globalThis as typeof globalThis & {
		$state?: <T>(initial: T) => T;
		$derived?: <T>(value: T) => T;
	};
	runtime.$state ??= (<T>(initial: T) => initial) as unknown as typeof $state;
	runtime.$derived ??= (<T>(value: T) => value) as unknown as typeof $derived;
	const [{ createSessionManager }, { createShell }] = await Promise.all([
		import('./sessions.svelte'),
		import('./store.svelte')
	]);
	return { createSessionManager, createShell };
}

describe('createSessionManager', () => {
	it('does not bump last used when selecting a workspace', async () => {
		const host = createMockHost();
		const { createShell } = await loadShellModules();
		const shell = createShell(host);
		await shell.load();
		const slug = 'default';
		const before = shell.workspaces.find((item) => item.slug === slug)?.lastUsedMs;

		await shell.select(slug);
		await flushMicrotasks();

		expect(shell.workspaces.find((item) => item.slug === slug)?.lastUsedMs).toBe(before);
		expect((await host.workspaceList()).find((item) => item.slug === slug)?.lastUsedMs).toBe(
			before
		);
	});

	it('bumps last used when a mounted agent session becomes active', async () => {
		const host = createMockHost();
		const sessions = [session({ id: 'first', order: 1 })];
		await host.workspaceUpdate('default', { lastUsedMs: 1, sessions });
		const { createSessionManager, createShell } = await loadShellModules();
		const shell = createShell(host);
		await shell.load();
		const manager = createSessionManager(shell, host, () => 'light');
		const active = shell.workspaces.find((item) => item.slug === 'default') as WorkspaceDto;

		await manager.activate(active, 'first');
		manager.noteActivity('first', 'output');
		await flushMicrotasks();

		expect(
			(await host.workspaceList()).find((item) => item.slug === active.slug)?.lastUsedMs
		).toBeGreaterThan(1);
	});

	it('bumps last used only once during a continuous output burst', async () => {
		const host = createMockHost();
		const sessions = [session({ id: 'first', order: 1 })];
		await host.workspaceUpdate('default', { sessions });
		const { createSessionManager, createShell } = await loadShellModules();
		const shell = createShell(host);
		await shell.load();
		const manager = createSessionManager(shell, host, () => 'light');
		const active = shell.workspaces.find((item) => item.slug === 'default') as WorkspaceDto;

		await manager.activate(active, 'first');
		manager.noteActivity('first', 'output');
		await flushMicrotasks();
		const update = vi.spyOn(host, 'workspaceUpdate');

		manager.noteActivity('first', 'output');
		await flushMicrotasks();

		expect(update.mock.calls.filter(([, patch]) => patch.lastUsedMs !== undefined)).toHaveLength(0);
	});

	it('activates the first tab by order on fresh boot when no active session is persisted', async () => {
		const host = createMockHost();
		await host.workspaceUpdate('default', {
			sessions: [session({ id: 'second', order: 2 }), session({ id: 'first', order: 1 })]
		});
		const { createSessionManager, createShell } = await loadShellModules();
		const shell = createShell(host);
		await shell.load();
		const manager = createSessionManager(shell, host, () => 'light');
		const active = shell.workspaces.find((item) => item.slug === 'default') as WorkspaceDto;

		await manager.ensureActive(active);

		expect(manager.activeOf(active.slug)).toBe(sortedSessions(active.sessions)[0].id);
	});

	it('restores the persisted active session on fresh boot when it still exists', async () => {
		const host = createMockHost();
		const sessions = [session({ id: 'first', order: 1 }), session({ id: 'second', order: 2 })];
		await host.workspaceUpdate('default', {
			sessions,
			activeSessionId: 'second'
		});
		const { createSessionManager, createShell } = await loadShellModules();
		const shell = createShell(host);
		await shell.load();
		const manager = createSessionManager(shell, host, () => 'light');
		const active = shell.workspaces.find((item) => item.slug === 'default') as WorkspaceDto;

		await manager.ensureActive(active);

		expect(manager.activeOf(active.slug)).toBe('second');
	});

	it('falls back to the first tab by order when the persisted active session is gone', async () => {
		const host = createMockHost();
		await host.workspaceUpdate('default', {
			sessions: [session({ id: 'second', order: 2 }), session({ id: 'first', order: 1 })],
			activeSessionId: 'missing'
		});
		const { createSessionManager, createShell } = await loadShellModules();
		const shell = createShell(host);
		await shell.load();
		const manager = createSessionManager(shell, host, () => 'light');
		const active = shell.workspaces.find((item) => item.slug === 'default') as WorkspaceDto;

		await manager.ensureActive(active);

		expect(manager.activeOf(active.slug)).toBe(sortedSessions(active.sessions)[0].id);
	});

	it('persists the active session when tabs are switched', async () => {
		const host = createMockHost();
		const sessions = [session({ id: 'first', order: 1 }), session({ id: 'second', order: 2 })];
		await host.workspaceUpdate('default', {
			sessions,
			activeSessionId: 'first'
		});
		const { createSessionManager, createShell } = await loadShellModules();
		const shell = createShell(host);
		await shell.load();
		const manager = createSessionManager(shell, host, () => 'light');
		const active = shell.workspaces.find((item) => item.slug === 'default') as WorkspaceDto;

		await manager.activate(active, 'second');
		await flushMicrotasks();

		const persisted = await host.workspaceList();
		expect(persisted.find((item) => item.slug === active.slug)?.activeSessionId).toBe('second');
		expect(shell.workspaces.find((item) => item.slug === active.slug)?.activeSessionId).toBe(
			'second'
		);
	});
});
