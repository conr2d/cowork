// The wizard's step model and pure navigation helpers. No runes — unit-tested
// directly. The reactive store (`store.svelte.ts`) wraps these.

import type { ResumeDto } from '$lib/host/types';

/** Every wizard step, in order: two onboarding steps then the seven setup stages. */
export const WIZARD_STEPS = [
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
] as const;
export type WizardStep = (typeof WIZARD_STEPS)[number];

/** The steps shown before any host work runs. */
export const ONBOARDING_STEPS = ['language', 'agents'] as const;
export const INTERSTITIAL_STEPS = ['permission'] as const;

export function isOnboardingStep(step: WizardStep): boolean {
	return (ONBOARDING_STEPS as readonly WizardStep[]).includes(step);
}

export function isInterstitialStep(step: WizardStep): boolean {
	return (INTERSTITIAL_STEPS as readonly WizardStep[]).includes(step);
}

/**
 * The step to start on. A pending `WslReady` resume (the wizard relaunched after
 * a reboot) skips onboarding and the wsl-enable stage, landing on provision.
 */
export function initialStep(resume: ResumeDto | null): WizardStep {
	return resume?.stage === 'WslReady' ? 'provision' : 'language';
}

/** The next step, clamped at the last step. */
export function nextStep(step: WizardStep): WizardStep {
	const i = WIZARD_STEPS.indexOf(step);
	return WIZARD_STEPS[Math.min(i + 1, WIZARD_STEPS.length - 1)];
}

/** The previous step, clamped at the first step. */
export function prevStep(step: WizardStep): WizardStep {
	const i = WIZARD_STEPS.indexOf(step);
	return WIZARD_STEPS[Math.max(i - 1, 0)];
}
