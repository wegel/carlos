// Draw the fixed header/composer around real, lazily fetched transcript rows.
import SwiftUI
import UIKit

struct PhoneSessionView: View {
    @Bindable var host: PhoneHostModel
    let sessions: () -> Void
    let settings: () -> Void
    @State private var questionDrafts: [String: PhoneQuestionDraft] = [:]
    @State private var editing: PhoneQuestionDraft?
    @State private var speech = PhoneDictation()
    @Environment(\.scenePhase) private var scenePhase

    private var info: PhoneSession? { host.transcript?.info ?? host.catalog.first { $0.id == host.selected } }
    private var availability: PhoneAvailability? {
        host.catalog.first { $0.id == host.selected }?.activity.availability ?? info?.activity.availability
    }
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
            PhoneSpeechNotice(speech: speech).fixedSessionBar()
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
            if availability == .closed || availability == .unavailable {
                Button("Reopen session") { host.reopen() }.frame(minHeight: 44)
                    .disabled(!host.connected || host.controlSubmitting).fixedSessionBar()
            }
            PhoneActivityLine(info: info, link: host.link, currentSnapshot: host.transcript?.currentSnapshot == true, dictating: speech.active).fixedSessionBar()
            composer.fixedSessionBar()
        }
        .background(PhoneTheme.mantle).foregroundStyle(PhoneTheme.text)
        .sheet(item: $editing) { draft in
            PhoneQuestionsView(host: host, card: draft.card, draft: draft)
        }
        .sheet(item: $host.promptEditor) { editor in
            PhonePromptEditView(host: host, editor: editor)
        }
        .onChange(of: host.selected) { _, _ in speech.cancel(); editing = nil; questionDrafts.removeAll() }
        .onChange(of: host.promptEditor?.id) { _, value in
            if value != nil {
                speech.cancel(keepText: true)
                UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
            }
        }
        .onDisappear {
            speech.cancel(keepText: true)
        }
        .onChange(of: scenePhase) { _, phase in
            // Permission prompts make the app inactive; only backgrounding cancels that request.
            if phase == .background || (phase != .active && speech.phase != .authorizing) {
                speech.cancel(keepText: true)
            }
        }
        .onChange(of: host.approvals.map(\.id)) { _, active in
            questionDrafts = questionDrafts.filter { active.contains($0.key) }
        }
    }

    private func openQuestions(card: PhoneApproval, questions: PhoneQuestions) {
        speech.cancel(keepText: true)
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        let draft = questionDrafts[card.id] ?? PhoneQuestionDraft(card: card.id, editor: questions)
        questionDrafts[card.id] = draft
        editing = draft
    }

    private func openSheet(_ show: () -> Void) {
        speech.cancel(keepText: true)
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        show()
    }

    private var header: some View {
        HStack(spacing: 2) {
            Button { openSheet(sessions) } label: { Image(systemName: "list.bullet").frame(width: 44, height: 44) }
                .accessibilityLabel("Sessions").accessibilityIdentifier("header.sessions")
            Button { openSheet(sessions) } label: {
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
            Button { openSheet(settings) } label: { Image(systemName: "ellipsis").frame(width: 44, height: 44) }
                .accessibilityLabel("Session settings").accessibilityIdentifier("header.settings")
        }
        .padding(.horizontal, 6).padding(.bottom, 6)
        .foregroundStyle(PhoneTheme.subtext1)
        .overlay(alignment: .bottom) { Rectangle().fill(PhoneTheme.surface0).frame(height: 1) }
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 4) {
            PhonePromptInput(text: Binding(get: { host.draft }, set: { host.editDraft($0) }), replacementVersion: host.draftVersion) {
                if !sendDisabled && !isStop { speech.cancel(keepText: true); host.send() }
            }
                .frame(minHeight: 44)
                .accessibilityLabel("Prompt").accessibilityIdentifier("composer.draft")
            PhoneMicrophoneButton(speech: speech) {
                let session = host.selected
                speech.start { [weak host] text in
                    guard let host, host.selected == session else { return }
                    host.draft += host.draft.isEmpty || host.draft.last?.isWhitespace == true ? text : " " + text
                }
            }.disabled(host.selected == nil || host.submitting)
            if info?.activity.canInterrupt == true && !host.draft.isEmpty {
                Menu {
                    Picker("Send while running", selection: $host.sendMode) {
                        Text("Queue for the next turn").tag(PhoneSendMode.queue)
                        Text("Steer the current response").tag(PhoneSendMode.steer)
                    }
                } label: {
                    HStack(spacing: 3) {
                        Text(host.sendMode == .steer ? "Steer" : "Queue")
                        Image(systemName: "chevron.down").font(.system(size: 8, weight: .semibold))
                    }
                    .font(PhoneTheme.mono(11)).foregroundStyle(PhoneTheme.subtext1)
                    .frame(minWidth: 44, minHeight: 44)
                }
                .disabled(host.submitting).accessibilityLabel("Send mode")
                .accessibilityValue(host.sendMode == .steer ? "Steer" : "Queue")
                .accessibilityIdentifier("composer.mode")
            }
            Button {
                if isStop { host.interrupt() } else { speech.cancel(keepText: true); host.send() }
            } label: {
                Image(systemName: isStop ? "stop.fill" : "arrow.up").font(.system(size: 20, weight: .semibold))
                    .frame(width: 44, height: 44).background(sendDisabled ? PhoneTheme.surface0 : isStop ? PhoneTheme.text : PhoneTheme.blue)
                    .foregroundStyle(sendDisabled ? PhoneTheme.overlay1 : PhoneTheme.crust).clipShape(Circle())
            }
            .disabled(sendDisabled)
            .accessibilityLabel(isStop ? "Stop" : "Send").accessibilityIdentifier(isStop ? "composer.stop" : "composer.send")
        }
        .padding(.leading, 16).padding(.trailing, 12).padding(.vertical, 8)
    }
}

private extension View {
    func fixedSessionBar() -> some View { background(PhoneTheme.mantle).zIndex(1) }
}
