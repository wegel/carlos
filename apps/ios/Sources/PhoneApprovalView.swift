// Show only offered native decisions; keep structured requests honest until their editor is available.
import SwiftUI

struct PhoneApprovalView: View {
    let host: PhoneHostModel
    let card: PhoneApproval

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label(card.caption.title, systemImage: "checkmark.shield")
                .font(PhoneTheme.prose(13, weight: .semibold)).foregroundStyle(PhoneTheme.yellow)
            ForEach(Array(card.caption.details.enumerated()), id: \.offset) { _, detail in
                Text(detail).font(PhoneTheme.mono(12.5)).textSelection(.enabled)
                    .padding(10).frame(maxWidth: .infinity, alignment: .leading)
                    .background(PhoneTheme.crust).clipShape(RoundedRectangle(cornerRadius: 8))
            }
            if let error = card.caption.inputError { Text(error).foregroundStyle(PhoneTheme.red) }
            if card.requiresInput {
                Text("This request needs structured input. Answer it in the terminal.")
                    .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.subtext0)
            }
            HStack(spacing: 8) {
                ForEach(choices, id: \.self) { decision in
                    Button { host.approve(card, decision: decision) } label: {
                        Text(decision.label).font(PhoneTheme.prose(13, weight: .semibold))
                            .frame(maxWidth: .infinity, minHeight: 44)
                            .background(decision == .once ? PhoneTheme.yellow : .clear)
                            .foregroundStyle(decision == .once ? PhoneTheme.crust : PhoneTheme.text)
                            .clipShape(RoundedRectangle(cornerRadius: 12))
                            .overlay { RoundedRectangle(cornerRadius: 12).stroke(PhoneTheme.surface1, lineWidth: 1) }
                    }
                    .disabled(!host.connected || host.transcript?.currentSnapshot != true || !card.active || card.submitted || host.controlSubmitting)
                    .accessibilityIdentifier("approval.\(card.id).\(decision.label)")
                }
            }
            if card.submitted { Text("Answer submitted; awaiting the backend").font(PhoneTheme.mono(11.5)) }
        }
        .padding(14).background(PhoneTheme.approval).clipShape(RoundedRectangle(cornerRadius: 14))
        .overlay { RoundedRectangle(cornerRadius: 14).stroke(PhoneTheme.yellow.opacity(0.4), lineWidth: 1) }
    }

    private var choices: [PhoneDecision] {
        card.choices.filter { !card.requiresInput || $0 == .deny || $0 == .cancel }
    }
}

extension PhoneDecision {
    var label: String {
        switch self { case .once: "Allow once"; case .session: "Allow for session"; case .deny: "Deny"; case .cancel: "Cancel" }
    }
}
