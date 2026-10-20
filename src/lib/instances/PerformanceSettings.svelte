<script lang="ts">
  import { getInstancePerformanceAdvice, type InstanceConfiguration, type InstanceSummary, type MemoryAdvice, type PerformanceMode } from '$lib/backend';
  let { instance, draft, busy = false, onModeChange, onMemoryChange }: { instance: InstanceSummary; draft: InstanceConfiguration; busy?: boolean; onModeChange: (mode: PerformanceMode | null) => void; onMemoryChange: (mib: number) => void } = $props();
  let advice = $state<MemoryAdvice | null>(null);
  let error = $state('');
  let loading = $state(false);
  let applying = $state(false);
  let adviceNotice = $state('');
  let adviceGeneration = 0;
  let undo = $state<{ memoryMib: number } | null>(null);
  const descriptions: Record<PerformanceMode, string> = {
    efficient: 'Conservative memory advice. No CPU throttling or speculative JVM tuning.',
    balanced: 'Conservative advice with Java’s default ergonomics.',
    performance: 'Currently the same advice as Balanced. Different tuning needs measured evidence.',
    custom: 'Manual control of the supported maximum heap and advanced arguments.'
  };
  const label = (mode: PerformanceMode | null | undefined) => mode ? mode[0].toUpperCase() + mode.slice(1) : 'Existing Configuration';
  $effect(() => {
    ++adviceGeneration;
    const id = instance.id, mode = draft.performanceMode ?? null;
    // Refresh after an authoritative save, without watching keystrokes or polling.
    const savedText = instance.configuration.additionalJvmArguments;
    void savedText;
    let current = true;
    advice = null; error = ''; loading = true; undo = null;
    getInstancePerformanceAdvice(id, mode).then(value => { if (current) advice = value; })
      .catch(reason => { if (current) error = reason instanceof Error ? reason.message : 'Memory advice is unavailable. Saved settings are unchanged.'; })
      .finally(() => { if (current) loading = false; });
    return () => { current = false; };
  });
  async function applyAdvice() {
    if (advice?.recommendedMib == null || busy) return;
    const id = instance.id, mode = draft.performanceMode ?? null, shown = advice.recommendedMib;
    const generation = adviceGeneration, originalHeap = draft.memoryMib;
    applying = true; adviceNotice = '';
    try {
      const fresh = await getInstancePerformanceAdvice(id, mode);
      if (generation !== adviceGeneration || id !== instance.id || mode !== (draft.performanceMode ?? null)) return;
      advice = fresh;
      if (draft.memoryMib !== originalHeap) { adviceNotice = 'Your draft changed. Review the recommendation before using it.'; return; }
      if (fresh.recommendedMib !== shown) { adviceNotice = 'Memory advice changed. Review the new value before using it.'; return; }
      undo = { memoryMib: draft.memoryMib };
      onMemoryChange(shown);
    } catch (reason) { if (generation === adviceGeneration) error = reason instanceof Error ? reason.message : 'Advice could not be refreshed. Your heap is unchanged.'; }
    finally { applying = false; }
  }
</script>

<section aria-labelledby="performance-heading" class="performance-settings">
  <h4 id="performance-heading" class="detail-section-title">Performance</h4>
  <p class="field-hint">Modes choose memory advice. They do not automatically change your heap or arguments. Performance improvements remain unmeasured.</p>
  <p class="field-hint" aria-live="polite">Saved mode: {label(instance.configuration.performanceMode)} · Saved maximum heap: {instance.configuration.memoryMib} MiB.</p>
  <label class="field">
    <span class="field-label">Configuration mode</span>
    <select aria-label="Configuration mode" value={draft.performanceMode ?? ''} disabled={busy} onchange={event => onModeChange((event.currentTarget.value || null) as PerformanceMode | null)} aria-describedby="mode-description">
      <option value="">Existing Configuration</option>
      <option value="efficient">Efficient</option>
      <option value="balanced">Balanced</option>
      <option value="performance">Performance</option>
      <option value="custom">Custom</option>
    </select>
  </label>
  <p id="mode-description" class="field-hint">{draft.performanceMode ? descriptions[draft.performanceMode] : 'Keeps the established JVM policy and your saved settings. Choose a mode only when you want to opt in.'}</p>
  {#if (draft.performanceMode ?? null) !== (instance.configuration.performanceMode ?? null)}<p class="field-hint" role="status">Unsaved mode: {label(draft.performanceMode)}. Heap and custom text are kept.</p>{/if}
  <label class="field">
    <span class="field-label">Memory (MiB)</span>
    <input type="number" min="512" max="32768" step="1" value={draft.memoryMib} oninput={event => { if (Number.isFinite(event.currentTarget.valueAsNumber)) onMemoryChange(event.currentTarget.valueAsNumber); }} disabled={busy} required aria-describedby="heap-description" />
  </label>
  <p id="heap-description" class="field-hint">Maximum Java heap, 512–32768 MiB. Java chooses the initial heap. Native Java and graphics need additional memory. More heap does not guarantee more FPS.</p>
  <div aria-live="polite">
    {#if loading}<p class="field-hint">Reading memory advice…</p>{:else if error}<p class="inline-message inline-message-error" role="alert">{error}</p>
    {:else if advice}
      <p class="field-hint">System RAM: {advice.facts.totalMib ?? 'unknown'} MiB · Available: {advice.facts.availableMib ?? 'unknown'} MiB · {advice.facts.architecture} · observation age {advice.facts.ageSeconds}s.</p>
      <p class="field-hint">Recommended maximum: {advice.recommendedMib == null ? 'unavailable' : `${advice.recommendedMib} MiB`} (conservative heuristic).</p>
      {#if advice.recommendedMib != null}<button type="button" class="btn" disabled={busy || applying || draft.memoryMib === advice.recommendedMib} onclick={() => void applyAdvice()}>Use {advice.recommendedMib} MiB in draft</button>{/if}
      <details><summary>How this advice was calculated</summary><p class="field-hint">{advice.explanation}</p><p class="field-hint">Source: {advice.facts.source}.</p>{#each advice.unknowns as unknown}<p class="field-hint">{unknown}</p>{/each}</details>
      {#each advice.argumentNotices as notice}<p class="field-hint">{notice}</p>{/each}
    {/if}
  </div>
  {#if adviceNotice}<p class="field-hint" role="status">{adviceNotice}</p>{/if}
  {#if undo}<button type="button" class="btn btn-quiet" disabled={busy} onclick={() => { if (undo) onMemoryChange(undo.memoryMib); undo = null; }}>Undo memory recommendation</button>{/if}
  <p class="field-hint">Save changes applies the draft to this instance’s next launch. Custom arguments remain in Java below. Initial-heap and collector controls are not supported in this phase.</p>
</section>

<style>
  .performance-settings { min-width: 0; overflow-wrap: anywhere; }
  .field-hint { font-size: var(--text-metadata); color: var(--color-text-muted); margin-block: var(--space-2); }
  .detail-section-title { font-size: .95rem; margin-block: var(--space-2) var(--space-3); }
  select, input { max-width: 100%; min-width: 0; }
  button { white-space: normal; max-width: 100%; margin-block: var(--space-2); }
  details { margin-block: var(--space-2); }
  summary { cursor: pointer; }
</style>
