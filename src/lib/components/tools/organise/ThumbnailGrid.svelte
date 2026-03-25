<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  interface Props {
    pdfPath: string;
    totalPages: number;
    selectedPages: Set<number>;
    disabled?: boolean;
    onToggle: (page: number) => void;
  }

  const { pdfPath, totalPages, selectedPages, disabled = false, onToggle }: Props = $props();

  const INITIAL_BATCH = 100;
  const CONCURRENT_RENDERS = 4;

  let visibleCount = $state(0);
  let thumbnails = $state<Map<number, string>>(new Map());
  let failedPages = $state<Set<number>>(new Set());
  let inFlightPages = $state<Set<number>>(new Set());
  let loading = $state(false);
  let error = $state<string | null>(null);
  let renderToken = 0;

  const visiblePages = $derived(
    Array.from({ length: visibleCount }, (_, index) => index + 1)
  );

  async function renderBatch(start: number, end: number) {
    const token = renderToken;
    const pendingPages: number[] = [];

    for (let page = start; page <= end; page += 1) {
      if (!thumbnails.has(page) && !failedPages.has(page) && !inFlightPages.has(page)) {
        pendingPages.push(page);
      }
    }

    if (pendingPages.length === 0) {
      return;
    }

    loading = true;
    error = null;
    inFlightPages = new Set([...inFlightPages, ...pendingPages]);

    try {
      for (let index = 0; index < pendingPages.length; index += CONCURRENT_RENDERS) {
        const chunk = pendingPages.slice(index, index + CONCURRENT_RENDERS);
        const results = await Promise.allSettled(
          chunk.map((page) =>
            invoke<{ page: number; data_url: string }>('render_page_thumbnail', {
              path: pdfPath,
              page,
              width: 96,
              height: 96,
            })
          )
        );

        if (token !== renderToken) {
          return;
        }

        const successful = results
          .filter((result): result is PromiseFulfilledResult<{ page: number; data_url: string }> => result.status === 'fulfilled')
          .map((result) => result.value);
        const failed = results.flatMap((result, index) =>
          result.status === 'rejected' ? [chunk[index]] : []
        );

        thumbnails = new Map([
          ...thumbnails,
          ...successful.map((result) => [result.page, result.data_url] as [number, string]),
        ]);

        if (failed.length > 0) {
          failedPages = new Set([...failedPages, ...failed]);
          error = 'Some thumbnails could not be rendered.';
        }
      }
    } catch (e: unknown) {
      if (token === renderToken) {
        error = e instanceof Error ? e.message : String(e);
      }
    } finally {
      if (token === renderToken) {
        inFlightPages = new Set(
          [...inFlightPages].filter((page) => !pendingPages.includes(page))
        );
        loading = false;
      }
    }
  }

  $effect(() => {
    const initialVisible = Math.min(INITIAL_BATCH, totalPages);

    renderToken += 1;
    visibleCount = initialVisible;
    thumbnails = new Map();
    failedPages = new Set();
    inFlightPages = new Set();
    loading = false;
    error = null;

    if (pdfPath && totalPages > 0) {
      void renderBatch(1, initialVisible);
    }
  });

  function loadMore() {
    if (loading || disabled) {
      return;
    }

    const nextVisible = Math.min(visibleCount + INITIAL_BATCH, totalPages);
    if (nextVisible <= visibleCount) {
      return;
    }

    const start = visibleCount + 1;
    visibleCount = nextVisible;
    void renderBatch(start, nextVisible);
  }
</script>

<div class="flex flex-col gap-3">
  <div class="grid grid-cols-[repeat(auto-fill,minmax(96px,1fr))] gap-2">
    {#each visiblePages as page (page)}
      {@const selected = selectedPages.has(page)}
      {@const thumbnail = thumbnails.get(page)}
      {@const failed = failedPages.has(page)}
      <button
        onclick={() => onToggle(page)}
        class={[
          'relative flex flex-col items-center gap-1 rounded-lg border-2 p-2 transition-colors',
          disabled ? 'cursor-not-allowed opacity-60' : '',
          selected ? 'border-teal-500 bg-teal-50' : 'border-stone-200 bg-white hover:border-stone-400',
        ].join(' ')}
        aria-pressed={selected}
        aria-label={`Page ${page}`}
        disabled={disabled}
      >
        {#if thumbnail}
          <img src={thumbnail} alt={`Page ${page}`} class="h-24 w-24 rounded object-contain" />
        {:else if failed}
          <div class="flex h-24 w-24 items-center justify-center rounded border border-red-100 bg-red-50">
            <span class="px-2 text-center text-[10px] text-red-500">Failed</span>
          </div>
        {:else}
          <div class="flex h-24 w-24 items-center justify-center rounded bg-stone-100 animate-pulse">
            <span class="text-xs text-stone-400">{page}</span>
          </div>
        {/if}
        <span class="text-xs text-stone-600">{page}</span>
        {#if selected}
          <span class="absolute right-1 top-1 flex h-4 w-4 items-center justify-center rounded-full bg-teal-500 text-white">
            <svg class="h-2.5 w-2.5" fill="currentColor" viewBox="0 0 20 20">
              <path fill-rule="evenodd" clip-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" />
            </svg>
          </span>
        {/if}
      </button>
    {/each}
  </div>

  {#if loading}
    <p class="text-center text-xs text-stone-400">Loading thumbnails...</p>
  {/if}

  {#if error}
    <p class="text-center text-xs text-red-600">{error}</p>
  {/if}

  {#if visibleCount < totalPages}
    <button
      onclick={loadMore}
      disabled={loading || disabled}
      class="self-center rounded border border-stone-300 px-3 py-1.5 text-sm text-stone-600 transition-colors hover:bg-stone-50 disabled:cursor-not-allowed disabled:opacity-50"
    >
      Load more ({totalPages - visibleCount} remaining)
    </button>
  {/if}
</div>
