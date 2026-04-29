<!-- Contacts management modal for selecting, editing, importing, and exporting voice personas. -->
<script lang="ts">
    import type { ContactProfile } from "$lib/openduck/types";
    import ConfirmDialog from "../ui/ConfirmDialog.svelte";

    let {
        contacts,
        selectedContact,
        selectedContactName,
        selectedContactIconUrl,
        getContactDisplayName,
        closeContactsPopup,
        selectContact,
        triggerContactImport,
        createNewContact,
        triggerContactIconUpload,
        triggerContactRefAudioUpload,
        triggerContactCubismModelZipUpload,
        handleResetSelectedContactIcon,
        handleResetSelectedContactRefAudio,
        handleResetSelectedContactCubismModel,
        handlePlaySelectedContactRefAudio,
        handleSelectedContactNameInput,
        handleSelectedContactPromptInput,
        handleSelectedContactMemoryToggleInput,
        handleDeleteContactMemory,
        handleClearAllMemories,
        handleAddContactMemory,
        handleUpdateContactMemory,
        handleSelectedContactGenderInput,
        handleSelectedContactRefTextInput,
        handleSelectedContactCubismModelInput,
        handleSelectedContactCubismExpressionInput,
        handleSelectedContactCubismEmotionMapInput,
        handleSelectedContactCubismScaleInput,
        handleSelectedContactCubismOffsetXInput,
        handleSelectedContactCubismOffsetYInput,
        handleSelectedContactCubismZoomInput,
        handleDeleteSelectedContact,
        handleExportSelectedContact,
        refAudioPlaying,
    } = $props<{
        contacts: ContactProfile[];
        selectedContact: ContactProfile | null;
        selectedContactName: string;
        selectedContactIconUrl: string;
        getContactDisplayName: (
            contact: Pick<ContactProfile, "name"> | null | undefined,
        ) => string;
        closeContactsPopup: () => void;
        selectContact: (contactId: string) => void;
        triggerContactImport: () => void;
        createNewContact: () => void;
        triggerContactIconUpload: () => void;
        triggerContactRefAudioUpload: () => void;
        triggerContactCubismModelZipUpload: () => void;
        handleResetSelectedContactIcon: () => Promise<void>;
        handleResetSelectedContactRefAudio: () => void;
        handleResetSelectedContactCubismModel: () => Promise<void>;
        handlePlaySelectedContactRefAudio: () => void;
        handleSelectedContactNameInput: (event: Event) => void;
        handleSelectedContactPromptInput: (event: Event) => void;
        handleSelectedContactMemoryToggleInput: (event: Event) => void;
        handleDeleteContactMemory: (memoryId: string) => void;
        handleClearAllMemories: () => void;
        handleAddContactMemory: (text: string) => void;
        handleUpdateContactMemory: (memoryId: string, text: string) => void;
        handleSelectedContactGenderInput: (event: Event) => void;
        handleSelectedContactRefTextInput: (event: Event) => void;
        handleSelectedContactCubismModelInput: (event: Event) => void;
        handleSelectedContactCubismExpressionInput: (event: Event) => void;
        handleSelectedContactCubismEmotionMapInput: (event: Event) => void;
        handleSelectedContactCubismScaleInput: (event: Event) => void;
        handleSelectedContactCubismOffsetXInput: (event: Event) => void;
        handleSelectedContactCubismOffsetYInput: (event: Event) => void;
        handleSelectedContactCubismZoomInput: (event: Event) => void;
        handleDeleteSelectedContact: () => Promise<void>;
        handleExportSelectedContact: (includeMemory: boolean) => Promise<void>;
        refAudioPlaying: boolean;
    }>();

    let emotionMapText = $state("");
    let emotionMapError = $state<string | null>(null);
    let lastSelectedContactId = $state<string | null>(null);
    let validationTimeout: ReturnType<typeof setTimeout> | null = null;
    let showDeleteConfirm = $state(false);
    let showMemoriesManager = $state(false);
    let showMemoryDeleteConfirm = $state(false);
    let showClearAllMemoriesConfirm = $state(false);
    let showExportMemoryConfirm = $state(false);
    let memoryToDeleteId = $state<string | null>(null);
    let memorySearchQuery = $state("");
    let isAddingMemory = $state(false);
    let editingMemoryId = $state<string | null>(null);
    let memoryInputText = $state("");

    const filteredMemories = $derived.by(() => {
        const memories = selectedContact?.memories || [];
        if (!memorySearchQuery.trim()) return memories;
        const query = memorySearchQuery.toLowerCase();
        return memories.filter((m) => m.text.toLowerCase().includes(query));
    });

    $effect(() => {
        if (selectedContact?.id !== lastSelectedContactId) {
            lastSelectedContactId = selectedContact?.id ?? null;
            emotionMapText = selectedContact?.cubismModel?.emotionMap
                ? JSON.stringify(
                      selectedContact.cubismModel.emotionMap,
                      null,
                      2,
                  )
                : "";
            emotionMapError = null;
            if (validationTimeout) clearTimeout(validationTimeout);
        }
    });

    function validateEmotionMap() {
        if (validationTimeout) clearTimeout(validationTimeout);

        if (!emotionMapText.trim()) {
            emotionMapError = null;
            return;
        }

        try {
            JSON.parse(emotionMapText);
            emotionMapError = null;
        } catch (err) {
            // Provide more detail if available from the JSON error
            if (err instanceof Error) {
                emotionMapError = err.message;
            } else {
                emotionMapError = "Invalid JSON";
            }
        }
    }

    function handleEmotionMapInput(event: Event) {
        if (validationTimeout) clearTimeout(validationTimeout);
        validationTimeout = setTimeout(validateEmotionMap, 1000);
        handleSelectedContactCubismEmotionMapInput(event);
    }

    const scaleOptions = Array.from({ length: 20 }, (_, i) => (i + 1) * 0.5);
</script>

<button
    type="button"
    class="contacts-modal-backdrop"
    aria-label="Close contacts"
    onclick={closeContactsPopup}
></button>
<div
    id="contacts-popup"
    class="contacts-popup"
    role="dialog"
    aria-label="Contacts"
    aria-modal="true"
>
    <div class="contacts-popup-header">
        <div class="contacts-popup-copy">
            <span class="contacts-popup-title">Contacts</span>
            <span class="contacts-popup-subtitle"
                >Switch the active voice persona</span
            >
        </div>
        <button
            type="button"
            class="conversation-close-btn"
            onclick={closeContactsPopup}
            aria-label="Close contacts"
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

    <div class="contacts-popup-body">
        <div class="contacts-sidebar">
            <div class="contacts-list">
                {#each contacts as contact (contact.id)}
                    <button
                        type="button"
                        class="contact-list-item"
                        class:active={contact.id === selectedContact?.id}
                        onclick={() => selectContact(contact.id)}
                    >
                        <span class="contact-list-avatar" aria-hidden="true">
                            <img
                                src={contact.iconDataUrl ?? "/icon.png"}
                                alt=""
                            />
                        </span>
                        <span class="contact-list-name"
                            >{getContactDisplayName(contact)}</span
                        >
                        {#if contact.cubismModel}
                            <span class="contact-list-badge">Live2D</span>
                        {/if}
                    </button>
                {/each}
            </div>

            <div class="contacts-sidebar-actions">
                <button
                    type="button"
                    class="utility-btn"
                    onclick={triggerContactImport}
                >
                    Import
                </button>
                <button
                    type="button"
                    class="utility-btn"
                    onclick={createNewContact}
                >
                    Add
                </button>
            </div>
        </div>

        <div class="contacts-editor">
            <div class="contacts-editor-scroll">
                <div class="contacts-editor-top">
                    <button
                        type="button"
                        class="contact-icon-picker"
                        onclick={triggerContactIconUpload}
                        aria-label="Upload contact icon"
                    >
                        <img src={selectedContactIconUrl} alt="" />
                    </button>

                    <div class="contacts-editor-copy">
                        <div class="contacts-editor-header">
                            <span class="contacts-editor-name"
                                >{selectedContactName}</span
                            >
                            {#if selectedContact?.cubismModel}
                                <span class="contact-list-badge">Live2D</span>
                            {/if}
                        </div>
                        <span class="contacts-editor-hint"
                            >Click the icon to upload a contact photo. The duck
                            icon stays as fallback.</span
                        >
                        {#if selectedContact?.hasCustomIcon}
                            <button
                                type="button"
                                class="utility-btn subtitle-action-btn contact-inline-btn"
                                onclick={handleResetSelectedContactIcon}
                            >
                                Reset Icon
                            </button>
                        {/if}
                    </div>
                </div>

                <label class="contact-field">
                    <span class="contact-field-label">Name</span>
                    <input
                        class="contact-text-input"
                        type="text"
                        value={selectedContact?.name ?? ""}
                        placeholder="Contact name"
                        oninput={handleSelectedContactNameInput}
                    />
                </label>

                <label class="contact-field">
                    <span class="contact-field-label">Gender</span>
                    <div class="contact-select-shell">
                        <select
                            class="contact-text-input contact-select"
                            value={selectedContact?.gender ?? "female"}
                            onchange={handleSelectedContactGenderInput}
                        >
                            <option value="female">Female</option>
                            <option value="male">Male</option>
                        </select>
                    </div>
                </label>

                <label class="contact-field contact-field-grow">
                    <span class="contact-field-label">Scenario</span>
                    <textarea
                        class="contact-textarea"
                        rows="6"
                        placeholder="Describe the character, roleplay, or scenario."
                        value={selectedContact?.prompt ?? ""}
                        oninput={handleSelectedContactPromptInput}
                    ></textarea>
                </label>

                <div class="contact-field">
                    <div class="contact-memory-row">
                        <div class="contact-memory-info">
                            <span class="contact-field-label">Memory</span>
                            <span class="contacts-editor-hint"
                                >{selectedContact?.memories?.length || 0} saved memories</span
                            >
                        </div>
                        <button
                            type="button"
                            class="manage-memories-btn"
                            onclick={() => (showMemoriesManager = true)}
                        >
                            Manage
                        </button>
                    </div>
                    <div class="contact-memory-toggle-field">
                        <div class="contact-memory-toggle-info">
                            <span class="contact-memory-toggle-label"
                                >Reference saved memories</span
                            >
                            <span class="contacts-editor-hint"
                                >Let {selectedContact?.name || "this character"} save and use memories when
                                responding.</span
                            >
                        </div>
                        <label class="ios-toggle">
                            <input
                                type="checkbox"
                                checked={selectedContact?.memoryEnabled !==
                                    false}
                                onchange={handleSelectedContactMemoryToggleInput}
                            />
                            <span class="ios-toggle-slider"></span>
                        </label>
                    </div>
                </div>

                <div class="contact-field">
                    <span class="contact-field-label"
                        >Audio Reference (Optional)</span
                    >
                    <div class="contact-audio-row">
                        <button
                            type="button"
                            class="utility-btn"
                            onclick={triggerContactRefAudioUpload}
                        >
                            {selectedContact?.refAudio
                                ? "Change Audio"
                                : "Upload Audio"}
                        </button>
                        {#if selectedContact?.refAudio}
                            <button
                                type="button"
                                class="utility-btn"
                                onclick={handlePlaySelectedContactRefAudio}
                                aria-label={refAudioPlaying
                                    ? "Stop reference audio"
                                    : "Play reference audio"}
                            >
                                {#if refAudioPlaying}
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        width="14"
                                        height="14"
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                        stroke="currentColor"
                                        stroke-width="2"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        ><rect
                                            x="4"
                                            y="4"
                                            width="16"
                                            height="16"
                                        /></svg
                                    >
                                {:else}
                                    <svg
                                        xmlns="http://www.w3.org/2000/svg"
                                        width="14"
                                        height="14"
                                        viewBox="0 0 24 24"
                                        fill="currentColor"
                                        stroke="currentColor"
                                        stroke-width="2"
                                        stroke-linecap="round"
                                        stroke-linejoin="round"
                                        ><polygon
                                            points="5 3 19 12 5 21 5 3"
                                        /></svg
                                    >
                                {/if}
                            </button>
                            <button
                                type="button"
                                class="utility-btn"
                                onclick={handleResetSelectedContactRefAudio}
                            >
                                Reset
                            </button>
                        {/if}
                    </div>
                    <span class="contacts-editor-hint"
                        >Experimental. Currently only supported by CosyVoice3
                        and Chatterbox models for voice cloning</span
                    >
                </div>
                <label class="contact-field">
                    <span class="contact-field-label"
                        >Transcription (Optional)</span
                    >
                    <input
                        class="contact-text-input"
                        type="text"
                        value={selectedContact?.refText ?? ""}
                        placeholder="Transcription of the reference audio you uploaded above."
                        oninput={handleSelectedContactRefTextInput}
                    />
                </label>

                <div class="contact-field">
                    <span class="contact-field-label"
                        >Cubism Model (Optional)</span
                    >
                    <div class="contact-audio-row">
                        <button
                            type="button"
                            class="utility-btn"
                            onclick={triggerContactCubismModelZipUpload}
                        >
                            {selectedContact?.cubismModel?.source === "zip"
                                ? "Change Model Zip"
                                : "Upload Model Zip"}
                        </button>
                        {#if selectedContact?.cubismModel}
                            <button
                                type="button"
                                class="utility-btn"
                                onclick={handleResetSelectedContactCubismModel}
                            >
                                Reset
                            </button>
                        {/if}
                        {#if selectedContact?.cubismModel?.source === "zip" && selectedContact.cubismModel.zipName}
                            <span class="contact-file-name"
                                >{selectedContact.cubismModel.zipName}</span
                            >
                        {/if}
                    </div>
                    <input
                        class="contact-text-input"
                        type="url"
                        value={selectedContact?.cubismModel?.url ?? ""}
                        placeholder="Or paste https://.../model3.json"
                        oninput={handleSelectedContactCubismModelInput}
                        hidden
                    />
                </div>

                <label class="contact-field" style="display: none">
                    <span class="contact-field-label"
                        >Cubism Model Expression (Optional)</span
                    >
                    <input
                        class="contact-text-input"
                        type="text"
                        value={selectedContact?.cubismModel?.expression ?? ""}
                        placeholder="Expression name (e.g. happy)"
                        oninput={handleSelectedContactCubismExpressionInput}
                    />
                </label>

                <label class="contact-field">
                    <div class="contact-field-header">
                        <span class="contact-field-label"
                            >Cubism Emotion Map (JSON, Optional)</span
                        >
                        {#if emotionMapError}
                            <span
                                class="contact-field-error"
                                title={emotionMapError}>{emotionMapError}</span
                            >
                        {/if}
                    </div>
                    <textarea
                        class="contact-text-input contact-textarea"
                        class:has-error={!!emotionMapError}
                        autocomplete="off"
                        autocorrect="off"
                        autocapitalize="off"
                        spellcheck="false"
                        data-gramm="false"
                        bind:value={emotionMapText}
                        placeholder={'{ "joy": "f01", "smile": "f04", "shy": "f06" }'}
                        oninput={handleEmotionMapInput}
                        onblur={validateEmotionMap}
                    ></textarea>
                </label>

                <div class="contact-field-row">
                    <label class="contact-field">
                        <span class="contact-field-label"
                            >Cubism Model Scale</span
                        >
                        <div class="contact-select-shell">
                            <select
                                class="contact-text-input contact-select"
                                value={selectedContact?.cubismModel?.scale ??
                                    1.55}
                                onchange={handleSelectedContactCubismScaleInput}
                            >
                                {#each scaleOptions as scale}
                                    <option value={scale}
                                        >{scale.toFixed(1)}</option
                                    >
                                {/each}
                                {#if selectedContact?.cubismModel?.scale && !scaleOptions.includes(selectedContact.cubismModel.scale)}
                                    <option
                                        value={selectedContact.cubismModel
                                            .scale}
                                        >{selectedContact.cubismModel.scale.toFixed(
                                            2,
                                        )}</option
                                    >
                                {/if}
                            </select>
                        </div>
                    </label>
                    <label class="contact-field">
                        <span class="contact-field-label"
                            >Cubism Model Zoom</span
                        >
                        <input
                            class="contact-text-input"
                            type="number"
                            step="0.1"
                            value={selectedContact?.cubismModel?.zoom ?? 1.0}
                            oninput={handleSelectedContactCubismZoomInput}
                        />
                    </label>
                    <label class="contact-field">
                        <span class="contact-field-label"
                            >Cubism Model Offset X</span
                        >
                        <input
                            class="contact-text-input"
                            type="number"
                            value={selectedContact?.cubismModel?.offsetX ?? 0}
                            oninput={handleSelectedContactCubismOffsetXInput}
                        />
                    </label>
                    <label class="contact-field">
                        <span class="contact-field-label"
                            >Cubism Model Offset Y</span
                        >
                        <input
                            class="contact-text-input"
                            type="number"
                            value={selectedContact?.cubismModel?.offsetY ?? 0}
                            oninput={handleSelectedContactCubismOffsetYInput}
                        />
                    </label>
                </div>
            </div>

            <div class="contacts-editor-actions">
                <button
                    type="button"
                    class="utility-btn"
                    disabled={contacts.length === 1}
                    onclick={() => (showDeleteConfirm = true)}
                >
                    Delete
                </button>
                <button
                    type="button"
                    class="download-btn"
                    onclick={() => {
                        if (selectedContact?.memories?.length) {
                            showExportMemoryConfirm = true;
                        } else {
                            handleExportSelectedContact(false);
                        }
                    }}
                >
                    Export
                </button>
            </div>
        </div>
    </div>

    {#if showExportMemoryConfirm}
        <ConfirmDialog
            title="Export with memory?"
            message="Do you want to include the saved memories in the exported file?"
            btnConfirm="With Memory"
            btnCancel="Without Memory"
            onConfirm={async () => {
                showExportMemoryConfirm = false;
                await handleExportSelectedContact(true);
            }}
            onCancel={async () => {
                showExportMemoryConfirm = false;
                await handleExportSelectedContact(false);
            }}
        />
    {/if}

    {#if showDeleteConfirm}
        <ConfirmDialog
            title="Delete character?"
            message="This action cannot be undone. Do you wish to continue?"
            onConfirm={async () => {
                await handleDeleteSelectedContact();
                showDeleteConfirm = false;
            }}
            onCancel={() => (showDeleteConfirm = false)}
        />
    {/if}

    {#if showMemoryDeleteConfirm}
        <ConfirmDialog
            title="Delete memory?"
            message="This action cannot be undone. Do you wish to continue?"
            onConfirm={() => {
                if (memoryToDeleteId) {
                    handleDeleteContactMemory(memoryToDeleteId);
                }
                showMemoryDeleteConfirm = false;
                memoryToDeleteId = null;
            }}
            onCancel={() => {
                showMemoryDeleteConfirm = false;
                memoryToDeleteId = null;
            }}
        />
    {/if}

    {#if showClearAllMemoriesConfirm}
        <ConfirmDialog
            title="Clear all memories?"
            message="This action cannot be undone. All saved memories for this character will be permanently deleted."
            onConfirm={() => {
                handleClearAllMemories();
                showClearAllMemoriesConfirm = false;
            }}
            onCancel={() => (showClearAllMemoriesConfirm = false)}
        />
    {/if}

    {#if showMemoriesManager}
        <div class="memories-manager-overlay">
            <button
                type="button"
                class="memories-manager-backdrop"
                onclick={() => (showMemoriesManager = false)}
                aria-label="Close memories"
            ></button>
            <div class="memories-manager-popup">
                <div class="memories-manager-header">
                    <div class="memories-manager-title-row">
                        <span class="memories-manager-title"
                            >Saved memories</span
                        >
                        <div class="memories-manager-title-actions">
                            <button
                                type="button"
                                class="memories-add-btn"
                                onclick={() => {
                                    isAddingMemory = true;
                                    editingMemoryId = null;
                                    memoryInputText = "";
                                }}
                            >
                                Add memory
                            </button>
                            {#if selectedContact && selectedContact.memories && selectedContact.memories.length > 0}
                                <button
                                    type="button"
                                    class="memories-clear-all-btn"
                                    onclick={() =>
                                        (showClearAllMemoriesConfirm = true)}
                                >
                                    Clear all
                                </button>
                            {/if}
                            <button
                                type="button"
                                class="conversation-close-btn"
                                onclick={() => (showMemoriesManager = false)}
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
                    </div>
                    <p class="memories-manager-description">
                        OpenDuck remembers and automatically manages useful
                        information from calls, making responses more relevant
                        and personal.
                    </p>
                </div>

                {#if isAddingMemory || editingMemoryId}
                    <div class="memory-edit-box">
                        <textarea
                            class="memory-edit-textarea"
                            placeholder="Type a memory to save..."
                            bind:value={memoryInputText}
                            rows="3"
                        ></textarea>
                        <div class="memory-edit-actions">
                            <button
                                type="button"
                                class="utility-btn"
                                onclick={() => {
                                    isAddingMemory = false;
                                    editingMemoryId = null;
                                    memoryInputText = "";
                                }}
                            >
                                Cancel
                            </button>
                            <button
                                type="button"
                                class="memory-save-btn"
                                disabled={!memoryInputText.trim()}
                                onclick={() => {
                                    if (editingMemoryId) {
                                        handleUpdateContactMemory(
                                            editingMemoryId,
                                            memoryInputText,
                                        );
                                    } else {
                                        handleAddContactMemory(memoryInputText);
                                    }
                                    isAddingMemory = false;
                                    editingMemoryId = null;
                                    memoryInputText = "";
                                }}
                            >
                                {editingMemoryId ? "Update" : "Save"}
                            </button>
                        </div>
                    </div>
                {/if}

                <div class="memories-list">
                    {#if filteredMemories.length === 0}
                        <div class="memories-empty">
                            {memorySearchQuery
                                ? "No matching memories found."
                                : "No memories saved yet."}
                        </div>
                    {:else}
                        {#each filteredMemories as memory (memory.id)}
                            <div class="memory-item">
                                <div class="memory-content">
                                    <div class="memory-text">{memory.text}</div>
                                    <div class="memory-meta">
                                        Saved on {new Date(
                                            memory.createdAt,
                                        ).toLocaleDateString(undefined, {
                                            year: "numeric",
                                            month: "long",
                                            day: "numeric",
                                        })}
                                    </div>
                                </div>
                                <div class="memory-item-actions">
                                    <button
                                        type="button"
                                        class="memory-action-btn"
                                        onclick={() => {
                                            editingMemoryId = memory.id;
                                            isAddingMemory = false;
                                            memoryInputText = memory.text;
                                        }}
                                        title="Edit memory"
                                    >
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            width="16"
                                            height="16"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="2"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            ><path
                                                d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"
                                            /><path
                                                d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"
                                            /></svg
                                        >
                                    </button>
                                    <button
                                        type="button"
                                        class="memory-delete-btn"
                                        onclick={() => {
                                            memoryToDeleteId = memory.id;
                                            showMemoryDeleteConfirm = true;
                                        }}
                                        title="Delete memory"
                                    >
                                        <svg
                                            xmlns="http://www.w3.org/2000/svg"
                                            width="16"
                                            height="16"
                                            viewBox="0 0 24 24"
                                            fill="none"
                                            stroke="currentColor"
                                            stroke-width="2"
                                            stroke-linecap="round"
                                            stroke-linejoin="round"
                                            ><polyline points="3 6 5 6 21 6" /><path
                                                d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"
                                            /><line
                                                x1="10"
                                                y1="11"
                                                x2="10"
                                                y2="17"
                                            /><line
                                                x1="14"
                                                y1="11"
                                                x2="14"
                                                y2="17"
                                            /></svg
                                        >
                                    </button>
                                </div>
                            </div>
                        {/each}
                    {/if}
                </div>
            </div>
        </div>
    {/if}
</div>
