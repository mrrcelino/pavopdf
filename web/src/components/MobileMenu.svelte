<script lang="ts">
  let open = $state(false);

  function toggle() {
    open = !open;
  }

  function close() {
    open = false;
  }

  const categories = [
    { name: 'Organise', slug: 'organise' },
    { name: 'Convert from PDF', slug: 'convert-from-pdf' },
    { name: 'Convert to PDF', slug: 'convert-to-pdf' },
    { name: 'Edit', slug: 'edit' },
    { name: 'Security & Repair', slug: 'security-repair' },
  ];
</script>

<button
  onclick={toggle}
  class="text-stone-300 hover:text-cream p-2"
  aria-label={open ? 'Close menu' : 'Open menu'}
  aria-expanded={open}
>
  {#if open}
    <svg class="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M6 18L18 6M6 6l12 12" />
    </svg>
  {:else}
    <svg class="w-6 h-6" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <path d="M4 6h16M4 12h16M4 18h16" />
    </svg>
  {/if}
</button>

{#if open}
  <div class="absolute top-16 left-0 right-0 bg-stone-950 border-b border-stone-800 px-4 py-4 space-y-1">
    <p class="text-xs uppercase tracking-wider text-stone-500 px-3 py-1">Tools</p>
    {#each categories as cat}
      <a
        href={`/#${cat.slug}`}
        onclick={close}
        class="block px-3 py-2 text-sm text-stone-300 hover:text-cream hover:bg-stone-900 rounded"
      >
        {cat.name}
      </a>
    {/each}
    <hr class="border-stone-800 my-2" />
    <a href="/download" onclick={close} class="block px-3 py-2 text-sm text-stone-300 hover:text-cream hover:bg-stone-900 rounded">Download</a>
    <a href="/about" onclick={close} class="block px-3 py-2 text-sm text-stone-300 hover:text-cream hover:bg-stone-900 rounded">About</a>
    <a
      href="https://github.com/mrrcelino/pavopdf"
      target="_blank"
      rel="noopener noreferrer"
      onclick={close}
      class="block px-3 py-2 text-sm text-stone-300 hover:text-cream hover:bg-stone-900 rounded"
    >
      GitHub
    </a>
  </div>
{/if}
