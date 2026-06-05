<script lang="ts">
	import * as m from '$lib/paraglide/messages';
	import type { Envelope, Kind } from '$lib/errors/registry';
	import { affordanceFor, DIAGNOSTICS_DIR } from './affordance';

	let {
		error,
		running,
		onRetry
	}: {
		error: Envelope;
		running: boolean;
		onRetry: () => void;
	} = $props();

	const affordance = $derived(affordanceFor(error.kind));

	function bodyFor(kind: Kind): string {
		switch (kind) {
			case 'Blocker':
				return m.error_blocker_body();
			case 'NeedsUserAction':
				return m.error_needs_action_body();
			case 'Transient':
				return m.error_transient_body();
			case 'Internal':
				return m.error_internal_body();
			case 'Cancelled':
				return m.error_cancelled_body();
		}
	}
</script>

<section class="flex w-full max-w-md flex-col gap-3 rounded border border-neutral-300 p-4">
	<div class="flex flex-col gap-1">
		<h2 class="text-sm font-semibold">{m.error_title()}</h2>
		<p class="text-sm text-neutral-600">{bodyFor(error.kind)}</p>
		<code class="text-xs text-neutral-400">{error.code}</code>
	</div>

	{#if affordance.showCause}
		<div class="flex flex-col gap-1">
			{#if error.cause}
				<span class="text-xs font-medium text-neutral-500">{m.error_cause_label()}</span>
				<pre
					class="overflow-x-auto rounded bg-neutral-100 p-2 text-xs text-neutral-700">{error.cause}</pre>
			{/if}
			<span class="text-xs text-neutral-400">{m.error_log_hint({ path: DIAGNOSTICS_DIR })}</span>
		</div>
	{/if}

	{#if affordance.canRetry}
		<button
			type="button"
			class="self-start rounded bg-neutral-900 px-4 py-2 text-sm font-medium text-neutral-50 disabled:opacity-40"
			disabled={running}
			onclick={onRetry}
		>
			{running ? m.error_retrying() : m.error_retry()}
		</button>
	{/if}
</section>
