/** The coding agents Cowork can install and log in (matches the guest `Agent`). */
export type AgentId = 'claude' | 'codex' | 'antigravity';

/**
 * The shell command that starts an agent's interactive login inside the
 * embedded terminal. The wizard sends this (followed by a newline) to the PTY
 * via `pty_write`. Each agent then either opens the host browser itself (via
 * WSL interop) or prints an OAuth URL; for the print-only case the user clicks
 * the surfaced "Open sign-in page" button (host browser) to finish signing in.
 *
 * claude uses the dedicated `claude auth login` (not bare `claude`) so it runs a
 * focused sign-in flow — like `codex login` — instead of dropping into the
 * onboarding REPL (which shows a theme picker and prints the URL inline).
 */
export function loginCommand(agent: AgentId): string {
	switch (agent) {
		case 'claude':
			return 'claude auth login';
		case 'codex':
			return 'codex login';
		case 'antigravity':
			return 'agy';
	}
}

/** The full line sent to the PTY to start an agent's login (command + newline). */
export function loginInput(agent: AgentId): string {
	return `${loginCommand(agent)}\n`;
}
