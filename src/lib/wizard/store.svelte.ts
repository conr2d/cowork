// The wizard's reactive state, built on Svelte 5 runes. Owns onboarding (step +
// agent selection) and the setup runner (executing the host operations for the
// preflight..agent-install stages, with per-kind retry). Testable decisions live
// in the pure modules ./agents, ./steps, ./affordance, ./runner; this wrapper is
// verified by svelte-check + build.

import type { Envelope } from '$lib/errors/registry';
import type { HostClient } from '$lib/host/client';
import type { AgentId } from '$lib/terminal/agent';
import { asEnvelope } from '$lib/host/client';
import { isValidSelection, toggleAgent } from './agents';
import { retryDelayMs, shouldAutoRetry } from './affordance';
import { loadBootstrapState } from './bootstrap';
import { firstPreflightFailure, RUNNER_STEPS, type RunnerStep } from './runner';
import { isOnboardingStep, nextStep, prevStep, type WizardStep } from './steps';

export interface Wizard {
	readonly step: WizardStep;
	readonly selectedAgents: readonly AgentId[];
	readonly canProceed: boolean;
	readonly error: Envelope | null;
	readonly progress: string | null;
	readonly running: boolean;
	readonly attempt: number;
	readonly autoRetrying: boolean;
	readonly rebooting: boolean;
	readonly done: boolean;
	toggleAgent(id: AgentId): void;
	next(): void;
	back(): void;
	retry(): void;
	bootstrap(): Promise<void>;
}

export function createWizard(host: HostClient): Wizard {
	let step = $state<WizardStep>('language');
	let selectedAgents = $state<AgentId[]>([]);
	let error = $state<Envelope | null>(null);
	let progress = $state<string | null>(null);
	let running = $state(false);
	let rebooting = $state(false);
	let done = $state(false);
	let attempt = $state(0);
	let autoRetrying = $state(false);

	const canProceed = $derived(step === 'agents' ? isValidSelection(selectedAgents) : true);

	let retryTimer: ReturnType<typeof setTimeout> | null = null;

	async function execute(def: RunnerStep): Promise<void> {
		switch (def.id) {
			case 'preflight': {
				const report = await host.preflightRun();
				const failure = firstPreflightFailure(report);
				if (failure) throw failure;
				return;
			}
			case 'wsl': {
				const outcome = await host.wslEnable(selectedAgents);
				if (outcome === 'RebootRequired') rebooting = true;
				return;
			}
			case 'provision':
				await host.provisionRun();
				return;
			case 'toolchain':
				await host.guestBootstrap((event) => {
					progress = event.step;
				});
				return;
			case 'agentInstall':
				await host.guestAgentInstall(selectedAgents, (event) => {
					progress = event.step;
				});
				return;
		}
	}

	function advance(): void {
		step = nextStep(step);
		if (RUNNER_STEPS.some((s) => s.id === step)) {
			void runActive();
		} else {
			done = true;
			void host.clearResume();
		}
	}

	async function runActive(): Promise<void> {
		const def = RUNNER_STEPS.find((s) => s.id === step);
		if (!def) return;
		running = true;
		error = null;
		progress = null;
		try {
			await execute(def);
			running = false;
			attempt = 0;
			autoRetrying = false;
			if (!rebooting) advance();
		} catch (caught) {
			running = false;
			const envelope = asEnvelope(caught);
			error = envelope;
			attempt += 1;
			if (shouldAutoRetry(envelope.kind, attempt)) {
				autoRetrying = true;
				retryTimer = setTimeout(() => {
					retryTimer = null;
					void runActive();
				}, retryDelayMs(attempt));
			} else {
				autoRetrying = false;
			}
		}
	}

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
		get error() {
			return error;
		},
		get progress() {
			return progress;
		},
		get running() {
			return running;
		},
		get attempt() {
			return attempt;
		},
		get autoRetrying() {
			return autoRetrying;
		},
		get rebooting() {
			return rebooting;
		},
		get done() {
			return done;
		},
		toggleAgent(id) {
			selectedAgents = toggleAgent(selectedAgents, id);
		},
		next() {
			step = nextStep(step);
			if (!isOnboardingStep(step)) void runActive();
		},
		back() {
			step = prevStep(step);
		},
		retry() {
			if (retryTimer) {
				clearTimeout(retryTimer);
				retryTimer = null;
			}
			autoRetrying = false;
			void runActive();
		},
		async bootstrap() {
			try {
				const boot = await loadBootstrapState(host);
				selectedAgents = boot.selectedAgents;
				step = boot.step;
				if (boot.resumed) {
					void runActive();
				}
			} catch (caught) {
				error = asEnvelope(caught);
			}
		}
	};
}
