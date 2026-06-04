import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

// Tauri expects a fixed dev port and quietens Vite's screen clearing so the
// Rust logs stay visible. `1420` is Tauri's convention but we keep SvelteKit's
// 5173 to match `devUrl` in tauri.conf.json.
export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	clearScreen: false,
	server: {
		port: 5173,
		strictPort: true
	}
});
