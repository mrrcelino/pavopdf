<script lang="ts">
  import { api } from '../../../api';
  import { operationStore } from '../../../stores/operation.svelte';
  import ProgressBar from '../../ui/ProgressBar.svelte';
  import type { Tool } from '../../../types';

  type Preset = 'small' | 'balanced' | 'high_quality';

  const PRESETS: { id: Preset; label: string; description: string }[] = [
    { id: 'small', label: 'Small file', description: '72 DPI — smallest size, lower quality' },
    { id: 'balanced', label: 'Balanced', description: '150 DPI — good size/quality trade-off' },
    { id: 'high_quality', label: 'High quality', description: '220 DPI — near-lossless, larger file' },
  ];

  let filePath = $state<string | null>(null);
  let preset = $state<Preset>('balanced');
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);
  let running = $state(false);
  let progress = $state(0);
  let progressLabel = $state('');

  const canRun = $derived(filePath !== null && !running);

  function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  async function pickFile() {
    if (running) return;
    const selected = await api.openFileDialog(false);
    if (!selected || selected.length === 0) return;
    filePath = selected[0];
    error = null;
    outputPath = null;
  }

  async function runCompress() {
    if (!canRun || !filePath) return;
    error = null;
    outputPath = null;

    const outFile = await api.saveFileDialog('compressed.pdf');
    if (!outFile) return;

    const opId = crypto.randomUUID();
    running = true;
    progress = 0;
    progressLabel = 'Starting…';
    operationStore.start(opId, 'compress' as Tool);

    try {
      const result = await api.processPdf({
        operation_id: opId,
        tool: 'compress',
        input_paths: [filePath],
        output_stem: 'compressed',
        options: { preset },
      });
      outputPath = result;
      progress = 100;
      progressLabel = 'Done';
      operationStore.complete(opId);
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
      operationStore.fail(opId, msg);
    } finally {
      running = false;
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 h-full overflow-hidden">
  <header class="shrink-0">
    <h2 class="text-lg font-semibold text-stone-800">Compress PDF</h2>
  </header>

  <p class="text-xs text-stone-500 bg-stone-50 border border-stone-200 rounded-lg px-3 py-2 shrink-0">
    Compression targets embedded images. Text and vector graphics are unaffected.
  </p>

  <button
    onclick={pickFile}
    class={[
      'shrink-0 flex items-center gap-3 p-3 rounded-lg border-2 text-left transition-colors',
      running ? 'opacity-60 cursor-not-allowed' : '',
      filePath
        ? 'border-stone-200 bg-white hover:border-teal-300'
        : 'border-dashed border-stone-300 hover:border-teal-400 hover:text-teal-600',
    ].join(' ')}
    aria-label="Pick a PDF file to compress"
    disabled={running}
  >
    <svg class="w-5 h-5 shrink-0 text-stone-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5"
        d="M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z" />
    </svg>
    {#if filePath}
      <span class="text-sm text-stone-700 truncate">{fileNameFromPath(filePath)}</span>
    {:else}
      <span class="text-sm text-stone-400">Click to select a PDF file...</span>
    {/if}
  </button>

  <fieldset class="shrink-0 flex flex-col gap-2" disabled={running}>
    <legend class="text-xs text-stone-500 mb-1">Compression preset</legend>
    {#each PRESETS as p (p.id)}
      <label
        class={[
          'flex items-start gap-3 p-3 rounded-lg border cursor-pointer transition-colors',
          preset === p.id
            ? 'border-teal-400 bg-teal-50'
            : 'border-stone-200 bg-white hover:border-stone-300',
        ].join(' ')}
      >
        <input
          type="radio"
          name="compress-preset"
          value={p.id}
          checked={preset === p.id}
          onchange={() => { preset = p.id; }}
          class="mt-0.5 accent-teal-600"
        />
        <div class="flex flex-col">
          <span class="text-sm font-medium text-stone-700">{p.label}</span>
          <span class="text-xs text-stone-400">{p.description}</span>
        </div>
      </label>
    {/each}
  </fieldset>

  <div class="flex-1"></div>

  {#if running}
    <ProgressBar value={progress} label={progressLabel} />
  {/if}

  {#if error}
    <p class="text-sm text-red-600 bg-red-50 rounded-lg px-3 py-2 shrink-0">{error}</p>
  {/if}

  {#if outputPath}
    <p class="text-sm text-green-700 bg-green-50 rounded-lg px-3 py-2 break-all shrink-0">
      Saved: {outputPath}
    </p>
  {/if}

  <button
    onclick={runCompress}
    disabled={!canRun}
    class="shrink-0 py-2 px-4 rounded-lg font-medium text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    style="background: #1B7A8A;"
  >
    {running ? 'Compressing…' : 'Compress PDF'}
  </button>
</div>
