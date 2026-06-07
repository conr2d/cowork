import { describe, expect, it } from 'vitest';

import { createUrlScanner, detectLatestUrl, stripAnsi } from './links';

// ESC built at runtime so this test source contains no literal control chars.
const ESC = String.fromCharCode(27);

describe('stripAnsi', () => {
	it('removes CSI color and cursor-move sequences', () => {
		expect(stripAnsi(`${ESC}[31mred${ESC}[0m`)).toBe('red');
		expect(stripAnsi(`ab${ESC}[2Kcd`)).toBe('abcd');
	});
});

describe('detectLatestUrl', () => {
	it('returns null while the only URL is still at the buffer end', () => {
		expect(detectLatestUrl('go to https://example.com/auth?x=1')).toBeNull();
	});

	it('returns a URL once a boundary follows it', () => {
		expect(detectLatestUrl('https://example.com/auth?x=1 ')).toBe('https://example.com/auth?x=1');
	});

	it('trims trailing punctuation', () => {
		expect(detectLatestUrl('go https://example.com/path. ')).toBe('https://example.com/path');
	});

	it('stops a URL at a wrapping bracket', () => {
		expect(detectLatestUrl('(https://example.com/a) ')).toBe('https://example.com/a');
	});

	it('emits a URL ended by an excluded delimiter at the buffer end', () => {
		// The closing paren is the final char, but being excluded from the URL
		// class it still proves the URL ended, so the URL is emitted.
		expect(detectLatestUrl('see https://example.com/a)')).toBe('https://example.com/a');
	});

	it('returns the last complete URL when several are present', () => {
		expect(detectLatestUrl('https://a.test/1 then https://b.test/2 ')).toBe('https://b.test/2');
	});
});

describe('createUrlScanner', () => {
	it('reconstructs a URL split by a cursor-move escape (wrap)', () => {
		const s = createUrlScanner();
		expect(s.push(`https://ex.test/very${ESC}[1Glongpath?a=1 `)).toBe(
			'https://ex.test/verylongpath?a=1'
		);
	});

	it('strips an escape sequence split across two chunks', () => {
		const s = createUrlScanner();
		// ESC arrives at the end of one chunk; the rest of the cursor-move + URL
		// tail arrives in the next. Stripping the whole accumulated tail rejoins it.
		expect(s.push(`https://ex.test/a${ESC}`)).toBeNull();
		expect(s.push('[1Gbcd ')).toBe('https://ex.test/abcd');
	});

	it('reassembles a URL split across chunks and emits it once', () => {
		const s = createUrlScanner();
		expect(s.push('https://ex.test/abc')).toBeNull(); // not yet terminated
		expect(s.push('def ')).toBe('https://ex.test/abcdef');
		expect(s.push(' still here ')).toBeNull(); // same URL → not re-emitted
	});

	it('emits a new URL when a different one completes', () => {
		const s = createUrlScanner();
		expect(s.push('https://one.test/x ')).toBe('https://one.test/x');
		expect(s.push('https://two.test/y ')).toBe('https://two.test/y');
	});
});
