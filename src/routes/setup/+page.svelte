<script lang="ts">
	import { onMount } from 'svelte';
	import type { AppBuildDto } from '$lib/host/types';
	import { loadAppBuild } from '$lib/host/build';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import WizardShell from '$lib/wizard/WizardShell.svelte';
	import { createWizard } from '$lib/wizard/store.svelte';
	import { host } from '$lib/host/client';

	const wizard = createWizard(host);
	let build = $state<AppBuildDto | null>(null);

	onMount(() => {
		void host.setWindowTheme('light');
		void loadAppBuild(host).then((value) => {
			build = value;
		});
	});

	// Finish: seed the default workspace (guarded - slug collision would mint
	// default-2), persist the setup-complete marker, enter the shell.
	async function finish(): Promise<void> {
		const list = await host.workspaceList();
		if (!list.some((w) => w.slug === 'default')) {
			await host.workspaceCreate('default', wizard.selectedAgents[0] ?? 'claude', 'blank');
		}
		await host.setupMarkComplete();
		await goto(resolve('/shell'));
	}
</script>

<WizardShell {wizard} onFinish={finish} {build} />
