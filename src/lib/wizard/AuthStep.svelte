<script lang="ts">
	import * as m from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import type { AgentId } from '$lib/terminal/login';
	import { loginInput } from '$lib/terminal/login';
	import { ptyWrite } from '$lib/terminal/pty';
	import Terminal from '$lib/components/Terminal.svelte';
	import { AGENT_CHOICES } from './agents';

	let { agents, onFinish }: { agents: readonly AgentId[]; onFinish: () => Promise<void> } =
		$props();

	const locale = getLocale();
	const choices = $derived(AGENT_CHOICES.filter((choice) => agents.includes(choice.id)));
	let loginAttempts = $state(0);
	let finishing = $state(false);
	let finishFailed = $state(false);
	let ptyId = $state<number | null>(null);
</script>

<main class="flex min-h-screen flex-col bg-neutral-50 text-neutral-900">
	<div class="flex flex-col gap-2 p-4">
		<h1 class="text-xl font-semibold tracking-tight">{m.auth_title()}</h1>
		<p class="text-sm text-neutral-500">{m.auth_body()}</p>
		<div class="flex flex-wrap gap-2">
			{#each choices as choice (choice.id)}
				<button
					type="button"
					class="rounded bg-neutral-900 px-4 py-2 text-sm font-medium text-neutral-50"
					onclick={() => {
						if (ptyId === null) return;
						void ptyWrite(ptyId, loginInput(choice.id));
						loginAttempts += 1;
					}}
				>
					{m.auth_login({ agent: choice.name })}
				</button>
			{/each}
		</div>
		<div class="flex items-center gap-3">
			<button
				type="button"
				class="rounded border border-neutral-900 px-4 py-2 text-sm font-semibold text-neutral-900 hover:bg-neutral-100 disabled:opacity-50"
				disabled={finishing}
				onclick={async () => {
					finishing = true;
					finishFailed = false;
					try {
						await onFinish();
					} catch {
						finishFailed = true;
					} finally {
						finishing = false;
					}
				}}
			>
				{m.auth_finish()} →
			</button>
			{#if finishFailed}
				<span class="text-sm text-red-700">{m.error_internal_body()}</span>
			{/if}
		</div>
	</div>
	<div class="min-h-0 flex-1 bg-neutral-900 p-2">
		<!-- workspace MUST match the guest bootstrap's workspace_path (~/workspaces/default);
		     a mismatch makes `wsl --cd` fail with chdir errno 2 and a scary banner. -->
		<Terminal
			distro="Cowork"
			workspace="~/workspaces/default"
			{locale}
			detectLinks
			{loginAttempts}
			onspawn={(id) => (ptyId = id)}
		/>
	</div>
</main>
