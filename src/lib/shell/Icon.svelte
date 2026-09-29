<script lang="ts">
  let { name, size = 22 }: { name: string; size?: number } = $props();
  // Original, simple geometric UI symbols. No icon font or remote dependency.
  const paths: Record<string, string> = {
    home: 'm3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1Z',
    instances: 'M3 4h18v13H3z M8 21h8 M12 17v4',
    settings: 'm9 3-1 3-3 1-2 3 2 2-1 3 2 3 3-1 3 2 3-2 3 1 2-3-1-3 2-2-2-3-3-1-1-3Z M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0',
    about: 'M12 8h.01 M12 11v6 M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
    developer: 'm8 6-6 6 6 6 m8-12 6 6-6 6 m-3-14-2 16',
    pencil: 'm15 4 5 5 M3 21l5-1L21 7a2 2 0 0 0-5-5L3 15Z',
    plus: 'M12 5v14 M5 12h14',
    minimize: 'M5 12h14',
    maximize: 'M5 5h14v14H5z',
    restore: 'M8 8h12v12H8z M4 16V4h12',
    close: 'm6 6 12 12 M18 6 6 18',
    left: 'm14 5-7 7 7 7',
    right: 'm10 5 7 7-7 7',
    grip: 'M8 5h.01 M16 5h.01 M8 12h.01 M16 12h.01 M8 19h.01 M16 19h.01',
    check: 'm5 12 4 4L19 6',
  };
</script>
<svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={paths[name] ?? paths.about} /></svg>
