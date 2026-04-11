// Small formatting helpers for user-visible status text on the home page.
export function normalizeErrorMessage(error: unknown) {
    let message = String(error).trim();

    if (message.startsWith('"') && message.endsWith('"')) {
        try {
            const parsed = JSON.parse(message);
            if (typeof parsed === "string") {
                message = parsed;
            }
        } catch {
            // Keep the raw string when it is not valid JSON.
        }
    }

    if (message.toLowerCase().startsWith("error: ")) {
        message = message.slice(7).trim();
    }

    return message;
}

export function normalizeDownloadErrorMessage(error: unknown) {
    const message = normalizeErrorMessage(error);

    if (message.toLowerCase().startsWith("download failed:")) {
        return message.slice("download failed:".length).trim();
    }

    return message;
}

export function formatDownloadPercent(progress: number) {
    if (progress >= 99.95) {
        return "100%";
    }
    if (progress < 1) {
        return `${progress.toFixed(2)}%`;
    }
    if (progress < 10) {
        return `${progress.toFixed(1)}%`;
    }
    return `${Math.round(progress)}%`;
}

export function formatMemoryUsage(bytes: number) {
    const gib = 1024 ** 3;
    const mib = 1024 ** 2;
    const kib = 1024;

    if (bytes >= gib) {
        return `${(bytes / gib).toFixed(2)} GB`;
    }
    if (bytes >= mib) {
        return `${Math.round(bytes / mib)} MB`;
    }
    if (bytes >= kib) {
        return `${Math.round(bytes / kib)} KB`;
    }
    return `${bytes} B`;
}
