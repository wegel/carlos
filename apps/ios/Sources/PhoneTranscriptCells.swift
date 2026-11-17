// Keep native cell identity separate from the source rows and their SwiftUI appearance.
import SwiftUI

struct PhoneTranscriptCellItem: Identifiable {
    enum Content {
        case history
        case empty(String)
        case entry(PhoneTranscriptItem)
        case approval(PhoneApproval)
        case recovery(PhoneOperation)
        case bottom
    }

    let id: String
    let content: Content

    @MainActor
    static func read(host: PhoneHostModel, width: CGFloat, preferences: PhonePreferences) -> [Self] {
        var result: [Self] = []
        if host.transcript?.hasOlder == true { result.append(Self(id: "transcript.history", content: .history)) }
        if host.selected == nil {
            result.append(Self(id: "transcript.empty", content: .empty("Choose a session")))
        } else if host.orderedEntries.isEmpty {
            result.append(Self(id: "transcript.empty", content: .empty(host.transcript?.ready == true ? "New session" : "Loading session…")))
        }
        result += PhoneTranscriptItem.read(host: host, width: max(0, width - 32), layout: preferences.layout, tools: preferences.toolMode)
            .map { Self(id: $0.id, content: .entry($0)) }
        result += host.approvals.map { Self(id: "approval/" + $0.id, content: .approval($0)) }
        result += host.operationOrder.compactMap { key in
            guard let operation = host.operations[key], operation.session == host.selected, operation.text != nil else { return nil }
            switch operation.state {
            case .rejected, .uncertain: return Self(id: "recovery/" + key, content: .recovery(operation))
            default: return nil
            }
        }
        result.append(Self(id: "transcript.bottom", content: .bottom))
        return result
    }
}

struct PhoneTranscriptCellContent: View {
    let host: PhoneHostModel
    let item: PhoneTranscriptCellItem
    let loadHistory: () -> Void
    let openQuestions: (PhoneApproval, PhoneQuestions) -> Void

    var body: some View {
        Group {
            switch item.content {
            case .history: PhoneHistoryButton(history: host.history, load: loadHistory)
            case .empty(let message):
                Text(message).font(PhoneTheme.prose(15, weight: .semibold))
                    .frame(maxWidth: .infinity).padding(.vertical, 40)
            case .entry(let row): PhoneTranscriptItemView(host: host, item: row).padding(.horizontal, 16)
            case .approval(let card): PhoneApprovalView(host: host, card: card, openQuestions: openQuestions).padding(.horizontal, 16).padding(.top, 18)
            case .recovery(let operation): PhoneRecoveryView(host: host, operation: operation).padding(.horizontal, 16).padding(.top, 18)
            case .bottom: Color.clear.frame(height: 1).accessibilityHidden(true)
            }
        }.frame(maxWidth: .infinity, alignment: .leading)
    }
}
