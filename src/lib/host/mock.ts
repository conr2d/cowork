// An in-memory `HostClient` for component tests and UI development (no Tauri).
// `createMockHost` returns a happy-path client; pass overrides to inject specific
// behaviors (e.g. a failing step, or a pending resume).

import type { AgentId } from '$lib/terminal/agent';
import type { HostClient } from './client';
import type {
	AppBuildDto,
	PreflightReport,
	ProgressEvent,
	ProvisionDto,
	ResumeDto,
	WorkspaceDto,
	WorkspacePatch,
	WslEnableDto
} from './types';

export interface MockHostOptions {
	build?: AppBuildDto;
	preflight?: PreflightReport;
	wslEnable?: WslEnableDto;
	provision?: ProvisionDto;
	bootstrapSteps?: ProgressEvent[];
	agentInstallSteps?: ProgressEvent[];
	resumeState?: ResumeDto | null;
	setupComplete?: boolean;
	/** Scripted session UUID capture results; unspecified agents report null. */
	sessionUuids?: Partial<Record<AgentId, string | null>>;
	/** Scripted session existence probe results; unspecified agents report true. */
	sessionChecks?: Partial<Record<AgentId, boolean>>;
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

const DEFAULT_WORKSPACE: WorkspaceDto = {
	name: 'default',
	slug: 'default',
	createdMs: 0,
	pinned: false,
	pinOrder: null,
	lastUsedMs: 0,
	defaultAgent: 'claude',
	preset: 'blank',
	sessions: []
};

function workspaceEnvelope(
	code: 'workspace.invalid_name' | 'workspace.not_found',
	kind: 'NeedsUserAction' | 'Internal',
	context: Record<string, string>
) {
	return { code, kind, stage: 'workspace', context };
}

function previewSlug(name: string, existing: string[]): string {
	const trimmed = name.trim();
	if (trimmed === '') {
		throw workspaceEnvelope('workspace.invalid_name', 'NeedsUserAction', { name });
	}
	let out = '';
	let lastWasDash = false;
	for (const ch of trimmed.toLocaleLowerCase()) {
		if (/[\p{L}\p{N}]/u.test(ch)) {
			out += ch;
			lastWasDash = false;
		} else if (ch === '-' || ch === '_') {
			out += ch;
			lastWasDash = ch === '-';
		} else if (/\s/u.test(ch) && !lastWasDash) {
			out += '-';
			lastWasDash = true;
		}
	}
	out = out
		.replace(/-+/g, '-')
		.replace(/^[-_]+|[-_]+$/g, '')
		.slice(0, 40);
	if (out === '') out = 'workspace';
	if (!existing.includes(out)) return out;
	for (let n = 2; ; n += 1) {
		const candidate = `${out}-${n}`;
		if (!existing.includes(candidate)) return candidate;
	}
}

export function createMockHost(options: MockHostOptions = {}): HostClient {
	const fail = options.failWith ?? {};
	const workspaces: WorkspaceDto[] = [{ ...DEFAULT_WORKSPACE }];
	let setupComplete = options.setupComplete ?? false;
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
		appBuild: () => rejectIf('appBuild', options.build ?? { version: '0.1.0', sha: 'abcdef0' }),
		preflightRun: () => rejectIf('preflightRun', options.preflight ?? PASS_REPORT),
		wslEnable: () => rejectIf('wslEnable', options.wslEnable ?? 'Ready'),
		provisionRun: () => rejectIf('provisionRun', options.provision ?? 'Ready'),
		guestSync: () => rejectIf('guestSync', false),
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
		getResumeState: () => rejectIf('getResumeState', options.resumeState ?? null),
		clearResume: () => rejectIf('clearResume', undefined),
		setupIsComplete: () => rejectIf('setupIsComplete', setupComplete),
		setupMarkComplete: async () => {
			if ('setupMarkComplete' in fail) return Promise.reject(fail.setupMarkComplete);
			setupComplete = true;
		},
		workspaceCreate: async (name: string, defaultAgent: AgentId, preset: string) => {
			if ('workspaceCreate' in fail) return Promise.reject(fail.workspaceCreate);
			const slug = previewSlug(
				name,
				workspaces.map((workspace) => workspace.slug)
			);
			const workspace: WorkspaceDto = {
				name: name.trim(),
				slug,
				createdMs: Date.now(),
				pinned: false,
				pinOrder: null,
				lastUsedMs: Date.now(),
				defaultAgent,
				preset,
				sessions: []
			};
			workspaces.push(workspace);
			return workspace;
		},
		workspaceList: () =>
			rejectIf(
				'workspaceList',
				workspaces.map((workspace) => ({ ...workspace }))
			),
		workspaceUpdate: async (slug: string, patch: WorkspacePatch) => {
			if ('workspaceUpdate' in fail) return Promise.reject(fail.workspaceUpdate);
			const workspace = workspaces.find((item) => item.slug === slug);
			if (!workspace) {
				return Promise.reject(workspaceEnvelope('workspace.not_found', 'Internal', { slug }));
			}
			if (patch.name !== undefined) {
				if (patch.name.trim() === '') {
					return Promise.reject(
						workspaceEnvelope('workspace.invalid_name', 'NeedsUserAction', { name: patch.name })
					);
				}
				workspace.name = patch.name.trim();
			}
			if (patch.pinned !== undefined) workspace.pinned = patch.pinned;
			if (patch.pinOrder !== undefined) workspace.pinOrder = patch.pinOrder;
			if (patch.lastUsedMs !== undefined) workspace.lastUsedMs = patch.lastUsedMs;
			if (patch.defaultAgent !== undefined) workspace.defaultAgent = patch.defaultAgent;
			if (patch.preset !== undefined) workspace.preset = patch.preset;
			if (patch.sessions !== undefined) {
				workspace.sessions = patch.sessions.map((session) => ({ ...session }));
			}
			return { ...workspace };
		},
		workspaceDelete: async (slug: string) => {
			if ('workspaceDelete' in fail) return Promise.reject(fail.workspaceDelete);
			const index = workspaces.findIndex((workspace) => workspace.slug === slug);
			if (index >= 0) workspaces.splice(index, 1);
		},
		workspaceSlugPreview: async (name: string) => {
			if ('workspaceSlugPreview' in fail) return Promise.reject(fail.workspaceSlugPreview);
			return previewSlug(
				name,
				workspaces.map((workspace) => workspace.slug)
			);
		},
		workspaceOpenFiles: async (slug: string) => {
			if ('workspaceOpenFiles' in fail) return Promise.reject(fail.workspaceOpenFiles);
			if (!workspaces.some((workspace) => workspace.slug === slug)) {
				return Promise.reject(workspaceEnvelope('workspace.not_found', 'Internal', { slug }));
			}
		},
		captureSessionUuid: (agent: AgentId) =>
			rejectIf('captureSessionUuid', options.sessionUuids?.[agent] ?? null),
		sessionCheck: (agent: AgentId) =>
			rejectIf('sessionCheck', options.sessionChecks?.[agent] ?? true),
		agentThemeSync: () => rejectIf('agentThemeSync', undefined)
	};
}
