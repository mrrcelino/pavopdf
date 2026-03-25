<script lang="ts">
  import { api } from '../../../api';
  import { operationStore } from '../../../stores/operation.svelte';
  import ProgressBar from '../../ui/ProgressBar.svelte';
  import type { Tool } from '../../../types';

  let filePaths = $state<string[]>([]);
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);
  let running = $state(false);
  let progress = $state(0);
  let progressLabel = $state('');
  let pageSize = $state<'fit' | 'a4'>('fit');

  const canRun = $derived(filePaths.length > 0 && !running);

  function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function stemFromPath(path: string): string {
    const fileName = fileNameFromPath(path);
    return fileName.replace(/\.pdf$/i, '') || 'output';
  }

  function inputStemFromPath(path: string): string {
    const fileName = fileNameFromPath(path);
    return fileName.replace(/\.\w+$/i, '') || 'output';
  }

  function dirFromPath(path: string): string {
    const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return index >= 0 ? path.slice(0, index) : '';
  }

  async function pickFiles() {
    if (running) return;
    const selected = await api.openFileDialog(true);
    if (!selected || selected.length === 0) return;
    filePaths = selected;
    error = null;
    outputPath = null;
  }

  async function runConvert() {
    if (!canRun || filePaths.length === 0) return;
    error = null;
    outputPath = null;

    const defaultStem = inputStemFromPath(filePaths[0]);
    const outFile = await api.saveFileDialog(`${defaultStem}.pdf`);
    if (!outFile) return;

    if (dirFromPath(outFile) !== dirFromPath(filePaths[0])) {
      error = 'Please choose a location in the same folder as the source files.';
      return;
    }

    const stem = stemFromPath(outFile);
    const opId = crypto.randomUUID();
    running = true;
    progress = 0;
    progressLabel = 'Starting...';
    operationStore.start(opId, 'image_to_pdf' as Tool);

    try {
      const result = await api.processPdf({
        operation_id: opId,
        tool: 'image_to_pdf',
        input_paths: filePaths,
        output_stem: stem,
        options: { page_size: pageSize },
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
  <header class="flex items-center justify-between shrink-0">
    <h2 class="text-lg font-semibold text-stone-800">Image to PDF</h2>
  </header>

  {#if filePaths.length === 0}
    <button
      class="flex-1 flex flex-col items-center justify-center border-2 border-dashed border-stone-300 rounded-xl text-stone-400 hover:border-teal-400 hover:text-teal-600 transition-colors cursor-pointer"
      onclick={pickFiles}
      aria-label="Select image files to convert"
    >
      <svg class="w-10 h-10 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 4v16m8-8H4" />
      </svg>
      <span class="text-sm">Click to select image files</span>
    </button>
  {:else}
    <div class="flex flex-col gap-1 p-3 rounded-lg border border-stone-200 bg-white overflow-y-auto max-h-48">
      <div class="flex items-center justify-between mb-1">
        <span class="text-xs font-medium text-stone-500">{filePaths.length} file{filePaths.length > 1 ? 's' : ''} selected</span>
        <button
          onclick={pickFiles}
          class="text-sm px-3 py-1.5 rounded border border-stone-300 hover:bg-stone-50 transition-colors"
        >
          Change
        </button>
      </div>
      {#each filePaths as fp}
        <span class="truncate text-sm text-stone-700">{fileNameFromPath(fp)}</span>
      {/each}
    </div>
  {/if}

  <div class="flex items-center gap-3 shrink-0">
    <label for="page-size" class="text-sm font-medium text-stone-700">Page size</label>
    <select
      id="page-size"
      bind:value={pageSize}
      class="text-sm border border-stone-300 rounded-lg px-3 py-1.5 bg-white text-stone-700"
    >
      <option value="fit">Fit to image</option>
      <option value="a4">A4</option>
    </select>
  </div>

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
    onclick={runConvert}
    disabled={!canRun}
    class="shrink-0 py-2 px-4 rounded-lg font-medium text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    style="background: #1B7A8A;"
  >
    {running ? 'Converting...' : 'Convert to PDF'}
  </button>
</div>
