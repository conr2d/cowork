<script lang="ts">
	import { locales, getLocale, setLocale } from '$lib/paraglide/runtime';

	// Autonyms - display each language in its own name; not translatable, so not message keys.
	const AUTONYMS: Record<string, string> = { en: 'English', ja: '日本語', ko: '한국어' };

	const current = getLocale();
</script>

<div class="flex gap-2" role="group" aria-label="Language">
	{#each locales as locale (locale)}
		<button
			type="button"
			class="rounded border px-3 py-1 text-sm {current === locale
				? 'border-neutral-900 font-medium'
				: 'border-neutral-300 text-neutral-600'}"
			aria-current={current === locale ? 'true' : undefined}
			onclick={() => setLocale(locale)}
		>
			{AUTONYMS[locale] ?? locale}
		</button>
	{/each}
</div>
