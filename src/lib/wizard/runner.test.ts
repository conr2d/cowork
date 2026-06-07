import { describe, expect, it } from 'vitest';

import type { Envelope } from '$lib/errors/registry';
import type { PreflightReport } from '$lib/host/types';
import { RUNNER_STEPS, firstPreflightFailure, formatElapsed, stepStatus } from './runner';

const envelope: Envelope = {
	code: 'preflight.elevation_unavailable',
	kind: 'Blocker',
	stage: 'preflight'
};

describe('RUNNER_STEPS', () => {
	it('runs preflight through agent-install in order', () => {
		expect(RUNNER_STEPS.map((s) => s.id)).toEqual([
			'preflight',
			'wsl',
			'provision',
			'toolchain',
			'agentInstall'
		]);
	});

	it('maps each step to its stage', () => {
		expect(RUNNER_STEPS.map((s) => s.stage)).toEqual([
			'preflight',
			'wsl-enable',
			'provision',
			'toolchain',
			'agent-install'
		]);
	});
});

describe('firstPreflightFailure', () => {
	it('returns null when all checks pass', () => {
		const report: PreflightReport = {
			outcomes: [{ check: 'Arch', status: 'Pass' }],
			can_proceed: true
		};
		expect(firstPreflightFailure(report)).toBeNull();
	});

	it('returns the first failing envelope', () => {
		const report: PreflightReport = {
			outcomes: [
				{ check: 'Arch', status: 'Pass' },
				{ check: 'Elevation', status: { Fail: envelope } }
			],
			can_proceed: false
		};
		expect(firstPreflightFailure(report)).toEqual(envelope);
	});
});

describe('stepStatus', () => {
	it('classifies done / active / pending against the current step', () => {
		expect(stepStatus('preflight', 'provision')).toBe('done');
		expect(stepStatus('provision', 'provision')).toBe('active');
		expect(stepStatus('toolchain', 'provision')).toBe('pending');
	});
});

describe('formatElapsed', () => {
	it('formats seconds as m:ss', () => {
		expect(formatElapsed(0)).toBe('0:00');
		expect(formatElapsed(5)).toBe('0:05');
		expect(formatElapsed(65)).toBe('1:05');
		expect(formatElapsed(600)).toBe('10:00');
	});

	it('clamps negative and floors fractional input', () => {
		expect(formatElapsed(-3)).toBe('0:00');
		expect(formatElapsed(9.9)).toBe('0:09');
	});
});
