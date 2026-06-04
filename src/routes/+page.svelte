<script lang="ts">
	import * as m from '$lib/paraglide/messages';
	import LanguageSelector from '$lib/components/LanguageSelector.svelte';
</script>

<main
	class="flex min-h-screen flex-col items-center justify-center gap-4 bg-neutral-50 text-neutral-900"
>
	<h1 class="text-3xl font-semibold tracking-tight">{m.app_name()}</h1>
	<p class="text-sm text-neutral-500">{m.app_tagline()}</p>
	<LanguageSelector />
</main>
