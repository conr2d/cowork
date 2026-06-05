// Frontend mirror of the Tauri command bridge's serialized types (WP9①). These
// shapes match the Rust `#[derive(Serialize)]` output exactly — including
// `PreflightReport`'s snake_case `can_proceed` (the Rust struct has no rename;
// do not "fix" it here, it must mirror the wire).

import type { Envelope, Stage } from '$lib/errors/registry';

export type { Envelope, Stage };

/** The 9 preflight checks (serde external enum → bare PascalCase strings). */
export type CheckId =
	| 'WindowsBuild'
	| 'Arch'
	| 'Virtualization'
	| 'HypervisorConflict'
	| 'Disk'
	| 'Elevation'
	| 'WslPolicy'
	| 'Store'
	| 'ControlledFolderAccess';

/** serde externally-tagged enum: `"Pass"` | `"Unknown"` | `{ Fail: Envelope }`. */
export type CheckStatus = 'Pass' | 'Unknown' | { Fail: Envelope };

export interface CheckOutcome {
	check: CheckId;
	status: CheckStatus;
}

export interface PreflightReport {
	outcomes: CheckOutcome[];
	can_proceed: boolean;
}

/** `wsl_enable` success result. */
export type WslEnableDto = 'Ready' | 'RebootRequired';

/** `provision_run` success result. */
export type ProvisionDto = 'Ready' | 'AlreadyExists';

/** Persisted reboot-resume state. */
export interface ResumeDto {
	stage: 'WslReady';
	selectedAgents: string[];
}

/** One live guest progress update (over the `ipc::Channel`). */
export interface ProgressEvent {
	stage: Stage;
	step: string;
}

/** Narrowing helper: did a `CheckStatus` fail? */
export function isFail(status: CheckStatus): status is { Fail: Envelope } {
	return typeof status === 'object' && status !== null && 'Fail' in status;
}
