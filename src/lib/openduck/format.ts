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

export function formatAppUpdateInstallError(error: unknown) {
    const message = normalizeErrorMessage(error);
    const normalized = message.toLowerCase();

    if (
        normalized.includes("cross-device link") ||
        normalized.includes("os error 18")
    ) {
        return [
            "OpenDuck downloaded the update, but it could not replace the current app bundle.",
            `Technical details: ${message}`,
            "Likely cause: the app is running from a different volume than the updater target. On macOS, this usually means OpenDuck is being launched directly from a mounted DMG, Downloads, or another external volume instead of /Applications.",
            "Try this:",
            "1. Quit OpenDuck.",
            "2. Move OpenDuck.app into /Applications.",
            "3. Launch it from /Applications.",
            "4. Retry the update.",
        ].join("\n\n");
    }

    if (
        normalized.includes("permission denied") ||
        normalized.includes("authentication failed")
    ) {
        return [
            "OpenDuck downloaded the update, but it was not allowed to replace the current app bundle.",
            `Technical details: ${message}`,
            "Try moving OpenDuck.app into /Applications, then retry the update. If macOS prompts for permission, approve it.",
        ].join("\n\n");
    }

    return [
        "OpenDuck failed while installing the downloaded update.",
        `Technical details: ${message}`,
    ].join("\n\n");
}

export function createReleaseNotesPreview(
    notes: string | null | undefined,
    maxLength = 150,
) {
    if (!notes) {
        return null;
    }

    const plainText = notes
        .replace(/\r\n?/g, "\n")
        .replace(/!\[([^\]]*)\]\([^)]+\)/g, "$1")
        .replace(/\[([^\]]+)\]\([^)]+\)/g, "$1")
        .replace(/`{1,3}([^`]+)`{1,3}/g, "$1")
        .replace(/(\*\*|__)(.*?)\1/g, "$2")
        .replace(/^#{1,6}\s+/gm, "")
        .replace(/^>\s?/gm, "")
        .replace(/^[\t ]*[-*+]\s+/gm, "")
        .replace(/^[\t ]*\d+\.\s+/gm, "")
        .replace(/^[\t ]*Full Changelog:\s*.*$/gim, "")
        .replace(/[^\S\n]+/g, " ")
        .replace(/\n{3,}/g, "\n\n")
        .trim();

    if (!plainText) {
        return null;
    }

    if (plainText.length <= maxLength) {
        return plainText;
    }

    return `${plainText.slice(0, maxLength).trimEnd()}...`;
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
