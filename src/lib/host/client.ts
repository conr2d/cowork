// The host client: a single typed seam the wizard uses to drive setup. The real
// implementation (`tauriHost`) calls Tauri commands; tests/components use the
// mock from `./mock`. Every `@tauri-apps/api` import is dynamic (inside the
// method) so this module is safe to import from prerendered routes.

import type { AgentId } from '$lib/terminal/login';
import type { Envelope } from '$lib/errors/registry';
import type {
	PreflightReport,
	ProgressEvent,
	ProvisionDto,
	ResumeDto,
	WslEnableDto
} from './types';

/** The setup operations the wizard performs. Methods reject with an `Envelope`. */
export interface HostClient {
	preflightRun(): Promise<PreflightReport>;
	wslEnable(selectedAgents: AgentId[]): Promise<WslEnableDto>;
	provisionRun(): Promise<ProvisionDto>;
	guestBootstrap(onProgress: (event: ProgressEvent) => void): Promise<void>;
	guestAgentInstall(agents: AgentId[], onProgress: (event: ProgressEvent) => void): Promise<void>;
	removeCoworkDistro(): Promise<void>;
	isResumeLaunch(): Promise<boolean>;
	getResumeState(): Promise<ResumeDto | null>;
	clearResume(): Promise<void>;
}

/** A Tauri command rejection is the serialized backend `Envelope`. */
export function asEnvelope(error: unknown): Envelope {
	return error as Envelope;
}

/** The production client, backed by the Tauri command bridge. */
export const tauriHost: HostClient = {
	async preflightRun() {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<PreflightReport>('preflight_run');
	},
	async wslEnable(selectedAgents) {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<WslEnableDto>('wsl_enable', { selectedAgents });
	},
	async provisionRun() {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<ProvisionDto>('provision_run');
	},
	async guestBootstrap(onProgress) {
		const { invoke, Channel } = await import('@tauri-apps/api/core');
		const channel = new Channel<ProgressEvent>();
		channel.onmessage = onProgress;
		await invoke('guest_bootstrap', { onProgress: channel });
	},
	async guestAgentInstall(agents, onProgress) {
		const { invoke, Channel } = await import('@tauri-apps/api/core');
		const channel = new Channel<ProgressEvent>();
		channel.onmessage = onProgress;
		await invoke('guest_agent_install', { agents, onProgress: channel });
	},
	async removeCoworkDistro() {
		const { invoke } = await import('@tauri-apps/api/core');
		await invoke('remove_cowork_distro');
	},
	async isResumeLaunch() {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<boolean>('is_resume_launch');
	},
	async getResumeState() {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<ResumeDto | null>('get_resume_state');
	},
	async clearResume() {
		const { invoke } = await import('@tauri-apps/api/core');
		await invoke('clear_resume');
	}
};
