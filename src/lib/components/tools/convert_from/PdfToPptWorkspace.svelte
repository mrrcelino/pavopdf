<script lang="ts">
  import { api } from '../../../api';
  import { operationStore } from '../../../stores/operation.svelte';
  import ProgressBar from '../../ui/ProgressBar.svelte';
  import type { Tool } from '../../../types';

  let filePath = $state<string | null>(null);
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);
  let running = $state(false);
  let progress = $state(0);
  let progressLabel = $state('');

  const canRun = $derived(filePath !== null && !running);

  function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function stemFromPath(path: string): string {
    const fileName = fileNameFromPath(path);
    return fileName.replace(/\.pdf$/i, '') || 'output';
  }

  async function pickFile() {
    if (running) return;
    const selected = await api.openFileDialog(false);
    if (!selected || selected.length === 0) return;
    filePath = selected[0];
    error = null;
    outputPath = null;
  }

  async function runConvert() {
    if (!canRun || !filePath) return;
    error = null;
    outputPath = null;

    const stem = stemFromPath(filePath);
    const outFile = await api.saveFileDialog(`${stem}.pptx`);
    if (!outFile) return;

    const opId = crypto.randomUUID();
    running = true;
    progress = 0;
    progressLabel = 'Starting...';
    operationStore.start(opId, 'pdf_to_ppt' as Tool);

    try {
      const result = await api.processPdf({
        operation_id: opId,
        tool: 'pdf_to_ppt',
        input_paths: [filePath],
        output_stem: stem,
        options: {},
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
    <h2 class="text-lg font-semibold text-stone-800">PDF to PowerPoint</h2>
  </header>

  {#if !filePath}
    <button
      class="flex-1 flex flex-col items-center justify-center border-2 border-dashed border-stone-300 rounded-xl text-stone-400 hover:border-teal-400 hover:text-teal-600 transition-colors cursor-pointer"
      onclick={pickFile}
      aria-label="Select a PDF file to convert"
    >
      <svg class="w-10 h-10 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 4v16m8-8H4" />
      </svg>
      <span class="text-sm">Click to select a PDF file</span>
    </button>
  {:else}
    <div class="flex items-center gap-2 p-3 rounded-lg border border-stone-200 bg-white">
      <span class="flex-1 truncate text-sm text-stone-700">{fileNameFromPath(filePath)}</span>
      <button
        onclick={pickFile}
        class="text-sm px-3 py-1.5 rounded border border-stone-300 hover:bg-stone-50 transition-colors"
      >
        Change
      </button>
    </div>
  {/if}

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
    {running ? 'Converting...' : 'Convert to PowerPoint'}
  </button>
</div>
