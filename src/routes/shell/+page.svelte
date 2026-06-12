<!--
  WP0 design DEMO — v0.2 app shell (browser-only mock at /shell; not production code).
  Reflects the locked session/nav model:
    Workspace ▸ Session(tab) ▸ Agent(brand icon). Two-level nav: sidebar=workspaces,
    header tab-strip=sessions. New Workspace ≠ New Session. Multiple sessions per workspace
    (shared files → advisory). New-session control shows the default agent's icon + picker.
    Sidebar = Pinned (manual order) + Recent (by last-used); row ⋯ menu. Light/dark, seamless,
    clay accent, IBM Plex. Drag-reorder is a WP2 behavior (not mocked here). Agent marks are
    monogram stand-ins; production needs real provider logos.
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import LiveTerminal from '$lib/components/LiveTerminal.svelte';

	// ?live=1 swaps the mock terminal for a real agent session via the pty-server harness.
	let live = $state(false);
	onMount(() => {
		live = new URLSearchParams(location.search).has('live');
	});

	type Agent = 'claude' | 'codex' | 'agy';
	// cold=restored, not spawned; idle=running, waiting; working=generating; done=finished, unread
	type Status = 'cold' | 'idle' | 'working' | 'done';
	interface Session {
		id: string;
		agent: Agent;
		n: number;
		status: Status;
	}
	// name = display label (renamable, any chars incl. Korean). slug = ~/workspaces/<slug> folder =
	// the stable, unique, IMMUTABLE id (renaming the label never moves the folder).
	interface Workspace {
		name: string;
		slug: string;
		pinned: boolean;
		def: Agent;
		sessions: Session[];
	}

	let workspaces = $state<Workspace[]>([
		{
			name: 'pdf-translate',
			slug: 'pdf-translate',
			pinned: true,
			def: 'claude',
			sessions: [
				{ id: 'a1', agent: 'claude', n: 1, status: 'working' },
				{ id: 'a2', agent: 'codex', n: 1, status: 'idle' }
			]
		},
		{
			name: 'proposal-draft',
			slug: 'proposal-draft',
			pinned: true,
			def: 'codex',
			sessions: [{ id: 'b1', agent: 'codex', n: 1, status: 'done' }]
		},
		{
			name: 'default',
			slug: 'default',
			pinned: false,
			def: 'claude',
			sessions: [{ id: 'c1', agent: 'claude', n: 1, status: 'cold' }]
		},
		{ name: 'invoice-ocr', slug: 'invoice-ocr', pinned: false, def: 'claude', sessions: [] }
	]);
	const agents: Agent[] = ['claude', 'codex', 'agy'];

	let collapsed = $state(false);
	let dark = $state(false);
	let activeSlug = $state('pdf-translate');
	let activeSessionId = $state<string | null>('a1');
	let pickerOpen = $state(false);
	let rowMenu = $state<string | null>(null);
	let idc = 100;

	const activeWs = $derived(workspaces.find((w) => w.slug === activeSlug)!);
	const activeSession = $derived(activeWs.sessions.find((s) => s.id === activeSessionId) ?? null);
	const pinned = $derived(workspaces.filter((w) => w.pinned));
	const recent = $derived(workspaces.filter((w) => !w.pinned));

	function resume(s: Session | undefined) {
		// viewing a session activates it: cold→idle (lazy spawn) and a finished one is now read
		if (s && (s.status === 'cold' || s.status === 'done')) s.status = 'idle';
	}
	function selectWs(slug: string) {
		activeSlug = slug;
		const ws = workspaces.find((w) => w.slug === slug);
		activeSessionId = ws?.sessions[0]?.id ?? null;
		resume(ws?.sessions[0]);
		rowMenu = null;
		pickerOpen = false;
	}
	function selectSession(id: string) {
		activeSessionId = id;
		resume(activeWs.sessions.find((x) => x.id === id));
	}
	function newSession(agent?: Agent) {
		const ws = activeWs;
		const ag = agent ?? ws.def;
		const n = ws.sessions.filter((s) => s.agent === ag).length + 1;
		const id = 'n' + idc++;
		ws.sessions.push({ id, agent: ag, n, status: 'idle' });
		ws.def = ag;
		activeSessionId = id;
		pickerOpen = false;
	}
	function closeSession(id: string) {
		const ws = activeWs;
		ws.sessions = ws.sessions.filter((s) => s.id !== id);
		if (activeSessionId === id) activeSessionId = ws.sessions[0]?.id ?? null;
	}

	// drag-reorder (native HTML5 DnD). tabs = free reorder; workspaces = within the pinned group only.
	// Demo only — WP2 would add a keyboard-accessible reorder (e.g. svelte-dnd-action).
	let dragTab = $state<string | null>(null);
	function tabDragOver(e: DragEvent, overId: string) {
		e.preventDefault();
		if (!dragTab || dragTab === overId) return;
		const arr = activeWs.sessions;
		const from = arr.findIndex((s) => s.id === dragTab);
		const to = arr.findIndex((s) => s.id === overId);
		if (from < 0 || to < 0) return;
		arr.splice(to, 0, arr.splice(from, 1)[0]);
	}
	let dragWs = $state<string | null>(null);
	function wsDragOver(e: DragEvent, overSlug: string) {
		e.preventDefault();
		if (!dragWs || dragWs === overSlug) return;
		const a = workspaces.find((w) => w.slug === dragWs);
		const b = workspaces.find((w) => w.slug === overSlug);
		if (!a || !b || !a.pinned || !b.pinned) return; // pinned-only reorder
		const from = workspaces.indexOf(a);
		const to = workspaces.indexOf(b);
		workspaces.splice(to, 0, workspaces.splice(from, 1)[0]);
	}
	function togglePin(slug: string) {
		const w = workspaces.find((x) => x.slug === slug)!;
		w.pinned = !w.pinned;
		rowMenu = null;
	}

	// folder slug from a display name: keep letters/numbers (incl. Korean), space/punct → '-', unique
	function slugify(name: string): string {
		const base =
			name
				.trim()
				.toLowerCase()
				.replace(/[^\p{L}\p{N}]+/gu, '-')
				.replace(/^-+|-+$/g, '') || 'workspace';
		let slug = base,
			i = 2;
		while (workspaces.some((w) => w.slug === slug)) slug = `${base}-${i++}`;
		return slug;
	}

	// Inline rename — edits the label only; slug/path is immutable
	let renaming = $state<string | null>(null);
	let renameVal = $state('');
	function startRename(w: Workspace) {
		renaming = w.slug;
		renameVal = w.name;
		rowMenu = null;
	}
	function applyRename(w: Workspace) {
		if (renaming !== w.slug) return;
		const v = renameVal.trim();
		if (v) w.name = v;
		renaming = null;
	}
	function focusSelect(node: HTMLInputElement) {
		node.focus();
		node.select();
	}

	// Delete confirmation
	let deleteTarget = $state<string | null>(null);
	const delWs = $derived(workspaces.find((w) => w.slug === deleteTarget) ?? null);
	function askDelete(w: Workspace) {
		deleteTarget = w.slug;
		rowMenu = null;
	}
	function doDelete() {
		const slug = deleteTarget;
		const wasActive = activeSlug === slug;
		workspaces = workspaces.filter((w) => w.slug !== slug);
		if (wasActive && workspaces[0]) selectWs(workspaces[0].slug);
		deleteTarget = null;
	}

	// New-workspace dialog
	let newWsOpen = $state(false);
	let newWsName = $state('');
	let newWsPreset = $state('blank');
	let newWsAgent = $state<Agent>('claude');
	const newWsSlug = $derived(newWsName.trim() ? slugify(newWsName) : '');
	const presets = [
		{ id: 'blank', name: 'Blank', desc: 'Start empty' },
		{ id: 'pdf', name: 'PDF translate', desc: 'Translate documents your way' },
		{ id: 'proposal', name: 'Proposal', desc: 'Draft proposals & documents' }
	];
	function openNewWs() {
		newWsName = '';
		newWsPreset = 'blank';
		newWsAgent = 'claude';
		newWsOpen = true;
	}
	function createWs() {
		const name = newWsName.trim();
		if (!name) return;
		const slug = slugify(name);
		workspaces.push({ name, slug, pinned: false, def: newWsAgent, sessions: [] });
		activeSlug = slug;
		activeSessionId = null;
		newWsOpen = false;
	}

	const bin = (a: Agent) => (a === 'codex' ? 'codex' : a === 'agy' ? 'agy' : 'claude');
	// Friendly product brand shown in chrome (non-devs know these, not the CLI tool names).
	const brand = (a: Agent) => (a === 'codex' ? 'ChatGPT' : a === 'agy' ? 'Gemini' : 'Claude');
	function wsState(w: Workspace): '' | 'cold' | 'idle' | 'working' | 'done' {
		if (w.sessions.some((s) => s.status === 'working')) return 'working';
		if (w.sessions.some((s) => s.status === 'done')) return 'done';
		if (w.sessions.some((s) => s.status === 'idle')) return 'idle';
		if (w.sessions.length) return 'cold';
		return '';
	}
</script>

<svelte:head>
	<link rel="preconnect" href="https://fonts.googleapis.com" />
	<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
	<link
		href="https://fonts.googleapis.com/css2?family=IBM+Plex+Mono:wght@400;500;600&family=IBM+Plex+Sans:wght@400;450;500;600&display=swap"
		rel="stylesheet"
	/>
</svelte:head>

{#snippet aicon(agent: Agent, big = false)}
	<span class="aicon {agent}" class:big>
		{#if agent === 'claude'}
			<svg viewBox="0 0 24 24"
				><path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" /></svg
			>
		{:else if agent === 'codex'}
			<svg viewBox="0 0 24 24"><path d="M9 8l-4 4 4 4M15 8l4 4-4 4" /></svg>
		{:else}
			<svg viewBox="0 0 24 24" class="fill"
				><path d="M12 3l1.9 5.6L19.5 10l-5.6 1.9L12 17.5l-1.9-5.6L4.5 10l5.6-1.9z" /></svg
			>
		{/if}
	</span>
{/snippet}

{#snippet statusGlyph(status: string)}
	{#if status === 'working'}
		<span class="spinner" title="Working"></span>
	{:else if status === 'done'}
		<span class="done-dot" title="Finished — open to view"></span>
	{/if}
{/snippet}

<svelte:window
	onclick={() => {
		pickerOpen = false;
		rowMenu = null;
	}}
	onkeydown={(e) => {
		if (e.key === 'Escape') {
			newWsOpen = false;
			deleteTarget = null;
			renaming = null;
			pickerOpen = false;
			rowMenu = null;
		}
	}}
/>

<div class="shell" class:collapsed class:dark>
	<!-- ░░ Sidebar ░░ -->
	<aside class="sidebar">
		<header class="brand">
			{#if !collapsed}<span class="wordmark">Cowork</span>{/if}
			<button
				class="side-toggle"
				title={collapsed ? 'Expand' : 'Collapse'}
				onclick={() => (collapsed = !collapsed)}
				aria-label="Toggle sidebar"
			>
				<svg viewBox="0 0 24 24" width="18" height="18"
					><rect width="18" height="18" x="3" y="3" rx="2" /><path d="M9 3v18" /></svg
				>
			</button>
		</header>

		<button class="new-ws" title="New workspace" onclick={openNewWs}>
			<svg viewBox="0 0 24 24" width="17" height="17"
				><path d="M5 12h14" /><path d="M12 5v14" /></svg
			>
			{#if !collapsed}<span>New workspace</span>{/if}
		</button>

		<nav class="ws-list">
			{#snippet wsRow(w: Workspace)}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class="ws-wrap"
					class:dragging={dragWs === w.slug}
					draggable={w.pinned && renaming !== w.slug}
					ondragstart={() => {
						if (w.pinned) dragWs = w.slug;
					}}
					ondragover={(e) => wsDragOver(e, w.slug)}
					ondrop={(e) => e.preventDefault()}
					ondragend={() => (dragWs = null)}
				>
					{#if renaming === w.slug}
						<div class="ws ws-edit">
							<span class="ws-glyph" data-agent={w.def}>{w.name.charAt(0).toUpperCase()}</span>
							<input
								class="ws-rename"
								bind:value={renameVal}
								use:focusSelect
								onkeydown={(e) => {
									if (e.key === 'Enter') applyRename(w);
									else if (e.key === 'Escape') renaming = null;
								}}
								onblur={() => applyRename(w)}
							/>
						</div>
					{:else}
						<button
							class="ws"
							class:active={w.slug === activeSlug}
							class:cold={wsState(w) === 'cold'}
							onclick={() => selectWs(w.slug)}
							title={w.name}
						>
							<span class="ws-glyph" data-agent={w.def}>{w.name.charAt(0).toUpperCase()}</span>
							{#if !collapsed}
								<span class="ws-text">
									<span class="ws-name">{w.name}</span>
									<span class="ws-meta"
										>{w.sessions.length
											? w.sessions.length + ' session' + (w.sessions.length > 1 ? 's' : '')
											: 'idle'}</span
									>
								</span>
								<span class="row-right">{@render statusGlyph(wsState(w))}</span>
							{/if}
						</button>
						{#if !collapsed}
							<div class="row-actions">
								<button
									class="act quick-pin"
									class:on={w.pinned}
									title={w.pinned ? 'Unpin' : 'Pin to top'}
									onclick={(e) => {
										e.stopPropagation();
										togglePin(w.slug);
									}}
								>
									<svg class="pin-svg" viewBox="0 0 24 24" width="14" height="14"
										><path d="M12 17v5M9 3h6l-1 7 3 3H7l3-3-1-7z" /></svg
									>
								</button>
								<button
									class="act kebab"
									title="More"
									onclick={(e) => {
										e.stopPropagation();
										pickerOpen = false;
										rowMenu = rowMenu === w.slug ? null : w.slug;
									}}
								>
									<svg viewBox="0 0 24 24" width="16" height="16" class="fill"
										><circle cx="12" cy="12" r="1.4" /><circle cx="19" cy="12" r="1.4" /><circle
											cx="5"
											cy="12"
											r="1.4"
										/></svg
									>
								</button>
							</div>
						{/if}
						{#if rowMenu === w.slug && !collapsed}
							<div class="rowmenu">
								<button class="rowmenu-item" onclick={() => togglePin(w.slug)}
									>{w.pinned ? 'Unpin' : 'Pin to top'}</button
								>
								<button class="rowmenu-item" onclick={() => startRename(w)}>Rename</button>
								<button class="rowmenu-item">Open files…</button>
								<button class="rowmenu-item danger" onclick={() => askDelete(w)}>Delete</button>
							</div>
						{/if}
					{/if}
				</div>
			{/snippet}

			{#if pinned.length && !collapsed}<p class="grp-label">Pinned</p>{/if}
			{#each pinned as w (w.slug)}{@render wsRow(w)}{/each}
			{#if !collapsed}<p class="grp-label">Recent</p>{/if}
			{#each recent as w (w.slug)}{@render wsRow(w)}{/each}
		</nav>

		<footer class="side-foot">
			<div class="foot-row">
				<button class="foot-btn" title="Setup">
					<svg viewBox="0 0 24 24" width="17" height="17"
						><path
							d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
						/><circle cx="12" cy="12" r="3" /></svg
					>
					{#if !collapsed}<span>Setup</span>{/if}
				</button>
				<button
					class="theme-btn"
					title="Toggle theme"
					aria-label="Toggle theme"
					onclick={() => (dark = !dark)}
				>
					{#if dark}
						<svg viewBox="0 0 24 24" width="17" height="17"
							><path d="M12 3a6.36 6.36 0 0 0 9 9 9 9 0 1 1-9-9Z" /></svg
						>
					{:else}
						<svg viewBox="0 0 24 24" width="17" height="17"
							><circle cx="12" cy="12" r="4" /><path
								d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"
							/></svg
						>
					{/if}
				</button>
			</div>
			{#if !collapsed}<span class="acct">um@kdccy.com</span>{/if}
		</footer>
	</aside>

	<!-- ░░ Main ░░ -->
	<main class="main">
		<div class="ctxbar">
			<span class="ctx-name">{activeWs.name}</span>
			<span class="ctx-path">~/workspaces/{activeWs.slug}</span>
			{#if activeWs.sessions.length > 1}
				<span
					class="advisory"
					title="Sessions share this workspace's files — avoid editing the same file at once"
				>
					<svg viewBox="0 0 24 24" width="12" height="12"
						><path
							d="M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"
						/></svg
					>
					shared files
				</span>
			{/if}
			<span class="ctx-spacer"></span>
			<button class="btn">
				<svg viewBox="0 0 24 24" width="15" height="15"
					><path
						d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"
					/></svg
				>
				Files
			</button>
		</div>

		<div class="tabstrip">
			{#each activeWs.sessions as s (s.id)}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<div
					class="sess-tab"
					class:on={s.id === activeSessionId}
					class:cold={s.status === 'cold'}
					class:dragging={dragTab === s.id}
					draggable="true"
					ondragstart={() => (dragTab = s.id)}
					ondragover={(e) => tabDragOver(e, s.id)}
					ondrop={(e) => e.preventDefault()}
					ondragend={() => (dragTab = null)}
				>
					<button type="button" class="sess-tab-main" onclick={() => selectSession(s.id)}>
						{@render aicon(s.agent)}
						<span class="tab-label">{brand(s.agent)} #{s.n}</span>
						{@render statusGlyph(s.status)}
					</button>
					<button
						type="button"
						class="tab-x"
						title="Close"
						onclick={(e) => {
							e.stopPropagation();
							closeSession(s.id);
						}}>×</button
					>
				</div>
			{/each}

			<div class="newsess">
				<button
					class="newsess-main"
					title="New {brand(activeWs.def)} session"
					onclick={() => newSession()}
				>
					{@render aicon(activeWs.def)}
					<svg class="plus" viewBox="0 0 24 24" width="13" height="13"
						><path d="M5 12h14M12 5v14" /></svg
					>
				</button>
				<button
					class="newsess-caret"
					title="Choose agent"
					onclick={(e) => {
						e.stopPropagation();
						rowMenu = null;
						pickerOpen = !pickerOpen;
					}}
				>
					<svg viewBox="0 0 24 24" width="12" height="12"><path d="m6 9 6 6 6-6" /></svg>
				</button>
				{#if pickerOpen}
					<div class="picker">
						<p class="picker-label">New session with…</p>
						{#each agents as a (a)}
							<button class="picker-item" onclick={() => newSession(a)}
								>{@render aicon(a)}<span>{brand(a)}</span></button
							>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<section class="term">
			<div class="term-inner">
				{#if !activeSession}
					<div class="empty">
						<p class="empty-title">No session in <b>{activeWs.name}</b> yet.</p>
						<p class="empty-sub">
							Drop files into the workspace, then start an agent to work on them.
						</p>
						<button class="empty-btn" onclick={() => newSession()}
							>{@render aicon(activeWs.def)} Start a {brand(activeWs.def)} session</button
						>
					</div>
				{:else if live}
					{#key activeSession.id}
						<LiveTerminal agent={activeSession.agent} slug={activeWs.slug} {dark} />
					{/key}
				{:else}
					<div class="t-dim">
						cowork@Cowork:~/workspaces/{activeWs.slug}$ {bin(activeSession.agent)}
					</div>
					{#if activeSession.agent === 'claude'}
						<div class="t-banner">
							<span class="t-accent">●</span> Claude Code · ~/workspaces/{activeWs.slug}
						</div>
						<div class="t-dim">
							Type your task, or paste a file into the Files folder and ask about it.
						</div>
						<div class="t-row">
							<span class="t-accent">&gt;</span> translate the quarterly report into Korean<span
								class="caret"
							></span>
						</div>
					{:else if activeSession.agent === 'codex'}
						<div class="t-banner">
							<span class="t-accent">●</span> Codex CLI · ~/workspaces/{activeWs.slug}
						</div>
						<div class="t-row"><span class="t-accent">&gt;</span> <span class="caret"></span></div>
					{:else}
						<div class="t-banner">
							<span class="t-accent">●</span> Antigravity · ~/workspaces/{activeWs.slug}
						</div>
						<div class="t-row"><span class="t-accent">&gt;</span> <span class="caret"></span></div>
					{/if}
					{#if activeSession.status === 'working'}<div class="t-work">
							<span class="spinner"></span> Working…
						</div>{/if}
				{/if}
			</div>
		</section>
	</main>

	{#if newWsOpen}
		<div class="modal-layer">
			<button class="modal-backdrop" aria-label="Close" onclick={() => (newWsOpen = false)}
			></button>
			<div class="modal" role="dialog" aria-modal="true">
				<h2 class="modal-title">New workspace</h2>
				<label class="field">
					<span class="field-label">Name</span>
					<input
						class="field-input"
						bind:value={newWsName}
						placeholder="e.g. Invoice OCR"
						spellcheck="false"
					/>
					{#if newWsSlug}<span class="slug-hint"
							>Folder: ~/workspaces/{newWsSlug} · name can be changed later</span
						>{/if}
				</label>
				<div class="field">
					<span class="field-label">Preset</span>
					<div class="preset-list">
						{#each presets as ps (ps.id)}
							<button
								class="preset"
								class:on={newWsPreset === ps.id}
								onclick={() => (newWsPreset = ps.id)}
							>
								<span class="preset-name">{ps.name}</span>
								<span class="preset-desc">{ps.desc}</span>
							</button>
						{/each}
					</div>
				</div>
				<div class="field">
					<span class="field-label">Default agent</span>
					<div class="agent-row">
						{#each agents as a (a)}
							<button
								class="agent-opt"
								class:on={newWsAgent === a}
								onclick={() => (newWsAgent = a)}
							>
								{@render aicon(a)}<span>{brand(a)}</span>
							</button>
						{/each}
					</div>
				</div>
				<div class="modal-actions">
					<button class="m-btn" onclick={() => (newWsOpen = false)}>Cancel</button>
					<button class="m-btn primary" disabled={!newWsName.trim()} onclick={createWs}
						>Create workspace</button
					>
				</div>
			</div>
		</div>
	{/if}

	{#if delWs}
		<div class="modal-layer">
			<button class="modal-backdrop" aria-label="Cancel" onclick={() => (deleteTarget = null)}
			></button>
			<div class="modal modal-sm" role="dialog" aria-modal="true">
				<h2 class="modal-title">Delete workspace?</h2>
				<p class="modal-text">
					<b>{delWs.name}</b> and its files at <code>~/workspaces/{delWs.slug}</code> will be permanently
					removed. This can't be undone.
				</p>
				<div class="modal-actions">
					<button class="m-btn" onclick={() => (deleteTarget = null)}>Cancel</button>
					<button class="m-btn danger" onclick={doDelete}>Delete workspace</button>
				</div>
			</div>
		</div>
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
		--live: #5a7d53;
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
		--live: #79a06b;
	}
	.shell svg {
		fill: none;
		stroke: currentColor;
		stroke-width: 1.7;
		stroke-linecap: round;
		stroke-linejoin: round;
		pointer-events: none;
	}
	.shell * {
		transition:
			background-color 0.2s ease,
			border-color 0.2s ease,
			color 0.16s ease,
			opacity 0.16s ease;
	}

	/* agent brand icons (monogram stand-ins) */
	.aicon {
		width: 18px;
		height: 18px;
		border-radius: 5px;
		display: grid;
		place-items: center;
		flex: 0 0 auto;
	}
	.aicon svg {
		width: 11px;
		height: 11px;
		stroke: #fff;
		stroke-width: 2.2;
	}
	.aicon svg.fill {
		fill: #fff;
		stroke: none;
	}
	.aicon.claude {
		background: var(--accent);
	}
	.aicon.codex {
		background: #3d6b57;
	}
	.aicon.agy {
		background: #7a6cae;
	}
	.aicon.big {
		width: 22px;
		height: 22px;
		border-radius: 6px;
	}
	.aicon.big svg {
		width: 13px;
		height: 13px;
	}

	/* ── Sidebar ── */
	.sidebar {
		width: 260px;
		flex: 0 0 auto;
		display: flex;
		flex-direction: column;
		padding: 16px 14px;
		gap: 4px;
		background: var(--paper);
		border-right: 1px solid var(--line);
		transition:
			width 0.18s ease,
			background-color 0.2s ease,
			border-color 0.2s ease;
	}
	.collapsed .sidebar {
		width: 62px;
	}
	.brand {
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 4px 4px 14px;
	}
	.wordmark {
		font-weight: 600;
		font-size: 15px;
		letter-spacing: -0.01em;
	}
	.side-toggle {
		margin-left: auto;
		color: var(--ink);
		width: 30px;
		height: 30px;
		border-radius: 8px;
		border: 1px solid var(--line);
		background: var(--paper);
		display: grid;
		place-items: center;
		flex: 0 0 auto;
	}
	.side-toggle:hover {
		background: var(--paper-2);
		border-color: color-mix(in srgb, var(--accent) 30%, var(--line));
	}
	.collapsed .brand {
		justify-content: center;
		padding: 4px 0 10px;
	}
	.collapsed .side-toggle {
		margin-left: 0;
	}

	.new-ws {
		display: flex;
		align-items: center;
		gap: 10px;
		padding: 10px;
		border-radius: 10px;
		color: var(--ink);
		font-size: 13.5px;
		font-weight: 500;
		border: 1px solid var(--line);
		background: var(--paper);
	}
	.new-ws:hover {
		background: var(--paper-2);
		border-color: color-mix(in srgb, var(--accent) 30%, var(--line));
	}
	.new-ws svg {
		color: var(--accent);
	}
	.collapsed .new-ws {
		justify-content: center;
		padding-left: 0;
		padding-right: 0;
	}

	.grp-label {
		font-size: 10.5px;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--ink-soft);
		padding: 12px 8px 4px;
	}
	/* demo: overflow visible so the ⋯ row menu isn't clipped. Production: portal menus + scroll. */
	.ws-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
		flex: 1 1 auto;
		overflow: visible;
		margin-top: 2px;
	}
	.ws-wrap {
		position: relative;
	}
	.ws-wrap[draggable='true'] .ws {
		cursor: grab;
	}
	.sess-tab[draggable='true'] {
		cursor: grab;
	}
	.dragging {
		opacity: 0.4;
	}
	.ws {
		display: flex;
		align-items: center;
		gap: 11px;
		padding: 9px 9px;
		border-radius: 9px;
		text-align: left;
		width: 100%;
	}
	.ws:hover {
		background: var(--paper-2);
	}
	.ws.active {
		background: var(--accent-soft);
	}
	.ws-glyph {
		width: 27px;
		height: 27px;
		flex: 0 0 auto;
		border-radius: 8px;
		display: grid;
		place-items: center;
		font-family: 'IBM Plex Mono', monospace;
		font-size: 12px;
		font-weight: 500;
		color: var(--ink);
		background: var(--paper-2);
		border: 1px solid var(--line);
	}
	.ws.active .ws-glyph {
		background: var(--glyph-active-bg);
		border-color: color-mix(in srgb, var(--accent) 30%, var(--line));
		color: var(--accent);
	}
	.ws-text {
		display: flex;
		flex-direction: column;
		min-width: 0;
		flex: 1 1 auto;
	}
	.ws-name {
		font-size: 13.5px;
		font-weight: 500;
		letter-spacing: -0.005em;
	}
	.ws-meta {
		font-family: 'IBM Plex Mono', monospace;
		font-size: 10.5px;
		color: var(--ink-soft);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.row-right {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		flex: 0 0 auto;
	}
	.pin-ic {
		color: var(--ink-soft);
	}
	.row-actions {
		position: absolute;
		right: 6px;
		top: 50%;
		transform: translateY(-50%);
		display: flex;
		gap: 2px;
		opacity: 0;
	}
	.ws-wrap:hover .row-actions {
		opacity: 1;
	}
	.ws-wrap:hover .row-right {
		opacity: 0;
	}
	.act {
		width: 26px;
		height: 26px;
		display: grid;
		place-items: center;
		border-radius: 7px;
		color: var(--ink-soft);
	}
	.act:hover {
		background: color-mix(in srgb, var(--ink) 10%, transparent);
		color: var(--ink);
	}
	.act .fill {
		fill: currentColor;
		stroke: none;
	}
	.quick-pin.on {
		color: var(--accent);
	}
	.quick-pin.on .pin-svg {
		fill: currentColor;
	}
	.rowmenu {
		position: absolute;
		right: 6px;
		top: 38px;
		z-index: 20;
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
	.rowmenu-item:hover {
		background: var(--paper-2);
	}
	.rowmenu-item.danger {
		color: #b3402f;
	}

	.collapsed .grp-label,
	.collapsed .acct {
		display: none;
	}
	.collapsed .ws {
		justify-content: center;
		padding-left: 0;
		padding-right: 0;
	}
	.collapsed .foot-row {
		flex-direction: column;
	}

	.live-dot {
		width: 7px;
		height: 7px;
		border-radius: 999px;
		flex: 0 0 auto;
		background: var(--live);
		box-shadow: 0 0 0 3px color-mix(in srgb, var(--live) 22%, transparent);
	}
	.live-dot.sm {
		width: 6px;
		height: 6px;
		box-shadow: none;
	}
	/* status: cold/idle shown by item brightness (.cold = muted); working=spinner, done=green dot */
	.ws.cold,
	.sess-tab.cold {
		opacity: 0.5;
	}
	.done-dot {
		width: 7px;
		height: 7px;
		border-radius: 999px;
		background: var(--live);
		flex: 0 0 auto;
	}
	.spinner {
		width: 12px;
		height: 12px;
		box-sizing: border-box;
		border-radius: 999px;
		border: 2px solid color-mix(in srgb, var(--accent) 28%, transparent);
		border-top-color: var(--accent);
		animation: spin 0.7s linear infinite;
		flex: 0 0 auto;
	}
	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.side-foot {
		margin-top: 6px;
		padding-top: 12px;
		border-top: 1px solid var(--line);
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.foot-row {
		display: flex;
		align-items: center;
		gap: 6px;
	}
	.foot-btn {
		display: flex;
		align-items: center;
		gap: 9px;
		padding: 8px;
		border-radius: 8px;
		font-size: 13px;
		color: var(--ink-soft);
		flex: 1 1 auto;
	}
	.foot-btn:hover,
	.theme-btn:hover {
		background: var(--paper-2);
		color: var(--ink);
	}
	.theme-btn {
		color: var(--ink-soft);
		padding: 8px;
		border-radius: 8px;
		display: grid;
		place-items: center;
		flex: 0 0 auto;
	}
	.acct {
		font-family: 'IBM Plex Mono', monospace;
		font-size: 10.5px;
		color: var(--ink-soft);
		padding: 0 8px;
	}

	/* ── Main ── */
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
	.ctx-name {
		font-weight: 600;
		font-size: 14px;
		letter-spacing: -0.01em;
	}
	.ctx-path {
		font-family: 'IBM Plex Mono', monospace;
		font-size: 11.5px;
		color: var(--ink-soft);
	}
	.advisory {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		font-size: 11px;
		color: #9a6a2e;
		background: color-mix(in srgb, #c98a3d 16%, transparent);
		padding: 3px 8px;
		border-radius: 999px;
	}
	.advisory svg {
		color: #b5792f;
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
	.btn:hover {
		background: var(--paper-2);
		border-color: color-mix(in srgb, var(--accent) 25%, var(--line));
	}
	.btn svg {
		color: var(--ink-soft);
	}

	/* demo: overflow visible so the new-session picker isn't clipped. Production: portal + scroll. */
	.tabstrip {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 46px;
		padding: 0 12px;
		border-bottom: 1px solid var(--line);
		background: var(--paper);
	}
	.sess-tab {
		display: inline-flex;
		align-items: center;
		height: 32px;
		padding: 0 6px 0 8px;
		border-radius: 9px;
		border: 1px solid transparent;
		color: var(--ink-soft);
		flex: 0 0 auto;
	}
	.sess-tab:hover {
		background: var(--paper-2);
	}
	.sess-tab.on {
		background: var(--accent-soft);
		color: var(--ink);
		border-color: color-mix(in srgb, var(--accent) 22%, var(--line));
	}
	.sess-tab-main {
		display: inline-flex;
		align-items: center;
		gap: 8px;
		height: 100%;
		color: inherit;
	}
	.tab-label {
		font-family: 'IBM Plex Mono', monospace;
		font-size: 12px;
	}
	.tab-x {
		display: grid;
		place-items: center;
		width: 16px;
		height: 16px;
		border-radius: 5px;
		font-size: 15px;
		line-height: 1;
		color: var(--ink-soft);
	}
	.tab-x:hover {
		background: color-mix(in srgb, var(--ink) 12%, transparent);
		color: var(--ink);
	}

	.newsess {
		position: relative;
		display: inline-flex;
		align-items: center;
		flex: 0 0 auto;
	}
	.newsess-main {
		display: inline-flex;
		align-items: center;
		gap: 5px;
		height: 32px;
		padding: 0 7px 0 6px;
		border: 1px solid var(--line);
		border-right: none;
		border-radius: 9px 0 0 9px;
		background: var(--paper);
		color: var(--ink-soft);
	}
	.newsess-caret {
		display: grid;
		place-items: center;
		height: 32px;
		width: 24px;
		border: 1px solid var(--line);
		border-radius: 0 9px 9px 0;
		background: var(--paper);
		color: var(--ink-soft);
	}
	.newsess-main:hover,
	.newsess-caret:hover {
		background: var(--paper-2);
		color: var(--ink);
	}
	.newsess-main .plus {
		width: 13px;
		height: 13px;
	}
	.picker {
		position: absolute;
		top: 38px;
		right: 0;
		z-index: 20;
		min-width: 172px;
		padding: 5px;
		border-radius: 11px;
		background: var(--menu-bg);
		border: 1px solid var(--line);
		box-shadow: 0 10px 30px -10px rgba(20, 14, 8, 0.38);
	}
	.picker-label {
		font-size: 10.5px;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--ink-soft);
		padding: 4px 8px 6px;
	}
	.picker-item {
		display: flex;
		align-items: center;
		gap: 10px;
		width: 100%;
		padding: 8px 8px;
		border-radius: 8px;
		text-align: left;
		font-size: 13px;
		color: var(--ink);
	}
	.picker-item:hover {
		background: var(--paper-2);
	}

	/* ── Terminal ── */
	.term {
		flex: 1 1 auto;
		min-height: 0;
		background: var(--term-bg);
	}
	.term-inner {
		height: 100%;
		padding: 22px 28px;
		font-family: 'IBM Plex Mono', monospace;
		font-size: 13px;
		line-height: 1.85;
		color: var(--term-fg);
		overflow: hidden;
	}
	.t-dim {
		color: var(--term-dim);
	}
	.t-banner {
		margin-top: 6px;
	}
	.t-accent {
		color: var(--term-accent);
	}
	.t-row {
		margin-top: 2px;
	}
	.t-work {
		margin-top: 4px;
		display: flex;
		align-items: center;
		gap: 8px;
		color: var(--term-dim);
	}
	.work-dot {
		width: 7px;
		height: 7px;
		border-radius: 999px;
		background: var(--term-accent);
		animation: pulse 1.3s ease-in-out infinite;
	}
	.caret {
		display: inline-block;
		width: 8px;
		height: 16px;
		margin-left: 2px;
		vertical-align: text-bottom;
		background: var(--term-fg);
		animation: blink 1.1s steps(1) infinite;
	}
	@keyframes blink {
		50% {
			opacity: 0;
		}
	}

	/* ── New-workspace modal ── */
	.modal-layer {
		position: fixed;
		inset: 0;
		z-index: 50;
		display: grid;
		place-items: center;
	}
	.modal-backdrop {
		position: absolute;
		inset: 0;
		background: rgba(20, 14, 8, 0.4);
		border: none;
		cursor: default;
	}
	.modal {
		position: relative;
		width: 420px;
		max-width: calc(100vw - 32px);
		padding: 22px;
		border-radius: 16px;
		background: var(--menu-bg);
		border: 1px solid var(--line);
		box-shadow: 0 24px 60px -20px rgba(20, 14, 8, 0.5);
		display: flex;
		flex-direction: column;
		gap: 16px;
		animation: menuIn 0.14s ease;
	}
	.modal-title {
		font-size: 16px;
		font-weight: 600;
		letter-spacing: -0.01em;
	}
	.field {
		display: flex;
		flex-direction: column;
		gap: 7px;
	}
	.field-label {
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--ink-soft);
	}
	.field-input {
		height: 38px;
		padding: 0 12px;
		border-radius: 9px;
		border: 1px solid var(--line);
		background: var(--paper);
		color: var(--ink);
		font-family: 'IBM Plex Mono', monospace;
		font-size: 13px;
	}
	.field-input:focus {
		outline: none;
		border-color: var(--accent);
	}
	.preset-list {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}
	.preset {
		display: flex;
		flex-direction: column;
		gap: 2px;
		padding: 9px 11px;
		border-radius: 10px;
		border: 1px solid var(--line);
		background: var(--paper);
		text-align: left;
	}
	.preset:hover {
		background: var(--paper-2);
	}
	.preset.on {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.preset-name {
		font-size: 13px;
		font-weight: 500;
		color: var(--ink);
	}
	.preset-desc {
		font-size: 11.5px;
		color: var(--ink-soft);
	}
	.agent-row {
		display: flex;
		gap: 6px;
	}
	.agent-opt {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		padding: 7px 11px;
		border-radius: 9px;
		border: 1px solid var(--line);
		background: var(--paper);
		font-size: 12.5px;
		color: var(--ink);
	}
	.agent-opt:hover {
		background: var(--paper-2);
	}
	.agent-opt.on {
		border-color: var(--accent);
		background: var(--accent-soft);
	}
	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
		margin-top: 4px;
	}
	.m-btn {
		height: 36px;
		padding: 0 14px;
		border-radius: 9px;
		border: 1px solid var(--line);
		background: var(--paper);
		color: var(--ink);
		font-size: 13px;
		font-weight: 500;
	}
	.m-btn:hover {
		background: var(--paper-2);
	}
	.m-btn.primary {
		background: var(--accent);
		border-color: var(--accent);
		color: #fff;
	}
	.m-btn.primary:hover {
		background: color-mix(in srgb, var(--accent) 88%, #000);
	}
	.m-btn.primary:disabled {
		opacity: 0.45;
	}
	.m-btn.danger {
		background: #b3402f;
		border-color: #b3402f;
		color: #fff;
	}
	.m-btn.danger:hover {
		background: #9c3526;
	}
	.modal-sm {
		width: 380px;
	}
	.modal-text {
		font-size: 13px;
		line-height: 1.55;
		color: var(--ink-soft);
	}
	.modal-text code,
	.slug-hint {
		font-family: 'IBM Plex Mono', monospace;
	}
	.modal-text code {
		color: var(--ink);
	}
	.slug-hint {
		font-size: 11px;
		color: var(--ink-soft);
	}

	/* inline rename */
	.ws-edit {
		display: flex;
		align-items: center;
		gap: 11px;
		padding: 9px 9px;
		border-radius: 9px;
	}
	.ws-rename {
		flex: 1 1 auto;
		min-width: 0;
		height: 26px;
		padding: 0 8px;
		border-radius: 7px;
		border: 1px solid var(--accent);
		background: var(--paper);
		color: var(--ink);
		font-size: 13.5px;
		font-weight: 500;
		font-family: inherit;
	}
	.ws-rename:focus {
		outline: none;
	}

	.empty {
		height: 100%;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 8px;
		font-family: 'IBM Plex Sans', sans-serif;
	}
	.empty-title {
		font-size: 15px;
		color: var(--ink);
	}
	.empty-sub {
		font-size: 12.5px;
		color: var(--term-dim);
		margin-bottom: 8px;
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
</style>
