import { isTauri, invoke as nativeInvoke } from '../../node_modules/@tauri-apps/api/core.js';
export { isTauri, nativeInvoke };
let handler: (command: string, args: unknown) => Promise<unknown> = nativeInvoke;
export function setReviewInvoke(next: typeof handler) { handler = next; }
// Development diagnostics can observe/extend the fixtures while preserving
// their handler. This module is never imported by the production application.
export function wrapReviewInvoke(wrap: (previous: typeof handler) => typeof handler) {
  handler = wrap(handler);
}
export function invoke<T>(command: string, args?: unknown): Promise<T> { return handler(command, args) as Promise<T>; }
