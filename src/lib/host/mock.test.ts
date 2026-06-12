import { describe, expect, it } from 'vitest';

import { createMockHost } from './mock';
import type { ProgressEvent } from './types';

describe('createMockHost', () => {
	it('returns a passing preflight report by default', async () => {
		const host = createMockHost();
		const report = await host.preflightRun();
		expect(report.can_proceed).toBe(true);
		expect(report.outcomes).toHaveLength(9);
	});

	it('streams the default bootstrap steps to onProgress', async () => {
		const host = createMockHost();
		const seen: ProgressEvent[] = [];
		await host.guestBootstrap((e) => seen.push(e));
		expect(seen.map((e) => e.step)).toEqual(['apt', 'brew']);
	});

	it('streams agent-install steps to onProgress', async () => {
		const host = createMockHost();
		const seen: ProgressEvent[] = [];
		await host.guestAgentInstall(['claude'], (e) => seen.push(e));
		expect(seen.map((e) => e.step)).toEqual(['install-claude']);
	});

	it('rejects the named method when failWith is set', async () => {
		const envelope = { code: 'wsl.install_failed', kind: 'Transient', stage: 'wsl-enable' };
		const host = createMockHost({ failWith: { wslEnable: envelope } });
		await expect(host.wslEnable(['claude'])).rejects.toEqual(envelope);
	});

	it('honors a pending resume state', async () => {
		const host = createMockHost({
			resumeLaunch: true,
			resumeState: { stage: 'WslReady', selectedAgents: ['codex'] }
		});
		expect(await host.isResumeLaunch()).toBe(true);
		expect(await host.getResumeState()).toEqual({ stage: 'WslReady', selectedAgents: ['codex'] });
	});

	it('tracks setup completion after marking complete', async () => {
		const host = createMockHost();
		expect(await host.setupIsComplete()).toBe(false);
		await host.setupMarkComplete();
		expect(await host.setupIsComplete()).toBe(true);
	});

	it('rejects setupMarkComplete when failWith is set', async () => {
		const envelope = { code: 'host.setup_marker_failed', kind: 'Internal', stage: 'done' };
		const host = createMockHost({ failWith: { setupMarkComplete: envelope } });
		await expect(host.setupMarkComplete()).rejects.toEqual(envelope);
	});

	it('creates workspaces with derived slugs and grows the list', async () => {
		const host = createMockHost();
		const created = await host.workspaceCreate('PDF Translate', 'codex', 'blank');
		expect(created.slug).toBe('pdf-translate');
		expect(await host.workspaceList()).toHaveLength(2);
	});

	it('rejects empty workspace names', async () => {
		const host = createMockHost();
		await expect(host.workspaceCreate(' ', 'claude', 'blank')).rejects.toMatchObject({
			code: 'workspace.invalid_name'
		});
	});

	it('suffixes workspace slug collisions', async () => {
		const host = createMockHost();
		const created = await host.workspaceCreate('default', 'claude', 'blank');
		expect(created.slug).toBe('default-2');
	});

	it('updates workspace names without changing slug and rejects unknown slugs', async () => {
		const host = createMockHost();
		const updated = await host.workspaceUpdate('default', { name: 'Renamed' });
		expect(updated.name).toBe('Renamed');
		expect(updated.slug).toBe('default');
		await expect(host.workspaceUpdate('missing', { name: 'Nope' })).rejects.toMatchObject({
			code: 'workspace.not_found'
		});
	});

	it('deletes workspaces idempotently', async () => {
		const host = createMockHost();
		await host.workspaceDelete('default');
		expect(await host.workspaceList()).toEqual([]);
		await host.workspaceDelete('default');
		expect(await host.workspaceList()).toEqual([]);
	});

	it('opens files for an existing workspace', async () => {
		const host = createMockHost();
		await expect(host.workspaceOpenFiles('default')).resolves.toBeUndefined();
	});

	it('rejects open files for an unknown workspace', async () => {
		const host = createMockHost();
		await expect(host.workspaceOpenFiles('missing')).rejects.toMatchObject({
			code: 'workspace.not_found'
		});
	});

	it('rejects open files with injected failure', async () => {
		const envelope = { code: 'workspace.open_files_failed', kind: 'Internal', stage: 'workspace' };
		const host = createMockHost({ failWith: { workspaceOpenFiles: envelope } });
		await expect(host.workspaceOpenFiles('default')).rejects.toEqual(envelope);
	});
});
