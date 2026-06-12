export type Theme = 'light' | 'dark';

const THEME_KEY = 'cowork.theme';
const COLLAPSED_KEY = 'cowork.sidebarCollapsed';

/** Persisted theme; default 'dark' (agy is illegible on light until the WP4 colorScheme adapter). */
export function loadTheme(): Theme {
	if (typeof localStorage === 'undefined') return 'dark';
	return localStorage.getItem(THEME_KEY) === 'light' ? 'light' : 'dark';
}

export function saveTheme(theme: Theme): void {
	if (typeof localStorage === 'undefined') return;
	localStorage.setItem(THEME_KEY, theme);
}

export function loadCollapsed(): boolean {
	if (typeof localStorage === 'undefined') return false;
	return localStorage.getItem(COLLAPSED_KEY) === '1';
}

export function saveCollapsed(collapsed: boolean): void {
	if (typeof localStorage === 'undefined') return;
	localStorage.setItem(COLLAPSED_KEY, collapsed ? '1' : '0');
}
