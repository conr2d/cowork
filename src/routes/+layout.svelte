<script lang="ts">
	import { onMount } from 'svelte';
	import '@fontsource/ibm-plex-sans/400.css';
	import '@fontsource/ibm-plex-sans/500.css';
	import '@fontsource/ibm-plex-sans/600.css';
	import '@fontsource/ibm-plex-mono/400.css';
	import '@fontsource/ibm-plex-mono/500.css';
	import '@fontsource/ibm-plex-mono/600.css';
	import '../app.css';
	import { formatAppBuild, loadAppBuild } from '$lib/host/build';
	import { tauriHost } from '$lib/host/client';
	import { getLocale } from '$lib/paraglide/runtime';

	let { children } = $props();

	$effect(() => {
		document.documentElement.lang = getLocale();
	});

	onMount(() => {
		void loadAppBuild(tauriHost).then((build) => {
			if (build) console.info(formatAppBuild(build));
		});
	});
</script>

{@render children()}
