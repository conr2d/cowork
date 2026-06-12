import { beforeEach, describe, expect, it } from 'vitest';

import { loadCollapsed, loadTheme, saveCollapsed, saveTheme } from './prefs';

describe('shell prefs', () => {
	beforeEach(() => {
		localStorage.clear();
	});

	it('defaults to dark theme and expanded sidebar', () => {
		expect(loadTheme()).toBe('dark');
		expect(loadCollapsed()).toBe(false);
	});

	it('round-trips theme', () => {
		saveTheme('light');
		expect(loadTheme()).toBe('light');
		saveTheme('dark');
		expect(loadTheme()).toBe('dark');
	});

	it('falls back to dark for garbage theme values', () => {
		localStorage.setItem('cowork.theme', 'sepia');
		expect(loadTheme()).toBe('dark');
	});

	it('round-trips sidebar collapsed state', () => {
		saveCollapsed(true);
		expect(loadCollapsed()).toBe(true);
		saveCollapsed(false);
		expect(loadCollapsed()).toBe(false);
	});
});
