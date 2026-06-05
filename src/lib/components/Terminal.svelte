<script lang="ts">
	import { onDestroy, onMount } from 'svelte';

	import type { Terminal as XTerm } from '@xterm/xterm';
	import '@xterm/xterm/css/xterm.css';

	import { base64ToBytes } from '$lib/terminal/decode';

	let {
		distro = 'Cowork',
		workspace = '~',
		locale = 'en'
	}: { distro?: string; workspace?: string; locale?: string } = $props();

	let container: HTMLDivElement | undefined = $state();
	let cleanup: (() => void) | undefined;

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

			const term = new Terminal({
				cursorBlink: true,
				fontFamily: 'Cascadia Code, Consolas, monospace',
				fontSize: 14,
				allowProposedApi: true,
				windowsPty: { backend: 'conpty' }
			});

			const fit = new FitAddon();
			term.loadAddon(fit);
			term.loadAddon(new Unicode11Addon());
			term.unicode.activeVersion = '11';
			term.loadAddon(new ClipboardAddon());

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

			const channel = new Channel<string>();
			channel.onmessage = (chunk) => term.write(base64ToBytes(chunk));

			await invoke('pty_spawn', {
				onData: channel,
				distro,
				workspace,
				locale,
				rows: term.rows,
				cols: term.cols
			});

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
				void invoke('pty_kill');
				term.dispose();
			};
		})();
	});

	onDestroy(() => cleanup?.());
</script>

<div bind:this={container} class="terminal-host"></div>

<style>
	.terminal-host {
		height: 100%;
		width: 100%;
	}
</style>
