<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { api } from '../../../api';
  import { operationStore } from '../../../stores/operation.svelte';
  import ProgressBar from '../../ui/ProgressBar.svelte';
  import ThumbnailGrid from './ThumbnailGrid.svelte';
  import type { Tool } from '../../../types';

  let filePath = $state<string | null>(null);
  let totalPages = $state(0);
  let selectedPages = $state<Set<number>>(new Set());
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);
  let currentOpId = $state<string | null>(null);
  let submitting = $state(false);

  const currentOperation = $derived(
    currentOpId ? operationStore.get(currentOpId) : undefined
  );
  const running = $derived(currentOperation?.status === 'running');
  const progress = $derived(currentOperation?.percent ?? 0);
  const progressLabel = $derived(currentOperation?.message ?? '');
  const busy = $derived(submitting || running);
  const remainingCount = $derived(totalPages - selectedPages.size);
  const canRun = $derived(
    filePath !== null &&
    !busy &&
    selectedPages.size > 0 &&
    selectedPages.size < totalPages
  );

  function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function stemFromPath(path: string): string {
    const fileName = fileNameFromPath(path);
    return fileName.replace(/\.pdf$/i, '') || 'pages_removed';
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
      error = null;
      outputPath = null;
      selectedPages = new Set();
      totalPages = count;
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

  function clearSelection() {
    selectedPages = new Set();
  }

  async function runRemove() {
    if (!canRun || !filePath) return;
    error = null;
    outputPath = null;
    submitting = true;
    let opId: string | null = null;

    try {
      const outFile = await api.saveFileDialog(`${stemFromPath(filePath)}_pages_removed.pdf`);
      if (!outFile) return;

      if (dirFromPath(outFile) !== dirFromPath(filePath)) {
        error = 'For now, choose a filename in the same folder as the source PDF. Remove still writes beside the source file.';
        return;
      }

      opId = crypto.randomUUID();
      currentOpId = opId;
      operationStore.start(opId, 'remove' as Tool);

      const result = await api.processPdf({
        operation_id: opId,
        tool: 'remove',
        input_paths: [filePath],
        output_stem: stemFromPath(outFile),
        options: {
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
    <h2 class="text-lg font-semibold text-stone-800">Remove Pages</h2>
    <button
      onclick={pickFile}
      disabled={busy}
      class="rounded border border-stone-300 px-3 py-1.5 text-sm transition-colors hover:bg-stone-50"
    >
      {filePath ? 'Change File' : 'Open PDF'}
    </button>
  </header>

  {#if !filePath}
    <button
      class="flex-1 cursor-pointer rounded-xl border-2 border-dashed border-stone-300 text-stone-400 transition-colors hover:border-teal-400 hover:text-teal-600"
      onclick={pickFile}
      aria-label="Open a PDF to remove pages"
      disabled={busy}
    >
      <div class="flex h-full flex-col items-center justify-center gap-2 p-6">
        <svg class="h-10 w-10" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
        </svg>
        <span class="text-sm">Click to open a PDF</span>
        <span class="text-xs text-stone-300">Choose pages to delete with thumbnail previews</span>
      </div>
    </button>
  {:else}
    <div class="flex items-center gap-4 shrink-0">
      <span class="text-sm text-stone-600">{fileNameFromPath(filePath)}</span>
      <span class="text-xs text-stone-400">
        {selectedPages.size} selected, {remainingCount} remaining
      </span>
      <button onclick={clearSelection} disabled={busy} class="text-xs text-stone-500 transition-colors hover:text-stone-700 disabled:cursor-not-allowed disabled:opacity-50">
        Clear selection
      </button>
    </div>

    {#if selectedPages.size >= totalPages && totalPages > 0}
      <div class="shrink-0 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs text-red-700">
        Cannot remove all pages. Leave at least one page in the document.
      </div>
    {/if}

    <p class="shrink-0 text-xs text-stone-400">
      Choose the base filename in the save dialog, in the same folder as the source PDF. The updated PDF is written next to the source file.
    </p>

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
    onclick={runRemove}
    disabled={!canRun}
    class="shrink-0 rounded-lg px-4 py-2 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
    style="background: #1B7A8A;"
  >
    {busy ? 'Removing...' : 'Remove Selected Pages'}
  </button>
</div>
