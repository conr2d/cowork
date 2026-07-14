<script lang="ts">
	import type { SessionDto, WorkspaceDto } from '$lib/host/types';
	import * as m from '$lib/paraglide/messages';
	import type { AgentId } from '$lib/terminal/agent';
	import AgentIcon from './AgentIcon.svelte';
	import { brand, type SessionStatus } from './model';

	let {
		workspace,
		tabs,
		activeId,
		statuses,
		oncreate,
		onactivate,
		onclose
	}: {
		workspace: WorkspaceDto;
		tabs: readonly SessionDto[];
		activeId: string | null;
		statuses: Readonly<Record<string, SessionStatus>>;
		oncreate: (agent: AgentId | null) => void;
		onactivate: (id: string) => void;
		onclose: (id: string) => void;
	} = $props();

	const AGENTS: readonly AgentId[] = ['claude', 'codex', 'antigravity'];
	const PICKER_WIDTH = 160;
	const EDGE_GAP = 8;

	let pickerOpen = $state(false);
	let pickBtn: HTMLButtonElement | undefined = $state();
	let pickerPos = $state({ top: 0, left: 0 });

	// The actions live INSIDE the scrolling rail (⊕ sits right after the last tab,
	// as browsers do), and that rail clips — which is what hid this menu once
	// already. So the menu escapes every clipping ancestor with position: fixed,
	// anchored to the button's measured rect and clamped to the window. It is
	// closed on scroll/resize rather than re-measured, because a menu that drifts
	// away from its button is worse than one that closes.
	function openPicker(): void {
		const rect = pickBtn?.getBoundingClientRect();
		if (!rect) return;
		const maxLeft = window.innerWidth - PICKER_WIDTH - EDGE_GAP;
		pickerPos = {
			top: rect.bottom + 4,
			left: Math.max(EDGE_GAP, Math.min(rect.left, maxLeft))
		};
		pickerOpen = true;
	}
</script>

<svelte:window
	onclick={() => (pickerOpen = false)}
	onresize={() => (pickerOpen = false)}
	onscroll={() => (pickerOpen = false)}
/>

<div class="tabbar">
	<div class="tabrail" role="tablist">
		{#each tabs as tab (tab.id)}
			<div class="tab" class:is-active={tab.id === activeId}>
				<button
					type="button"
					role="tab"
					aria-selected={tab.id === activeId}
					class="tab-main"
					onclick={() => onactivate(tab.id)}
				>
					<span class="dot st-{statuses[tab.id] ?? 'cold'}" aria-hidden="true"></span>
					<AgentIcon agent={tab.agent} />
					<span class="tab-title">{tab.title}</span>
				</button>
				<button
					type="button"
					class="tab-close"
					aria-label={m.tab_close()}
					onclick={() => onclose(tab.id)}>×</button
				>
			</div>
		{/each}
		<div class="tabactions">
			<button type="button" class="addbtn" title={m.tab_new()} onclick={() => oncreate(null)}>
				＋
			</button>
			<button
				bind:this={pickBtn}
				type="button"
				class="pickbtn"
				aria-label={m.tab_new_with()}
				onclick={(event) => {
					event.stopPropagation();
					if (pickerOpen) pickerOpen = false;
					else openPicker();
				}}>▾</button
			>
		</div>
	</div>
</div>

{#if pickerOpen}
	<div
		class="picker"
		role="menu"
		style:top={`${pickerPos.top}px`}
		style:left={`${pickerPos.left}px`}
		style:width={`${PICKER_WIDTH}px`}
	>
		{#each AGENTS as agent (agent)}
			<button
				type="button"
				class="picker-item"
				class:is-default={agent === workspace.defaultAgent}
				onclick={() => {
					pickerOpen = false;
					oncreate(agent);
				}}
			>
				<AgentIcon {agent} />
				<span>{brand(agent)}</span>
			</button>
		{/each}
	</div>
{/if}

<style>
	.tabbar {
		display: flex;
		align-items: center;
		gap: 4px;
		min-height: 36px;
		padding: 0 10px;
		border-bottom: 1px solid var(--line);
		background: var(--paper);
		overflow: visible;
	}
	.tabrail {
		display: flex;
		min-width: 0;
		flex: 1 1 auto;
		align-items: center;
		gap: 4px;
		overflow: auto hidden;
		padding-bottom: 1px;
		scrollbar-width: thin;
	}
	.tab {
		display: inline-flex;
		align-items: center;
		border: 1px solid transparent;
		border-radius: 8px 8px 0 0;
		flex: 0 0 auto;
	}
	.tab.is-active {
		background: var(--term-bg);
		border-color: var(--line);
		border-bottom-color: var(--term-bg);
		margin-bottom: -1px;
	}
	.tab-main {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		padding: 6px 4px 6px 10px;
		font-size: 12.5px;
		color: var(--ink);
	}
	.tab:not(.is-active) .tab-main {
		color: var(--ink-soft);
	}
	.tab-title {
		max-width: 140px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.dot {
		width: 7px;
		height: 7px;
		border-radius: 50%;
		flex: 0 0 auto;
	}
	.dot.st-cold {
		border: 1.5px solid var(--ink-soft);
	}
	.dot.st-idle {
		background: var(--ink-soft);
	}
	.dot.st-working {
		background: var(--accent);
		animation: pulse 1.2s ease-in-out infinite;
	}
	.dot.st-done {
		background: var(--line);
	}
	@keyframes pulse {
		50% {
			opacity: 0.35;
		}
	}
	.tab-close {
		padding: 4px 8px 4px 4px;
		font-size: 14px;
		line-height: 1;
		color: var(--ink-soft);
		opacity: 0;
	}
	.tab:hover .tab-close,
	.tab.is-active .tab-close {
		opacity: 1;
	}
	.tab-close:hover {
		color: var(--accent);
	}
	.addbtn,
	.pickbtn {
		padding: 4px 7px;
		border-radius: 7px;
		font-size: 13px;
		color: var(--ink-soft);
		flex: 0 0 auto;
	}
	.addbtn:hover,
	.pickbtn:hover {
		background: var(--paper-2);
		color: var(--ink);
	}
	.pickbtn {
		font-size: 10px;
	}
	.tabactions {
		display: flex;
		flex: 0 0 auto;
		align-items: center;
		gap: 4px;
	}
	/* Rendered at the component root, outside the clipping rail, and positioned
	   from the button's measured rect (see openPicker). Fixed, so no ancestor's
	   overflow can cut it off and no ancestor's scroll can carry it away. */
	.picker {
		position: fixed;
		z-index: 30;
		padding: 4px;
		border-radius: 10px;
		background: var(--menu-bg);
		border: 1px solid var(--line);
		box-shadow: 0 8px 28px -8px rgba(20, 14, 8, 0.34);
	}
	.picker-item {
		display: flex;
		width: 100%;
		align-items: center;
		gap: 8px;
		text-align: left;
		padding: 7px 9px;
		border-radius: 7px;
		font-size: 12.5px;
		color: var(--ink);
	}
	.picker-item:hover {
		background: var(--paper-2);
	}
	.picker-item.is-default {
		font-weight: 600;
	}
</style>
