<script lang="ts">
  import { api } from '../../../api';
  import { operationStore } from '../../../stores/operation.svelte';
  import ProgressBar from '../../ui/ProgressBar.svelte';
  import type { Tool } from '../../../types';

  interface Region {
    page: number;
    x: number;
    y: number;
    width: number;
    height: number;
  }

  let filePath = $state<string | null>(null);
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);
  let running = $state(false);
  let progress = $state(0);
  let progressLabel = $state('');

  let regions = $state<Region[]>([{ page: 1, x: 0, y: 0, width: 100, height: 20 }]);

  const canRun = $derived(filePath !== null && regions.length > 0 && !running);

  function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function addRegion() {
    regions = [...regions, { page: 1, x: 0, y: 0, width: 100, height: 20 }];
  }

  function removeRegion(index: number) {
    regions = regions.filter((_, i) => i !== index);
  }

  async function pickFile() {
    if (running) return;
    const selected = await api.openFileDialog(false);
    if (!selected || selected.length === 0) return;
    filePath = selected[0];
    error = null;
    outputPath = null;
  }

  async function runRedact() {
    if (!canRun || !filePath) return;
    error = null;
    outputPath = null;

    const opId = crypto.randomUUID();
    running = true;
    progress = 0;
    progressLabel = 'Starting...';
    operationStore.start(opId, 'redact' as Tool);

    try {
      const result = await api.processPdf({
        operation_id: opId,
        tool: 'redact',
        input_paths: [filePath],
        output_stem: '',
        options: {
          regions: regions.map(r => ({
            page: r.page,
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
          })),
        },
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
    <h2 class="text-lg font-semibold text-stone-800">Redact PDF</h2>
  </header>

  <div class="shrink-0 text-sm text-amber-700 bg-amber-50 border border-amber-200 rounded-lg px-3 py-2">
    Visual-only redaction. Draws black rectangles over specified areas. Underlying text bytes are NOT removed.
  </div>

  {#if !filePath}
    <button
      class="flex-1 flex flex-col items-center justify-center border-2 border-dashed border-stone-300 rounded-xl text-stone-400 hover:border-teal-400 hover:text-teal-600 transition-colors cursor-pointer"
      onclick={pickFile}
      aria-label="Select a PDF file"
    >
      <svg class="w-10 h-10 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 4v16m8-8H4" />
      </svg>
      <span class="text-sm">Click to select a PDF file</span>
    </button>
  {:else}
    <div class="flex items-center gap-2 p-3 rounded-lg border border-stone-200 bg-white shrink-0">
      <span class="flex-1 truncate text-sm text-stone-700">{fileNameFromPath(filePath)}</span>
      <button
        onclick={pickFile}
        class="text-sm px-3 py-1.5 rounded border border-stone-300 hover:bg-stone-50 transition-colors"
      >
        Change
      </button>
    </div>

    <div class="flex flex-col gap-3 overflow-y-auto">
      {#each regions as region, i}
        <div class="p-3 rounded-lg border border-stone-200 bg-stone-50">
          <div class="flex items-center justify-between mb-2">
            <span class="text-sm font-medium text-stone-600">Region {i + 1}</span>
            <button
              onclick={() => removeRegion(i)}
              class="text-xs px-2 py-1 rounded border border-red-200 text-red-600 hover:bg-red-50 transition-colors"
              disabled={regions.length <= 1}
            >
              Remove
            </button>
          </div>
          <div class="grid grid-cols-5 gap-2">
            <label class="flex flex-col gap-0.5">
              <span class="text-xs text-stone-500">Page</span>
              <input
                type="number"
                bind:value={region.page}
                min="1"
                class="px-2 py-1.5 rounded border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
              />
            </label>
            <label class="flex flex-col gap-0.5">
              <span class="text-xs text-stone-500">X</span>
              <input
                type="number"
                bind:value={region.x}
                min="0"
                class="px-2 py-1.5 rounded border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
              />
            </label>
            <label class="flex flex-col gap-0.5">
              <span class="text-xs text-stone-500">Y</span>
              <input
                type="number"
                bind:value={region.y}
                min="0"
                class="px-2 py-1.5 rounded border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
              />
            </label>
            <label class="flex flex-col gap-0.5">
              <span class="text-xs text-stone-500">Width</span>
              <input
                type="number"
                bind:value={region.width}
                min="1"
                class="px-2 py-1.5 rounded border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
              />
            </label>
            <label class="flex flex-col gap-0.5">
              <span class="text-xs text-stone-500">Height</span>
              <input
                type="number"
                bind:value={region.height}
                min="1"
                class="px-2 py-1.5 rounded border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
              />
            </label>
          </div>
        </div>
      {/each}

      <button
        onclick={addRegion}
        class="text-sm px-3 py-2 rounded-lg border border-dashed border-stone-300 text-stone-500 hover:border-teal-400 hover:text-teal-600 transition-colors"
      >
        + Add Region
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
    onclick={runRedact}
    disabled={!canRun}
    class="shrink-0 py-2 px-4 rounded-lg font-medium text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    style="background: #1B7A8A;"
  >
    {running ? 'Redacting...' : 'Redact PDF'}
  </button>
</div>
