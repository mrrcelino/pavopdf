<script lang="ts">
  import { api } from '../../../api';
  import { operationStore } from '../../../stores/operation.svelte';
  import ProgressBar from '../../ui/ProgressBar.svelte';
  import type { Tool } from '../../../types';

  type SplitMode = 'range' | 'every_n';

  let filePath = $state<string | null>(null);
  let mode = $state<SplitMode>('range');
  let rangeStr = $state('');
  let everyN = $state(1);
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);
  let outputHint = $state<string | null>(null);
  let currentOpId = $state<string | null>(null);
  let submitting = $state(false);

  const currentOperation = $derived(
    currentOpId ? operationStore.get(currentOpId) : undefined
  );
  const running = $derived(currentOperation?.status === 'running');
  const progress = $derived(currentOperation?.percent ?? 0);
  const progressLabel = $derived(currentOperation?.message ?? '');
  const busy = $derived(submitting || running);

  const canRun = $derived(
    filePath !== null &&
    !busy &&
    (mode === 'every_n' ? everyN >= 1 : rangeStr.trim().length > 0)
  );

  function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function stemFromPath(path: string): string {
    const fileName = fileNameFromPath(path);
    return fileName.replace(/\.pdf$/i, '') || 'split';
  }

  function dirFromPath(path: string): string {
    const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return index >= 0 ? path.slice(0, index) : '';
  }

  async function pickFile() {
    if (busy) return;
    const selected = await api.openFileDialog(false);
    if (!selected || selected.length === 0) return;
    filePath = selected[0];
    error = null;
    outputPath = null;
    outputHint = null;
  }

  async function runSplit() {
    if (!canRun || !filePath) return;
    error = null;
    outputPath = null;
    outputHint = null;
    submitting = true;
    let opId: string | null = null;

    try {
      const outFile = await api.saveFileDialog(`${stemFromPath(filePath)}.pdf`);
      if (!outFile) return;

      if (dirFromPath(outFile) !== dirFromPath(filePath)) {
        outputHint = 'Only the chosen filename is used right now. Split files are still written next to the source PDF.';
      }

      opId = crypto.randomUUID();
      currentOpId = opId;
      operationStore.start(opId, 'split' as Tool);
      submitting = false;

      const options = mode === 'every_n'
        ? { every_n_pages: everyN }
        : { range: rangeStr.trim() };

      const result = await api.processPdf({
        operation_id: opId,
        tool: 'split',
        input_paths: [filePath],
        output_stem: stemFromPath(outFile),
        options,
      });
      outputPath = result;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
      if (opId) {
        operationStore.fail(opId, msg);
      }
    } finally {
      submitting = false;
    }
  }
</script>

<div class="flex flex-col gap-4 p-4 h-full overflow-hidden">
  <header class="shrink-0">
    <h2 class="text-lg font-semibold text-stone-800">Split PDF</h2>
  </header>

  <button
    onclick={pickFile}
    class={[
      'shrink-0 flex items-center gap-3 p-3 rounded-lg border-2 text-left transition-colors',
      busy ? 'opacity-60 cursor-not-allowed' : '',
      filePath
        ? 'border-stone-200 bg-white hover:border-teal-300'
        : 'border-dashed border-stone-300 hover:border-teal-400 hover:text-teal-600',
    ].join(' ')}
    aria-label="Pick a PDF file to split"
    disabled={busy}
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

  <div class="shrink-0 flex gap-4">
    <label class="flex items-center gap-2 cursor-pointer">
      <input
        type="radio"
        name="split-mode"
        value="range"
        checked={mode === 'range'}
        disabled={busy}
        onchange={() => { mode = 'range'; }}
        class="accent-teal-600"
      />
      <span class="text-sm text-stone-700">Page ranges</span>
    </label>
    <label class="flex items-center gap-2 cursor-pointer">
      <input
        type="radio"
        name="split-mode"
        value="every_n"
        checked={mode === 'every_n'}
        disabled={busy}
        onchange={() => { mode = 'every_n'; }}
        class="accent-teal-600"
      />
      <span class="text-sm text-stone-700">Every N pages</span>
    </label>
  </div>

  {#if mode === 'range'}
    <div class="shrink-0 flex flex-col gap-1">
      <label for="range-input" class="text-xs text-stone-500">Page ranges</label>
      <input
        id="range-input"
        type="text"
        bind:value={rangeStr}
        placeholder="1-3,5,7-9"
        disabled={busy}
        class="w-full px-3 py-2 rounded-lg border border-stone-300 text-sm text-stone-800 placeholder-stone-300 focus:outline-none focus:border-teal-400 disabled:opacity-50"
      />
      <p class="text-xs text-stone-400">Comma-separated ranges, e.g. 1-3,5,7-9</p>
    </div>
  {:else}
    <div class="shrink-0 flex flex-col gap-1">
      <label for="every-n-input" class="text-xs text-stone-500">Pages per chunk</label>
      <input
        id="every-n-input"
        type="number"
        bind:value={everyN}
        min="1"
        disabled={busy}
        class="w-full px-3 py-2 rounded-lg border border-stone-300 text-sm text-stone-800 focus:outline-none focus:border-teal-400 disabled:opacity-50"
      />
      <p class="text-xs text-stone-400">Split into files of N pages each</p>
    </div>
  {/if}

  {#if filePath}
    <p class="text-xs text-stone-400 shrink-0">
      Choose the base filename in the save dialog. Split files are always written next to the source PDF.
    </p>
  {/if}

  {#if outputHint}
    <p class="text-xs text-amber-700 bg-amber-50 border border-amber-200 rounded-lg px-3 py-2 shrink-0">
      {outputHint}
    </p>
  {/if}

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
    onclick={runSplit}
    disabled={!canRun}
    class="shrink-0 py-2 px-4 rounded-lg font-medium text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    style="background: #1B7A8A;"
  >
    {busy ? 'Splitting...' : 'Split PDF'}
  </button>
</div>
