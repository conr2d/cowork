<script lang="ts">
	import Terminal from '$lib/components/Terminal.svelte';

	const sessionId = crypto.randomUUID();
</script>

<div class="terminal-page">
	<Terminal {sessionId} />
</div>

<style>
	.terminal-page {
		height: 100vh;
		width: 100vw;
		background-color: #1e1e1e;
	}
</style>
