<script lang="ts">
	import type { AgentId } from '$lib/terminal/agent';

	let { agent, big = false }: { agent: AgentId; big?: boolean } = $props();
</script>

<span class="aicon {agent}" class:big>
	{#if agent === 'claude'}
		<svg viewBox="0 0 24 24">
			<path d="M12 3v18M3 12h18M5.6 5.6l12.8 12.8M18.4 5.6 5.6 18.4" />
		</svg>
	{:else if agent === 'codex'}
		<svg viewBox="0 0 24 24"><path d="M9 8l-4 4 4 4M15 8l4 4-4 4" /></svg>
	{:else}
		<svg viewBox="0 0 24 24" class="fill">
			<path d="M12 3l1.9 5.6L19.5 10l-5.6 1.9L12 17.5l-1.9-5.6L4.5 10l5.6-1.9z" />
		</svg>
	{/if}
</span>

<style>
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
		fill: none;
		stroke: #fff;
		stroke-width: 2.2;
		stroke-linecap: round;
		stroke-linejoin: round;
		pointer-events: none;
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
	.aicon.antigravity {
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
</style>
