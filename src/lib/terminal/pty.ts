// A tiny client for writing to keyed embedded-terminal PTY sessions. Sessions
// are spawned by Terminal.svelte; the wizard's auth step uses this to send each
// agent's login command. The `@tauri-apps/api` import is dynamic so this module
// is safe to import from prerendered routes.

/** Send UTF-8 `data` to PTY session `id` (best-effort; unknown id is a no-op). */
export async function ptyWrite(id: number, data: string): Promise<void> {
	const { invoke } = await import('@tauri-apps/api/core');
	await invoke('pty_write', { id, data });
}

/** Kill PTY session `id`. Unknown/already-killed ids are a no-op. */
export async function ptyKill(id: number): Promise<void> {
	const { invoke } = await import('@tauri-apps/api/core');
	await invoke('pty_kill', { id });
}
