// The host client: a single typed seam the wizard uses to drive setup. The real
// implementation (`tauriHost`) calls Tauri commands; tests/components use the
// mock from `./mock`. Every `@tauri-apps/api` import is dynamic (inside the
// method) so this module is safe to import from prerendered routes.

import type { AgentId } from '$lib/terminal/agent';
import type { Envelope } from '$lib/errors/registry';
import type {
	PreflightReport,
	ProgressEvent,
	ProvisionDto,
	ResumeDto,
	WorkspaceDto,
	WorkspacePatch,
	WslEnableDto
} from './types';

/** The setup operations the wizard performs. Methods reject with an `Envelope`. */
export interface HostClient {
	preflightRun(): Promise<PreflightReport>;
	wslEnable(selectedAgents: AgentId[]): Promise<WslEnableDto>;
	provisionRun(): Promise<ProvisionDto>;
	/** Re-inject the guest CLI if the installed copy is stale. */
	guestSync(): Promise<boolean>;
	guestBootstrap(onProgress: (event: ProgressEvent) => void): Promise<void>;
	guestAgentInstall(agents: AgentId[], onProgress: (event: ProgressEvent) => void): Promise<void>;
	removeCoworkDistro(): Promise<void>;
	isResumeLaunch(): Promise<boolean>;
	getResumeState(): Promise<ResumeDto | null>;
	clearResume(): Promise<void>;
	setupIsComplete(): Promise<boolean>;
	setupMarkComplete(): Promise<void>;
	workspaceCreate(name: string, defaultAgent: AgentId, preset: string): Promise<WorkspaceDto>;
	workspaceList(): Promise<WorkspaceDto[]>;
	workspaceUpdate(slug: string, patch: WorkspacePatch): Promise<WorkspaceDto>;
	workspaceDelete(slug: string): Promise<void>;
	workspaceSlugPreview(name: string): Promise<string>;
	workspaceOpenFiles(slug: string): Promise<void>;
	captureSessionUuid(agent: AgentId, slug: string, sinceMs: number): Promise<string | null>;
	agentThemeSync(theme: 'light' | 'dark'): Promise<void>;
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
	async guestSync() {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<boolean>('guest_sync');
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
	},
	async setupIsComplete() {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<boolean>('setup_is_complete');
	},
	async setupMarkComplete() {
		const { invoke } = await import('@tauri-apps/api/core');
		await invoke('setup_mark_complete');
	},
	async workspaceCreate(name, defaultAgent, preset) {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<WorkspaceDto>('workspace_create', { name, defaultAgent, preset });
	},
	async workspaceList() {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<WorkspaceDto[]>('workspace_list');
	},
	async workspaceUpdate(slug, patch) {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<WorkspaceDto>('workspace_update', { slug, patch });
	},
	async workspaceDelete(slug) {
		const { invoke } = await import('@tauri-apps/api/core');
		await invoke('workspace_delete', { slug });
	},
	async workspaceSlugPreview(name) {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<string>('workspace_slug_preview', { name });
	},
	async workspaceOpenFiles(slug) {
		const { invoke } = await import('@tauri-apps/api/core');
		await invoke('workspace_open_files', { slug });
	},
	async captureSessionUuid(agent, slug, sinceMs) {
		const { invoke } = await import('@tauri-apps/api/core');
		return invoke<string | null>('capture_session_uuid', { agent, slug, sinceMs });
	},
	async agentThemeSync(theme) {
		const { invoke } = await import('@tauri-apps/api/core');
		await invoke('agent_theme_sync', { theme });
	}
};
