<script lang="ts">
  import Sidebar from './Sidebar.svelte';
  import Dashboard from './Dashboard.svelte';
  import ToolWorkspace from './ToolWorkspace.svelte';
  import SpotlightSearch from '../ui/SpotlightSearch.svelte';
  import Toast from '../ui/Toast.svelte';
  import { activeToolStore } from '../../stores/active-tool.svelte';
  import { settingsStore } from '../../stores/settings.svelte';
  import { recentFilesStore } from '../../stores/recent-files.svelte';
  import { onMount } from 'svelte';

  onMount(async () => {
    await Promise.all([settingsStore.load(), recentFilesStore.load()]);
  });

  let spotlightOpen = $state(false);

  function handleKeydown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
      e.preventDefault();
      spotlightOpen = true;
    }
    if (e.key === 'Escape') {
      spotlightOpen = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="app-shell h-screen flex overflow-hidden bg-cream font-sans select-none">
  {#if activeToolStore.view === 'workspace'}
    <Sidebar />
  {/if}

  <main class="flex-1 overflow-hidden flex flex-col">
    {#if activeToolStore.view === 'dashboard'}
      <Dashboard onOpenSpotlight={() => spotlightOpen = true} />
    {:else}
      <ToolWorkspace />
    {/if}
  </main>

  {#if spotlightOpen}
    <SpotlightSearch onClose={() => spotlightOpen = false} />
  {/if}

  <Toast />
</div>
