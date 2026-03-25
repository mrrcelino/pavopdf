<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { api } from '../../../api';
  import { operationStore } from '../../../stores/operation.svelte';
  import ProgressBar from '../../ui/ProgressBar.svelte';
  import type { Tool } from '../../../types';

  interface PageEntry {
    originalPage: number;
    thumbnailUrl: string | null;
  }

  const CONCURRENT_RENDERS = 4;

  let filePath = $state<string | null>(null);
  let pages = $state<PageEntry[]>([]);
  let draggingIndex = $state<number | null>(null);
  let dragOverIndex = $state<number | null>(null);
  let loadingThumbnails = $state(false);
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
  const busy = $derived(submitting || running || loadingThumbnails);
  const canRun = $derived(filePath !== null && pages.length > 0 && !busy);

  function fileNameFromPath(path: string): string {
    return path.split(/[\\/]/).pop() ?? path;
  }

  function stemFromPath(path: string): string {
    const fileName = fileNameFromPath(path);
    return fileName.replace(/\.pdf$/i, '') || 'reordered';
  }

  function dirFromPath(path: string): string {
    const index = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
    return index >= 0 ? path.slice(0, index) : '';
  }

  async function loadPages(path: string) {
    try {
      const totalPages = await invoke<number>('get_page_count', { path });
      pages = Array.from({ length: totalPages }, (_, index) => ({
        originalPage: index + 1,
        thumbnailUrl: null,
      }));

      loadingThumbnails = true;
      for (let index = 0; index < totalPages; index += CONCURRENT_RENDERS) {
        const chunk = pages.slice(index, index + CONCURRENT_RENDERS);
        const results = await Promise.allSettled(
          chunk.map((entry) =>
            invoke<{ page: number; data_url: string }>('render_page_thumbnail', {
              path,
              page: entry.originalPage,
              width: 96,
              height: 96,
            })
          )
        );

        const thumbnailMap = new Map<number, string>(
          results.flatMap((result) =>
            result.status === 'fulfilled'
              ? [[result.value.page, result.value.data_url] as [number, string]]
              : []
          )
        );
        const failed = results.some((result) => result.status === 'rejected');

        pages = pages.map((entry) => ({
          ...entry,
          thumbnailUrl: thumbnailMap.get(entry.originalPage) ?? entry.thumbnailUrl,
        }));

        if (failed) {
          error = 'Some thumbnails could not be rendered.';
        }
      }
    } catch (e: unknown) {
      const msg = e instanceof Error ? e.message : String(e);
      error = msg;
      pages = [];
    } finally {
      loadingThumbnails = false;
    }
  }

  async function pickFile() {
    if (busy) return;
    const selected = await api.openFileDialog(false);
    if (!selected || selected.length === 0) return;

    filePath = selected[0];
    error = null;
    outputPath = null;
    await loadPages(selected[0]);
  }

  function onDragStart(index: number) {
    draggingIndex = index;
  }

  function onDragOver(event: DragEvent, index: number) {
    event.preventDefault();
    dragOverIndex = index;
  }

  function onDrop(targetIndex: number) {
    if (busy) return;
    if (draggingIndex === null || draggingIndex === targetIndex) {
      draggingIndex = null;
      dragOverIndex = null;
      return;
    }

    const reordered = [...pages];
    const [moved] = reordered.splice(draggingIndex, 1);
    reordered.splice(targetIndex, 0, moved);
    pages = reordered;
    draggingIndex = null;
    dragOverIndex = null;
  }

  function onDragEnd() {
    draggingIndex = null;
    dragOverIndex = null;
  }

  async function runReorder() {
    if (!canRun || !filePath) return;
    error = null;
    outputPath = null;
    submitting = true;
    let opId: string | null = null;

    try {
      const outFile = await api.saveFileDialog(`${stemFromPath(filePath)}_reordered.pdf`);
      if (!outFile) return;

      if (dirFromPath(outFile) !== dirFromPath(filePath)) {
        error = 'For now, choose a filename in the same folder as the source PDF. Reorder still writes beside the source file.';
        return;
      }

      opId = crypto.randomUUID();
      currentOpId = opId;
      operationStore.start(opId, 'reorder' as Tool);

      const result = await api.processPdf({
        operation_id: opId,
        tool: 'reorder',
        input_paths: [filePath],
        output_stem: stemFromPath(outFile),
        options: {
          pages: pages.map((entry) => entry.originalPage),
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
    <h2 class="text-lg font-semibold text-stone-800">Reorder Pages</h2>
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
      aria-label="Open a PDF to reorder pages"
      disabled={busy}
    >
      <div class="flex h-full flex-col items-center justify-center gap-2 p-6">
        <svg class="h-10 w-10" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M4 7h16M7 4h10v16H7zM9 11h6M9 15h6" />
        </svg>
        <span class="text-sm">Click to open a PDF</span>
        <span class="text-xs text-stone-300">Drag thumbnails into a new page order</span>
      </div>
    </button>
  {:else}
    <div class="shrink-0 text-sm text-stone-600">
      {fileNameFromPath(filePath)}
    </div>
    <p class="shrink-0 text-xs text-stone-400">
      Drag cards to change page order. Choose the base filename in the save dialog, in the same folder as the source PDF. The reordered PDF is written next to the source file.
    </p>

    <div class="flex-1 overflow-y-auto">
      <div class="grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-3">
        {#each pages as entry, index (entry.originalPage)}
          <button
            draggable="true"
            disabled={busy}
            ondragstart={() => onDragStart(index)}
            ondragover={(event) => onDragOver(event, index)}
            ondrop={() => onDrop(index)}
            ondragend={onDragEnd}
            class={[
              'relative flex flex-col items-center gap-2 rounded-lg border-2 bg-white p-3 text-left transition-colors',
              busy ? 'cursor-not-allowed opacity-60' : '',
              dragOverIndex === index ? 'border-teal-500 bg-teal-50' : 'border-stone-200 hover:border-stone-300',
            ].join(' ')}
            aria-label={`Page ${entry.originalPage}, position ${index + 1}`}
          >
            <span class="absolute left-2 top-2 text-xs text-stone-400">#{index + 1}</span>
            {#if entry.thumbnailUrl}
              <img src={entry.thumbnailUrl} alt={`Page ${entry.originalPage}`} class="h-24 w-24 rounded object-contain" />
            {:else}
              <div class="flex h-24 w-24 items-center justify-center rounded bg-stone-100">
                <span class="text-xs text-stone-400">{entry.originalPage}</span>
              </div>
            {/if}
            <span class="text-sm text-stone-700">Page {entry.originalPage}</span>
          </button>
        {/each}
      </div>
    </div>
  {/if}

  {#if loadingThumbnails}
    <p class="shrink-0 text-xs text-stone-400">Loading thumbnails...</p>
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
    onclick={runReorder}
    disabled={!canRun}
    class="shrink-0 rounded-lg px-4 py-2 font-medium text-white transition-colors disabled:cursor-not-allowed disabled:opacity-50"
    style="background: #1B7A8A;"
  >
    {busy ? 'Reordering...' : 'Save Reordered PDF'}
  </button>
</div>
