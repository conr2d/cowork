<script lang="ts">
	import * as m from '$lib/paraglide/messages';
	import type { AgentId } from '$lib/terminal/login';
	import { AGENT_CHOICES } from './agents';

	let {
		selected,
		canContinue,
		onToggle,
		onBack,
		onContinue
	}: {
		selected: readonly AgentId[];
		canContinue: boolean;
		onToggle: (id: AgentId) => void;
		onBack: () => void;
		onContinue: () => void;
	} = $props();

	const descriptions: Record<AgentId, () => string> = {
		claude: m.agent_claude_desc,
		codex: m.agent_codex_desc,
		antigravity: m.agent_antigravity_desc
	};
</script>

<main
	class="flex min-h-screen flex-col items-center justify-center gap-6 bg-neutral-50 text-neutral-900"
>
	<div class="flex flex-col items-center gap-2 text-center">
		<h1 class="text-2xl font-semibold tracking-tight">{m.wizard_agents_title()}</h1>
		<p class="max-w-md text-sm text-neutral-500">{m.wizard_agents_body()}</p>
	</div>

	<div class="flex w-full max-w-md flex-col gap-2">
		{#each AGENT_CHOICES as choice (choice.id)}
			{@const isSelected = selected.includes(choice.id)}
			<button
				type="button"
				aria-pressed={isSelected}
				class="flex flex-col gap-0.5 rounded border px-4 py-3 text-left {isSelected
					? 'border-neutral-900 bg-neutral-100'
					: 'border-neutral-300'}"
				onclick={() => onToggle(choice.id)}
			>
				<span class="text-sm font-medium">{choice.name}</span>
				<span class="text-xs text-neutral-500">{descriptions[choice.id]()}</span>
			</button>
		{/each}
	</div>

	{#if !canContinue}
		<p class="text-xs text-neutral-500">{m.wizard_agents_min_hint()}</p>
	{/if}

	<div class="flex gap-2">
		<button
			type="button"
			class="rounded border border-neutral-300 px-4 py-2 text-sm text-neutral-600"
			onclick={onBack}
		>
			{m.action_back()}
		</button>
		<button
			type="button"
			class="rounded bg-neutral-900 px-4 py-2 text-sm font-medium text-neutral-50 disabled:opacity-40"
			disabled={!canContinue}
			onclick={onContinue}
		>
			{m.action_continue()}
		</button>
	</div>
</main>
