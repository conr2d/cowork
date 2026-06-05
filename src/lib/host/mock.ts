// An in-memory `HostClient` for component tests and UI development (no Tauri).
// `createMockHost` returns a happy-path client; pass overrides to inject specific
// behaviors (e.g. a failing step, or a pending resume).

import type { AgentId } from '$lib/terminal/login';
import type { HostClient } from './client';
import type {
	PreflightReport,
	ProgressEvent,
	ProvisionDto,
	ResumeDto,
	WslEnableDto
} from './types';

export interface MockHostOptions {
	preflight?: PreflightReport;
	wslEnable?: WslEnableDto;
	provision?: ProvisionDto;
	bootstrapSteps?: ProgressEvent[];
	agentInstallSteps?: ProgressEvent[];
	resumeLaunch?: boolean;
	resumeState?: ResumeDto | null;
	/** If set, the named method rejects with this value (an Envelope). */
	failWith?: Partial<Record<keyof HostClient, unknown>>;
}

const PASS_REPORT: PreflightReport = {
	outcomes: [
		{ check: 'WindowsBuild', status: 'Pass' },
		{ check: 'Arch', status: 'Pass' },
		{ check: 'Virtualization', status: 'Pass' },
		{ check: 'HypervisorConflict', status: 'Pass' },
		{ check: 'Disk', status: 'Pass' },
		{ check: 'Elevation', status: 'Pass' },
		{ check: 'WslPolicy', status: 'Pass' },
		{ check: 'Store', status: 'Pass' },
		{ check: 'ControlledFolderAccess', status: 'Pass' }
	],
	can_proceed: true
};

export function createMockHost(options: MockHostOptions = {}): HostClient {
	const fail = options.failWith ?? {};
	const rejectIf = <T>(method: keyof HostClient, value: T): Promise<T> =>
		method in fail ? Promise.reject(fail[method]) : Promise.resolve(value);

	const stream = async (
		method: keyof HostClient,
		steps: ProgressEvent[],
		onProgress: (event: ProgressEvent) => void
	): Promise<void> => {
		if (method in fail) return Promise.reject(fail[method]);
		for (const step of steps) onProgress(step);
		return Promise.resolve();
	};

	return {
		preflightRun: () => rejectIf('preflightRun', options.preflight ?? PASS_REPORT),
		wslEnable: () => rejectIf('wslEnable', options.wslEnable ?? 'Ready'),
		provisionRun: () => rejectIf('provisionRun', options.provision ?? 'Ready'),
		guestBootstrap: (onProgress) =>
			stream(
				'guestBootstrap',
				options.bootstrapSteps ?? [
					{ stage: 'toolchain', step: 'apt' },
					{ stage: 'toolchain', step: 'brew' }
				],
				onProgress
			),
		guestAgentInstall: (_agents: AgentId[], onProgress) =>
			stream(
				'guestAgentInstall',
				options.agentInstallSteps ?? [{ stage: 'agent-install', step: 'install-claude' }],
				onProgress
			),
		removeCoworkDistro: () => rejectIf('removeCoworkDistro', undefined),
		isResumeLaunch: () => rejectIf('isResumeLaunch', options.resumeLaunch ?? false),
		getResumeState: () => rejectIf('getResumeState', options.resumeState ?? null),
		clearResume: () => rejectIf('clearResume', undefined)
	};
}
