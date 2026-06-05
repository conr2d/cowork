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
});
