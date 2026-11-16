// Pull one consistent, bounded UI batch on the transport queue, then apply it in one main turn.
import Foundation

struct PhoneHostUpdate: Sendable {
    var catalog: [PhoneSession]?
    var transcript: PhoneTranscript?
    var order: [String]?
    var rows: [PhoneEntry] = []
    var operations: [PhoneOperation] = []

    static func read(
        client: PhoneClient, batch: PhoneBatch, selected: String?,
        knownIDs: Set<String>, cachedIDs: Set<String>
    ) throws -> PhoneHostUpdate {
        var update = PhoneHostUpdate()
        if batch.catalogChanged { update.catalog = try client.catalog() }
        update.operations = try batch.operations.compactMap { try client.operation(operation: $0) }
        guard let selected else { return update }
        let changes = batch.sessions.filter { $0.session == selected }
        guard !changes.isEmpty else { return update }
        update.transcript = try client.transcript(session: selected)
        let changed = Set(changes.flatMap(\.entries))
        let reorder = changes.contains { $0.window.reset || $0.window.removedFront > 0 || $0.window.removedBack > 0 }
            || !changed.isSubset(of: knownIDs)
        var wanted = changed.intersection(cachedIDs)
        if reorder, let transcript = update.transcript {
            update.order = try order(client: client, session: selected, count: transcript.count)
            wanted.formUnion(update.order?.suffix(64) ?? [])
        }
        let ids = Array(wanted)
        for offset in stride(from: 0, to: ids.count, by: 64) {
            update.rows += try client.entries(session: selected, entries: Array(ids[offset..<min(offset + 64, ids.count)]))
        }
        return update
    }

    private static func order(client: PhoneClient, session: String, count: UInt64) throws -> [String] {
        var ids: [String] = []
        for offset in stride(from: UInt64(0), to: count, by: 256) {
            ids += try client.entryIds(session: session, offset: UInt32(offset), limit: 256)
        }
        return ids
    }
}
