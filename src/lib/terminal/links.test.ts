import { describe, expect, it } from 'vitest';

import { createUrlScanner, detectLatestUrl, isLoginUrl, stripAnsi } from './links';

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

	it('honors an accept predicate, returning the last accepted URL', () => {
		expect(
			detectLatestUrl(
				'docs https://docs.test/x then https://id.test/oauth?client_id=1 ',
				isLoginUrl
			)
		).toBe('https://id.test/oauth?client_id=1');
	});
});

describe('isLoginUrl', () => {
	it('accepts standard OAuth authorize / device URLs (agent-agnostic)', () => {
		expect(isLoginUrl('https://claude.ai/oauth/authorize?client_id=x')).toBe(true);
		expect(isLoginUrl('https://accounts.google.com/o/oauth2/v2/auth?response_type=code')).toBe(
			true
		);
		expect(isLoginUrl('https://auth.openai.com/x?code_challenge=y')).toBe(true);
		expect(isLoginUrl('https://example.com/device?user_code=ABCD')).toBe(true);
	});

	it('rejects incidental links (MOTD, docs)', () => {
		expect(isLoginUrl('https://ubuntu.com/pro')).toBe(false);
		expect(isLoginUrl('https://documentation.ubuntu.com/wsl/')).toBe(false);
	});
});

describe('createUrlScanner', () => {
	it('reconstructs a sign-in URL split by a cursor-move escape (wrap)', () => {
		const s = createUrlScanner();
		expect(s.push(`https://claude.ai/oauth/auth${ESC}[1Gorize?client_id=abc `)).toBe(
			'https://claude.ai/oauth/authorize?client_id=abc'
		);
	});

	it('strips an escape sequence split across two chunks', () => {
		const s = createUrlScanner();
		// ESC arrives at the end of one chunk; the rest of the cursor-move + URL
		// tail arrives in the next. Stripping the whole accumulated tail rejoins it.
		expect(s.push(`https://claude.ai/oauth/authorize?client_id=a${ESC}`)).toBeNull();
		expect(s.push('[1Gbc ')).toBe('https://claude.ai/oauth/authorize?client_id=abc');
	});

	it('reassembles a sign-in URL split across chunks and emits it once', () => {
		const s = createUrlScanner();
		expect(s.push('https://claude.ai/oauth/authorize?client_id=a')).toBeNull(); // not yet terminated
		expect(s.push('bc ')).toBe('https://claude.ai/oauth/authorize?client_id=abc');
		expect(s.push(' still here ')).toBeNull(); // same URL → not re-emitted
	});

	it('emits a new sign-in URL when a different one completes', () => {
		const s = createUrlScanner();
		expect(s.push('https://one.test/oauth/authorize?client_id=1 ')).toBe(
			'https://one.test/oauth/authorize?client_id=1'
		);
		expect(s.push('https://two.test/oauth/authorize?client_id=2 ')).toBe(
			'https://two.test/oauth/authorize?client_id=2'
		);
	});

	it('ignores incidental non-sign-in URLs (e.g. the MOTD)', () => {
		const s = createUrlScanner();
		expect(s.push('Welcome! Learn more at https://ubuntu.com/pro ')).toBeNull();
		expect(s.push('Docs: https://documentation.ubuntu.com/wsl/ ')).toBeNull();
	});
});
