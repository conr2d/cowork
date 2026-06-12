<script lang="ts">
	import type { HostClient } from '$lib/host/client';
	import * as m from '$lib/paraglide/messages';
	import type { AgentId } from '$lib/terminal/login';
	import AgentIcon from './AgentIcon.svelte';
	import { brand, PRESETS } from './model';
	import type { Shell } from './store.svelte';

	let { shell, host, onclose }: { shell: Shell; host: HostClient; onclose: () => void } = $props();

	const agents: readonly AgentId[] = ['claude', 'codex', 'antigravity'];
	let name = $state('');
	let agent = $state<AgentId>('claude');
	let presetId = $state<'blank' | 'pdf' | 'proposal'>('blank');
	let slug = $state('');
	let submitting = $state(false);

	let previewTimer: ReturnType<typeof setTimeout> | null = null;
	let previewToken = 0;

	function presetName(id: 'blank' | 'pdf' | 'proposal'): string {
		if (id === 'pdf') return m.preset_pdf_name();
		if (id === 'proposal') return m.preset_proposal_name();
		return m.preset_blank_name();
	}

	function presetDesc(id: 'blank' | 'pdf' | 'proposal'): string {
		if (id === 'pdf') return m.preset_pdf_desc();
		if (id === 'proposal') return m.preset_proposal_desc();
		return m.preset_blank_desc();
	}

	$effect(() => {
		const currentName = name.trim();
		const token = ++previewToken;
		if (previewTimer) clearTimeout(previewTimer);
		if (!currentName) {
			slug = '';
			return;
		}
		previewTimer = setTimeout(() => {
			void (async () => {
				try {
					const preview = await host.workspaceSlugPreview(currentName);
					if (token === previewToken && name.trim() === currentName) slug = preview;
				} catch {
					if (token === previewToken) slug = '';
				}
			})();
		}, 250);
	});

	async function submit(): Promise<void> {
		const value = name.trim();
		if (!value || submitting) return;
		submitting = true;
		await shell.create(value, agent, presetId);
		submitting = false;
		if (!shell.error) onclose();
	}
</script>

<div class="modal-layer">
	<button type="button" class="modal-backdrop" aria-label={m.action_cancel()} onclick={onclose}
	></button>
	<div class="modal" role="dialog" aria-modal="true" tabindex="-1">
		<h2 class="modal-title">{m.ws_new_title()}</h2>
		<label class="field">
			<span class="field-label">{m.ws_name_label()}</span>
			<input
				class="field-input"
				bind:value={name}
				placeholder={m.ws_name_placeholder()}
				spellcheck="false"
				onkeydown={(event) => {
					if (event.key === 'Enter') void submit();
				}}
			/>
			{#if slug}<span class="slug-hint">{m.ws_folder_hint({ path: `~/workspaces/${slug}` })}</span
				>{/if}
		</label>
		<div class="field">
			<span class="field-label">{m.ws_preset_label()}</span>
			<div class="preset-list">
				{#each PRESETS as preset (preset.id)}
					<button
						type="button"
						class="preset"
						class:on={presetId === preset.id}
						onclick={() => (presetId = preset.id)}
					>
						<span class="preset-name">{presetName(preset.id)}</span>
						<span class="preset-desc">{presetDesc(preset.id)}</span>
					</button>
				{/each}
			</div>
		</div>
		<div class="field">
			<span class="field-label">{m.ws_agent_label()}</span>
			<div class="agent-row">
				{#each agents as id (id)}
					<button
						type="button"
						class="agent-opt"
						class:on={agent === id}
						onclick={() => (agent = id)}
					>
						<AgentIcon agent={id} big />
						<span>{brand(id)}</span>
					</button>
				{/each}
			</div>
		</div>
		<div class="modal-actions">
			<button type="button" class="m-btn" onclick={onclose}>{m.action_cancel()}</button>
			<button
				type="button"
				class="m-btn primary"
				disabled={!name.trim() || submitting}
				onclick={() => void submit()}
			>
				{m.ws_create_action()}
			</button>
		</div>
		{#if shell.error}<p class="dialog-error">{shell.error.code}</p>{/if}
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
	.modal-title {
		font-size: 16px;
		font-weight: 600;
		letter-spacing: 0;
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
	.slug-hint,
	.dialog-error {
		font-family: 'IBM Plex Mono', monospace;
		font-size: 11px;
		color: var(--ink-soft);
	}
	.dialog-error {
		color: #b3402f;
		text-align: right;
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
		flex-wrap: wrap;
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
</style>
