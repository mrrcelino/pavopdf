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

  let page = $state(1);
  let x = $state(100);
  let y = $state(100);
  let width = $state(200);
  let height = $state(80);

  let canvasEl = $state<HTMLCanvasElement | null>(null);
  let isDrawing = $state(false);
  let hasDrawn = $state(false);

  const canRun = $derived(filePath !== null && hasDrawn && !running);

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

  function getCanvasCoords(e: MouseEvent | TouchEvent): { cx: number; cy: number } {
    if (!canvasEl) return { cx: 0, cy: 0 };
    const rect = canvasEl.getBoundingClientRect();
    if ('touches' in e) {
      const touch = e.touches[0];
      return { cx: touch.clientX - rect.left, cy: touch.clientY - rect.top };
    }
    return { cx: e.clientX - rect.left, cy: e.clientY - rect.top };
  }

  function startDrawing(e: MouseEvent | TouchEvent) {
    if (!canvasEl) return;
    isDrawing = true;
    const { cx, cy } = getCanvasCoords(e);
    const ctx = canvasEl.getContext('2d');
    if (!ctx) return;
    ctx.beginPath();
    ctx.moveTo(cx, cy);
  }

  function draw(e: MouseEvent | TouchEvent) {
    if (!isDrawing || !canvasEl) return;
    e.preventDefault();
    const { cx, cy } = getCanvasCoords(e);
    const ctx = canvasEl.getContext('2d');
    if (!ctx) return;
    ctx.lineWidth = 2;
    ctx.lineCap = 'round';
    ctx.strokeStyle = '#1a1a1a';
    ctx.lineTo(cx, cy);
    ctx.stroke();
    hasDrawn = true;
  }

  function stopDrawing() {
    isDrawing = false;
  }

  function clearCanvas() {
    if (!canvasEl) return;
    const ctx = canvasEl.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, canvasEl.width, canvasEl.height);
    hasDrawn = false;
  }

  async function runSign() {
    if (!canRun || !filePath || !canvasEl) return;
    error = null;
    outputPath = null;

    const dataUrl = canvasEl.toDataURL('image/png');
    const signatureBase64 = dataUrl.replace('data:image/png;base64,', '');

    const opId = crypto.randomUUID();
    running = true;
    progress = 0;
    progressLabel = 'Starting...';
    operationStore.start(opId, 'sign' as Tool);

    try {
      const result = await api.processPdf({
        operation_id: opId,
        tool: 'sign',
        input_paths: [filePath],
        output_stem: '',
        options: {
          signature_png_base64: signatureBase64,
          page,
          x,
          y,
          width,
          height,
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
    <h2 class="text-lg font-semibold text-stone-800">Sign PDF</h2>
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
      <div class="flex flex-col gap-1">
        <div class="flex items-center justify-between">
          <span class="text-sm font-medium text-stone-600">Draw Signature</span>
          <button
            onclick={clearCanvas}
            class="text-xs px-2 py-1 rounded border border-stone-300 hover:bg-stone-50 transition-colors text-stone-500"
          >
            Clear
          </button>
        </div>
        <canvas
          bind:this={canvasEl}
          width="400"
          height="150"
          class="w-full rounded-lg border border-stone-200 bg-white cursor-crosshair touch-none"
          onmousedown={startDrawing}
          onmousemove={draw}
          onmouseup={stopDrawing}
          onmouseleave={stopDrawing}
          ontouchstart={startDrawing}
          ontouchmove={draw}
          ontouchend={stopDrawing}
        ></canvas>
        {#if !hasDrawn}
          <span class="text-xs text-stone-400">Draw your signature above</span>
        {/if}
      </div>

      <div class="grid grid-cols-2 gap-3">
        <label class="flex flex-col gap-1">
          <span class="text-sm font-medium text-stone-600">Page</span>
          <input
            type="number"
            bind:value={page}
            min="1"
            class="px-3 py-2 rounded-lg border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-sm font-medium text-stone-600">X</span>
          <input
            type="number"
            bind:value={x}
            min="0"
            class="px-3 py-2 rounded-lg border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-sm font-medium text-stone-600">Y</span>
          <input
            type="number"
            bind:value={y}
            min="0"
            class="px-3 py-2 rounded-lg border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-sm font-medium text-stone-600">Width</span>
          <input
            type="number"
            bind:value={width}
            min="1"
            class="px-3 py-2 rounded-lg border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
          />
        </label>
        <label class="flex flex-col gap-1">
          <span class="text-sm font-medium text-stone-600">Height</span>
          <input
            type="number"
            bind:value={height}
            min="1"
            class="px-3 py-2 rounded-lg border border-stone-200 text-sm text-stone-700 focus:outline-none focus:border-teal-400"
          />
        </label>
      </div>
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
    onclick={runSign}
    disabled={!canRun}
    class="shrink-0 py-2 px-4 rounded-lg font-medium text-white transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
    style="background: #1B7A8A;"
  >
    {running ? 'Signing...' : 'Sign PDF'}
  </button>
</div>
