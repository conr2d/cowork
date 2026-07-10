<script lang="ts">
	import * as m from '$lib/paraglide/messages';

	let { onFinish }: { onFinish: () => Promise<void> } = $props();

	let finishing = $state(false);
	let finishFailed = $state(false);

	async function runFinish(): Promise<void> {
		finishing = true;
		finishFailed = false;
		try {
			await onFinish();
		} catch {
			finishFailed = true;
		} finally {
			finishing = false;
		}
	}
</script>

<main
	class="flex min-h-screen flex-col items-center justify-center gap-6 bg-neutral-50 p-8 text-neutral-900"
>
	<div class="flex max-w-md flex-col gap-3 text-center">
		<h1 class="text-2xl font-semibold tracking-tight">{m.done_title()}</h1>
		<p class="text-sm text-neutral-500">{m.done_body()}</p>
	</div>
	<div class="flex flex-col items-center gap-2">
		<button
			type="button"
			class="rounded bg-neutral-900 px-5 py-2.5 text-sm font-semibold text-neutral-50 disabled:opacity-50"
			disabled={finishing}
			onclick={() => void runFinish()}
		>
			{m.done_open()} →
		</button>
		{#if finishFailed}
			<span class="text-sm text-red-700">{m.error_internal_body()}</span>
		{/if}
	</div>
</main>
