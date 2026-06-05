import { describe, expect, it } from 'vitest';

import { base64ToBytes } from './decode';

describe('base64ToBytes', () => {
	it('decodes ASCII text', () => {
		expect(Array.from(base64ToBytes(btoa('hello')))).toEqual([104, 101, 108, 108, 111]);
	});

	it('decodes raw bytes including zero and high values', () => {
		// base64 of [0x00, 0xFF, 0x10, 0x80]
		expect(Array.from(base64ToBytes('AP8QgA=='))).toEqual([0, 255, 16, 128]);
	});

	it('decodes a UTF-8 multibyte sequence losslessly', () => {
		// "한" = UTF-8 [0xED, 0x95, 0x9C]
		expect(Array.from(base64ToBytes('7ZWc'))).toEqual([0xed, 0x95, 0x9c]);
	});

	it('decodes an empty chunk', () => {
		expect(base64ToBytes('').length).toBe(0);
	});
});
