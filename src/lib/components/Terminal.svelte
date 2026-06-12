<script lang="ts">
	import { onDestroy, onMount } from 'svelte';

	import type { Terminal as XTerm } from '@xterm/xterm';
	import '@xterm/xterm/css/xterm.css';

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
		loginAttempts = 0,
		theme = undefined,
		autorun = undefined
	}: {
		distro?: string;
		workspace?: string;
		locale?: string;
		detectLinks?: boolean;
		loginAttempts?: number;
		theme?: 'light' | 'dark';
		autorun?: string;
	} = $props();

	let container: HTMLDivElement | undefined = $state();
	let cleanup: (() => void) | undefined;
	let detectedUrl = $state<string | null>(null);
	let termRef = $state<XTerm | undefined>(undefined);
	let openUrl: ((url: string) => Promise<void>) | undefined;

	async function loadCanvasFallback(term: XTerm): Promise<void> {
		const { CanvasAddon } = await import('@xterm/addon-canvas');
		term.loadAddon(new CanvasAddon());
	}

	onMount(() => {
		// Init runs in an async IIFE; onMount itself must return the (sync) cleanup.
		void (async () => {
			if (!container) return;

			// Dynamic imports: xterm touches `document`, so it must never load during
			// SSR/prerender — only here, client-side, after mount.
			const { Terminal } = await import('@xterm/xterm');
			const { FitAddon } = await import('@xterm/addon-fit');
			const { Unicode11Addon } = await import('@xterm/addon-unicode11');
			const { ClipboardAddon } = await import('@xterm/addon-clipboard');
			const { WebglAddon } = await import('@xterm/addon-webgl');
			const { invoke, Channel } = await import('@tauri-apps/api/core');
			const { WebLinksAddon } = await import('@xterm/addon-web-links');
			const opener = await import('@tauri-apps/plugin-opener');
			openUrl = opener.openUrl;

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
			term.loadAddon(
				new WebLinksAddon((_event, uri) => {
					void openUrl!(uri);
				})
			);

			term.open(container);

			// WebGL renderer with Canvas fallback (streaming-output performance).
			try {
				const webgl = new WebglAddon();
				webgl.onContextLoss(() => {
					webgl.dispose();
					void loadCanvasFallback(term);
				});
				term.loadAddon(webgl);
			} catch {
				await loadCanvasFallback(term);
			}

			// Fit BEFORE spawn so the ConPTY is the measured size from the first byte
			// (a hardcoded default corrupts a full-screen TUI's first frame).
			fit.fit();

			const scanner = createUrlScanner();
			const decoder = new TextDecoder();
			const channel = new Channel<string>();
			channel.onmessage = (chunk) => {
				const bytes = base64ToBytes(chunk);
				term.write(bytes);
				// Only scan once a login has been triggered — otherwise the shell's
				// startup banner (e.g. the Ubuntu MOTD URL) would surface the button
				// before the user has started signing in.
				if (detectLinks && loginAttempts > 0) {
					const url = scanner.push(decoder.decode(bytes, { stream: true }));
					if (url) detectedUrl = url;
				}
			};

			const generation = await invoke<number>('pty_spawn', {
				onData: channel,
				distro,
				workspace,
				locale,
				rows: term.rows,
				cols: term.cols
			});
			term.focus();
			if (autorun) {
				await invoke('pty_write', { data: `${autorun}\n` });
			}

			const dataSub = term.onData((data) => {
				void invoke('pty_write', { data });
			});

			const observer = new ResizeObserver(() => {
				fit.fit();
				void invoke('pty_resize', { rows: term.rows, cols: term.cols });
			});
			observer.observe(container);

			cleanup = () => {
				dataSub.dispose();
				observer.disconnect();
				void invoke('pty_kill', { generation });
				term.dispose();
			};
		})();
	});

	// Focus the terminal each time a login is triggered, so the user can type /
	// paste (e.g. an auth code) without having to click into the terminal first.
	$effect(() => {
		if (loginAttempts > 0) termRef?.focus();
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
	{#if detectLinks && detectedUrl}
		<button
			type="button"
			class="flex items-center gap-2 self-start rounded bg-neutral-100 px-3 py-1.5 text-sm font-medium text-neutral-900 hover:bg-neutral-200"
			onclick={() => {
				if (detectedUrl && openUrl) void openUrl(detectedUrl);
			}}
		>
			<span aria-hidden="true">🔗</span>
			<span>{m.auth_open_login()}</span>
		</button>
	{/if}
	<div bind:this={container} class="terminal-host"></div>
</div>

<style>
	.terminal-wrap {
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
</style>
