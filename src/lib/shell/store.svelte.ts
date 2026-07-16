import type { Envelope } from '$lib/errors/registry';
import { asEnvelope, type HostClient } from '$lib/host/client';
import type { SessionDto, WorkspaceDto } from '$lib/host/types';
import type { AgentId } from '$lib/terminal/agent';
import { initialSlug, pinnedOf, recentOf } from './model';

export interface Shell {
	readonly workspaces: readonly WorkspaceDto[];
	readonly activeSlug: string | null;
	readonly active: WorkspaceDto | null;
	readonly pinned: readonly WorkspaceDto[];
	readonly recent: readonly WorkspaceDto[];
	readonly loading: boolean;
	readonly error: Envelope | null;
	reportError(error: Envelope): void;
	load(): Promise<void>;
	select(slug: string): Promise<void>;
	create(name: string, agent: AgentId, preset: string): Promise<void>;
	rename(slug: string, name: string): Promise<void>;
	remove(slug: string): Promise<void>;
	openFiles(slug: string): Promise<void>;
	setPinned(slug: string, pinned: boolean): Promise<void>;
	reorderPinned(slugs: readonly string[]): Promise<void>;
	updateSessions(slug: string, sessions: SessionDto[]): Promise<void>;
	setDefaultAgent(slug: string, agent: AgentId): Promise<void>;
	setActiveSession(slug: string, sessionId: string): Promise<void>;
	bumpLastUsed(slug: string): Promise<void>;
}

export function createShell(host: HostClient): Shell {
	let workspaces = $state<WorkspaceDto[]>([]);
	let activeSlug = $state<string | null>(null);
	let loading = $state(true);
	let error = $state<Envelope | null>(null);

	const active = $derived(workspaces.find((workspace) => workspace.slug === activeSlug) ?? null);
	const pinned = $derived(pinnedOf(workspaces));
	const recent = $derived(recentOf(workspaces));

	function replaceWorkspace(updated: WorkspaceDto): void {
		const index = workspaces.findIndex((workspace) => workspace.slug === updated.slug);
		if (index >= 0) {
			workspaces[index] = updated;
		} else {
			workspaces.push(updated);
		}
	}

	function selectLocal(slug: string | null): void {
		activeSlug = slug;
	}

	async function patchWorkspace(slug: string, patch: Parameters<HostClient['workspaceUpdate']>[1]) {
		const updated = await host.workspaceUpdate(slug, patch);
		replaceWorkspace(updated);
		return updated;
	}

	return {
		get workspaces() {
			return workspaces;
		},
		get activeSlug() {
			return activeSlug;
		},
		get active() {
			return active;
		},
		get pinned() {
			return pinned;
		},
		get recent() {
			return recent;
		},
		get loading() {
			return loading;
		},
		get error() {
			return error;
		},
		reportError(nextError) {
			error = nextError;
		},
		async load() {
			loading = true;
			try {
				let listed = await host.workspaceList();
				if (listed.length === 0) {
					listed = [await host.workspaceCreate('default', 'claude', 'blank')];
				}
				workspaces = listed;
				activeSlug = initialSlug(workspaces);
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			} finally {
				loading = false;
			}
		},
		async select(slug) {
			activeSlug = slug;
		},
		async bumpLastUsed(slug) {
			try {
				await patchWorkspace(slug, { lastUsedMs: Date.now() });
			} catch {
				// Best-effort recency; a failed bump must never disrupt the session or
				// surface UI. Recency is not worth an error bar.
			}
		},
		async create(name, agent, preset) {
			try {
				const created = await host.workspaceCreate(name, agent, preset);
				replaceWorkspace(created);
				activeSlug = created.slug;
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			}
		},
		async rename(slug, name) {
			try {
				await patchWorkspace(slug, { name });
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			}
		},
		async remove(slug) {
			try {
				await host.workspaceDelete(slug);
				workspaces = workspaces.filter((workspace) => workspace.slug !== slug);
				if (activeSlug === slug) selectLocal(initialSlug(workspaces));
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			}
		},
		async openFiles(slug) {
			try {
				await host.workspaceOpenFiles(slug);
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			}
		},
		async setPinned(slug, pinned) {
			try {
				const maxPinOrder = Math.max(
					-1,
					...workspaces.map((workspace) => workspace.pinOrder ?? -1)
				);
				await patchWorkspace(slug, { pinned, pinOrder: pinned ? maxPinOrder + 1 : null });
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			}
		},
		async reorderPinned(slugs) {
			try {
				for (const [index, slug] of slugs.entries()) {
					await patchWorkspace(slug, { pinOrder: index });
				}
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			}
		},
		async updateSessions(slug, sessions) {
			try {
				await patchWorkspace(slug, { sessions });
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			}
		},
		async setDefaultAgent(slug, agent) {
			try {
				await patchWorkspace(slug, { defaultAgent: agent });
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			}
		},
		async setActiveSession(slug, sessionId) {
			try {
				await patchWorkspace(slug, { activeSessionId: sessionId });
				error = null;
			} catch (caught) {
				error = asEnvelope(caught);
			}
		}
	};
}
