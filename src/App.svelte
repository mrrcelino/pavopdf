<script lang="ts">
  const appName = 'PavoPDF';
</script>

<main class="min-h-screen bg-cream flex flex-col items-center justify-center">
  <div class="text-center p-8">
    <h1 class="text-4xl font-bold text-teal mb-4">{appName}</h1>
    <p class="text-lg text-gray-600 mb-6">
      Free, open-source, offline PDF tools
    </p>
    <div class="flex gap-4 justify-center">
      <button
        class="px-6 py-3 bg-peach text-white rounded-lg font-semibold hover:bg-peach-dark transition-colors"
      >
        Open PDF
      </button>
      <button
        class="px-6 py-3 border-2 border-teal text-teal rounded-lg font-semibold hover:bg-teal hover:text-white transition-colors"
      >
        Merge Files
      </button>
    </div>
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
  }
</style>
