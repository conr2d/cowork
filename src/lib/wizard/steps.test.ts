import { describe, expect, it } from 'vitest';

import { initialStep, isOnboardingStep, nextStep, prevStep } from './steps';

describe('initialStep', () => {
	it('starts at language with no resume', () => {
		expect(initialStep(null)).toBe('language');
	});

	it('jumps to provision on a WslReady resume', () => {
		expect(initialStep({ stage: 'WslReady', selectedAgents: ['codex'] })).toBe('provision');
	});
});

describe('nextStep / prevStep', () => {
	it('advances and clamps at the end', () => {
		expect(nextStep('language')).toBe('agents');
		expect(nextStep('done')).toBe('done');
	});

	it('retreats and clamps at the start', () => {
		expect(prevStep('agents')).toBe('language');
		expect(prevStep('language')).toBe('language');
	});
});

describe('isOnboardingStep', () => {
	it('is true only for onboarding steps', () => {
		expect(isOnboardingStep('language')).toBe(true);
		expect(isOnboardingStep('agents')).toBe(true);
		expect(isOnboardingStep('preflight')).toBe(false);
	});
});
