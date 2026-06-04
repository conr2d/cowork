import { beforeEach, describe, expect, it } from 'vitest';
import * as m from '$lib/paraglide/messages';
import { setLocale } from '$lib/paraglide/runtime';

describe('i18n', () => {
	beforeEach(() => {
		setLocale('en', { reload: false });
	});

	it('switch swaps strings', () => {
		setLocale('en', { reload: false });
		expect(m.app_tagline()).toBe('Set up an AI coding agent, safely.');

		setLocale('ja', { reload: false });
		expect(m.app_tagline()).toBe('AIコーディングエージェントを、安全にセットアップ。');
	});

	it('parameter interpolation', () => {
		expect(m.disk_required({ gib: 16 })).toContain('16');

		setLocale('ja', { reload: false });
		expect(m.disk_required({ gib: 16 })).toContain('16');
	});

	it('base-locale fallback', () => {
		setLocale('ja', { reload: false });
		expect(m.fallback_probe()).toBe(
			'This string exists only in English to verify base-locale fallback.'
		);
	});
});
