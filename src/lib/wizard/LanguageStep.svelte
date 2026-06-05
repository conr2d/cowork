<script lang="ts">
	import * as m from '$lib/paraglide/messages';
	import LanguageSelector from '$lib/components/LanguageSelector.svelte';

	let { onContinue }: { onContinue: () => void } = $props();
</script>

<main
	class="flex min-h-screen flex-col items-center justify-center gap-6 bg-neutral-50 text-neutral-900"
>
	<div class="flex flex-col items-center gap-2 text-center">
		<h1 class="text-3xl font-semibold tracking-tight">{m.wizard_welcome_title()}</h1>
		<p class="max-w-md text-sm text-neutral-500">{m.wizard_welcome_body()}</p>
	</div>
	<LanguageSelector />
	<button
		type="button"
		class="rounded bg-neutral-900 px-4 py-2 text-sm font-medium text-neutral-50"
		onclick={onContinue}
	>
		{m.action_continue()}
	</button>
</main>
