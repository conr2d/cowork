<script lang="ts">
	let { label }: { label: string | null } = $props();
</script>

{#if label}
	<span class="build-stamp">{label}</span>
{/if}

<style>
	.build-stamp {
		font-size: 11px;
		line-height: 1.2;
		color: color-mix(in srgb, currentColor 48%, transparent);
		white-space: nowrap;
	}
</style>
