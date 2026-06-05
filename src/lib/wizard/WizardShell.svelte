<script lang="ts">
	import { onMount } from 'svelte';
	import * as m from '$lib/paraglide/messages';
	import type { Wizard } from './store.svelte';
	import AgentStep from './AgentStep.svelte';
	import LanguageStep from './LanguageStep.svelte';
	import RunnerView from './RunnerView.svelte';

	let { wizard }: { wizard: Wizard } = $props();

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
	<!-- Auth/terminal handoff scaffold. WP9④ wires Terminal.svelte + loginCommand. -->
	<main
		class="flex min-h-screen flex-col items-center justify-center gap-2 bg-neutral-50 text-neutral-900"
	>
		<p class="text-sm text-neutral-500">{m.wizard_setup_preparing()}</p>
	</main>
{:else}
	<RunnerView {wizard} />
{/if}
