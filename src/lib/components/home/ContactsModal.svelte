<!-- Contacts management modal for selecting, editing, importing, and exporting voice personas. -->
<script lang="ts">
    import type { ContactProfile } from "$lib/openduck/types";

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
        handleExportSelectedContact: () => Promise<void>;
        refAudioPlaying: boolean;
    }>();

    let emotionMapText = $state("");
    let emotionMapError = $state<string | null>(null);
    let lastSelectedContactId = $state<string | null>(null);
    let validationTimeout: ReturnType<typeof setTimeout> | null = null;

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
                        placeholder="Transcription of the reference audio."
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

                <label class="contact-field">
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
                            >Cubism Emotion Map (JSON Optional)</span
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
                        <span class="contact-field-label">Scale</span>
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
                        <span class="contact-field-label">Zoom</span>
                        <input
                            class="contact-text-input"
                            type="number"
                            step="0.1"
                            value={selectedContact?.cubismModel?.zoom ?? 1.0}
                            oninput={handleSelectedContactCubismZoomInput}
                        />
                    </label>
                    <label class="contact-field">
                        <span class="contact-field-label">Offset X</span>
                        <input
                            class="contact-text-input"
                            type="number"
                            value={selectedContact?.cubismModel?.offsetX ?? 0}
                            oninput={handleSelectedContactCubismOffsetXInput}
                        />
                    </label>
                    <label class="contact-field">
                        <span class="contact-field-label">Offset Y</span>
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
                    onclick={handleDeleteSelectedContact}
                >
                    Delete
                </button>
                <button
                    type="button"
                    class="download-btn"
                    onclick={handleExportSelectedContact}
                >
                    Export
                </button>
            </div>
        </div>
    </div>
</div>
