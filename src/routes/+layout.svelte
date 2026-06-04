<script lang="ts">
	import '../app.css';
	import { getLocale } from '$lib/paraglide/runtime';

	let { children } = $props();

	$effect(() => {
		document.documentElement.lang = getLocale();
	});
</script>

{@render children()}
