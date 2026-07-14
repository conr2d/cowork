import { describe, expect, it } from 'vitest';

import {
	initialStep,
	isInterstitialStep,
	isOnboardingStep,
	nextStep,
	prevStep,
	WIZARD_STEPS
} from './steps';

describe('initialStep', () => {
	it('starts at language with no resume', () => {
		expect(initialStep(null)).toBe('language');
	});

	it('jumps to provision on a WslReady resume', () => {
		expect(initialStep({ stage: 'WslReady', selectedAgents: ['codex'] })).toBe('provision');
	});
});

describe('nextStep / prevStep', () => {
	it('places permission between preflight and wsl', () => {
		expect(WIZARD_STEPS).toEqual([
			'language',
			'agents',
			'preflight',
			'permission',
			'wsl',
			'provision',
			'toolchain',
			'agentInstall',
			'auth',
			'done'
		]);
	});

	it('advances and clamps at the end', () => {
		expect(nextStep('language')).toBe('agents');
		expect(nextStep('preflight')).toBe('permission');
		expect(nextStep('permission')).toBe('wsl');
		expect(nextStep('done')).toBe('done');
	});

	it('retreats and clamps at the start', () => {
		expect(prevStep('agents')).toBe('language');
		expect(prevStep('wsl')).toBe('permission');
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

describe('isInterstitialStep', () => {
	it('is true only for permission', () => {
		expect(isInterstitialStep('permission')).toBe(true);
		expect(isInterstitialStep('preflight')).toBe(false);
		expect(isInterstitialStep('wsl')).toBe(false);
	});
});
