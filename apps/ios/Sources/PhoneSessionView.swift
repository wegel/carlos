// Draw the fixed header/composer around real, lazily fetched transcript rows.
import SwiftUI

struct PhoneSessionView: View {
    @Bindable var host: PhoneHostModel
    let sessions: () -> Void
    let connection: () -> Void

    private var info: PhoneSession? { host.transcript?.info ?? host.catalog.first { $0.id == host.selected } }
    private var isStop: Bool { info?.activity.phase != .idle && info != nil && host.draft.isEmpty }

    var body: some View {
        VStack(spacing: 0) {
            header
            if let error = host.error {
                HStack {
                    Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red)
                    Spacer()
                    Button { host.error = nil } label: { Image(systemName: "xmark").frame(width: 44, height: 44) }
                        .accessibilityLabel("Dismiss error")
                }.padding(.leading, 16).background(PhoneTheme.base)
            }
            PhoneTranscriptView(host: host)
            if info?.activity.availability == .closed || info?.activity.availability == .unavailable {
                Button("Reopen session") { host.reopen() }.frame(minHeight: 44).disabled(!host.connected || host.controlSubmitting)
            }
            HStack {
                Text(host.connected ? (info?.status ?? "choose a session") : "disconnected")
                Spacer()
                if let model = info?.usage.model { Text(model).lineLimit(1) }
            }
            .font(PhoneTheme.mono(11.5)).foregroundStyle(PhoneTheme.overlay2)
            .padding(.horizontal, 16).frame(height: 28)
            .overlay(alignment: .top) { Rectangle().fill(PhoneTheme.surface0).frame(height: 1) }
            composer
        }
        .background(PhoneTheme.mantle).foregroundStyle(PhoneTheme.text)
    }

    private var header: some View {
        HStack(spacing: 0) {
            Button(action: sessions) { Image(systemName: "list.bullet").frame(width: 44, height: 44) }
                .accessibilityLabel("Sessions").accessibilityIdentifier("header.sessions")
            Button(action: sessions) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(info?.title ?? "Carlos").font(PhoneTheme.prose(16, weight: .semibold)).lineLimit(1)
                    HStack(spacing: 6) {
                        Circle().fill(host.connected ? (info?.statusColor ?? PhoneTheme.overlay1) : PhoneTheme.red)
                            .frame(width: 7, height: 7)
                        Text(info?.subtitle ?? host.configuration.name).font(PhoneTheme.mono(11)).lineLimit(1)
                            .foregroundStyle(PhoneTheme.overlay2)
                    }
                }.frame(maxWidth: .infinity, minHeight: 44, alignment: .leading)
            }.buttonStyle(.plain).accessibilityIdentifier("header.title")
            Button(action: connection) { Image(systemName: "network").frame(width: 44, height: 44) }
                .accessibilityLabel("Connection").accessibilityIdentifier("header.connection")
        }
        .overlay(alignment: .bottom) { Rectangle().fill(PhoneTheme.surface0).frame(height: 1) }
    }

    private var composer: some View {
        HStack(alignment: .bottom, spacing: 8) {
            TextField("", text: $host.draft, axis: .vertical)
                .font(PhoneTheme.prose()).lineLimit(1...5).padding(.horizontal, 14).padding(.vertical, 12)
                .frame(minHeight: 44).background(PhoneTheme.base)
                .clipShape(RoundedRectangle(cornerRadius: 22))
                .overlay { RoundedRectangle(cornerRadius: 22).stroke(PhoneTheme.surface0, lineWidth: 1) }
                .overlay(alignment: .trailing) {
                    if host.draft.isEmpty {
                        Text(info?.title ?? "").font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.overlay2)
                            .lineLimit(1).padding(.trailing, 14).allowsHitTesting(false)
                    }
                }
                .disabled(host.submitting).accessibilityLabel("Prompt").accessibilityIdentifier("composer.draft")
            Button { if isStop { host.interrupt() } else { host.send() } } label: {
                Image(systemName: isStop ? "stop.fill" : "arrow.up").font(.system(size: 20, weight: .semibold))
                    .frame(width: 44, height: 44).background(isStop ? PhoneTheme.text : PhoneTheme.blue)
                    .foregroundStyle(PhoneTheme.crust).clipShape(Circle())
            }
            .disabled(!host.connected || host.submitting || host.controlSubmitting ||
                (isStop ? host.transcript?.currentSnapshot != true : host.transcript?.ready != true || host.draft.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty))
            .accessibilityLabel(isStop ? "Stop" : "Send").accessibilityIdentifier(isStop ? "composer.stop" : "composer.send")
        }
        .padding(.horizontal, 12).padding(.vertical, 8)
    }
}
