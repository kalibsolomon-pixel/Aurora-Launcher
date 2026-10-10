<script lang="ts">
  let { name, size = 22 }: { name: string; size?: number } = $props();
  // Original, simple geometric UI symbols. No icon font or remote dependency.
  const paths: Record<string, string> = {
    home: 'm3 10 9-7 9 7v10a1 1 0 0 1-1 1h-5v-7H9v7H4a1 1 0 0 1-1-1Z',
    instances: 'M3 4h18v13H3z M8 21h8 M12 17v4',
    cosmetics: 'M8 3 2 7l3 5 1-1v10h12V11l1 1 3-5-6-4-4 3Z',
    settings: 'M12.000 3.000 L13.756 3.173 L14.679 5.533 L15.889 6.180 L18.364 5.636 L19.483 7.000 L18.467 9.321 L18.865 10.634 L21.000 12.000 L20.827 13.756 L18.467 14.679 L17.820 15.889 L18.364 18.364 L17.000 19.483 L14.679 18.467 L13.366 18.865 L12.000 21.000 L10.244 20.827 L9.321 18.467 L8.111 17.820 L5.636 18.364 L4.517 17.000 L5.533 14.679 L5.135 13.366 L3.000 12.000 L3.173 10.244 L5.533 9.321 L6.180 8.111 L5.636 5.636 L7.000 4.517 L9.321 5.533 L10.634 5.135 Z M15 12a3 3 0 1 1-6 0 3 3 0 0 1 6 0',
    about: 'M12 8h.01 M12 11v6 M22 12a10 10 0 1 1-20 0 10 10 0 0 1 20 0',
    developer: 'm8 6-6 6 6 6 m8-12 6 6-6 6 m-3-14-2 16',
    pencil: 'm15 4 5 5 M3 21l5-1L21 7a2 2 0 0 0-5-5L3 15Z',
    plus: 'M12 5v14 M5 12h14',
    download: 'M12 3v12 m-5-5 5 5 5-5 M5 16v5h14v-5',
    minimize: 'M5 12h14',
    maximize: 'M5 5h14v14H5z',
    restore: 'M8 8h12v12H8z M4 16V4h12',
    close: 'm6 6 12 12 M18 6 6 18',
    left: 'm14 5-7 7 7 7',
    right: 'm10 5 7 7-7 7',
    grip: 'M8 5h.01 M16 5h.01 M8 12h.01 M16 12h.01 M8 19h.01 M16 19h.01',
    check: 'm5 12 4 4L19 6',
    warning: 'M12 3 2 21h20Z M12 9v5 M12 17h.01',
    chevron: 'm6 9 6 6 6-6',
    mod: 'm12 3 9 5v8l-9 5-9-5V8Z M3 8l9 5 9-5 M12 13v8 M7.5 5.5l9 5',
    resourcePack: 'M4 4h16v16H4Z m0 12 5-5 4 4 3-3 4 4 M8 8h.01',
    shader: 'M12 2v2 M12 20v2 M2 12h2 M20 12h2 m-15-7 1.5 1.5 M17.5 17.5 19 19 M5 19l1.5-1.5 M17.5 6.5 19 5 M17 12a5 5 0 1 1-10 0 5 5 0 0 1 10 0',
    file: 'M5 3h9l5 5v13H5Z M14 3v5h5 M8 12h8 M8 16h6',
    modpack: 'M4 6h16v14H4Z M8 3h8 M4 10h16 M10 14h4',
    server: 'M4 4h16v6H4Z M4 14h16v6H4Z M7 7h.01 M7 17h.01 M12 10v4',
  };
</script>
<svg style="flex-shrink: 0; aspect-ratio: 1;" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d={paths[name] ?? paths.about} /></svg>
