/**
 * Decode one base64-framed PTY output chunk (sent by the host over the Tauri
 * Channel) back to raw bytes. The host base64-encodes raw PTY bytes so a
 * multibyte UTF-8 sequence split across read chunks survives transport intact;
 * xterm's `write(Uint8Array)` buffers any incomplete trailing sequence until the
 * next chunk, so per-chunk decoding is lossless.
 */
export function base64ToBytes(b64: string): Uint8Array {
	const binary = atob(b64);
	const bytes = new Uint8Array(binary.length);
	for (let i = 0; i < binary.length; i++) {
		bytes[i] = binary.charCodeAt(i);
	}
	return bytes;
}
