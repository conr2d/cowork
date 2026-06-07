// Detect a complete URL from the raw PTY byte stream so it can be surfaced as a
// clickable affordance, independent of how xterm wraps it on screen. Stripping
// ANSI/VT escapes first also rejoins a URL the terminal split across rows with
// cursor-move sequences. Pure and unit-tested; the Terminal component feeds it
// decoded chunks.
//
// ESC/BEL are built at runtime via String.fromCharCode so this source file
// contains no literal control characters.

const ESC = String.fromCharCode(27);
const BEL = String.fromCharCode(7);

// CSI (incl. the cursor moves used to soft-wrap a long line) and OSC
// (string-terminated by BEL or ST). Built from ESC/BEL so the source stays
// control-character-free and the no-control-regex lint never trips.
const ANSI = new RegExp(
	`${ESC}\\[[0-9:;<=>?]*[ -/]*[@-~]` + `|${ESC}\\][^${BEL}${ESC}]*(?:${BEL}|${ESC}\\\\)`,
	'g'
);

// An http(s) URL: a run with no whitespace, quote, or bracket. Excluding the
// common URL-wrapping brackets keeps trailing delimiters out of the match.
const URL_RE = /https?:\/\/[^\s"'`<>\\^{}|()[\]]+/g;

// Trailing punctuation that is almost never part of the URL itself.
const TRAILING = /[.,;:!?'"]+$/;

/** Remove ANSI/VT escape sequences from a chunk of terminal output. */
export function stripAnsi(text: string): string {
	return text.replace(ANSI, '');
}

/** Whether a URL looks like an OAuth / sign-in authorization URL, rather than an
 * incidental link in terminal output (the Ubuntu MOTD, docs links, etc.). Keyed
 * on standard OAuth 2.0 authorize/device markers, not on a specific agent's
 * host, so it stays precise while surviving an agent changing its auth domain. */
export function isLoginUrl(url: string): boolean {
	const u = url.toLowerCase();
	return (
		u.includes('/oauth') ||
		u.includes('/authorize') ||
		u.includes('/device') ||
		u.includes('client_id=') ||
		u.includes('response_type=') ||
		u.includes('code_challenge=') ||
		u.includes('user_code=')
	);
}

/** The last COMPLETE URL in `buffer` accepted by `accept` — "complete" meaning it
 * is followed by at least one more character, so we know it fully arrived — with
 * trailing punctuation trimmed. Returns null when there is none (or the only
 * candidate is still at the very end of the buffer, i.e. possibly still arriving).
 * `accept` defaults to any URL; the scanner passes [`isLoginUrl`] so only a real
 * sign-in URL is surfaced. */
export function detectLatestUrl(
	buffer: string,
	accept: (url: string) => boolean = () => true
): string | null {
	let last: string | null = null;
	for (const match of buffer.matchAll(URL_RE)) {
		const idx = match.index ?? 0;
		const end = idx + match[0].length;
		if (end >= buffer.length) break; // still arriving; wait for more
		const url = match[0].replace(TRAILING, '');
		if (accept(url)) last = url;
	}
	return last;
}

export interface UrlScanner {
	/** Feed a decoded PTY text chunk; returns a newly-completed URL, else null. */
	push(text: string): string | null;
}

/** Stateful scanner over a bounded tail of PTY text; reports each newly-completed
 * sign-in URL (per [`isLoginUrl`]) exactly once (until a different one completes).
 * Incidental links (the MOTD, docs) are ignored. The raw tail is kept and stripped
 * whole on every push, so an escape sequence split across two chunks (e.g. ESC in
 * one read, the rest in the next) is still rejoined and removed. */
export function createUrlScanner(maxTail = 8192): UrlScanner {
	let raw = '';
	let lastEmitted: string | null = null;
	return {
		push(text: string): string | null {
			raw = (raw + text).slice(-maxTail);
			const url = detectLatestUrl(stripAnsi(raw), isLoginUrl);
			if (url && url !== lastEmitted) {
				lastEmitted = url;
				return url;
			}
			return null;
		}
	};
}
