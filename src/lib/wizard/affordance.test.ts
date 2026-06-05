import { describe, expect, it } from 'vitest';

import { AUTO_RETRY_MAX, affordanceFor, retryDelayMs, shouldAutoRetry } from './affordance';

describe('affordanceFor', () => {
	it('blocks a Blocker with no retry', () => {
		expect(affordanceFor('Blocker')).toEqual({
			blocking: true,
			canRetry: false,
			autoRetry: false,
			showCause: false
		});
	});

	it('lets a NeedsUserAction retry manually only', () => {
		const a = affordanceFor('NeedsUserAction');
		expect(a.canRetry).toBe(true);
		expect(a.autoRetry).toBe(false);
	});

	it('auto-retries a Transient', () => {
		expect(affordanceFor('Transient').autoRetry).toBe(true);
	});

	it('shows the cause for an Internal', () => {
		expect(affordanceFor('Internal').showCause).toBe(true);
	});
});

describe('shouldAutoRetry', () => {
	it('auto-retries a Transient until the cap', () => {
		expect(shouldAutoRetry('Transient', 1)).toBe(true);
		expect(shouldAutoRetry('Transient', AUTO_RETRY_MAX - 1)).toBe(true);
		expect(shouldAutoRetry('Transient', AUTO_RETRY_MAX)).toBe(false);
	});

	it('never auto-retries non-Transient kinds', () => {
		expect(shouldAutoRetry('Blocker', 1)).toBe(false);
		expect(shouldAutoRetry('Internal', 1)).toBe(false);
		expect(shouldAutoRetry('NeedsUserAction', 1)).toBe(false);
	});
});

describe('retryDelayMs', () => {
	it('backs off exponentially from one second', () => {
		expect(retryDelayMs(1)).toBe(1000);
		expect(retryDelayMs(2)).toBe(2000);
		expect(retryDelayMs(3)).toBe(4000);
	});
});
