/** The coding agents Cowork can install and log in (matches the guest `Agent`). */
export type AgentId = 'claude' | 'codex' | 'antigravity';

/**
 * The shell command that starts an agent's interactive login inside the
 * embedded terminal. The wizard sends this (followed by a newline) to the PTY
 * via `pty_write`; the agent then prints its OAuth URL, which the user clicks
 * (web-links → host browser) to finish signing in.
 */
export function loginCommand(agent: AgentId): string {
	switch (agent) {
		case 'claude':
			return 'claude';
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
