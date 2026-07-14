<script lang="ts">
	import { onMount } from 'svelte';
	import AppBuildStamp from '$lib/components/AppBuildStamp.svelte';
	import { formatAppBuild } from '$lib/host/build';
	import type { AppBuildDto } from '$lib/host/types';
	import type { Wizard } from './store.svelte';
	import AgentStep from './AgentStep.svelte';
	import DoneStep from './DoneStep.svelte';
	import LanguageStep from './LanguageStep.svelte';
	import RunnerView from './RunnerView.svelte';

	let {
		wizard,
		onFinish,
		build
	}: {
		wizard: Wizard;
		onFinish: () => Promise<void>;
		build: AppBuildDto | null;
	} = $props();

	onMount(() => {
		void wizard.bootstrap();
	});

	const buildLabel = $derived(build ? formatAppBuild(build) : null);
</script>

<div class="wizard-shell">
	{#if wizard.step === 'language'}
		<LanguageStep onContinue={() => wizard.next()} />
	{:else if wizard.step === 'agents'}
		<AgentStep
			selected={wizard.selectedAgents}
			canContinue={wizard.canProceed}
			onToggle={(id) => wizard.toggleAgent(id)}
			onBack={() => wizard.back()}
			onContinue={() => wizard.next()}
		/>
	{:else if wizard.step === 'auth' || wizard.step === 'done'}
		<DoneStep {onFinish} />
	{:else}
		<RunnerView {wizard} />
	{/if}

	<footer class="build-footer">
		<AppBuildStamp label={buildLabel} />
	</footer>
</div>

<style>
	.wizard-shell {
		position: relative;
	}
	.build-footer {
		position: fixed;
		right: 20px;
		bottom: 16px;
		pointer-events: none;
	}
</style>
