<script lang="ts">
	import * as m from '$lib/paraglide/messages';
	import type { Wizard } from './store.svelte';
	import { RUNNER_STEPS, stepStatus } from './runner';
	import ErrorPanel from './ErrorPanel.svelte';

	let { wizard }: { wizard: Wizard } = $props();

	const labels: Record<(typeof RUNNER_STEPS)[number]['id'], () => string> = {
		preflight: m.step_preflight,
		wsl: m.step_wsl,
		provision: m.step_provision,
		toolchain: m.step_toolchain,
		agentInstall: m.step_agent_install
	};
</script>

<main
	class="flex min-h-screen flex-col items-center justify-center gap-6 bg-neutral-50 text-neutral-900"
>
	<h1 class="text-2xl font-semibold tracking-tight">{m.runner_title()}</h1>

	<ol class="flex w-full max-w-md flex-col gap-2">
		{#each RUNNER_STEPS as runnerStep (runnerStep.id)}
			{@const phase = stepStatus(runnerStep.id, wizard.step)}
			<li class="flex items-center gap-3 text-sm">
				<span
					class="inline-block h-2 w-2 rounded-full {phase === 'done'
						? 'bg-neutral-900'
						: phase === 'active'
							? 'bg-neutral-500'
							: 'bg-neutral-300'}"
				></span>
				<span class={phase === 'pending' ? 'text-neutral-400' : 'text-neutral-900'}>
					{labels[runnerStep.id]()}
				</span>
				{#if phase === 'active' && wizard.progress}
					<span class="text-xs text-neutral-400">{wizard.progress}</span>
				{/if}
			</li>
		{/each}
	</ol>

	{#if wizard.rebooting}
		<p class="max-w-md text-center text-sm text-neutral-600">{m.runner_reboot_body()}</p>
	{:else if wizard.error}
		<ErrorPanel error={wizard.error} running={wizard.running} onRetry={() => wizard.retry()} />
	{/if}
</main>
