<script lang="ts">
	import { onMount } from 'svelte';
	import * as m from '$lib/paraglide/messages';
	import type { Wizard } from './store.svelte';
	import AgentStep from './AgentStep.svelte';
	import LanguageStep from './LanguageStep.svelte';

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
{:else}
	<!-- Runner phase scaffold. WP9③ replaces this with the 7-step runner and the
	     per-kind error UI; for now it only confirms the store handed off correctly. -->
	<main
		class="flex min-h-screen flex-col items-center justify-center gap-2 bg-neutral-50 text-neutral-900"
	>
		{#if wizard.resumeError}
			<p class="text-sm text-neutral-500">{wizard.resumeError.code}</p>
		{:else}
			<p class="text-sm text-neutral-500">{m.wizard_setup_preparing()}</p>
		{/if}
	</main>
{/if}
