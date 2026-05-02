import {
  BACKGROUNDS_STORAGE_KEY,
  BACKGROUND_IMAGES_DB_NAME,
  BACKGROUND_IMAGES_STORE_NAME,
  BACKGROUND_ASSETS_DB_VERSION,
} from "./config";
import type { Background } from "./types";

let backgroundDbPromise: Promise<IDBDatabase> | null = null;

function openBackgroundDatabase() {
  if (backgroundDbPromise) {
    return backgroundDbPromise;
  }

  backgroundDbPromise = new Promise<IDBDatabase>((resolve, reject) => {
    const request = window.indexedDB.open(
      BACKGROUND_IMAGES_DB_NAME,
      BACKGROUND_ASSETS_DB_VERSION,
    );

    request.onupgradeneeded = () => {
      const database = request.result;
      if (!database.objectStoreNames.contains(BACKGROUND_IMAGES_STORE_NAME)) {
        database.createObjectStore(BACKGROUND_IMAGES_STORE_NAME);
      }
    };

    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(
        request.error ?? new Error("Failed to open the backgrounds database."),
      );
  });

  return backgroundDbPromise;
}

async function runBackgroundStoreRequest(
  mode: IDBTransactionMode,
  requestFactory: (store: IDBObjectStore) => IDBRequest,
) {
  const database = await openBackgroundDatabase();

  return new Promise<unknown>((resolve, reject) => {
    const transaction = database.transaction(
      BACKGROUND_IMAGES_STORE_NAME,
      mode,
    );
    const store = transaction.objectStore(BACKGROUND_IMAGES_STORE_NAME);
    const request = requestFactory(store);

    request.onsuccess = () => resolve(request.result);
    request.onerror = () =>
      reject(
        request.error ?? new Error("A background storage request failed."),
      );
    transaction.onerror = () =>
      reject(
        transaction.error ?? new Error("A background transaction failed."),
      );
  });
}

export async function saveUserBackground(id: string, dataUrl: string) {
  if (typeof window === "undefined" || !("indexedDB" in window)) {
    return;
  }

  await runBackgroundStoreRequest("readwrite", (store) =>
    store.put(dataUrl, id),
  );
}

export async function deleteUserBackground(id: string) {
  if (typeof window === "undefined" || !("indexedDB" in window)) {
    return;
  }

  await runBackgroundStoreRequest("readwrite", (store) => store.delete(id));
}

export async function loadUserBackground(id: string) {
  if (typeof window === "undefined" || !("indexedDB" in window)) {
    return null;
  }

  const result = await runBackgroundStoreRequest("readonly", (store) =>
    store.get(id),
  );

  return typeof result === "string" ? result : null;
}

export async function loadBackgroundsFromStorage(): Promise<{
  backgrounds: Background[];
  selectedBackgroundId: string | null;
}> {
  if (typeof window === "undefined") {
    return {
      backgrounds: [],
      selectedBackgroundId: 0,
    };
  }

  const rawPayload = window.localStorage.getItem(BACKGROUNDS_STORAGE_KEY);
  let selectedBackgroundId: string | null = "";
  let userBackgroundsMetadata: Background[] = [];

  if (rawPayload) {
    try {
      const parsed = JSON.parse(rawPayload);
      selectedBackgroundId =
        parsed.selectedBackgroundId !== undefined
          ? parsed.selectedBackgroundId
          : "";
      userBackgroundsMetadata = parsed.userBackgrounds ?? [];
    } catch (err) {
      console.error("Failed to parse background preferences:", err);
    }
  }

  const backgrounds = [];

  // Load user backgrounds from IndexedDB
  for (const meta of userBackgroundsMetadata) {
    const dataUrl = await loadUserBackground(meta.id);
    if (dataUrl) {
      backgrounds.push({
        ...meta,
        url: dataUrl,
      });
    }
  }

  return {
    backgrounds,
    selectedBackgroundId,
  };
}

export function persistBackgroundPreferences(
  selectedBackgroundId: string | null,
  userBackgrounds: Background[],
) {
  if (typeof window === "undefined") {
    return;
  }

  const payload = {
    version: 1,
    selectedBackgroundId,
    userBackgrounds: userBackgrounds.map((bg) => ({
      id: bg.id,
      name: bg.name,
      isStock: false,
    })),
  };

  window.localStorage.setItem(BACKGROUNDS_STORAGE_KEY, JSON.stringify(payload));
}
