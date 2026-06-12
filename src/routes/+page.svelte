<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { tauriHost } from '$lib/host/client';

	// Boot gate: setup-complete? -> shell; otherwise the first-run wizard.
	// Renders nothing - the redirect is immediate and both targets paint fast.
	onMount(async () => {
		const complete = await tauriHost.setupIsComplete().catch(() => false);
		await goto(resolve(complete ? '/shell' : '/setup'), { replaceState: true });
	});
</script>
