<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { WorkspaceDto } from '$lib/host/types';
	import * as m from '$lib/paraglide/messages';
	import { dndzone, type DndEvent } from 'svelte-dnd-action';
	import WorkspaceRow from './WorkspaceRow.svelte';
	import type { Shell } from './store.svelte';

	type PinnedItem = { id: string; ws: WorkspaceDto };

	let {
		shell,
		collapsed,
		theme,
		renamingSlug,
		oncollapse,
		ontheme,
		onnew,
		onmenu,
		onrename,
		onlistscroll
	}: {
		shell: Shell;
		collapsed: boolean;
		theme: 'light' | 'dark';
		renamingSlug: string | null;
		oncollapse: () => void;
		ontheme: () => void;
		onnew: () => void;
		onmenu: (slug: string, x: number, y: number) => void;
		onrename: (slug: string, name: string) => void;
		onlistscroll: () => void;
	} = $props();

	let pinnedItems = $derived(shell.pinned.map((ws) => ({ id: ws.slug, ws })));
</script>

<aside class="sidebar">
	<header class="brand">
		{#if !collapsed}<span class="wordmark">{m.app_name()}</span>{/if}
		<button
			type="button"
			class="side-toggle"
			title={collapsed ? m.shell_expand() : m.shell_collapse()}
			onclick={oncollapse}
			aria-label={collapsed ? m.shell_expand() : m.shell_collapse()}
		>
			<svg viewBox="0 0 24 24" width="18" height="18">
				<rect width="18" height="18" x="3" y="3" rx="2" />
				<path d="M9 3v18" />
			</svg>
		</button>
	</header>

	<button type="button" class="new-ws" title={m.shell_new_workspace()} onclick={onnew}>
		<svg viewBox="0 0 24 24" width="17" height="17">
			<path d="M5 12h14" />
			<path d="M12 5v14" />
		</svg>
		{#if !collapsed}<span>{m.shell_new_workspace()}</span>{/if}
	</button>

	<nav class="ws-list" onscroll={onlistscroll}>
		{#if shell.pinned.length > 0 && !collapsed}<p class="grp-label">{m.shell_pinned()}</p>{/if}
		<div
			use:dndzone={{ items: pinnedItems, flipDurationMs: 150, dropTargetStyle: {} }}
			onconsider={(event: CustomEvent<DndEvent<PinnedItem>>) => (pinnedItems = event.detail.items)}
			onfinalize={(event: CustomEvent<DndEvent<PinnedItem>>) => {
				pinnedItems = event.detail.items;
				void shell.reorderPinned(pinnedItems.map((item) => item.id));
			}}
		>
			{#each pinnedItems as item (item.id)}
				<WorkspaceRow
					ws={item.ws}
					active={item.id === shell.activeSlug}
					{collapsed}
					renaming={renamingSlug === item.id}
					onselect={(slug) => void shell.select(slug)}
					onpin={(slug, pinned) => void shell.setPinned(slug, pinned)}
					{onmenu}
					{onrename}
				/>
			{/each}
		</div>

		{#if !collapsed}<p class="grp-label">{m.shell_recent()}</p>{/if}
		{#each shell.recent as ws (ws.slug)}
			<WorkspaceRow
				{ws}
				active={ws.slug === shell.activeSlug}
				{collapsed}
				renaming={renamingSlug === ws.slug}
				onselect={(slug) => void shell.select(slug)}
				onpin={(slug, pinned) => void shell.setPinned(slug, pinned)}
				{onmenu}
				{onrename}
			/>
		{/each}
	</nav>

	<footer class="side-foot">
		<div class="foot-row">
			<button
				type="button"
				class="foot-btn"
				title={m.shell_setup()}
				onclick={() => void goto(resolve('/setup'))}
			>
				<svg viewBox="0 0 24 24" width="17" height="17">
					<path
						d="M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z"
					/>
					<circle cx="12" cy="12" r="3" />
				</svg>
				{#if !collapsed}<span>{m.shell_setup()}</span>{/if}
			</button>
			<button
				type="button"
				class="theme-btn"
				title={m.shell_theme()}
				aria-label={m.shell_theme()}
				onclick={ontheme}
			>
				{#if theme === 'dark'}
					<svg viewBox="0 0 24 24" width="17" height="17">
						<path d="M12 3a6.36 6.36 0 0 0 9 9 9 9 0 1 1-9-9Z" />
					</svg>
				{:else}
					<svg viewBox="0 0 24 24" width="17" height="17">
						<circle cx="12" cy="12" r="4" />
						<path
							d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"
						/>
					</svg>
				{/if}
			</button>
		</div>
	</footer>
</aside>

<style>
	.sidebar {
		width: 260px;
		flex: 0 0 auto;
		display: flex;
		flex-direction: column;
		min-height: 0;
		padding: 16px 14px;
		gap: 4px;
		background: var(--paper);
		border-right: 1px solid var(--line);
		transition:
			width 0.18s ease,
			background-color 0.2s ease,
			border-color 0.2s ease;
	}
	:global(.collapsed) .sidebar {
		width: 62px;
	}
	.sidebar svg {
		fill: none;
		stroke: currentColor;
		stroke-width: 1.7;
		stroke-linecap: round;
		stroke-linejoin: round;
		pointer-events: none;
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
		letter-spacing: 0;
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
	:global(.collapsed) .brand {
		justify-content: center;
		padding: 4px 0 10px;
	}
	:global(.collapsed) .side-toggle {
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
	:global(.collapsed) .new-ws {
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
	.ws-list {
		display: flex;
		flex-direction: column;
		gap: 2px;
		flex: 1 1 auto;
		overflow-y: auto;
		overflow-x: hidden;
		margin-top: 2px;
	}
	:global(.collapsed) .grp-label {
		display: none;
	}
	:global(.collapsed) :global(.ws) {
		justify-content: center;
		padding-left: 0;
		padding-right: 0;
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
	:global(.collapsed) .foot-row {
		flex-direction: column;
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
</style>
