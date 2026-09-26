/** Shell modality is independent of global/workspace navigation. */
export const accountManager = new class {
  open = $state(false);
  show(): void { this.open = true; }
  close(): void { this.open = false; }
}();
