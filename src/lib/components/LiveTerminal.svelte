<!--
  Test-harness terminal: real xterm wired to the pty-server websocket (NOT production).
  Lets the /shell chrome host a real agent session (?live=1) to validate Architecture A
  and the agent-TUI-on-light legibility spike. Production uses Terminal.svelte (Tauri PTY).
-->
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import type { Terminal as XTerm } from '@xterm/xterm';

	let {
		agent = 'claude',
		slug = 'default',
		dark = false
	}: { agent?: string; slug?: string; dark?: boolean } = $props();
	let host: HTMLDivElement;
	let cleanup: (() => void) | null = null;
	let term: XTerm | null = null;

	const lightTheme = {
		background: '#faf8f5',
		foreground: '#2c2620',
		cursor: '#b5532e',
		cursorAccent: '#faf8f5',
		selectionBackground: '#e6d6c8'
	};
	const darkTheme = {
		background: '#1a1916',
		foreground: '#ece6da',
		cursor: '#e0875c',
		cursorAccent: '#1a1916',
		selectionBackground: '#3a342b'
	};
	const themeFor = (d: boolean) => (d ? darkTheme : lightTheme);

	// keep the live xterm's theme in sync with the app theme (don't remount = don't kill the session)
	$effect(() => {
		const t = themeFor(dark);
		if (term) {
			term.options.theme = t;
			term.refresh(0, term.rows - 1);
		}
	});

	onMount(() => {
		let disposed = false;
		(async () => {
			const { Terminal } = await import('@xterm/xterm');
			const { FitAddon } = await import('@xterm/addon-fit');
			await import('@xterm/xterm/css/xterm.css');
			if (disposed || !host) return;
			const t = new Terminal({
				fontFamily: '"IBM Plex Mono", ui-monospace, monospace',
				fontSize: 13,
				cursorBlink: true,
				theme: themeFor(dark),
				allowProposedApi: true
			});
			term = t;
			const fit = new FitAddon();
			t.loadAddon(fit);
			t.open(host);
			fit.fit();

			const ws = new WebSocket(
				`ws://${location.hostname}:7682/?agent=${encodeURIComponent(agent)}&slug=${encodeURIComponent(slug)}`
			);
			const sendResize = () => {
				if (ws.readyState === 1) ws.send(`\x01resize:${t.cols}:${t.rows}`);
			};
			ws.onmessage = (e) => {
				if (typeof e.data === 'string') t.write(e.data);
			};
			ws.onopen = () => {
				sendResize();
				t.focus();
			};
			t.onData((d: string) => {
				if (ws.readyState === 1) ws.send(d);
			});
			const ro = new ResizeObserver(() => {
				try {
					fit.fit();
					sendResize();
				} catch {
					// fit can throw mid-teardown; the observer is disconnected right after
				}
			});
			ro.observe(host);

			cleanup = () => {
				ro.disconnect();
				try {
					ws.close();
				} catch {
					// closing an already-failed socket throws; nothing to clean up
				}
				term?.dispose();
			};
		})();
		return () => {
			disposed = true;
		};
	});
	onDestroy(() => cleanup?.());
</script>

<div class="live-term" bind:this={host}></div>

<style>
	.live-term {
		height: 100%;
		width: 100%;
	}
</style>
