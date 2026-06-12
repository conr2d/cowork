<script lang="ts">
	import * as m from '$lib/paraglide/messages';
	import type { WorkspaceDto } from '$lib/host/types';
	import { brand } from './model';

	let {
		ws,
		active = false,
		collapsed = false,
		renaming = false,
		onselect,
		onpin,
		onmenu,
		onrename
	}: {
		ws: WorkspaceDto;
		active?: boolean;
		collapsed?: boolean;
		renaming?: boolean;
		onselect: (slug: string) => void;
		onpin: (slug: string, pinned: boolean) => void;
		onmenu: (slug: string, x: number, y: number) => void;
		onrename: (slug: string, name: string) => void;
	} = $props();

	let renameValue = $state('');
	let input: HTMLInputElement | undefined = $state();

	$effect(() => {
		if (renaming) {
			renameValue = ws.name;
			setTimeout(() => {
				input?.focus();
				input?.select();
			});
		}
	});

	function applyRename(): void {
		const value = renameValue.trim();
		onrename(ws.slug, value || ws.name);
	}

	function menuPosition(node: HTMLButtonElement): void {
		const rect = node.getBoundingClientRect();
		onmenu(ws.slug, rect.right - 150, rect.bottom + 4);
	}
</script>

<div class="ws-wrap">
	{#if renaming}
		<div class="ws ws-edit">
			<span class="ws-glyph">{ws.name.charAt(0).toUpperCase()}</span>
			<input
				bind:this={input}
				class="ws-rename"
				bind:value={renameValue}
				onkeydown={(event) => {
					if (event.key === 'Enter') applyRename();
					else if (event.key === 'Escape') onrename(ws.slug, ws.name);
				}}
				onblur={applyRename}
			/>
		</div>
	{:else}
		<button
			type="button"
			class="ws"
			class:active
			onclick={() => onselect(ws.slug)}
			title={ws.name}
			aria-label={ws.name}
		>
			<span class="ws-glyph">{ws.name.charAt(0).toUpperCase()}</span>
			{#if !collapsed}
				<span class="ws-text">
					<span class="ws-name">{ws.name}</span>
					<span class="ws-meta">{brand(ws.defaultAgent)} · {ws.preset}</span>
				</span>
			{/if}
		</button>
		{#if !collapsed}
			<div class="row-actions">
				<button
					type="button"
					class="act quick-pin"
					class:on={ws.pinned}
					title={ws.pinned ? m.ws_menu_unpin() : m.ws_menu_pin()}
					aria-label={ws.pinned ? m.ws_menu_unpin() : m.ws_menu_pin()}
					onclick={(event) => {
						event.stopPropagation();
						onpin(ws.slug, !ws.pinned);
					}}
				>
					<svg class="pin-svg" viewBox="0 0 24 24" width="14" height="14">
						<path d="M12 17v5M9 3h6l-1 7 3 3H7l3-3-1-7z" />
					</svg>
				</button>
				<button
					type="button"
					class="act kebab"
					title={m.ws_menu_more()}
					aria-label={m.ws_menu_more()}
					onclick={(event) => {
						event.stopPropagation();
						menuPosition(event.currentTarget);
					}}
				>
					<svg viewBox="0 0 24 24" width="16" height="16" class="fill">
						<circle cx="12" cy="12" r="1.4" />
						<circle cx="19" cy="12" r="1.4" />
						<circle cx="5" cy="12" r="1.4" />
					</svg>
				</button>
			</div>
		{/if}
	{/if}
</div>

<style>
	.ws-wrap {
		position: relative;
	}
	.ws {
		display: flex;
		align-items: center;
		gap: 11px;
		padding: 9px;
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
		padding-right: 52px;
	}
	.ws-name {
		font-size: 13.5px;
		font-weight: 500;
		letter-spacing: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.ws-meta {
		font-family: 'IBM Plex Mono', monospace;
		font-size: 10.5px;
		color: var(--ink-soft);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
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
	.ws-wrap:hover .row-actions,
	.row-actions:focus-within {
		opacity: 1;
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
	.act svg {
		fill: none;
		stroke: currentColor;
		stroke-width: 1.7;
		stroke-linecap: round;
		stroke-linejoin: round;
		pointer-events: none;
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
	.ws-edit {
		display: flex;
		align-items: center;
		gap: 11px;
		padding: 9px;
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
</style>
