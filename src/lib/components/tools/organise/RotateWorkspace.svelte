<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { api } from '../../../api';
  import { operationStore } from '../../../stores/operation.svelte';
  import ProgressBar from '../../ui/ProgressBar.svelte';
  import ThumbnailGrid from './ThumbnailGrid.svelte';
  import type { Tool } from '../../../types';

  const ROTATION_OPTIONS = [
    { degrees: 90, label: '90 clockwise' },
    { degrees: 180, label: '180' },
    { degrees: 270, label: '90 counter-clockwise' },
  ];

  let filePath = $state<string | null>(null);
  let totalPages = $state(0);
  let selectedPages = $state<Set<number>>(new Set());
  let degrees = $state(90);
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
  const canRun = $derived(filePath !== null && !busy && selectedPages.size > 0);

  function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function stemFromPath(path: string): string {
    const fileName = fileNameFromPath(path);
    return fileName.replace(/\.pdf$/i, '') || 'rotated';
  }

  function dirFromPath(path: string): string {
    const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return index >= 0 ? path.slice(0, index) : '';
  }

  async function pickFile() {
    if (busy) return;
    const selected = await api.openFileDialog(false);
    if (!selected || selected.length === 0) return;

    try {
      const nextFile = selected[0];
      const count = await invoke<number>('get_page_count', { path: nextFile });
      filePath = nextFile;
      totalPages = count;
      selectedPages = new Set();
      error = null;
      outputPath = null;
      outputHint = null;
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
    }
  }

  function togglePage(page: number) {
    const next = new Set(selectedPages);
    if (next.has(page)) {
      next.delete(page);
    } else {
      next.add(page);
    }
    selectedPages = next;
  }

  function selectAll() {
    selectedPages = new Set(Array.from({ length: totalPages }, (_, index) => index + 1));
  }

  function clearSelection() {
    selectedPages = new Set();
  }

  async function runRotate() {
    if (!canRun || !filePath) return;
    error = null;
    outputPath = null;
    outputHint = null;
    submitting = true;
    let opId: string | null = null;

    try {
      const outFile = await api.saveFileDialog(`${stemFromPath(filePath)}_rotated.pdf`);
      if (!outFile) return;

      if (dirFromPath(outFile) !== dirFromPath(filePath)) {
        error = 'For now, choose a filename in the same folder as the source PDF. Rotate still writes beside the source file.';
        return;
      }

      opId = crypto.randomUUID();
      currentOpId = opId;
      operationStore.start(opId, 'rotate' as Tool);

      const result = await api.processPdf({
        operation_id: opId,
        tool: 'rotate',
        input_paths: [filePath],
        output_stem: stemFromPath(outFile),
        options: {
          degrees,
          pages: Array.from(selectedPages).sort((a, b) => a - b),
        },
      });
      outputPath = result;
      operationStore.complete(opId);
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

<div class="flex h-full flex-col gap-4 overflow-hidden p-4">
  <header class="flex items-center justify-between shrink-0">
    <h2 class="text-lg font-semibold text-stone-800">Rotate Pages</h2>
    <button
      onclick={pickFile}
      class="rounded border border-stone-300 px-3 py-1.5 text-sm transition-colors hover:bg-stone-50"
      disabled={busy}
    >
      {filePath ? 'Change File' : 'Open PDF'}
    </button>
  </header>

  {#if !filePath}
    <button
      class="flex-1 cursor-pointer rounded-xl border-2 border-dashed border-stone-300 text-stone-400 transition-colors hover:border-teal-400 hover:text-teal-600"
      onclick={pickFile}
      aria-label="Open a PDF to rotate pages"
      disabled={busy}
    >
      <div class="flex h-full flex-col items-center justify-center gap-2 p-6">
        <svg class="h-10 w-10" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4 4v6h6M20 20v-6h-6M5.636 18.364A9 9 0 1118.364 5.636" />
        </svg>
        <span class="text-sm">Click to open a PDF</span>
        <span class="text-xs text-stone-300">Choose pages to rotate with thumbnail previews</span>
      </div>
    </button>
  {:else}
    <div class="flex flex-wrap items-center gap-3 shrink-0">
      <span class="text-sm text-stone-600">{fileNameFromPath(filePath)}</span>
      <span class="text-xs text-stone-400">{selectedPages.size} of {totalPages} pages selected</span>
      <button onclick={selectAll} disabled={busy} class="text-xs text-stone-500 transition-colors hover:text-stone-700 disabled:cursor-not-allowed disabled:opacity-50">Select all</button>
      <button onclick={clearSelection} disabled={busy} class="text-xs text-stone-500 transition-colors hover:text-stone-700 disabled:cursor-not-allowed disabled:opacity-50">Clear</button>
    </div>

    <fieldset class="flex flex-col gap-2 shrink-0">
      <legend class="mb-1 text-xs text-stone-500">Rotation</legend>
      {#each ROTATION_OPTIONS as option (option.degrees)}
        <label
          class={[
            'flex items-center gap-3 rounded-lg border p-3 transition-colors',
            degrees === option.degrees ? 'border-teal-500 bg-teal-50' : 'border-stone-200 bg-white hover:border-stone-300',
          ].join(' ')}
        >
          <input
            type="radio"
            bind:group={degrees}
            value={option.degrees}
            disabled={busy}
            class="accent-teal-600"
          />
          <span class="text-sm text-stone-700">{option.label}</span>
        </label>
      {/each}
    </fieldset>

    <p class="text-xs text-stone-400 shrink-0">
      Choose the base filename in the save dialog, in the same folder as the source PDF. The rotated PDF is written next to the source file.
    </p>

    {#if outputHint}
      <p class="text-xs text-amber-700 bg-amber-50 border border-amber-200 rounded-lg px-3 py-2 shrink-0">
        {outputHint}
      </p>
    {/if}

    <div class="flex-1 overflow-y-auto">
      <ThumbnailGrid
        pdfPath={filePath}
        {totalPages}
        {selectedPages}
        disabled={busy}
        onToggle={togglePage}
      />
    </div>
  {/if}

  {#if running}
    <ProgressBar value={progress} label={progressLabel} />
  {/if}

  {#if error}
    <p class="shrink-0 rounded-lg bg-red-50 px-3 py-2 text-sm text-red-600">{error}</p>
  {/if}

  {#if outputPath}
    <p class="shrink-0 break-all rounded-lg bg-green-50 px-3 py-2 text-sm text-green-700">
      Saved: {outputPath}
    </p>
  {/if}

  <button
    onclick={runRotate}
    disabled={!canRun}
    class="shrink-0 rounded-lg px-4 py-2 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
    style="background: #1B7A8A;"
  >
    {busy ? 'Rotating...' : 'Rotate Pages'}
  </button>
</div>
