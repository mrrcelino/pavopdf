<script lang="ts">
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { TOOLS } from '../../tools-registry';
  import CompressWorkspace from '../tools/organise/CompressWorkspace.svelte';
  import MergeWorkspace from '../tools/organise/MergeWorkspace.svelte';
  import RemoveWorkspace from '../tools/organise/RemoveWorkspace.svelte';
  import ReorderWorkspace from '../tools/organise/ReorderWorkspace.svelte';
  import RotateWorkspace from '../tools/organise/RotateWorkspace.svelte';
  import SplitWorkspace from '../tools/organise/SplitWorkspace.svelte';

  const tool = $derived(TOOLS.find(t => t.id === activeToolStore.tool));
</script>

<div class="flex-1 overflow-y-auto p-6 bg-cream">
  {#if tool}
    <div class="mb-1 text-xs text-stone-400">
      All Tools > <span class="text-teal font-semibold">{tool.label}</span>
    </div>
    <h1 class="text-xl font-bold text-stone-800 mb-4">{tool.label}</h1>

    {#if tool.id === 'merge'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <MergeWorkspace />
      </div>
    {:else if tool.id === 'split'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <SplitWorkspace />
      </div>
    {:else if tool.id === 'compress'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <CompressWorkspace />
      </div>
    {:else if tool.id === 'rotate'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <RotateWorkspace />
      </div>
    {:else if tool.id === 'reorder'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <ReorderWorkspace />
      </div>
    {:else if tool.id === 'remove'}
      <div class="bg-white rounded-xl border border-stone-200 overflow-hidden">
        <RemoveWorkspace />
      </div>
    {:else}
      <div class="bg-white rounded-xl border border-stone-200 p-8 text-center text-stone-400">
        <div class="text-4xl mb-2">{tool.icon}</div>
        <p class="text-sm">{tool.description}</p>
        <p class="text-xs mt-2 text-stone-300">Implementation coming in Plans 2-6</p>
      </div>
    {/if}
  {/if}
</div>
