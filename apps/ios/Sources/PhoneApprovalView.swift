// Show only offered decisions and keep every answer attached to the original live card.
import SwiftUI

struct PhoneApprovalView: View {
    let host: PhoneHostModel
    let card: PhoneApproval
    @Environment(\.openURL) private var openURL

    private var enabled: Bool {
        host.connected && host.transcript?.currentSnapshot == true && card.active && !card.submitted && !host.controlSubmitting
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Label(card.caption.title, systemImage: "checkmark.shield")
                .font(PhoneTheme.prose(13, weight: .semibold)).foregroundStyle(PhoneTheme.yellow)
            if !card.caption.details.isEmpty {
                Text(card.caption.details.joined(separator: "\n")).font(PhoneTheme.mono(12.5)).textSelection(.enabled)
                    .padding(10).frame(maxWidth: .infinity, alignment: .leading)
                    .background(PhoneTheme.crust).clipShape(RoundedRectangle(cornerRadius: 8))
            }
            if let error = card.caption.inputError { Text(error).foregroundStyle(PhoneTheme.red) }
            input
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
                    .disabled(!enabled)
                    .accessibilityIdentifier("approval.\(card.id).\(decision.label)")
                }
            }
            if card.submitted { Text("Answer submitted; awaiting the backend").font(PhoneTheme.mono(11.5)) }
        }
        .padding(14).background(PhoneTheme.approval).clipShape(RoundedRectangle(cornerRadius: 14))
        .overlay { RoundedRectangle(cornerRadius: 14).stroke(PhoneTheme.yellow.opacity(0.4), lineWidth: 1) }
    }

    private var choices: [PhoneDecision] {
        if card.input == nil { return card.choices }
        if case .browser? = card.input { return card.choices }
        return card.choices.filter { $0 == .deny || $0 == .cancel }
    }

    @ViewBuilder private var input: some View {
        if let input = card.input {
            switch input {
            case .fields(let questions):
                PhoneQuestionsView(host: host, questions: questions, enabled: enabled)
            case .browser(let address):
                Text("Open the link to complete the browser step, then return here and choose whether to allow it.")
                    .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.subtext0)
                if let url = URL(string: address), ["https", "http"].contains(url.scheme?.lowercased() ?? ""), url.host != nil {
                    Button { openURL(url) } label: { Label("Open \(url.host ?? "browser")", systemImage: "safari").frame(minHeight: 44) }
                        .disabled(!enabled).accessibilityIdentifier("approval.browser")
                } else { Text("This browser link is unavailable").foregroundStyle(PhoneTheme.red) }
            case .form:
                Text("This form needs its native editor. Answer it in the terminal for now.")
                    .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.subtext0)
            case .unsupported(let reason):
                Text(reason).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.yellow)
            }
        }
    }
}

extension PhoneDecision {
    var label: String {
        switch self { case .once: "Allow once"; case .session: "Allow for session"; case .deny: "Deny"; case .cancel: "Cancel" }
    }
}
