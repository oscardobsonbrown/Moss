import AppKit
import CryptoKit
import Darwin
import Foundation

enum UpdateError: Error {
    case invalidArguments
    case invalidPublicKey
    case invalidSignature
    case cannotSaveKey
}

func bytes(_ path: String) throws -> Data {
    try Data(contentsOf: URL(fileURLWithPath: path), options: .mappedIfSafe)
}

func run(_ args: [String]) throws {
    switch (args.first, args.count) {
    case ("keygen", 2):
        let key = Curve25519.Signing.PrivateKey()
        let fd = open(args[1], O_WRONLY | O_CREAT | O_EXCL, 0o600)
        guard fd >= 0 else { throw UpdateError.cannotSaveKey }
        let file = FileHandle(fileDescriptor: fd, closeOnDealloc: true)
        do {
            try file.write(contentsOf: key.rawRepresentation)
            try file.close()
        } catch {
            try? file.close()
            try? FileManager.default.removeItem(atPath: args[1])
            throw error
        }
        print(key.publicKey.rawRepresentation.base64EncodedString())
    case ("sign", 4):
        let key = try Curve25519.Signing.PrivateKey(rawRepresentation: bytes(args[1]))
        try key.signature(for: bytes(args[2])).write(
            to: URL(fileURLWithPath: args[3]), options: .withoutOverwriting)
    case ("verify", 4):
        guard let keyBytes = Data(base64Encoded: args[1]) else { throw UpdateError.invalidPublicKey }
        let key = try Curve25519.Signing.PublicKey(rawRepresentation: keyBytes)
        guard try key.isValidSignature(bytes(args[2]), for: bytes(args[3])) else {
            throw UpdateError.invalidSignature
        }
    case ("recover", 2):
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        app.activate(ignoringOtherApps: true)
        let alert = NSAlert()
        alert.messageText = "Moss couldn’t install the update"
        alert.informativeText = "You can reopen Moss or download the update from GitHub."
        alert.alertStyle = .warning
        alert.addButton(withTitle: "Open Moss")
        alert.addButton(withTitle: "Open GitHub release")
        if alert.runModal() == .alertSecondButtonReturn {
            if let url = URL(string: args[1]) { NSWorkspace.shared.open(url) }
            exit(2)
        }
    default:
        throw UpdateError.invalidArguments
    }
}

do {
    try run(Array(CommandLine.arguments.dropFirst()))
} catch {
    fputs("Moss update helper: \(error)\n", stderr)
    exit(1)
}
