import { isTauri, invoke as nativeInvoke } from '../../node_modules/@tauri-apps/api/core.js';
export { isTauri, nativeInvoke };
let handler: (command: string, args: unknown) => Promise<unknown> = nativeInvoke;
export function setReviewInvoke(next: typeof handler) { handler = next; }
export function invoke<T>(command: string, args?: unknown): Promise<T> { return handler(command, args) as Promise<T>; }
