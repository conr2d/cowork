// How the UI treats each error `kind`, plus the auto-retry policy. Pure and
// unit-tested; the store and ErrorPanel consume these.

import type { Kind } from '$lib/errors/registry';

export interface Affordance {
	/** A Blocker stops the run; nothing to retry here. */
	blocking: boolean;
	/** Offer a manual Retry button. */
	canRetry: boolean;
	/** Retry automatically (with backoff) before falling back to manual retry. */
	autoRetry: boolean;
	/** Show the redacted English `cause` and the diagnostics location (Internal bugs). */
	showCause: boolean;
}

export function affordanceFor(kind: Kind): Affordance {
	switch (kind) {
		case 'Blocker':
			return { blocking: true, canRetry: false, autoRetry: false, showCause: false };
		case 'NeedsUserAction':
			return { blocking: false, canRetry: true, autoRetry: false, showCause: false };
		case 'Transient':
			return { blocking: false, canRetry: true, autoRetry: true, showCause: false };
		case 'Internal':
			return { blocking: false, canRetry: true, autoRetry: false, showCause: true };
		case 'Cancelled':
			return { blocking: false, canRetry: true, autoRetry: false, showCause: false };
	}
}

/** Maximum automatic retries for a Transient failure before falling back to manual. */
export const AUTO_RETRY_MAX = 3;

/** Backoff for the n-th auto-retry (1-based): 1s, 2s, 4s. */
export function retryDelayMs(attempt: number): number {
	return 1000 * 2 ** Math.max(0, attempt - 1);
}

/** Whether the just-incremented attempt (1-based) should auto-retry. */
export function shouldAutoRetry(kind: Kind, attempt: number): boolean {
	return affordanceFor(kind).autoRetry && attempt < AUTO_RETRY_MAX;
}

/** Where redacted diagnostics live on the host (shown for Internal errors). */
export const DIAGNOSTICS_DIR = '%LOCALAPPDATA%\\Cowork';
