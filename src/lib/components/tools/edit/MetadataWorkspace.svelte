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

  let title = $state('');
  let author = $state('');
  let subject = $state('');
  let keywords = $state('');
  let creator = $state('');

  let clearFields = $state<Set<string>>(new Set());

  function markClear(field: string) {
    clearFields = new Set([...clearFields, field]);
  }

  function unmarkClear(field: string) {
    const next = new Set(clearFields);
    next.delete(field);
    clearFields = next;
  }

  const hasAnyField = $derived(
    title.trim() !== '' ||
    author.trim() !== '' ||
    subject.trim() !== '' ||
    keywords.trim() !== '' ||
    creator.trim() !== '' ||
    clearFields.size > 0
  );

  const canRun = $derived(filePath !== null && !running && hasAnyField);

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

  async function runEdit() {
    if (!canRun || !filePath) return;
    error = null;
    outputPath = null;

    const options: Record<string, string> = {};
    // Non-empty field = set value; clearFields entry = send "" to remove metadata
    if (title.trim() !== '') options.title = title.trim();
    else if (clearFields.has('title')) options.title = '';
    if (author.trim() !== '') options.author = author.trim();
    else if (clearFields.has('author')) options.author = '';
    if (subject.trim() !== '') options.subject = subject.trim();
    else if (clearFields.has('subject')) options.subject = '';
    if (keywords.trim() !== '') options.keywords = keywords.trim();
    else if (clearFields.has('keywords')) options.keywords = '';
    if (creator.trim() !== '') options.creator = creator.trim();
    else if (clearFields.has('creator')) options.creator = '';

    const opId = crypto.randomUUID();
    running = true;
    progress = 0;
    progressLabel = 'Starting...';
    operationStore.start(opId, 'edit' as Tool);

    try {
      const result = await api.processPdf({
        operation_id: opId,
        tool: 'edit',
        input_paths: [filePath],
        output_stem: '',
        options,
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
    <h2 class="text-lg font-semibold text-stone-800">Edit PDF Metadata</h2>
  </header>

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
    <div class="flex items-center gap-2 p-3 rounded-lg border border-stone-200 bg-white">
      <span class="flex-1 truncate text-sm text-stone-700">{fileNameFromPath(filePath)}</span>
      <button
        onclick={pickFile}
        class="text-sm px-3 py-1.5 rounded border border-stone-300 hover:bg-stone-50 transition-colors"
      >
        Change
      </button>
    </div>

    <div class="flex flex-col gap-3 overflow-y-auto">
      {#snippet metaField(label: string, field: string, value: string, setValue: (v: string) => void, placeholder: string)}
        <div class="flex flex-col gap-1">
          <span class="text-sm font-medium text-stone-600">{label}</span>
          <div class="flex gap-1.5">
            <input
              type="text"
              {value}
              oninput={(e: Event) => { setValue((e.target as HTMLInputElement).value); unmarkClear(field); }}
              {placeholder}
              disabled={clearFields.has(field)}
              class="flex-1 px-3 py-2 rounded-lg border text-sm text-stone-700 focus:outline-none focus:border-teal-400 {clearFields.has(field) ? 'border-red-300 bg-red-50 line-through' : 'border-stone-200'}"
            />
            {#if clearFields.has(field)}
              <button
                type="button"
                onclick={() => unmarkClear(field)}
                class="px-2 py-1 rounded border border-stone-300 text-xs text-stone-600 hover:bg-stone-50 transition-colors"
                title="Undo clear"
              >Undo</button>
            {:else}
              <button
                type="button"
                onclick={() => { setValue(''); markClear(field); }}
                class="px-2 py-1 rounded border border-red-200 text-xs text-red-600 hover:bg-red-50 transition-colors"
                title="Clear existing metadata for {label}"
              >Clear</button>
            {/if}
          </div>
          {#if clearFields.has(field)}
            <span class="text-xs text-red-500">Will be removed from PDF</span>
          {/if}
        </div>
      {/snippet}

      {@render metaField('Title', 'title', title, (v) => title = v, 'Document title')}
      {@render metaField('Author', 'author', author, (v) => author = v, 'Author name')}
      {@render metaField('Subject', 'subject', subject, (v) => subject = v, 'Document subject')}
      {@render metaField('Keywords', 'keywords', keywords, (v) => keywords = v, 'Comma-separated keywords')}
      {@render metaField('Creator', 'creator', creator, (v) => creator = v, 'Creator application')}
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
    onclick={runEdit}
    disabled={!canRun}
    class="shrink-0 py-2 px-4 rounded-lg font-medium text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    style="background: #1B7A8A;"
  >
    {running ? 'Updating...' : 'Update Metadata'}
  </button>
</div>
