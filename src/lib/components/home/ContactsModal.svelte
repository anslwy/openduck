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
        handleResetSelectedContactIcon,
        handleResetSelectedContactRefAudio,
        handlePlaySelectedContactRefAudio,
        handleSelectedContactNameInput,
        handleSelectedContactPromptInput,
        handleSelectedContactRefTextInput,
        handleDeleteSelectedContact,
        handleExportSelectedContact,
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
        handleResetSelectedContactIcon: () => Promise<void>;
        handleResetSelectedContactRefAudio: () => void;
        handlePlaySelectedContactRefAudio: () => void;
        handleSelectedContactNameInput: (event: Event) => void;
        handleSelectedContactPromptInput: (event: Event) => void;
        handleSelectedContactRefTextInput: (event: Event) => void;
        handleDeleteSelectedContact: () => Promise<void>;
        handleExportSelectedContact: () => Promise<void>;
    }>();
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
                        <span class="contact-list-name"
                            >{getContactDisplayName(contact)}</span
                        >
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
                        <span class="contacts-editor-name"
                            >{selectedContactName}</span
                        >
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

                <label class="contact-field contact-field-grow">
                    <span class="contact-field-label">Prompt</span>
                    <textarea
                        class="contact-textarea"
                        rows="6"
                        placeholder="Describe how this contact should respond."
                        value={selectedContact?.prompt ?? ""}
                        oninput={handleSelectedContactPromptInput}
                    ></textarea>
                </label>

                <div class="contact-field">
                    <span class="contact-field-label">Audio Reference</span>
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
                                aria-label="Play reference audio"
                            >
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
                                    ><polygon points="5 3 19 12 5 21 5 3" /></svg
                                >
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
                        >Optional. Used for voice cloning with CosyVoice3
                        models.</span
                    >
                </div>

                <label class="contact-field">
                    <span class="contact-field-label">Transcription</span>
                    <input
                        class="contact-text-input"
                        type="text"
                        value={selectedContact?.refText ?? ""}
                        placeholder="Transcription of the reference audio."
                        oninput={handleSelectedContactRefTextInput}
                    />
                </label>
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
