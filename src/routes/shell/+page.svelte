<script lang="ts">
	import { onMount, untrack } from 'svelte';

	import Terminal from '$lib/components/Terminal.svelte';
	import type { Envelope } from '$lib/errors/registry';
	import { tauriHost } from '$lib/host/client';
	import type { WorkspaceDto } from '$lib/host/types';
	import * as m from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import ConfirmDeleteDialog from '$lib/shell/ConfirmDeleteDialog.svelte';
	import NewWorkspaceDialog from '$lib/shell/NewWorkspaceDialog.svelte';
	import Sidebar from '$lib/shell/Sidebar.svelte';
	import TabStrip from '$lib/shell/TabStrip.svelte';
	import { sessionLaunch, sortedSessions } from '$lib/shell/model';
	import { loadCollapsed, loadTheme, saveCollapsed, saveTheme } from '$lib/shell/prefs';
	import { createSessionManager } from '$lib/shell/sessions.svelte';
	import { createShell } from '$lib/shell/store.svelte';

	const shell = createShell(tauriHost);

	let theme = $state(loadTheme());
	const manager = createSessionManager(shell, tauriHost, () => theme);
	let collapsed = $state(loadCollapsed());
	let newWorkspaceOpen = $state(false);
	let renamingSlug = $state<string | null>(null);
	let deleteTarget = $state<WorkspaceDto | null>(null);
	let menuFor = $state<{ slug: string; x: number; y: number } | null>(null);
	// Dismissal is by envelope identity, not code — a NEW failure that happens
	// to carry the same code must resurface the bar.
	let dismissedError = $state<Envelope | null>(null);

	const menuWorkspace = $derived(
		menuFor
			? (shell.workspaces.find((workspace) => workspace.slug === menuFor?.slug) ?? null)
			: null
	);
	const visibleError = $derived(shell.error !== dismissedError ? shell.error : null);

	// Workspace activation drives the tab lifecycle; untrack the manager call so
	// this effect only depends on the active workspace itself.
	$effect(() => {
		const workspace = shell.active;
		if (workspace) untrack(() => void manager.ensureActive(workspace));
	});

	// Deleted workspaces / closed sessions unmount their terminals (which kills
	// the PTYs). prune() untracks its own state internally.
	$effect(() => {
		manager.prune(shell.workspaces);
	});

	onMount(() => {
		void (async () => {
			// Sync before anything probes the guest: a rebuilt app or a missing/corrupt
			// installed binary must re-inject the shipped bytes into the distro.
			// Best-effort: a failed sync still boots; guest calls surface real errors.
			await tauriHost.guestSync().catch(() => {});
			await shell.load();
		})();
	});

	function toggleTheme(): void {
		theme = theme === 'dark' ? 'light' : 'dark';
		saveTheme(theme);
	}

	function toggleCollapsed(): void {
		collapsed = !collapsed;
		saveCollapsed(collapsed);
		menuFor = null;
	}

	function startRename(slug: string): void {
		renamingSlug = slug;
		menuFor = null;
	}

	function rename(slug: string, name: string): void {
		renamingSlug = null;
		const workspace = shell.workspaces.find((item) => item.slug === slug);
		if (workspace && name.trim() !== workspace.name) void shell.rename(slug, name);
	}

	function openMenu(slug: string, x: number, y: number): void {
		menuFor = { slug, x, y };
	}

	function closeOverlays(): void {
		menuFor = null;
	}
</script>

<svelte:window
	onclick={closeOverlays}
	onkeydown={(event) => {
		if (event.key === 'Escape') {
			newWorkspaceOpen = false;
			deleteTarget = null;
			renamingSlug = null;
			menuFor = null;
		}
	}}
/>

<div class="shell" class:collapsed class:dark={theme === 'dark'}>
	<Sidebar
		{shell}
		{collapsed}
		{theme}
		{renamingSlug}
		oncollapse={toggleCollapsed}
		ontheme={toggleTheme}
		onnew={() => {
			newWorkspaceOpen = true;
			menuFor = null;
		}}
		onmenu={openMenu}
		onrename={rename}
		onlistscroll={() => (menuFor = null)}
	/>

	<main class="main">
		<div class="ctxbar">
			{#if shell.active}
				<span class="ctx-name">{shell.active.name}</span>
				<span class="ctx-path">~/workspaces/{shell.active.slug}</span>
			{/if}
			<span class="ctx-spacer"></span>
			<button
				type="button"
				class="btn"
				disabled={!shell.active}
				title={m.shell_files()}
				onclick={() => {
					if (shell.active) void shell.openFiles(shell.active.slug);
				}}
			>
				<svg viewBox="0 0 24 24" width="15" height="15">
					<path
						d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"
					/>
				</svg>
				{m.shell_files()}
			</button>
		</div>

		{#if shell.active}
			<TabStrip
				workspace={shell.active}
				tabs={sortedSessions(shell.active.sessions)}
				activeId={manager.activeOf(shell.active.slug)}
				statuses={manager.statuses}
				oncreate={(agent) => {
					if (shell.active) void manager.create(shell.active, agent);
				}}
				onactivate={(id) => {
					if (shell.active) void manager.activate(shell.active, id);
				}}
				onclose={(id) => {
					if (shell.active) void manager.close(shell.active, id);
				}}
			/>
		{/if}

		{#if manager.advisorySlug !== null}
			<div class="advisorybar">
				<span>{m.session_advisory()}</span>
				<button
					type="button"
					aria-label={m.action_cancel()}
					onclick={() => manager.dismissAdvisory()}>×</button
				>
			</div>
		{/if}

		{#if visibleError}
			<div class="errorbar">
				<span>{m.error_title()}</span>
				<code>{visibleError.code}</code>
				<button
					type="button"
					aria-label={m.action_cancel()}
					onclick={() => (dismissedError = visibleError)}>×</button
				>
			</div>
		{/if}

		<section class="term">
			{#if shell.loading}
				<!-- Keep the surface quiet while the host list resolves. -->
			{:else if !shell.active}
				<div class="empty">
					<p class="empty-title">{m.ws_none_title()}</p>
					<button type="button" class="empty-btn" onclick={() => (newWorkspaceOpen = true)}>
						{m.ws_none_action()}
					</button>
				</div>
			{/if}
			{#each manager.open as ref (ref.sessionId)}
				{@const workspace = shell.workspaces.find((item) => item.slug === ref.slug)}
				{@const session = workspace?.sessions.find((item) => item.id === ref.sessionId)}
				{#if workspace && session}
					{@const isActive =
						ref.slug === shell.activeSlug && manager.activeOf(ref.slug) === session.id}
					<div class="term-slot" class:is-active={isActive}>
						<Terminal
							distro="Cowork"
							workspace={`~/workspaces/${ref.slug}`}
							locale={getLocale()}
							{theme}
							active={isActive}
							detectLinks
							autorun={sessionLaunch(
								session.agent,
								manager.launchUuid(session.id, session.agentSessionUuid ?? null),
								manager.isRestore(session.id)
							)}
							onactivity={(event) => manager.noteActivity(session.id, event)}
							onspawn={() => manager.noteSpawn(session.id)}
						/>
					</div>
				{/if}
			{/each}
		</section>
	</main>

	{#if menuFor && menuWorkspace}
		<div class="rowmenu" style:left={`${menuFor.x}px`} style:top={`${menuFor.y}px`} role="menu">
			<button
				type="button"
				class="rowmenu-item"
				onclick={() => {
					void shell.setPinned(menuWorkspace.slug, !menuWorkspace.pinned);
					menuFor = null;
				}}>{menuWorkspace.pinned ? m.ws_menu_unpin() : m.ws_menu_pin()}</button
			>
			<button type="button" class="rowmenu-item" onclick={() => startRename(menuWorkspace.slug)}
				>{m.ws_menu_rename()}</button
			>
			<button
				type="button"
				class="rowmenu-item"
				onclick={() => {
					void shell.openFiles(menuWorkspace.slug);
					menuFor = null;
				}}>{m.ws_menu_open_files()}</button
			>
			<button
				type="button"
				class="rowmenu-item danger"
				onclick={() => {
					deleteTarget = menuWorkspace;
					menuFor = null;
				}}>{m.ws_menu_delete()}</button
			>
		</div>
	{/if}

	{#if newWorkspaceOpen}
		<NewWorkspaceDialog {shell} host={tauriHost} onclose={() => (newWorkspaceOpen = false)} />
	{/if}

	{#if deleteTarget}
		<ConfirmDeleteDialog {shell} workspace={deleteTarget} onclose={() => (deleteTarget = null)} />
	{/if}
</div>

<style>
	.shell {
		--paper: #faf8f5;
		--paper-2: #f1ede7;
		--ink: #1c1a17;
		--ink-soft: #76705f;
		--line: #e6e0d6;
		--accent: #b5532e;
		--accent-soft: #f0e3da;
		--term-bg: #faf8f5;
		--term-fg: #2c2620;
		--term-dim: #9a9180;
		--term-accent: #b5532e;
		--menu-bg: #fff;
		--glyph-active-bg: #fff;
		position: fixed;
		inset: 0;
		display: flex;
		font-family: 'IBM Plex Sans', system-ui, sans-serif;
		color: var(--ink);
		background: var(--paper);
		-webkit-font-smoothing: antialiased;
	}
	.shell.dark {
		--paper: #1a1916;
		--paper-2: #252119;
		--ink: #ece6da;
		--ink-soft: #948c7a;
		--line: #2e2a23;
		--accent: #d2774a;
		--accent-soft: #2c2117;
		--term-bg: #1a1916;
		--term-fg: #ece6da;
		--term-dim: #8c8676;
		--term-accent: #e0875c;
		--menu-bg: #252119;
		--glyph-active-bg: #2c2117;
	}
	.shell * {
		transition:
			background-color 0.2s ease,
			border-color 0.2s ease,
			color 0.16s ease,
			opacity 0.16s ease;
	}
	.main {
		flex: 1 1 auto;
		display: flex;
		flex-direction: column;
		min-width: 0;
		background: var(--term-bg);
	}
	.ctxbar {
		display: flex;
		align-items: center;
		gap: 12px;
		height: 44px;
		padding: 0 18px;
		border-bottom: 1px solid var(--line);
		background: var(--paper);
	}
	.ctxbar svg {
		fill: none;
		stroke: currentColor;
		stroke-width: 1.7;
		stroke-linecap: round;
		stroke-linejoin: round;
		pointer-events: none;
	}
	.ctx-name {
		font-weight: 600;
		font-size: 14px;
		letter-spacing: 0;
	}
	.ctx-path {
		font-family: 'IBM Plex Mono', monospace;
		font-size: 11.5px;
		color: var(--ink-soft);
	}
	.ctx-spacer {
		flex: 1 1 auto;
	}
	.btn {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		height: 32px;
		padding: 0 11px;
		border-radius: 8px;
		font-size: 12.5px;
		font-weight: 500;
		color: var(--ink);
		border: 1px solid var(--line);
		background: var(--paper);
		flex: 0 0 auto;
	}
	.btn:disabled {
		opacity: 0.55;
	}
	.btn svg {
		color: var(--ink-soft);
	}
	.errorbar {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 30px;
		padding: 5px 18px;
		border-bottom: 1px solid var(--line);
		background: var(--accent-soft);
		color: var(--ink);
		font-size: 12px;
	}
	.errorbar code {
		font-family: 'IBM Plex Mono', monospace;
		color: var(--accent);
	}
	.errorbar button {
		margin-left: auto;
		font-size: 16px;
		line-height: 1;
		color: var(--ink-soft);
	}
	.advisorybar {
		display: flex;
		align-items: center;
		gap: 8px;
		min-height: 30px;
		padding: 5px 18px;
		border-bottom: 1px solid var(--line);
		background: var(--paper-2);
		color: var(--ink-soft);
		font-size: 12px;
	}
	.advisorybar button {
		margin-left: auto;
		font-size: 16px;
		line-height: 1;
		color: var(--ink-soft);
	}
	.term {
		position: relative;
		flex: 1 1 auto;
		min-height: 0;
		background: var(--term-bg);
		color: var(--term-fg);
	}
	.term-slot {
		position: absolute;
		inset: 0;
		visibility: hidden;
	}
	.term-slot.is-active {
		visibility: visible;
	}
	.empty {
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 12px;
	}
	.empty-title {
		font-size: 15px;
		color: var(--ink);
	}
	.empty-btn {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		padding: 9px 14px;
		border-radius: 10px;
		border: 1px solid var(--line);
		background: var(--paper);
		color: var(--ink);
		font-size: 13px;
		font-weight: 500;
	}
	.empty-btn:hover {
		background: var(--paper-2);
		border-color: color-mix(in srgb, var(--accent) 30%, var(--line));
	}
	.rowmenu {
		position: fixed;
		z-index: 30;
		min-width: 150px;
		padding: 4px;
		border-radius: 10px;
		background: var(--menu-bg);
		border: 1px solid var(--line);
		box-shadow: 0 8px 28px -8px rgba(20, 14, 8, 0.34);
	}
	.rowmenu-item {
		display: block;
		width: 100%;
		text-align: left;
		padding: 7px 9px;
		border-radius: 7px;
		font-size: 12.5px;
		color: var(--ink);
	}
	.rowmenu-item:hover:not(:disabled) {
		background: var(--paper-2);
	}
	.rowmenu-item:disabled {
		opacity: 0.45;
	}
	.rowmenu-item.danger {
		color: #b3402f;
	}
</style>
