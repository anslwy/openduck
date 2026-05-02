<script lang="ts">
  import type { Background } from "$lib/openduck/types";
  import { fade } from "svelte/transition";

  let {
    backgrounds,
    selectedBackgroundId,
    onSelect,
    onUnset,
    onUpload,
    onDelete,
    onClose,
  } = $props<{
    backgrounds: Background[];
    selectedBackgroundId: string | null;
    onSelect: (background: Background) => void;
    onUnset: () => void;
    onUpload: (file: File) => void;
    onDelete: (background: Background) => void;
    onClose: () => void;
  }>();

  let fileInput: HTMLInputElement;

  function handleFileChange(event: Event) {
    const target = event.target as HTMLInputElement;
    const file = target.files?.[0];
    if (file) {
      onUpload(file);
    }
    target.value = "";
  }
</script>

<button
  type="button"
  class="about-modal-backdrop"
  aria-label="Close Backgrounds"
  onclick={onClose}
></button>

<div
  class="about-modal"
  role="dialog"
  aria-labelledby="background-modal-title"
  aria-modal="true"
>
  <div class="about-modal-header">
    <div class="about-modal-copy">
      <span class="about-modal-title" id="background-modal-title">Backgrounds</span>
      <span class="about-modal-subtitle">Select or upload a background image</span>
    </div>
    <button
      type="button"
      class="conversation-close-btn"
      onclick={onClose}
      aria-label="Close Backgrounds"
    >
      <svg
        xmlns="http://www.w3.org/2000/svg"
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.2"
        stroke-linecap="round"
        stroke-linejoin="round"
        ><line x1="18" y1="6" x2="6" y2="18" /><line
          x1="6"
          y1="6"
          x2="18"
          y2="18"
        /></svg
      >
    </button>
  </div>

  <div class="about-modal-content">
    <div class="background-grid">
      <button
        type="button"
        class="background-unset-card"
        class:selected={selectedBackgroundId === null}
        onclick={onUnset}
      >
        <div class="unset-icon">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </div>
        <span>None</span>
      </button>

      <button
        type="button"
        class="background-upload-card"
        onclick={() => fileInput.click()}
      >
        <div class="upload-icon">
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="24"
            height="24"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="2"
            stroke-linecap="round"
            stroke-linejoin="round"
          >
            <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
            <polyline points="17 8 12 3 7 8" />
            <line x1="12" y1="3" x2="12" y2="15" />
          </svg>
        </div>
        <span>Upload</span>
        <input
          type="file"
          accept="image/*"
          bind:this={fileInput}
          onchange={handleFileChange}
          style="display: none"
        />
      </button>

      {#each backgrounds as bg (bg.id)}
        <div
          class="background-card"
          class:selected={bg.id === selectedBackgroundId}
        >
          <button
            type="button"
            class="background-card-inner"
            onclick={() => onSelect(bg)}
          >
            <img src={bg.url} alt={bg.name} />
            <div class="background-card-overlay">
              <span>{bg.name}</span>
            </div>
          </button>
          {#if !bg.isStock}
            <button
              type="button"
              class="background-delete-btn"
              onclick={() => onDelete(bg)}
              title="Delete background"
            >
              <svg
                xmlns="http://www.w3.org/2000/svg"
                width="14"
                height="14"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="3"
                stroke-linecap="round"
                stroke-linejoin="round"
              >
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          {/if}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .background-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 12px;
    padding: 16px;
  }

  .background-card, .background-upload-card, .background-unset-card {
    aspect-ratio: 16 / 9;
    border-radius: 8px;
    overflow: hidden;
    position: relative;
    border: 2px solid transparent;
    background: rgba(255, 255, 255, 0.05);
    transition: all 0.2s ease;
  }

  .background-card.selected, .background-unset-card.selected {
    border-color: #7fe37c;
    box-shadow: 0 0 10px rgba(127, 227, 124, 0.3);
  }

  .background-card-inner {
    width: 100%;
    height: 100%;
    padding: 0;
    border: none;
    background: none;
    cursor: pointer;
    display: block;
  }

  .background-card img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .background-card-overlay {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    padding: 4px 8px;
    background: rgba(0, 0, 0, 0.6);
    color: white;
    font-size: 11px;
    text-align: center;
    opacity: 0;
    transition: opacity 0.2s ease;
  }

  .background-card:hover .background-card-overlay {
    opacity: 1;
  }

  .background-upload-card, .background-unset-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    cursor: pointer;
    border: 2px dashed rgba(255, 255, 255, 0.2);
    color: rgba(255, 255, 255, 0.6);
  }

  .background-upload-card:hover, .background-unset-card:hover {
    border-color: rgba(255, 255, 255, 0.4);
    color: white;
    background: rgba(255, 255, 255, 0.1);
  }

  .background-delete-btn {
    position: absolute;
    top: 4px;
    right: 4px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: rgba(0, 0, 0, 0.5);
    border: none;
    color: white;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.2s ease;
  }

  .background-card:hover .background-delete-btn {
    opacity: 1;
  }

  .background-delete-btn:hover {
    background: rgba(255, 59, 48, 0.8);
  }
</style>
