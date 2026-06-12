<script lang="ts">
	import { onMount } from 'svelte';
	import type { HostClient } from '$lib/host/client';
	import type { Wizard } from './store.svelte';
	import AgentStep from './AgentStep.svelte';
	import AuthStep from './AuthStep.svelte';
	import LanguageStep from './LanguageStep.svelte';
	import RunnerView from './RunnerView.svelte';

	let {
		wizard,
		host,
		onFinish
	}: { wizard: Wizard; host: HostClient; onFinish: () => Promise<void> } = $props();

	onMount(() => {
		void wizard.bootstrap();
	});
</script>

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
	<AuthStep agents={wizard.selectedAgents} {host} {onFinish} />
{:else}
	<RunnerView {wizard} />
{/if}
