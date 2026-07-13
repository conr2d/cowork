import type { HostClient } from '$lib/host/client';
import type { AgentId } from '$lib/terminal/agent';
import { parseAgentIds } from './agents';
import { initialStep, type WizardStep } from './steps';

export interface BootstrapState {
	step: WizardStep;
	selectedAgents: AgentId[];
	resumed: boolean;
}

export async function loadBootstrapState(
	host: Pick<HostClient, 'getResumeState'>
): Promise<BootstrapState> {
	const resume = await host.getResumeState();
	if (resume) {
		return {
			step: initialStep(resume),
			selectedAgents: parseAgentIds(resume.selectedAgents),
			resumed: true
		};
	}

	return {
		step: initialStep(null),
		selectedAgents: [],
		resumed: false
	};
}
