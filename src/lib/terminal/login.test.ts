import { describe, expect, it } from 'vitest';

import { loginCommand, loginInput } from './login';

describe('loginCommand', () => {
	it('maps each agent to its native login command', () => {
		expect(loginCommand('claude')).toBe('claude auth login');
		expect(loginCommand('codex')).toBe('codex login');
		expect(loginCommand('antigravity')).toBe('agy');
	});
});

describe('loginInput', () => {
	it('appends a newline so the PTY runs the command', () => {
		expect(loginInput('codex')).toBe('codex login\n');
		expect(loginInput('claude')).toBe('claude auth login\n');
	});
});
