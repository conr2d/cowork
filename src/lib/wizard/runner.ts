// The setup runner's step model and pure helpers. The reactive store drives the
// async host calls; everything decidable without I/O lives here and is tested.

import type { Envelope, Stage } from '$lib/errors/registry';
import type { PreflightReport } from '$lib/host/types';
import { isFail } from '$lib/host/types';
import { WIZARD_STEPS, type WizardStep } from './steps';

/** A runner step's id (a subset of WizardStep) and the stage it reports. */
export interface RunnerStep {
	id: 'preflight' | 'wsl' | 'provision' | 'toolchain' | 'agentInstall';
	stage: Stage;
}

/** The host operations the runner executes, in order. */
export const RUNNER_STEPS: readonly RunnerStep[] = [
	{ id: 'preflight', stage: 'preflight' },
	{ id: 'wsl', stage: 'wsl-enable' },
	{ id: 'provision', stage: 'provision' },
	{ id: 'toolchain', stage: 'toolchain' },
	{ id: 'agentInstall', stage: 'agent-install' }
] as const;

/** The first failing preflight check's envelope, or null when all checks pass. */
export function firstPreflightFailure(report: PreflightReport): Envelope | null {
	for (const outcome of report.outcomes) {
		if (isFail(outcome.status)) return outcome.status.Fail;
	}
	return null;
}

export type RunPhase = 'done' | 'active' | 'pending';

/** A runner step's display phase relative to the wizard's current step. */
export function stepStatus(id: RunnerStep['id'], current: WizardStep): RunPhase {
	const at = WIZARD_STEPS.indexOf(id);
	const cur = WIZARD_STEPS.indexOf(current);
	if (at < cur) return 'done';
	if (at === cur) return 'active';
	return 'pending';
}

/** Format an elapsed duration in whole seconds as `m:ss` (e.g. 65 → "1:05").
 * Negative or fractional input is clamped/floored to whole non-negative seconds. */
export function formatElapsed(seconds: number): string {
	const safe = Math.max(0, Math.floor(seconds));
	const mins = Math.floor(safe / 60);
	const secs = safe % 60;
	return `${mins}:${secs.toString().padStart(2, '0')}`;
}
