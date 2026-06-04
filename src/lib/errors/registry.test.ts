import { describe, expect, it } from 'vitest';

import { ERROR_KINDS, errorCodes } from './registry';

describe('error registry', () => {
	it('every code has a valid kind', () => {
		const validKinds: readonly string[] = ERROR_KINDS;
		for (const entry of Object.values(errorCodes)) {
			expect(validKinds.includes(entry.kind)).toBe(true);
		}
	});

	it('contextKeys is always an array', () => {
		for (const entry of Object.values(errorCodes)) {
			expect(Array.isArray(entry.contextKeys)).toBe(true);
		}
	});

	it('known codes are present', () => {
		expect('common.cancelled' in errorCodes).toBe(true);
		expect('preflight.windows_build_unsupported' in errorCodes).toBe(true);
	});
});
