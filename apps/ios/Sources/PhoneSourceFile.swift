// Spool accepted original bytes privately; a complete Copy never depends on a visible text window.
import Foundation

final class PhoneBodyBudget: @unchecked Sendable {
    static let limit = 32 * 1024 * 1024
    private let lock = NSLock()
    private var used = 0

    func reserve(_ bytes: Int) throws {
        try lock.withLock {
            guard bytes <= Self.limit - used else {
                throw PhoneError.Capacity(message: "Open message bodies use 32 MiB; close an expanded body to read more. Accepted text is retained.")
            }
            used += bytes
        }
    }

    func release(_ bytes: Int) { lock.withLock { used -= bytes } }
}

// Appends run on the host's transport queue. Reads open an independent file handle
// and stop at a captured accepted byte count, so a later append cannot alter a copy.
final class PhoneSourceFile: @unchecked Sendable {
    let url: URL
    private let budget: PhoneBodyBudget
    private var accepted: UInt64 = 0

    init(budget: PhoneBodyBudget) throws {
        self.budget = budget
        url = FileManager.default.temporaryDirectory.appendingPathComponent("carlos-source-\(UUID().uuidString)")
        guard FileManager.default.createFile(atPath: url.path, contents: nil, attributes: [
            .posixPermissions: 0o600,
            .protectionKey: FileProtectionType.completeUntilFirstUserAuthentication
        ]) else { throw CocoaError(.fileWriteUnknown) }
    }

    deinit {
        budget.release(Int(accepted))
        try? FileManager.default.removeItem(at: url)
    }

    // Queue-confined; callers never use the file size as a protocol cursor.
    var byteCount: UInt64 { accepted }

    func append(_ text: String, at offset: UInt64) throws {
        guard offset == accepted else { throw PhoneError.StaleRead(message: "Original bytes are not contiguous; accepted text is retained") }
        let data = Data(text.utf8)
        guard !data.isEmpty else { return }
        try budget.reserve(data.count)
        do {
            let handle = try FileHandle(forWritingTo: url)
            defer { try? handle.close() }
            try handle.seek(toOffset: accepted)
            do { try handle.write(contentsOf: data) }
            catch { try? handle.truncate(atOffset: accepted); throw error }
            accepted += UInt64(data.count)
        } catch {
            budget.release(data.count)
            throw error
        }
    }

    func text(upTo end: UInt64) throws -> String {
        guard end <= UInt64(PhoneBodyBudget.limit) else { throw PhoneError.Capacity(message: "Original body exceeds the device cache allowance") }
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        var data = Data()
        data.reserveCapacity(Int(end))
        while UInt64(data.count) < end {
            let remaining = Int(end) - data.count
            guard let part = try handle.read(upToCount: min(65536, remaining)), !part.isEmpty else {
                throw CocoaError(.fileReadCorruptFile)
            }
            data.append(part)
        }
        guard let text = String(data: data, encoding: .utf8) else { throw CocoaError(.fileReadInapplicableStringEncoding) }
        return text
    }
}
