import { describe, expect, it } from 'vitest';

import { loginCommand } from './login';

describe('loginCommand', () => {
	it('maps each agent to its native login command', () => {
		expect(loginCommand('claude')).toBe('claude');
		expect(loginCommand('codex')).toBe('codex login');
		expect(loginCommand('antigravity')).toBe('agy');
	});
});
