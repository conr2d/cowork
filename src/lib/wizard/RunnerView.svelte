<script lang="ts">
	import * as m from '$lib/paraglide/messages';
	import type { Wizard } from './store.svelte';
	import { RUNNER_STEPS, stepStatus, formatElapsed } from './runner';
	import ErrorPanel from './ErrorPanel.svelte';

	let { wizard }: { wizard: Wizard } = $props();

	const labels: Record<(typeof RUNNER_STEPS)[number]['id'], () => string> = {
		preflight: m.step_preflight,
		wsl: m.step_wsl,
		provision: m.step_provision,
		toolchain: m.step_toolchain,
		agentInstall: m.step_agent_install
	};

	const guidance: Record<(typeof RUNNER_STEPS)[number]['id'], () => string> = {
		preflight: m.guide_preflight,
		wsl: m.guide_wsl,
		provision: m.guide_provision,
		toolchain: m.guide_toolchain,
		agentInstall: m.guide_agent_install
	};

	const activeRunnerStep = $derived(
		RUNNER_STEPS.find((runnerStep) => stepStatus(runnerStep.id, wizard.step) === 'active') ?? null
	);

	const rightPaneMessage = $derived(
		activeRunnerStep ? guidance[activeRunnerStep.id]() : m.guide_wsl()
	);

	// Live elapsed seconds for the active step — the decisive "still working"
	// signal when a single step runs quietly for minutes. Resets whenever the
	// active step changes; ticks only while a step is running.
	let elapsed = $state(0);
	$effect(() => {
		const step = wizard.step;
		const running = wizard.running;
		elapsed = 0;
		if (!running || !RUNNER_STEPS.some((s) => s.id === step)) return;
		let seconds = 0;
		const id = setInterval(() => {
			seconds += 1;
			elapsed = seconds;
		}, 1000);
		return () => clearInterval(id);
	});
</script>

<main class="min-h-screen bg-neutral-50 text-neutral-900">
	<div class="mx-auto flex min-h-screen max-w-6xl flex-col md:flex-row">
		<section
			class="flex flex-1 flex-col justify-center gap-6 border-b border-neutral-200 px-6 py-10 md:border-b-0 md:border-r md:px-10"
		>
			<h1 class="text-2xl font-semibold tracking-tight">{m.runner_title()}</h1>

			<ol class="flex w-full max-w-md flex-col gap-2">
				{#each RUNNER_STEPS as runnerStep (runnerStep.id)}
					{@const phase = stepStatus(runnerStep.id, wizard.step)}
					<li class="flex items-center gap-3 text-sm">
						{#if phase === 'done'}
							<span class="inline-block h-2 w-2 rounded-full bg-neutral-900"></span>
						{:else if phase === 'active'}
							<span
								class="inline-block h-3 w-3 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-900"
							></span>
						{:else}
							<span class="inline-block h-2 w-2 rounded-full bg-neutral-300"></span>
						{/if}
						<span class={phase === 'pending' ? 'text-neutral-400' : 'text-neutral-900'}>
							{labels[runnerStep.id]()}
						</span>
						{#if phase === 'active' && wizard.progress}
							<span class="text-xs text-neutral-400">{wizard.progress}</span>
						{/if}
						{#if phase === 'active' && wizard.running}
							<span class="ml-auto text-xs tabular-nums text-neutral-400"
								>{formatElapsed(elapsed)}</span
							>
						{/if}
					</li>
				{/each}
			</ol>

			{#if wizard.rebooting}
				<p class="max-w-md text-sm text-neutral-600">{m.runner_reboot_body()}</p>
			{:else if wizard.error}
				<ErrorPanel
					error={wizard.error}
					running={wizard.running}
					autoRetrying={wizard.autoRetrying}
					attempt={wizard.attempt}
					onRetry={() => wizard.retry()}
				/>
			{/if}
		</section>

		<section class="flex flex-1 items-center px-6 py-10 md:px-10">
			{#if wizard.step === 'permission'}
				<div class="w-full max-w-xl rounded-2xl border border-neutral-200 bg-white p-6 shadow-sm">
					<h2 class="text-xl font-semibold tracking-tight">{m.permission_title()}</h2>
					<div class="mt-4 space-y-4 text-sm leading-6 text-neutral-700">
						<p>{m.permission_uac()}</p>
						<p>{m.permission_welcome()}</p>
						<p class="text-neutral-500">{m.runner_active_hint()}</p>
					</div>
					<button
						type="button"
						class="mt-6 inline-flex items-center justify-center rounded-full bg-neutral-900 px-5 py-2.5 text-sm font-medium text-neutral-50 transition hover:bg-neutral-800"
						onclick={() => wizard.next()}
					>
						{m.permission_continue()}
					</button>
				</div>
			{:else}
				<div class="w-full max-w-xl rounded-2xl border border-neutral-200 bg-white p-6 shadow-sm">
					<h2 class="text-xl font-semibold tracking-tight">{m.guide_heading()}</h2>
					<p class="mt-4 text-sm leading-6 text-neutral-700">{rightPaneMessage}</p>
					<p class="mt-6 text-sm text-neutral-500">{m.runner_active_hint()}</p>
				</div>
			{/if}
		</section>
	</div>
</main>
