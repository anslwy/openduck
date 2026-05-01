import AppKit
import Foundation
import SwiftUI
import Translation

private enum HelperCommand: String {
    case status
    case prepare
    case translate
}

private struct HelperRequest {
    let command: HelperCommand
    let sourceLanguage: String
    let targetLanguage: String
    let text: String?
}

private struct HelperResponse: Encodable {
    let ok: Bool
    let status: String?
    let translatedText: String?
    let error: String?
}

private enum HelperTimeoutError: LocalizedError {
    case timedOut(String)

    var errorDescription: String? {
        switch self {
        case .timedOut(let operation):
            return "\(operation) timed out."
        }
    }
}

private func writeResponse(_ response: HelperResponse) {
    let encoder = JSONEncoder()
    encoder.outputFormatting = [.withoutEscapingSlashes]

    do {
        let data = try encoder.encode(response)
        FileHandle.standardOutput.write(data)
        FileHandle.standardOutput.write(Data("\n".utf8))
    } catch {
        let fallback = #"{"ok":false,"error":"Failed to encode helper response."}"# + "\n"
        FileHandle.standardOutput.write(Data(fallback.utf8))
    }
}

private func failAndExit(_ message: String, code: Int32 = 2) -> Never {
    writeResponse(HelperResponse(ok: false, status: nil, translatedText: nil, error: message))
    Darwin.exit(code)
}

private func scheduleProcessTimeout(seconds: Double, operationName: String) {
    DispatchQueue.global().asyncAfter(deadline: .now() + seconds) {
        writeResponse(
            HelperResponse(
                ok: false,
                status: nil,
                translatedText: nil,
                error: "\(operationName) timed out."
            )
        )
        Darwin.exit(1)
    }
}

private func parseRequest(_ arguments: [String]) -> HelperRequest {
    guard arguments.count >= 4 else {
        failAndExit("Usage: apple-translation-helper <status|prepare|translate> <source-language> <target-language> [base64-text]")
    }

    guard let command = HelperCommand(rawValue: arguments[1]) else {
        failAndExit("Unsupported helper command: \(arguments[1])")
    }

    let sourceLanguage = arguments[2].trimmingCharacters(in: .whitespacesAndNewlines)
    let targetLanguage = arguments[3].trimmingCharacters(in: .whitespacesAndNewlines)

    guard !sourceLanguage.isEmpty, !targetLanguage.isEmpty else {
        failAndExit("Source and target languages are required.")
    }

    var text: String?
    if command == .translate {
        guard arguments.count >= 5 else {
            failAndExit("Translate command requires base64 text.")
        }

        guard
            let data = Data(base64Encoded: arguments[4]),
            let decodedText = String(data: data, encoding: .utf8)
        else {
            failAndExit("Translate command received invalid base64 text.")
        }
        text = decodedText
    }

    return HelperRequest(
        command: command,
        sourceLanguage: sourceLanguage,
        targetLanguage: targetLanguage,
        text: text
    )
}

@available(macOS 15.0, *)
private func languageStatusName(_ status: LanguageAvailability.Status) -> String {
    switch status {
    case .installed:
        return "installed"
    case .supported:
        return "supported"
    case .unsupported:
        return "unsupported"
    @unknown default:
        return "unknown"
    }
}

private func withTimeout<T>(
    seconds: UInt64,
    operationName: String,
    operation: @escaping @Sendable () async throws -> T
) async throws -> T {
    try await withThrowingTaskGroup(of: T.self) { group in
        group.addTask {
            try await operation()
        }
        group.addTask {
            try await Task.sleep(nanoseconds: seconds * 1_000_000_000)
            throw HelperTimeoutError.timedOut(operationName)
        }

        let result = try await group.next()!
        group.cancelAll()
        return result
    }
}

@available(macOS 15.0, *)
private struct HelperView: View {
    let request: HelperRequest
    let finish: (HelperResponse, Int32) -> Void

    @State private var configuration: TranslationSession.Configuration?
    @State private var didStartSessionTask = false

    private var sourceLanguage: Locale.Language {
        Locale.Language(identifier: request.sourceLanguage)
    }

    private var targetLanguage: Locale.Language {
        Locale.Language(identifier: request.targetLanguage)
    }

    var body: some View {
        VStack(spacing: 12) {
            ProgressView()
            Text(message)
                .font(.system(size: 13, weight: .medium))
                .multilineTextAlignment(.center)
        }
        .padding(24)
        .frame(width: 380, height: 132)
        .task {
            if request.command != .status {
                configuration = TranslationSession.Configuration(
                    source: sourceLanguage,
                    target: targetLanguage
                )
            }
        }
        .translationTask(configuration) { session in
            guard !didStartSessionTask else {
                return
            }
            didStartSessionTask = true
            await run(session)
        }
    }

    private var message: String {
        switch request.command {
        case .status:
            return "Checking Apple translation languages..."
        case .prepare:
            return "Preparing Apple translation languages..."
        case .translate:
            return "Translating with Apple..."
        }
    }

    private func checkStatus() async {
        do {
            scheduleProcessTimeout(
                seconds: 15,
                operationName: "Apple translation language check"
            )
            let sourceLanguage = sourceLanguage
            let targetLanguage = targetLanguage
            let status = try await withTimeout(
                seconds: 15,
                operationName: "Apple translation language check"
            ) {
                let availability = LanguageAvailability()
                return await availability.status(from: sourceLanguage, to: targetLanguage)
            }
            finish(
                HelperResponse(
                    ok: true,
                    status: languageStatusName(status),
                    translatedText: nil,
                    error: nil
                ),
                0
            )
        } catch {
            finish(
                HelperResponse(
                    ok: false,
                    status: nil,
                    translatedText: nil,
                    error: error.localizedDescription
                ),
                1
            )
        }
    }

    private func run(_ session: TranslationSession) async {
        do {
            switch request.command {
            case .status:
                await checkStatus()
            case .prepare:
                try await session.prepareTranslation()
                await checkStatus()
            case .translate:
                scheduleProcessTimeout(seconds: 60, operationName: "Apple translation")
                let text = request.text ?? ""
                let response = try await withTimeout(
                    seconds: 60,
                    operationName: "Apple translation"
                ) {
                    try await session.translate(text)
                }
                finish(
                    HelperResponse(
                        ok: true,
                        status: nil,
                        translatedText: response.targetText,
                        error: nil
                    ),
                    0
                )
            }
        } catch {
            finish(
                HelperResponse(
                    ok: false,
                    status: nil,
                    translatedText: nil,
                    error: String(describing: error)
                ),
                1
            )
        }
    }
}

@available(macOS 15.0, *)
private func runStatusCommandAndExit(_ request: HelperRequest) async -> Never {
    do {
        scheduleProcessTimeout(
            seconds: 15,
            operationName: "Apple translation language check"
        )
        let sourceLanguage = Locale.Language(identifier: request.sourceLanguage)
        let targetLanguage = Locale.Language(identifier: request.targetLanguage)
        let status = try await withTimeout(
            seconds: 15,
            operationName: "Apple translation language check"
        ) {
            let availability = LanguageAvailability()
            return await availability.status(from: sourceLanguage, to: targetLanguage)
        }
        writeResponse(
            HelperResponse(
                ok: true,
                status: languageStatusName(status),
                translatedText: nil,
                error: nil
            )
        )
        Darwin.exit(0)
    } catch {
        writeResponse(
            HelperResponse(
                ok: false,
                status: nil,
                translatedText: nil,
                error: error.localizedDescription
            )
        )
        Darwin.exit(1)
    }
}

@available(macOS 15.0, *)
private final class TranslationHostAppDelegate: NSObject, NSApplicationDelegate {
    private let request: HelperRequest
    private var window: NSWindow?
    private var didStart = false

    init(request: HelperRequest) {
        self.request = request
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        start()
    }

    func start() {
        guard !didStart else {
            return
        }
        didStart = true

        if request.command == .status {
            Task {
                await checkStatus()
            }
            return
        }

        let isInteractive = request.command == .prepare
        let contentView = HelperView(request: request) { [weak self] response, code in
            self?.finish(response, code)
        }

        let rect = isInteractive
            ? NSRect(x: 0, y: 0, width: 380, height: 132)
            : NSRect(x: -10_000, y: -10_000, width: 1, height: 1)
        let styleMask: NSWindow.StyleMask = isInteractive
            ? [.titled, .closable]
            : [.borderless]
        let window = NSWindow(
            contentRect: rect,
            styleMask: styleMask,
            backing: .buffered,
            defer: false
        )
        window.title = "Apple Translation"
        window.contentView = NSHostingView(rootView: contentView)
        window.isReleasedWhenClosed = false

        self.window = window

        if isInteractive {
            window.center()
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
        } else {
            window.orderFront(nil)
        }
    }

    private func checkStatus() async {
        do {
            let sourceLanguage = Locale.Language(identifier: request.sourceLanguage)
            let targetLanguage = Locale.Language(identifier: request.targetLanguage)
            let status = try await withTimeout(
                seconds: 15,
                operationName: "Apple translation language check"
            ) {
                let availability = LanguageAvailability()
                return await availability.status(from: sourceLanguage, to: targetLanguage)
            }
            finish(
                HelperResponse(
                    ok: true,
                    status: languageStatusName(status),
                    translatedText: nil,
                    error: nil
                ),
                0
            )
        } catch {
            finish(
                HelperResponse(
                    ok: false,
                    status: nil,
                    translatedText: nil,
                    error: error.localizedDescription
                ),
                1
            )
        }
    }

    private func finish(_ response: HelperResponse, _ code: Int32) {
        writeResponse(response)
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.1) {
            NSApplication.shared.terminate(nil)
        }
    }
}

@main
private struct AppleTranslationHelperMain {
    static func main() {
        let request = parseRequest(ProcessInfo.processInfo.arguments)

        guard #available(macOS 15.0, *) else {
            writeResponse(
                HelperResponse(
                    ok: false,
                    status: nil,
                    translatedText: nil,
                    error: "Apple translation requires macOS 15 or later."
                )
            )
            Darwin.exit(1)
        }

        if request.command == .status {
            Task {
                await runStatusCommandAndExit(request)
            }
            RunLoop.main.run()
            Darwin.exit(1)
        }

        let app = NSApplication.shared
        app.setActivationPolicy(request.command == .prepare ? .regular : .accessory)
        let delegate = TranslationHostAppDelegate(request: request)
        app.delegate = delegate
        DispatchQueue.main.async {
            delegate.start()
        }
        app.run()
    }
}
