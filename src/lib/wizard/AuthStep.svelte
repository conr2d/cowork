<script lang="ts">
	import * as m from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import type { AgentId } from '$lib/terminal/login';
	import { loginInput } from '$lib/terminal/login';
	import { ptyWrite } from '$lib/terminal/pty';
	import Terminal from '$lib/components/Terminal.svelte';
	import { AGENT_CHOICES } from './agents';

	let { agents }: { agents: readonly AgentId[] } = $props();

	const locale = getLocale();
	const choices = $derived(AGENT_CHOICES.filter((choice) => agents.includes(choice.id)));
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
					onclick={() => void ptyWrite(loginInput(choice.id))}
				>
					{m.auth_login({ agent: choice.name })}
				</button>
			{/each}
		</div>
	</div>
	<div class="min-h-0 flex-1 bg-neutral-900 p-2">
		<!-- workspace MUST match the guest bootstrap's workspace_path (~/workspaces/default);
		     a mismatch makes `wsl --cd` fail with chdir errno 2 and a scary banner. -->
		<Terminal distro="Cowork" workspace="~/workspaces/default" {locale} detectLinks />
	</div>
</main>
