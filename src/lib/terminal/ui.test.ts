import { describe, expect, it } from 'vitest';

import {
	clampViewport,
	hasSelection,
	MIN_TERMINAL_COLS,
	MIN_TERMINAL_ROWS,
	resolveTerminalShortcut,
	shouldActivateLink
} from '$lib/terminal/ui';

describe('clampViewport', () => {
	it('enforces a non-zero terminal size', () => {
		expect(clampViewport(0, 0)).toEqual({
			rows: MIN_TERMINAL_ROWS,
			cols: MIN_TERMINAL_COLS
		});
	});

	it('keeps larger measured dimensions', () => {
		expect(clampViewport(42, 120)).toEqual({ rows: 42, cols: 120 });
	});
});

describe('resolveTerminalShortcut', () => {
	it('copies selected text instead of forwarding Ctrl+C', () => {
		expect(
			resolveTerminalShortcut(
				{ key: 'c', ctrlKey: true, altKey: false, metaKey: false, shiftKey: false },
				true
			)
		).toBe('copy-selection');
	});

	it('passes Ctrl+C through when nothing is selected', () => {
		expect(
			resolveTerminalShortcut(
				{ key: 'c', ctrlKey: true, altKey: false, metaKey: false, shiftKey: false },
				false
			)
		).toBe('pass-through');
	});

	it('swallows Ctrl+V for paste', () => {
		expect(
			resolveTerminalShortcut(
				{ key: 'v', ctrlKey: true, altKey: false, metaKey: false, shiftKey: false },
				false
			)
		).toBe('paste');
	});
});

describe('terminal link activation', () => {
	it('opens only on primary clicks without a selection', () => {
		expect(shouldActivateLink(0, false)).toBe(true);
		expect(shouldActivateLink(2, false)).toBe(false);
		expect(shouldActivateLink(0, true)).toBe(false);
	});
});

describe('hasSelection', () => {
	it('treats empty strings as no selection', () => {
		expect(hasSelection('')).toBe(false);
		expect(hasSelection('copied text')).toBe(true);
	});
});

describe('clampViewport guards against a non-measurement', () => {
	it('falls back to the minimum when a dimension is not finite', () => {
		expect(clampViewport(Number.NaN, Number.NaN)).toEqual({
			rows: MIN_TERMINAL_ROWS,
			cols: MIN_TERMINAL_COLS
		});
		expect(clampViewport(Number.POSITIVE_INFINITY, 100)).toEqual({
			rows: MIN_TERMINAL_ROWS,
			cols: 100
		});
	});
});
