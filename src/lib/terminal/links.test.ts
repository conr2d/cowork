import { describe, expect, it } from 'vitest';

import { createUrlScanner, detectLatestUrl, isLoginUrl, joinWrappedUrls, stripAnsi } from './links';

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

describe('joinWrappedUrls', () => {
	it('rejoins a query string split by a hard newline', () => {
		expect(joinWrappedUrls('https://example.com/auth?client_id=1&scope=x+\nmore=y ')).toBe(
			'https://example.com/auth?client_id=1&scope=x+more=y '
		);
	});

	it('does not join ordinary text after a URL', () => {
		expect(joinWrappedUrls('see https://example.com/x\nThanks for reading ')).toBe(
			'see https://example.com/x\nThanks for reading '
		);
	});

	it('does not join when the left URL run has no query string', () => {
		expect(joinWrappedUrls('https://example.com/path\nmore=stuff ')).toBe(
			'https://example.com/path\nmore=stuff '
		);
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
	const claudeOauthLine1 =
		'https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback&scope=org%3Acreate_api_key+';
	const claudeOauthLine2 =
		'user%3Aprofile+user%3Ainference+user%3Asessions%3Aclaude_code+user%3Amcp_servers+user%3Afile_upload&code_challenge=Xwxzb6ose-rPo5j_9-7MrN098KeodZ5bpNUVE7JYkUc&code_challenge_method=S256&state=3akXD9SuO9BCifNDwNMGxuxycQZ_uKYn3-rZxgbnYm8';

	it('reconstructs a sign-in URL split by a cursor-move escape (wrap)', () => {
		const s = createUrlScanner();
		expect(s.push(`https://claude.ai/oauth/auth${ESC}[1Gorize?client_id=abc `)).toBe(
			'https://claude.ai/oauth/authorize?client_id=abc'
		);
	});

	it("reassembles Claude's real first-run OAuth URL split by a hard newline", () => {
		// Captured from claude 2.1.207 real first-run OAuth output.
		const s = createUrlScanner();
		const split = `${claudeOauthLine1}\n${claudeOauthLine2}`;
		const expected = `${claudeOauthLine1}${claudeOauthLine2}`;

		expect(s.push(`${split} `)).toBe(expected);
		expect(expected.endsWith('&state=3akXD9SuO9BCifNDwNMGxuxycQZ_uKYn3-rZxgbnYm8')).toBe(true);
		expect(expected.includes('\n')).toBe(false);
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

	it('keeps a single-line OAuth URL unchanged', () => {
		const s = createUrlScanner();
		expect(
			s.push(
				'https://auth.openai.com/oauth/authorize?client_id=abc&response_type=code&code_challenge=xyz '
			)
		).toBe(
			'https://auth.openai.com/oauth/authorize?client_id=abc&response_type=code&code_challenge=xyz'
		);
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

	it('reassembles a sign-in URL split across two hard newlines', () => {
		const s = createUrlScanner();
		expect(
			s.push(
				'https://id.test/oauth/authorize?client_id=abc&\nscope=org%3Acreate_api_key+user%3Aprofile&\nresponse_type=code&code_challenge=xyz '
			)
		).toBe(
			'https://id.test/oauth/authorize?client_id=abc&scope=org%3Acreate_api_key+user%3Aprofile&response_type=code&code_challenge=xyz'
		);
	});
});
