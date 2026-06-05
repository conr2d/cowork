// The wizard's reactive state, built on Svelte 5 runes. Holds the current step
// and the selected agents, exposes navigation, and bootstraps a reboot-resume
// through the injected `HostClient`. Testable logic lives in the pure modules
// `./agents` and `./steps`; this wrapper is verified by `svelte-check` + build.

import type { Envelope } from '$lib/errors/registry';
import type { HostClient } from '$lib/host/client';
import type { AgentId } from '$lib/terminal/login';
import { asEnvelope } from '$lib/host/client';
import { isValidSelection, parseAgentIds, toggleAgent } from './agents';
import { initialStep, nextStep, prevStep, type WizardStep } from './steps';

export interface Wizard {
	readonly step: WizardStep;
	readonly selectedAgents: readonly AgentId[];
	readonly canProceed: boolean;
	readonly resumeError: Envelope | null;
	toggleAgent(id: AgentId): void;
	next(): void;
	back(): void;
	bootstrap(): Promise<void>;
}

export function createWizard(host: HostClient): Wizard {
	let step = $state<WizardStep>('language');
	let selectedAgents = $state<AgentId[]>([]);
	let resumeError = $state<Envelope | null>(null);

	const canProceed = $derived(step === 'agents' ? isValidSelection(selectedAgents) : true);

	return {
		get step() {
			return step;
		},
		get selectedAgents() {
			return selectedAgents;
		},
		get canProceed() {
			return canProceed;
		},
		get resumeError() {
			return resumeError;
		},
		toggleAgent(id) {
			selectedAgents = toggleAgent(selectedAgents, id);
		},
		next() {
			step = nextStep(step);
		},
		back() {
			step = prevStep(step);
		},
		async bootstrap() {
			try {
				if (await host.isResumeLaunch()) {
					const resume = await host.getResumeState();
					if (resume) {
						selectedAgents = parseAgentIds(resume.selectedAgents);
						step = initialStep(resume);
						return;
					}
				}
				step = initialStep(null);
			} catch (error) {
				resumeError = asEnvelope(error);
			}
		}
	};
}
