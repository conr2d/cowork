// A tiny client for writing to the embedded terminal's PTY. The session is
// spawned by Terminal.svelte; the wizard's auth step uses this to send each
// agent's login command. The `@tauri-apps/api` import is dynamic so this module
// is safe to import from prerendered routes.

/** Send UTF-8 `data` to the embedded terminal's PTY (best-effort). */
export async function ptyWrite(data: string): Promise<void> {
	const { invoke } = await import('@tauri-apps/api/core');
	await invoke('pty_write', { data });
}
