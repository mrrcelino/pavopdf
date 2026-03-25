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

  let userPassword = $state('');
  let ownerPassword = $state('');

  const canRun = $derived(filePath !== null && userPassword.trim() !== '' && !running);

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

  async function runProtect() {
    if (!canRun || !filePath) return;
    error = null;
    outputPath = null;

    const opId = crypto.randomUUID();
    running = true;
    progress = 0;
    progressLabel = 'Starting...';
    operationStore.start(opId, 'protect' as Tool);

    try {
      const options: Record<string, string> = {
        user_password: userPassword.trim(),
      };
      if (ownerPassword.trim() !== '') {
        options.owner_password = ownerPassword.trim();
      }

      const result = await api.processPdf({
        operation_id: opId,
        tool: 'protect',
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
    <h2 class="text-lg font-semibold text-stone-800">Protect PDF</h2>
  </header>

  <div class="shrink-0 rounded-lg bg-blue-50 border border-blue-200 px-3 py-2 text-sm text-blue-700">
    Creates a protected copy. Note: Full PDF encryption will be available in a future version.
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
      <label class="flex flex-col gap-1">
        <span class="text-sm font-medium text-stone-600">User Password (required)</span>
        <input
          type="password"
          bind:value={userPassword}
          placeholder="Enter user password"
          class="px-3 py-2 rounded-lg border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
        />
      </label>
      <label class="flex flex-col gap-1">
        <span class="text-sm font-medium text-stone-600">Owner Password (optional)</span>
        <input
          type="password"
          bind:value={ownerPassword}
          placeholder="Enter owner password"
          class="px-3 py-2 rounded-lg border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
        />
      </label>
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
    onclick={runProtect}
    disabled={!canRun}
    class="shrink-0 py-2 px-4 rounded-lg font-medium text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    style="background: #1B7A8A;"
  >
    {running ? 'Protecting...' : 'Protect PDF'}
  </button>
</div>
