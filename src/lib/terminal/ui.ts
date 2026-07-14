export const MIN_TERMINAL_ROWS = 1;
export const MIN_TERMINAL_COLS = 2;

export interface TerminalViewport {
	rows: number;
	cols: number;
}

export interface TerminalShortcutEvent {
	key: string;
	ctrlKey: boolean;
	altKey: boolean;
	metaKey: boolean;
	shiftKey: boolean;
}

export type TerminalShortcutAction = 'copy-selection' | 'paste' | 'pass-through';

export function hasSelection(selection: string | null | undefined): boolean {
	return (selection ?? '').length > 0;
}

/** A measurement that is not a finite number is no measurement at all: Math.max
 * propagates NaN, so an unguarded clamp would hand a NaN viewport straight to the
 * ConPTY — the very thing this exists to prevent. */
function floorOr(value: number, fallback: number): number {
	return Number.isFinite(value) ? Math.max(Math.floor(value), fallback) : fallback;
}

export function clampViewport(rows: number, cols: number): TerminalViewport {
	return {
		rows: floorOr(rows, MIN_TERMINAL_ROWS),
		cols: floorOr(cols, MIN_TERMINAL_COLS)
	};
}

export function resolveTerminalShortcut(
	event: TerminalShortcutEvent,
	selectionPresent: boolean
): TerminalShortcutAction {
	if (event.altKey || event.metaKey || event.shiftKey || !event.ctrlKey) {
		return 'pass-through';
	}

	switch (event.key.toLowerCase()) {
		case 'c':
			return selectionPresent ? 'copy-selection' : 'pass-through';
		case 'v':
			return 'paste';
		default:
			return 'pass-through';
	}
}

export function shouldActivateLink(button: number, selectionPresent: boolean): boolean {
	return button === 0 && !selectionPresent;
}
