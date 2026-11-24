<script lang="ts">
  import { fade } from 'svelte/transition';

  export let toast: { id: number; message: string; type: 'success' | 'error' | 'info' };
  export let onDismiss: () => void;

  const typeColors = {
    success: 'bg-green-500',
    error: 'bg-red-500',
    info: 'bg-blue-500',
  };
</script>

<div
  class="fixed bottom-4 right-4 z-50"
  transition:fade
>
  <div
    class={`flex items-center gap-3 px-4 py-3 rounded-md shadow-lg ${typeColors[toast.type]} text-white`}
  >
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
      {#if toast.type === 'success'}
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
      {:else if toast.type === 'error'}
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
      {:else}
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      {/if}
    </svg>
    <span class="text-sm">{toast.message}</span>
    <button
      onclick={onDismiss}
      class="ml-4 text-white/70 hover:text-white"
    >
      <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
      </svg>
    </button>
  </div>
</div>
