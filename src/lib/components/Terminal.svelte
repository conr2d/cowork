<script lang="ts">
	import { onDestroy, onMount } from 'svelte';

	import type { Terminal as XTerm } from '@xterm/xterm';
	import '@xterm/xterm/css/xterm.css';

	import type { Envelope, Kind } from '$lib/errors/registry';
	import { asEnvelope } from '$lib/host/client';
	import * as m from '$lib/paraglide/messages';
	import { base64ToBytes } from '$lib/terminal/decode';
	import { createUrlScanner } from '$lib/terminal/links';

	const XTERM_LIGHT = {
		background: '#faf8f5',
		foreground: '#2c2620',
		cursor: '#b5532e',
		cursorAccent: '#faf8f5',
		selectionBackground: '#e6d6c8'
	};
	const XTERM_DARK = {
		background: '#1a1916',
		foreground: '#ece6da',
		cursor: '#e0875c',
		cursorAccent: '#1a1916',
		selectionBackground: '#3a342b'
	};

	function palette(value: 'light' | 'dark') {
		return value === 'light' ? XTERM_LIGHT : XTERM_DARK;
	}

	let {
		distro = 'Cowork',
		workspace = '~',
		locale = 'en',
		detectLinks = false,
		theme = undefined,
		autorun = undefined,
		onspawn = undefined,
		active = false,
		onactivity = undefined
	}: {
		distro?: string;
		workspace?: string;
		locale?: string;
		detectLinks?: boolean;
		theme?: 'light' | 'dark';
		autorun?: string;
		onspawn?: (id: number) => void;
		active?: boolean;
		onactivity?: (event: 'output' | 'exit') => void;
	} = $props();

	let container: HTMLDivElement | undefined = $state();
	let cleanup: (() => void) | undefined;
	let detectedUrl = $state<string | null>(null);
	let termRef = $state<XTerm | undefined>(undefined);
	let openUrl = $state<((url: string) => Promise<void>) | undefined>(undefined);
	let initFailure = $state<{ envelope: Envelope } | { message: string } | null>(null);
	let initNotice = $state<string | null>(null);

	function clearDetectedUrl() {
		detectedUrl = null;
	}

	function refocusTerminal() {
		termRef?.focus();
	}

	function dismissDetectedUrl() {
		clearDetectedUrl();
		refocusTerminal();
	}

	function openDetectedUrl() {
		if (detectedUrl && openUrl) void openUrl(detectedUrl);
		refocusTerminal();
	}

	function bodyFor(kind: Kind): string {
		switch (kind) {
			case 'Blocker':
				return m.error_blocker_body();
			case 'NeedsUserAction':
				return m.error_needs_action_body();
			case 'Transient':
				return m.error_transient_body();
			case 'Internal':
				return m.error_internal_body();
			case 'Cancelled':
				return m.error_cancelled_body();
		}
	}

	function messageFrom(error: unknown): string {
		if (error instanceof Error && error.message.trim()) return error.message;
		if (typeof error === 'string' && error.trim()) return error;
		if (typeof error === 'object' && error !== null) {
			const maybeMessage = (error as { message?: unknown }).message;
			if (typeof maybeMessage === 'string' && maybeMessage.trim()) return maybeMessage;
		}
		return 'Unknown error';
	}

	function isEnvelope(error: unknown): error is Envelope {
		if (typeof error !== 'object' || error === null) return false;
		const maybe = error as Partial<Envelope>;
		return (
			typeof maybe.code === 'string' &&
			typeof maybe.kind === 'string' &&
			typeof maybe.stage === 'string'
		);
	}

	function captureFailure(error: unknown): void {
		initFailure = isEnvelope(error)
			? { envelope: asEnvelope(error) }
			: { message: messageFrom(error) };
	}

	// Attribute a failing chunk load to its module. The specifier must stay a
	// STRING LITERAL inside import() — Vite only code-splits imports it can analyze
	// statically, and a variable specifier silently drops the chunk from the bundle
	// (it survives the dev server, then 404s in the packaged app).
	async function loadModule<T>(name: string, load: () => Promise<T>): Promise<T> {
		try {
			return await load();
		} catch (error) {
			throw new Error(`Failed to load ${name}: ${messageFrom(error)}`);
		}
	}

	async function loadCanvasFallback(term: XTerm): Promise<void> {
		const { CanvasAddon } = await loadModule(
			'@xterm/addon-canvas',
			() => import('@xterm/addon-canvas')
		);
		term.loadAddon(new CanvasAddon());
	}

	onMount(() => {
		// Init runs in an async IIFE; onMount itself must return the (sync) cleanup.
		void (async () => {
			try {
				if (!container) return;
				clearDetectedUrl();
				initFailure = null;
				initNotice = null;

				// Dynamic imports: xterm touches `document`, so it must never load during
				// SSR/prerender — only here, client-side, after mount.
				const { Terminal } = await loadModule('@xterm/xterm', () => import('@xterm/xterm'));
				const { FitAddon } = await loadModule('@xterm/addon-fit', () => import('@xterm/addon-fit'));
				const { Unicode11Addon } = await loadModule(
					'@xterm/addon-unicode11',
					() => import('@xterm/addon-unicode11')
				);
				const { ClipboardAddon } = await loadModule(
					'@xterm/addon-clipboard',
					() => import('@xterm/addon-clipboard')
				);
				const { invoke, Channel } = await loadModule(
					'@tauri-apps/api/core',
					() => import('@tauri-apps/api/core')
				);
				const { WebLinksAddon } = await loadModule(
					'@xterm/addon-web-links',
					() => import('@xterm/addon-web-links')
				);

				try {
					const opener = await loadModule(
						'@tauri-apps/plugin-opener',
						() => import('@tauri-apps/plugin-opener')
					);
					openUrl = opener.openUrl;
				} catch (error) {
					openUrl = undefined;
					initNotice = messageFrom(error);
					console.error(error);
				}

				const term = new Terminal({
					cursorBlink: true,
					fontFamily: theme
						? "'IBM Plex Mono', Cascadia Code, Consolas, monospace"
						: 'Cascadia Code, Consolas, monospace',
					fontSize: 14,
					allowProposedApi: true,
					windowsPty: { backend: 'conpty' },
					theme: theme ? palette(theme) : undefined
				});
				termRef = term;

				const fit = new FitAddon();
				term.loadAddon(fit);
				term.loadAddon(new Unicode11Addon());
				term.unicode.activeVersion = '11';
				term.loadAddon(new ClipboardAddon());
				// Make URLs (e.g. an agent's OAuth login link) clickable → open in the
				// host Windows browser via the opener plugin. This is the reliable
				// handoff: WSL-side xdg-open/$BROWSER is unreliable (wslu was archived),
				// so the host opens the browser, not the guest.
				if (openUrl) {
					term.loadAddon(
						new WebLinksAddon((_event, uri) => {
							const target = detectedUrl && detectedUrl.startsWith(uri) ? detectedUrl : uri;
							void openUrl?.(target);
						})
					);
				}

				term.open(container);

				// WebGL renderer with Canvas fallback (streaming-output performance).
				try {
					const { WebglAddon } = await loadModule(
						'@xterm/addon-webgl',
						() => import('@xterm/addon-webgl')
					);
					const webgl = new WebglAddon();
					webgl.onContextLoss(() => {
						webgl.dispose();
						void loadCanvasFallback(term).catch((error) => {
							initNotice = `Failed to load @xterm/addon-webgl after context loss; ${messageFrom(error)}`;
							console.error(error);
						});
					});
					term.loadAddon(webgl);
				} catch (error) {
					initNotice = messageFrom(error);
					console.error(error);
					await loadCanvasFallback(term).catch((fallbackError) => {
						initNotice = `${messageFrom(error)}; canvas fallback failed: ${messageFrom(fallbackError)}`;
						console.error(fallbackError);
					});
				}

				// Fit BEFORE spawn so the ConPTY is the measured size from the first byte
				// (a hardcoded default corrupts a full-screen TUI's first frame).
				fit.fit();

				const scanner = createUrlScanner();
				const decoder = new TextDecoder();
				const channel = new Channel<string>();
				channel.onmessage = (chunk) => {
					if (chunk === '') {
						// Host EOF sentinel: the PTY child exited.
						onactivity?.('exit');
						return;
					}
					const bytes = base64ToBytes(chunk);
					term.write(bytes);
					onactivity?.('output');
					// createUrlScanner only reports OAuth/device sign-in URLs (isLoginUrl),
					// so an agent's own login prompt surfaces the button while the shell's
					// MOTD and docs links never do.
					if (detectLinks && openUrl) {
						const url = scanner.push(decoder.decode(bytes, { stream: true }));
						if (url) detectedUrl = url;
					}
				};

				const sessionId = await invoke<number>('pty_spawn', {
					onData: channel,
					distro,
					workspace,
					locale,
					rows: term.rows,
					cols: term.cols
				});
				onspawn?.(sessionId);
				term.focus();
				if (autorun) {
					await invoke('pty_write', { id: sessionId, data: `${autorun}\n` });
				}

				const dataSub = term.onData((data) => {
					void invoke('pty_write', { id: sessionId, data });
				});

				const observer = new ResizeObserver(() => {
					if (!container || container.offsetWidth === 0 || container.offsetHeight === 0) return;
					fit.fit();
					void invoke('pty_resize', { id: sessionId, rows: term.rows, cols: term.cols });
				});
				observer.observe(container);

				cleanup = () => {
					dataSub.dispose();
					observer.disconnect();
					void invoke('pty_kill', { id: sessionId });
					term.dispose();
				};
			} catch (error) {
				captureFailure(error);
				console.error(error);
			}
		})();
	});

	// Focus when this terminal's tab becomes the active one.
	$effect(() => {
		if (active) termRef?.focus();
	});

	$effect(() => {
		if (theme && termRef) {
			termRef.options.theme = palette(theme);
			termRef.refresh(0, termRef.rows - 1);
			// Claude adapts live; Codex may keep stale background bands until the
			// session respawns. WP4 adds respawn + resume for that adapter.
		}
	});

	onDestroy(() => cleanup?.());
</script>

<div class="terminal-wrap">
	{#if initFailure}
		<section class="terminal-error" class:is-dark={theme === 'dark'}>
			<div class="terminal-error-copy">
				<h2>{m.error_title()}</h2>
				{#if 'envelope' in initFailure}
					<p>{bodyFor(initFailure.envelope.kind)}</p>
					<code>{initFailure.envelope.code}</code>
					{#if initFailure.envelope.cause}
						<div class="terminal-error-cause">
							<span>{m.error_cause_label()}</span>
							<pre>{initFailure.envelope.cause}</pre>
						</div>
					{/if}
				{:else}
					<p>{initFailure.message}</p>
				{/if}
			</div>
		</section>
	{/if}
	{#if initNotice}
		<div class="terminal-notice" class:is-dark={theme === 'dark'}>
			<strong>Terminal notice</strong>
			<span>{initNotice}</span>
		</div>
	{/if}
	{#if detectLinks && detectedUrl && openUrl}
		<div class="signin-toast" class:is-dark={theme === 'dark'}>
			<button type="button" class="signin-open" onclick={openDetectedUrl}>
				<span aria-hidden="true">🔗</span>
				<span class="signin-label">{m.auth_open_login()}</span>
			</button>
			<button
				type="button"
				class="signin-dismiss"
				aria-label={m.auth_dismiss()}
				onclick={dismissDetectedUrl}
			>
				<span aria-hidden="true">✕</span>
			</button>
		</div>
	{/if}
	<div bind:this={container} class="terminal-host"></div>
</div>

<style>
	.terminal-wrap {
		position: relative;
		display: flex;
		height: 100%;
		width: 100%;
		flex-direction: column;
		gap: 0.5rem;
	}
	.terminal-host {
		min-height: 0;
		width: 100%;
		flex: 1 1 auto;
	}
	.terminal-error,
	.terminal-notice {
		position: absolute;
		inset-inline: 0;
		z-index: 20;
		border-bottom: 1px solid var(--terminal-overlay-border);
		background: var(--terminal-overlay-bg);
		color: var(--terminal-overlay-fg);

		--terminal-overlay-bg: rgb(250 250 250 / 0.97);
		--terminal-overlay-fg: rgb(23 23 23);
		--terminal-overlay-border: rgb(229 229 229);
		--terminal-overlay-muted: rgb(115 115 115);
		--terminal-overlay-panel: rgb(245 245 245);
	}
	.terminal-error.is-dark,
	.terminal-notice.is-dark {
		--terminal-overlay-bg: rgb(23 23 23 / 0.97);
		--terminal-overlay-fg: rgb(245 245 245);
		--terminal-overlay-border: rgb(64 64 64);
		--terminal-overlay-muted: rgb(163 163 163);
		--terminal-overlay-panel: rgb(38 38 38);
	}
	.terminal-error {
		inset: 0;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 1rem;
	}
	.terminal-error-copy {
		display: flex;
		width: min(100%, 40rem);
		flex-direction: column;
		gap: 0.5rem;
		border: 1px solid var(--terminal-overlay-border);
		border-radius: 0.75rem;
		padding: 1rem;
		background: var(--terminal-overlay-bg);
		box-shadow: 0 8px 28px -8px rgba(20, 14, 8, 0.34);
	}
	.terminal-error-copy h2 {
		font-size: 0.9375rem;
		font-weight: 600;
	}
	.terminal-error-copy p,
	.terminal-notice span,
	.terminal-error-cause span {
		font-size: 0.875rem;
	}
	.terminal-error-copy code,
	.terminal-notice strong {
		font-family: 'IBM Plex Mono', monospace;
		font-size: 0.75rem;
	}
	.terminal-error-copy code {
		color: var(--terminal-overlay-muted);
	}
	.terminal-error-cause {
		display: flex;
		flex-direction: column;
		gap: 0.25rem;
	}
	.terminal-error-cause span {
		color: var(--terminal-overlay-muted);
	}
	.terminal-error-cause pre {
		overflow-x: auto;
		border-radius: 0.5rem;
		padding: 0.75rem;
		background: var(--terminal-overlay-panel);
		font-size: 0.75rem;
		white-space: pre-wrap;
		word-break: break-word;
	}
	.terminal-notice {
		top: 0;
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 0.75rem;
	}

	/* Overlaid, not in the flex flow: the terminal must not resize when the toast
	   toggles — a reflow mid-login corrupts the agent's TUI. Colors follow the
	   session's theme, like the xterm palette above; the shell's dark class does
	   not reach into this component. */
	.signin-toast {
		position: absolute;
		inset-inline: 0;
		top: 0;
		z-index: 10;
		display: flex;
		align-items: center;
		gap: 0.5rem;
		padding: 0.5rem 0.75rem;
		font-size: 0.875rem;
		border-bottom: 1px solid var(--toast-border);
		background: var(--toast-bg);
		color: var(--toast-fg);

		--toast-bg: rgb(250 250 250 / 0.97);
		--toast-fg: rgb(23 23 23);
		--toast-border: rgb(229 229 229);
		--toast-muted: rgb(115 115 115);
		--toast-hover: rgb(229 229 229);
	}
	.signin-toast.is-dark {
		--toast-bg: rgb(23 23 23 / 0.97);
		--toast-fg: rgb(245 245 245);
		--toast-border: rgb(64 64 64);
		--toast-muted: rgb(163 163 163);
		--toast-hover: rgb(64 64 64);
	}
	.signin-open {
		display: flex;
		min-width: 0;
		flex: 1 1 auto;
		align-items: center;
		gap: 0.5rem;
		text-align: left;
		font-weight: 500;
		color: inherit;
	}
	.signin-label {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.signin-dismiss {
		flex: 0 0 auto;
		border-radius: 0.25rem;
		padding: 0.25rem;
		color: var(--toast-muted);
	}
	.signin-open:hover,
	.signin-dismiss:hover {
		color: var(--toast-fg);
	}
	.signin-dismiss:hover {
		background: var(--toast-hover);
	}
	.signin-open:focus-visible,
	.signin-dismiss:focus-visible {
		outline: 2px solid var(--toast-muted);
		outline-offset: 1px;
	}
</style>
