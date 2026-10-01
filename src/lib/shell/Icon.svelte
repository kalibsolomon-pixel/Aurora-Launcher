<script lang="ts">
  let { name, size = 22 }: { name: string; size?: number } = $props();
  // Original, simple geometric UI symbols. No icon font or remote dependency.
  const paths: Record<string, string> = {
    home: 'm3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1Z',
    instances: 'M3 4h18v13H3z M8 21h8 M12 17v4',
    settings: 'M12.000 3.000 L13.756 3.173 L14.679 5.533 L15.889 6.180 L18.364 5.636 L19.483 7.000 L18.467 9.321 L18.865 10.634 L21.000 12.000 L20.827 13.756 L18.467 14.679 L17.820 15.889 L18.364 18.364 L17.000 19.483 L14.679 18.467 L13.366 18.865 L12.000 21.000 L10.244 20.827 L9.321 18.467 L8.111 17.820 L5.636 18.364 L4.517 17.000 L5.533 14.679 L5.135 13.366 L3.000 12.000 L3.173 10.244 L5.533 9.321 L6.180 8.111 L5.636 5.636 L7.000 4.517 L9.321 5.533 L10.634 5.135 Z M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0',
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
    chevron: 'm6 9 6 6 6-6',
  };
</script>
<svg style="flex-shrink: 0; aspect-ratio: 1;" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={paths[name] ?? paths.about} /></svg>
