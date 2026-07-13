import { describe, expect, it } from 'vitest';

import { createMockHost } from '$lib/host/mock';
import { loadBootstrapState } from './bootstrap';

describe('loadBootstrapState', () => {
	it('lands on provision and restores agent selection for a pending resume state', async () => {
		await expect(
			loadBootstrapState(
				createMockHost({
					resumeState: { stage: 'WslReady', selectedAgents: ['codex'] }
				})
			)
		).resolves.toEqual({
			step: 'provision',
			selectedAgents: ['codex'],
			resumed: true
		});
	});

	it('lands on language when no resume state exists', async () => {
		await expect(loadBootstrapState(createMockHost())).resolves.toEqual({
			step: 'language',
			selectedAgents: [],
			resumed: false
		});
	});
});
