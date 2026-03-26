<script lang="ts">
  let detectedPlatform = $state<string | null>(null);

  $effect(() => {
    const ua = navigator.userAgent.toLowerCase();
    const platform = (navigator as any).userAgentData?.platform?.toLowerCase() ?? navigator.platform?.toLowerCase() ?? '';

    if (platform.includes('win') || ua.includes('windows')) {
      detectedPlatform = 'windows';
    } else if (platform.includes('mac') || ua.includes('macintosh') || ua.includes('mac os')) {
      detectedPlatform = 'macos';
    } else if (platform.includes('linux') || ua.includes('linux')) {
      detectedPlatform = 'linux';
    }

    if (detectedPlatform) {
      const cards = document.querySelectorAll('[data-platform]');
      cards.forEach((card) => {
        const el = card as HTMLElement;
        if (el.dataset.platform === detectedPlatform) {
          el.classList.add('ring-2', 'ring-teal', 'shadow-md');
        }
      });
    }
  });
</script>

{#if detectedPlatform}
  <p class="text-sm text-stone-500 text-center mb-6">
    Detected platform: <span class="font-medium text-stone-700 capitalize">{detectedPlatform === 'macos' ? 'macOS' : detectedPlatform}</span>
  </p>
{/if}
