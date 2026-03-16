<script lang="ts">
  import { api } from '../../../api';
  import { operationStore } from '../../../stores/operation.svelte';
  import ProgressBar from '../../ui/ProgressBar.svelte';
  import type { Tool } from '../../../types';

  let files = $state<string[]>([]);
  let draggingIndex = $state<number | null>(null);
  let dragOverIndex = $state<number | null>(null);
  let outputPath = $state<string | null>(null);
  let error = $state<string | null>(null);
  let running = $state(false);
  let progress = $state(0);
  let progressLabel = $state('');

  const canMerge = $derived(files.length >= 2 && !running);

  async function addFiles() {
    const selected = await api.openFileDialog(true);
    if (!selected || selected.length === 0) return;
    // Immutable: create new array, deduplicate
    files = [...files, ...selected.filter(p => !files.includes(p))];
    error = null;
  }

  function removeFile(index: number) {
    files = files.filter((_, i) => i !== index);
  }

  function onDragStart(index: number) { draggingIndex = index; }
  function onDragOver(e: DragEvent, index: number) { e.preventDefault(); dragOverIndex = index; }
  function onDragEnd() { draggingIndex = null; dragOverIndex = null; }

  function onDrop(targetIndex: number) {
    if (draggingIndex === null || draggingIndex === targetIndex) {
      draggingIndex = null; dragOverIndex = null; return;
    }
    const reordered = [...files];
    const [moved] = reordered.splice(draggingIndex, 1);
    reordered.splice(targetIndex, 0, moved);
    files = reordered;
    draggingIndex = null; dragOverIndex = null;
  }

  async function runMerge() {
    if (!canMerge) return;
    error = null;
    outputPath = null;

    const outFile = await api.saveFileDialog('merged.pdf');
    if (!outFile) return;

    const opId = crypto.randomUUID();
    running = true;
    progress = 0;
    progressLabel = 'Starting…';
    operationStore.start(opId, 'merge' as Tool);

    try {
      const result = await api.processPdf({
        operation_id: opId,
        tool: 'merge',
        input_paths: files,
        output_stem: 'merged',
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
    <h2 class="text-lg font-semibold text-stone-800">Merge PDF</h2>
    <button
      onclick={addFiles}
      class="text-sm px-3 py-1.5 rounded border border-stone-300 hover:bg-stone-50 transition-colors"
    >
      + Add Files
    </button>
  </header>

  {#if files.length === 0}
    <button
      class="flex-1 flex flex-col items-center justify-center border-2 border-dashed border-stone-300 rounded-xl text-stone-400 hover:border-teal-400 hover:text-teal-600 transition-colors cursor-pointer"
      onclick={addFiles}
      aria-label="Add PDF files to merge"
    >
      <svg class="w-10 h-10 mb-2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M12 4v16m8-8H4" />
      </svg>
      <span class="text-sm">Click to add PDF files</span>
      <span class="text-xs mt-1 text-stone-300">Drag rows to reorder after adding</span>
    </button>
  {:else}
    <ul class="flex-1 overflow-y-auto space-y-1 min-h-0">
      {#each files as file, i (file)}
        <li
          draggable="true"
          ondragstart={() => onDragStart(i)}
          ondragover={(e) => onDragOver(e, i)}
          ondrop={() => onDrop(i)}
          ondragend={onDragEnd}
          class={[
            'flex items-center gap-2 p-2 rounded-lg border bg-white cursor-grab active:cursor-grabbing transition-colors text-sm',
            dragOverIndex === i ? 'border-teal-400 bg-teal-50' : 'border-stone-200',
          ].join(' ')}
          aria-label={`File ${i + 1}: ${file.split(/[\\/]/).pop()}`}
        >
          <span class="text-stone-300 select-none text-base">⠿</span>
          <span class="text-stone-400 text-xs w-5 text-right shrink-0">{i + 1}</span>
          <span class="flex-1 truncate text-stone-700">{file.split(/[\\/]/).pop()}</span>
          <button
            onclick={() => removeFile(i)}
            class="text-stone-300 hover:text-red-500 shrink-0 transition-colors"
            aria-label="Remove file"
          >✕</button>
        </li>
      {/each}
    </ul>
    <p class="text-xs text-stone-400 shrink-0">
      {files.length} file{files.length !== 1 ? 's' : ''} — drag rows to reorder
    </p>
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
    onclick={runMerge}
    disabled={!canMerge}
    class="shrink-0 py-2 px-4 rounded-lg font-medium text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    style="background: #1B7A8A;"
  >
    {running ? 'Merging…' : 'Merge PDFs'}
  </button>
</div>
