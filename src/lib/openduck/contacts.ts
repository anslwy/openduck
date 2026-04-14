// Helpers for contact metadata, contact icon storage, and import/export related browser-side utilities.
import {
    CONTACTS_STORAGE_KEY,
    CONTACT_ICONS_DB_NAME,
    CONTACT_ICONS_STORE_NAME,
    DEFAULT_CONTACT_ID,
    DEFAULT_VOICE_SYSTEM_PROMPT,
} from "./config";
import type {
    ContactProfile,
    StoredContactProfile,
    StoredContactsPayload,
} from "./types";

let contactIconsDbPromise: Promise<IDBDatabase> | null = null;

export function createDefaultContact(): ContactProfile {
    return {
        id: DEFAULT_CONTACT_ID,
        name: "OpenDuck",
        prompt: DEFAULT_VOICE_SYSTEM_PROMPT,
        hasCustomIcon: false,
        iconDataUrl: null,
        refAudio: null,
        refText: null,
    };
}

export function getContactDisplayName(
    contact: Pick<ContactProfile, "name"> | null | undefined,
) {
    const name = contact?.name.trim();
    return name ? name : "Untitled contact";
}

export function createContactId() {
    if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
        return crypto.randomUUID();
    }

    return `contact-${Date.now()}-${Math.random().toString(16).slice(2)}`;
}

export function createStoredContactsPayload(
    contactList: ContactProfile[],
    activeContactId: string,
): StoredContactsPayload {
    return {
        version: 1,
        selectedContactId: activeContactId,
        contacts: contactList.map((contact) => ({
            id: contact.id,
            name: contact.name,
            prompt: contact.prompt,
            hasCustomIcon: contact.hasCustomIcon,
            refAudio: contact.refAudio,
            refText: contact.refText,
        })),
    };
}

function normalizeStoredContactProfile(
    value: unknown,
): StoredContactProfile | null {
    if (!value || typeof value !== "object") {
        return null;
    }

    const record = value as Record<string, unknown>;
    const id =
        typeof record.id === "string" && record.id.trim()
            ? record.id.trim()
            : null;
    if (!id) {
        return null;
    }

    return {
        id,
        name: typeof record.name === "string" ? record.name : "",
        prompt:
            typeof record.prompt === "string" && record.prompt.trim().length > 0
                ? record.prompt
                : DEFAULT_VOICE_SYSTEM_PROMPT,
        hasCustomIcon: Boolean(record.hasCustomIcon),
        refAudio: typeof record.refAudio === "string" ? record.refAudio : null,
        refText: typeof record.refText === "string" ? record.refText : null,
    };
}

export function slugifyContactName(name: string) {
    const slug = name
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, "");
    return slug || "contact";
}

export function readFileAsDataUrl(file: File) {
    return new Promise<string>((resolve, reject) => {
        const reader = new FileReader();
        reader.onload = () => {
            if (typeof reader.result === "string") {
                resolve(reader.result);
                return;
            }

            reject(new Error("Failed to read file as a data URL."));
        };
        reader.onerror = () =>
            reject(reader.error ?? new Error("Failed to read file."));
        reader.readAsDataURL(file);
    });
}

function openContactIconsDatabase() {
    if (contactIconsDbPromise) {
        return contactIconsDbPromise;
    }

    contactIconsDbPromise = new Promise<IDBDatabase>((resolve, reject) => {
        const request = window.indexedDB.open(CONTACT_ICONS_DB_NAME, 1);

        request.onupgradeneeded = () => {
            const database = request.result;
            if (!database.objectStoreNames.contains(CONTACT_ICONS_STORE_NAME)) {
                database.createObjectStore(CONTACT_ICONS_STORE_NAME);
            }
        };

        request.onsuccess = () => resolve(request.result);
        request.onerror = () =>
            reject(
                request.error ??
                    new Error("Failed to open the contacts icon database."),
            );
    });

    return contactIconsDbPromise;
}

async function runContactIconStoreRequest(
    mode: IDBTransactionMode,
    requestFactory: (store: IDBObjectStore) => IDBRequest,
) {
    const database = await openContactIconsDatabase();

    return new Promise<unknown>((resolve, reject) => {
        const transaction = database.transaction(CONTACT_ICONS_STORE_NAME, mode);
        const store = transaction.objectStore(CONTACT_ICONS_STORE_NAME);
        const request = requestFactory(store);

        request.onsuccess = () => resolve(request.result);
        request.onerror = () =>
            reject(
                request.error ??
                    new Error("A contacts icon storage request failed."),
            );
        transaction.onerror = () =>
            reject(
                transaction.error ??
                    new Error("A contacts icon transaction failed."),
            );
    });
}

async function loadStoredContactIcon(contactId: string) {
    if (typeof window === "undefined" || !("indexedDB" in window)) {
        return null;
    }

    const result = await runContactIconStoreRequest("readonly", (store) =>
        store.get(contactId),
    );

    return typeof result === "string" ? result : null;
}

export async function saveStoredContactIcon(contactId: string, dataUrl: string) {
    if (typeof window === "undefined" || !("indexedDB" in window)) {
        return;
    }

    await runContactIconStoreRequest("readwrite", (store) =>
        store.put(dataUrl, contactId),
    );
}

export async function deleteStoredContactIcon(contactId: string) {
    if (typeof window === "undefined" || !("indexedDB" in window)) {
        return;
    }

    await runContactIconStoreRequest("readwrite", (store) =>
        store.delete(contactId),
    );
}

export async function loadContactsFromStorage() {
    if (typeof window === "undefined") {
        return {
            contacts: [createDefaultContact()],
            selectedContactId: DEFAULT_CONTACT_ID,
        };
    }

    const rawPayload = window.localStorage.getItem(CONTACTS_STORAGE_KEY);
    if (!rawPayload) {
        return {
            contacts: [createDefaultContact()],
            selectedContactId: DEFAULT_CONTACT_ID,
        };
    }

    try {
        const parsed = JSON.parse(rawPayload) as {
            contacts?: unknown;
            selectedContactId?: unknown;
        };
        const storedContacts = Array.isArray(parsed.contacts)
            ? parsed.contacts
                  .map((contact) => normalizeStoredContactProfile(contact))
                  .filter(
                      (contact): contact is StoredContactProfile =>
                          contact !== null,
                  )
            : [];

        if (storedContacts.length === 0) {
            return {
                contacts: [createDefaultContact()],
                selectedContactId: DEFAULT_CONTACT_ID,
            };
        }

        const contactsWithIcons = await Promise.all(
            storedContacts.map(async (contact) => {
                const iconDataUrl = contact.hasCustomIcon
                    ? await loadStoredContactIcon(contact.id)
                    : null;

                return {
                    ...contact,
                    hasCustomIcon: Boolean(iconDataUrl),
                    iconDataUrl,
                };
            }),
        );
        const activeContactId =
            typeof parsed.selectedContactId === "string" &&
            contactsWithIcons.some(
                (contact) => contact.id === parsed.selectedContactId,
            )
                ? parsed.selectedContactId
                : contactsWithIcons[0].id;

        return {
            contacts: contactsWithIcons,
            selectedContactId: activeContactId,
        };
    } catch (err) {
        console.error("Failed to load stored contacts:", err);
        return {
            contacts: [createDefaultContact()],
            selectedContactId: DEFAULT_CONTACT_ID,
        };
    }
}
