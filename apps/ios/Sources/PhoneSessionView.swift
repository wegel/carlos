// Draw the fixed header/composer around real, lazily fetched transcript rows.
import SwiftUI

struct PhoneSessionView: View {
    @Bindable var host: PhoneHostModel
    let sessions: () -> Void
    let settings: () -> Void
    @State private var questionDrafts: [String: PhoneQuestionDraft] = [:]
    @State private var editing: PhoneQuestionDraft?

    private var info: PhoneSession? { host.transcript?.info ?? host.catalog.first { $0.id == host.selected } }
    private var statusColor: Color {
        if case .disconnected = host.link { return PhoneTheme.red }
        return host.connected && host.transcript?.currentSnapshot == true ? (info?.statusColor ?? PhoneTheme.overlay1) : PhoneTheme.overlay1
    }
    private var isStop: Bool { info?.activity.phase != .idle && info != nil && host.draft.isEmpty }
    private var sendDisabled: Bool {
        !host.connected || host.submitting || host.controlSubmitting ||
            (isStop ? host.transcript?.currentSnapshot != true || info?.activity.canInterrupt != true : host.transcript?.ready != true ||
                host.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
    }

    var body: some View {
        VStack(spacing: 0) {
            header.fixedSessionBar()
            if let error = host.error {
                HStack {
                    Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red)
                        .accessibilityIdentifier("session.error")
                    Spacer()
                    Button { host.error = nil } label: { Image(systemName: "xmark").frame(width: 44, height: 44) }
                        .accessibilityLabel("Dismiss error")
                }.padding(.leading, 16).background(PhoneTheme.base).fixedSessionBar()
            }
            PhoneTranscriptView(host: host, openQuestions: openQuestions)
            if info?.activity.availability == .closed || info?.activity.availability == .unavailable {
                Button("Reopen session") { host.reopen() }.frame(minHeight: 44)
                    .disabled(!host.connected || host.controlSubmitting).fixedSessionBar()
            }
            PhoneActivityLine(info: info, link: host.link, currentSnapshot: host.transcript?.currentSnapshot == true).fixedSessionBar()
            composer.fixedSessionBar()
        }
        .background(PhoneTheme.mantle).foregroundStyle(PhoneTheme.text)
        .sheet(item: $editing) { draft in
            PhoneQuestionsView(host: host, card: draft.card, draft: draft)
        }
        .onChange(of: host.selected) { _, _ in editing = nil; questionDrafts.removeAll() }
        .onChange(of: host.approvals.map(\.id)) { _, active in
            questionDrafts = questionDrafts.filter { active.contains($0.key) }
        }
    }

    private func openQuestions(card: PhoneApproval, questions: PhoneQuestions) {
        let draft = questionDrafts[card.id] ?? PhoneQuestionDraft(card: card.id, editor: questions)
        questionDrafts[card.id] = draft
        editing = draft
    }

    private var header: some View {
        HStack(spacing: 2) {
            Button(action: sessions) { Image(systemName: "list.bullet").frame(width: 44, height: 44) }
                .accessibilityLabel("Sessions").accessibilityIdentifier("header.sessions")
            Button(action: sessions) {
                VStack(spacing: 3) {
                    Text(info?.title ?? "Carlos").font(PhoneTheme.prose(16, weight: .semibold)).lineLimit(1)
                    HStack(spacing: 6) {
                        Circle().fill(statusColor)
                            .frame(width: 7, height: 7)
                        Text(info?.subtitle ?? host.configuration.name).font(PhoneTheme.mono(11)).lineLimit(1)
                            .foregroundStyle(PhoneTheme.overlay2)
                    }
                }.frame(maxWidth: .infinity, minHeight: 44)
            }.buttonStyle(.plain).accessibilityIdentifier("header.title")
            Button(action: settings) { Image(systemName: "ellipsis").frame(width: 44, height: 44) }
                .accessibilityLabel("Session settings").accessibilityIdentifier("header.settings")
        }
        .padding(.horizontal, 6).padding(.bottom, 6)
        .foregroundStyle(PhoneTheme.subtext1)
        .overlay(alignment: .bottom) { Rectangle().fill(PhoneTheme.surface0).frame(height: 1) }
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 8) {
            TextField("", text: $host.draft, axis: .vertical)
                .font(PhoneTheme.prose()).lineLimit(1...5).padding(.horizontal, 14).padding(.vertical, 12)
                .frame(minHeight: 44).background(PhoneTheme.base)
                .clipShape(RoundedRectangle(cornerRadius: 22))
                .overlay { RoundedRectangle(cornerRadius: 22).stroke(info?.activity.ralph == true ? PhoneTheme.pink.opacity(0.6) : PhoneTheme.surface0, lineWidth: 1) }
                .overlay(alignment: .leading) {
                    if info?.activity.ralph == true { Capsule().fill(PhoneTheme.pink).frame(width: 3).padding(.vertical, 10).padding(.leading, 6) }
                }
                .overlay(alignment: .trailing) {
                    if host.draft.isEmpty {
                        GeometryReader { geometry in
                            Text(info?.title ?? "").font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.overlay2)
                                .lineLimit(1).frame(maxWidth: geometry.size.width * 0.7, alignment: .trailing)
                                .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .trailing).padding(.trailing, 14)
                        }.allowsHitTesting(false)
                    }
                }
                .disabled(host.submitting).accessibilityLabel("Prompt").accessibilityIdentifier("composer.draft")
            Button { if isStop { host.interrupt() } else { host.send() } } label: {
                Image(systemName: isStop ? "stop.fill" : "arrow.up").font(.system(size: 20, weight: .semibold))
                    .frame(width: 44, height: 44).background(sendDisabled ? PhoneTheme.surface0 : isStop ? PhoneTheme.text : PhoneTheme.blue)
                    .foregroundStyle(sendDisabled ? PhoneTheme.overlay1 : PhoneTheme.crust).clipShape(Circle())
            }
            .disabled(sendDisabled)
            .accessibilityLabel(isStop ? "Stop" : "Send").accessibilityIdentifier(isStop ? "composer.stop" : "composer.send")
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
    }
}

private extension View {
    func fixedSessionBar() -> some View { background(PhoneTheme.mantle).zIndex(1) }
}
