<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { tauriHost } from '$lib/host/client';

	// Boot gate: setup-complete? -> shell; otherwise the first-run wizard.
	onMount(async () => {
		const complete = await tauriHost.setupIsComplete().catch(() => false);
		await goto(resolve(complete ? '/shell' : '/setup'), { replaceState: true });
	});
</script>

<div
	class="flex min-h-screen flex-col items-center justify-center gap-6 bg-neutral-50 text-neutral-900"
>
	<h1 class="text-3xl font-semibold tracking-tight">Cowork</h1>
	<span
		class="inline-block h-6 w-6 animate-spin rounded-full border-2 border-neutral-300 border-t-neutral-900"
		aria-hidden="true"
	></span>
</div>
