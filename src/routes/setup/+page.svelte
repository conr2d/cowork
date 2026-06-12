<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import WizardShell from '$lib/wizard/WizardShell.svelte';
	import { createWizard } from '$lib/wizard/store.svelte';
	import { tauriHost } from '$lib/host/client';

	const wizard = createWizard(tauriHost);

	// Finish: seed the default workspace (guarded - slug collision would mint
	// default-2), persist the setup-complete marker, enter the shell.
	async function finish(): Promise<void> {
		const list = await tauriHost.workspaceList();
		if (!list.some((w) => w.slug === 'default')) {
			await tauriHost.workspaceCreate('default', wizard.selectedAgents[0] ?? 'claude', 'blank');
		}
		await tauriHost.setupMarkComplete();
		await goto(resolve('/shell'));
	}
</script>

<WizardShell {wizard} host={tauriHost} onFinish={finish} />
