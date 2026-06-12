<script lang="ts">
	import type { WorkspaceDto } from '$lib/host/types';
	import * as m from '$lib/paraglide/messages';
	import type { Shell } from './store.svelte';

	let {
		shell,
		workspace,
		onclose
	}: { shell: Shell; workspace: WorkspaceDto; onclose: () => void } = $props();

	async function remove(): Promise<void> {
		await shell.remove(workspace.slug);
		if (!shell.error) onclose();
	}
</script>

<div class="modal-layer">
	<button type="button" class="modal-backdrop" aria-label={m.action_cancel()} onclick={onclose}
	></button>
	<div class="modal modal-sm" role="dialog" aria-modal="true" tabindex="-1">
		<h2 class="modal-title">{m.ws_delete_title()}</h2>
		<p class="modal-text">
			{m.ws_delete_body({ name: workspace.name, path: `~/workspaces/${workspace.slug}` })}
		</p>
		<div class="modal-actions">
			<button type="button" class="m-btn" onclick={onclose}>{m.action_cancel()}</button>
			<button type="button" class="m-btn danger" onclick={() => void remove()}
				>{m.ws_delete_action()}</button
			>
		</div>
	</div>
</div>

<style>
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
	}
	.modal-sm {
		width: 380px;
	}
	.modal-title {
		font-size: 16px;
		font-weight: 600;
		letter-spacing: 0;
	}
	.modal-text {
		font-size: 13px;
		line-height: 1.55;
		color: var(--ink-soft);
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
	.m-btn.danger {
		background: #b3402f;
		border-color: #b3402f;
		color: #fff;
	}
	.m-btn.danger:hover {
		background: #9c3526;
	}
</style>
