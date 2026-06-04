import data from '../../../errors.json';

export const ERROR_KINDS = [
	'Blocker',
	'NeedsUserAction',
	'Transient',
	'Internal',
	'Cancelled'
] as const;
export type Kind = (typeof ERROR_KINDS)[number];

export const STAGES = [
	'preflight',
	'wsl-enable',
	'provision',
	'toolchain',
	'agent-install',
	'auth',
	'done'
] as const;
export type Stage = (typeof STAGES)[number];

export const errorCodes = data.codes;
export type Code = keyof typeof errorCodes;

export interface Envelope {
	code: Code;
	kind: Kind;
	stage: Stage;
	context?: Record<string, string>;
	cause?: string;
}
