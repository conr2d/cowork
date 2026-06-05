<script lang="ts">
	import WizardShell from '$lib/wizard/WizardShell.svelte';
	import { createWizard } from '$lib/wizard/store.svelte';
	import { tauriHost } from '$lib/host/client';

	const wizard = createWizard(tauriHost);
</script>

<WizardShell {wizard} />
