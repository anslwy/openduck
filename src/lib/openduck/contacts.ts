// Helpers for contact metadata, contact icon storage, and import/export related browser-side utilities.
import {
  BUILT_IN_VOICE_CALL_PROMPT,
  CONTACTS_STORAGE_KEY,
  CONTACT_ICONS_DB_NAME,
  CONTACT_ICONS_STORE_NAME,
  DEFAULT_CONTACT_PROMPT,
  DEFAULT_CONTACT_ID,
} from "./config";
import type {
  ContactGender,
  ContactProfile,
  StoredContactProfile,
  StoredContactsPayload,
} from "./types";

let contactIconsDbPromise: Promise<IDBDatabase> | null = null;

const BUILT_IN_VOICE_CALL_PROMPT_FRAGMENTS = [
  BUILT_IN_VOICE_CALL_PROMPT,
  "You are in a live voice call right now.",
  "Reply like a natural spoken conversation.",
  "Use plain short sentences only.",
  "Use plain sentences only.",
  "Never use markdown, bullets, headings, numbered lists, code fences, tables, emojis, or stage directions.",
  "Keep responses concise, direct, and easy to speak aloud.",
  "Try to ask follow-up questions more often.",
  "Make sure your first sentence in every reply is short.",
  "Make sure your first sentence in your every reply is short.",
  "Answer directly.",
];

const LEGACY_DEFAULT_CONTACT_PROMPTS = new Set([
  DEFAULT_CONTACT_PROMPT,
  "You are a friendly voice AI assistant from OpenDuck.",
]);

type ImportedContactProfile = {
  version?: number;
  name?: unknown;
  prompt?: unknown;
  iconDataUrl?: unknown;
  gender?: unknown;
  refAudio?: unknown;
  refText?: unknown;
};

const builtInContactModules = import.meta.glob(
  "../../../characters/*.openduck",
  {
    eager: true,
    query: "?raw",
    import: "default",
  },
);

export function normalizeContactGender(value: unknown): ContactGender | null {
  if (typeof value !== "string") {
    return null;
  }

  const normalized = value.trim().toLowerCase();
  if (normalized === "male" || normalized === "female") {
    return normalized;
  }

  return null;
}

function stripBuiltInVoiceCallPrompt(prompt: string) {
  let scenarioPrompt = prompt.trim().replace(/\s+/g, " ");

  for (const fragment of BUILT_IN_VOICE_CALL_PROMPT_FRAGMENTS) {
    scenarioPrompt = scenarioPrompt.replaceAll(fragment, "");
  }

  return scenarioPrompt.replace(/\s+/g, " ").trim();
}

export function normalizeContactPrompt(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }

  const trimmedValue = value.trim();
  if (!trimmedValue) {
    return null;
  }

  const scenarioPrompt = stripBuiltInVoiceCallPrompt(trimmedValue);
  return scenarioPrompt || DEFAULT_CONTACT_PROMPT;
}

function normalizeImportedContactProfile(
  value: unknown,
): Omit<ContactProfile, "id" | "hasCustomIcon"> | null {
  if (!value || typeof value !== "object") {
    return null;
  }

  const record = value as ImportedContactProfile;
  const name =
    typeof record.name === "string" && record.name.trim().length > 0
      ? record.name
      : null;
  const prompt = normalizeContactPrompt(record.prompt);

  if (!name || !prompt) {
    return null;
  }

  const iconDataUrl =
    typeof record.iconDataUrl === "string" &&
    record.iconDataUrl.startsWith("data:image/")
      ? record.iconDataUrl
      : null;

  return {
    name,
    prompt,
    iconDataUrl,
    gender: normalizeContactGender(record.gender),
    refAudio: typeof record.refAudio === "string" ? record.refAudio : null,
    refText: typeof record.refText === "string" ? record.refText : null,
  };
}

function createDefaultContactList(): ContactProfile[] {
  const builtInContacts = Object.entries(builtInContactModules)
    .sort(([leftPath], [rightPath]) => {
      const leftIsOpenDuck = leftPath.endsWith("/openduck.openduck");
      const rightIsOpenDuck = rightPath.endsWith("/openduck.openduck");

      if (leftIsOpenDuck && !rightIsOpenDuck) {
        return -1;
      }

      if (!leftIsOpenDuck && rightIsOpenDuck) {
        return 1;
      }

      return leftPath.localeCompare(rightPath);
    })
    .map(([path, rawText]) => {
      if (typeof rawText !== "string") {
        return null;
      }

      try {
        const parsed = JSON.parse(rawText);
        const importedContact = normalizeImportedContactProfile(parsed);
        if (!importedContact) {
          return null;
        }

        const id = path.endsWith("/openduck.openduck")
          ? DEFAULT_CONTACT_ID
          : createContactId();

        return {
          id,
          ...importedContact,
          hasCustomIcon: Boolean(importedContact.iconDataUrl),
        };
      } catch (err) {
        console.error(`Failed to parse built-in contact ${path}:`, err);
        return null;
      }
    })
    .filter((contact): contact is ContactProfile => contact !== null);

  return builtInContacts.length > 0
    ? builtInContacts
    : [createDefaultContact()];
}

function createContactSignature(
  contact: Pick<StoredContactProfile, "name" | "prompt">,
) {
  return `${contact.name.trim().toLowerCase()}\n${contact.prompt.trim()}`;
}

function createBuiltInContactIndex(contactList: ContactProfile[]) {
  const index = new Map<string, ContactProfile>();

  for (const contact of contactList) {
    index.set(createContactSignature(contact), contact);
  }

  return index;
}

function shouldReplaceWithBuiltInContacts(
  storedContacts: StoredContactProfile[],
): boolean {
  if (storedContacts.length === 0) {
    return true;
  }

  if (storedContacts.length !== 1) {
    return false;
  }

  const [contact] = storedContacts;
  return (
    contact.name.trim() === "OpenDuck" &&
    LEGACY_DEFAULT_CONTACT_PROMPTS.has(contact.prompt) &&
    !contact.hasCustomIcon &&
    contact.refAudio === null &&
    contact.refText === null
  );
}

export function createDefaultContact(): ContactProfile {
  return {
    id: DEFAULT_CONTACT_ID,
    name: "OpenDuck",
    prompt: DEFAULT_CONTACT_PROMPT,
    hasCustomIcon: false,
    iconDataUrl: null,
    gender: "female",
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
      gender: contact.gender,
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
    typeof record.id === "string" && record.id.trim() ? record.id.trim() : null;
  if (!id) {
    return null;
  }

  return {
    id,
    name: typeof record.name === "string" ? record.name : "",
    prompt: normalizeContactPrompt(record.prompt) ?? DEFAULT_CONTACT_PROMPT,
    hasCustomIcon: Boolean(record.hasCustomIcon),
    gender: normalizeContactGender(record.gender),
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
        request.error ?? new Error("A contacts icon storage request failed."),
      );
    transaction.onerror = () =>
      reject(
        transaction.error ?? new Error("A contacts icon transaction failed."),
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

export async function saveStoredContactIcon(
  contactId: string,
  dataUrl: string,
) {
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
  const defaultContacts = createDefaultContactList();
  const builtInContactsBySignature = createBuiltInContactIndex(defaultContacts);
  const defaultSelectedContactId = defaultContacts[0]?.id ?? DEFAULT_CONTACT_ID;

  if (typeof window === "undefined") {
    return {
      contacts: defaultContacts,
      selectedContactId: defaultSelectedContactId,
    };
  }

  const rawPayload = window.localStorage.getItem(CONTACTS_STORAGE_KEY);
  if (!rawPayload) {
    return {
      contacts: defaultContacts,
      selectedContactId: defaultSelectedContactId,
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
            (contact): contact is StoredContactProfile => contact !== null,
          )
      : [];

    if (shouldReplaceWithBuiltInContacts(storedContacts)) {
      return {
        contacts: defaultContacts,
        selectedContactId: defaultSelectedContactId,
      };
    }

    const contactsWithIcons = await Promise.all(
      storedContacts.map(async (contact) => {
        const builtInContact = builtInContactsBySignature.get(
          createContactSignature(contact),
        );
        const storedIconDataUrl = contact.hasCustomIcon
          ? await loadStoredContactIcon(contact.id)
          : null;
        const iconDataUrl =
          storedIconDataUrl ?? builtInContact?.iconDataUrl ?? null;

        return {
          ...contact,
          gender: contact.gender ?? builtInContact?.gender ?? null,
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
      contacts: defaultContacts,
      selectedContactId: defaultSelectedContactId,
    };
  }
}
