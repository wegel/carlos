// Update the local clock without a network request or an idle animation timer.
import SwiftUI

struct PhoneActivityLine: View {
    let info: PhoneSession?
    let connected: Bool
    var dictating = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var active: Bool { connected && info?.activity.phase != .idle && info != nil }

    var body: some View {
        Group {
            if active {
                TimelineView(.periodic(from: .now, by: 1)) { timeline in contents(now: timeline.date) }
            } else { contents(now: .now) }
        }
        .font(PhoneTheme.mono(11.5)).foregroundStyle(PhoneTheme.overlay2)
        .padding(.horizontal, 16).frame(height: 28)
        .overlay(alignment: .top) {
            Rectangle().fill(PhoneTheme.surface0).frame(height: 1)
            if active {
                PhoneScanner(color: info?.activity.ralph == true ? PhoneTheme.pink : PhoneTheme.mauve,
                    tip: info?.activity.ralph == true ? PhoneTheme.ralphScannerTip : PhoneTheme.scannerTip,
                    reduceMotion: reduceMotion)
            }
        }
        .accessibilityIdentifier("session.activity")
    }

    private func contents(now: Date) -> some View {
        HStack(spacing: 8) {
            if info?.activity.ralph == true { Text("RALPH").bold().foregroundStyle(PhoneTheme.pink) }
            Text(status(now: now)).foregroundStyle(dictating ? PhoneTheme.red : info?.activity.phase == .approval ? PhoneTheme.yellow : PhoneTheme.overlay2)
                .fixedSize(horizontal: true, vertical: false)
            Spacer(minLength: 0)
            if let context = info?.usage.context, context.maximum > 0 {
                let percent = Double(context.used) / Double(context.maximum) * 100
                HStack(spacing: 4) {
                    Text("\(count(context.used))/\(count(context.maximum))")
                    Text("\(Int(min(percent, 999)))%").foregroundStyle(percent > 90 ? PhoneTheme.red : percent > 70 ? PhoneTheme.yellow : PhoneTheme.blue)
                }.fixedSize()
            }
            if let model = info?.usage.model { Text(model).lineLimit(1).truncationMode(.middle).layoutPriority(-1) }
            if info?.backend == .codex, let quota = info?.usage.quota {
                quotaLabel(quota, now: now)
            }
        }
    }

    private func status(now: Date) -> String {
        if dictating { return "dictating" }
        guard connected else { return "disconnected" }
        guard let info else { return "choose a session" }
        guard active else { return info.status }
        let label = info.activity.phase == .approval ? "awaiting approval" : "working"
        guard let start = info.activity.startedAt else { return label }
        let seconds = max(0, Int(now.timeIntervalSince1970 - Double(start)))
        return "\(label) · \(seconds / 60):\(String(format: "%02d", seconds % 60))"
    }

    private func count(_ value: UInt64) -> String {
        value < 1000 ? "\(value)" : "\(value / 1000)k"
    }

    private func quotaLabel(_ quota: PhoneQuota, now: Date) -> some View {
        let days = quota.resetsAt.map { max(0, (Double($0) - now.timeIntervalSince1970) / 86400) }
        let needed = days.map { min(100, $0 / 7 * 100) } ?? 0
        let color = quota.remainingPercent <= 0 ? PhoneTheme.red : quota.remainingPercent >= needed ? PhoneTheme.blue : quota.remainingPercent >= needed * 0.75 ? PhoneTheme.yellow : PhoneTheme.red
        return (Text("W\(Int(quota.remainingPercent))%").foregroundColor(color)
            + Text(days.map { String(format: "/%.1fd", $0) } ?? "").foregroundColor(PhoneTheme.overlay2)).fixedSize()
    }
}

private struct PhoneScanner: View {
    let color: Color
    let tip: Color
    let reduceMotion: Bool
    @State private var end = false

    var body: some View {
        GeometryReader { geometry in
            Rectangle().fill(LinearGradient(stops: [
                .init(color: .clear, location: 0), .init(color: color, location: 0.7), .init(color: tip, location: 1)
            ], startPoint: .leading, endPoint: .trailing))
                .frame(width: 110, height: 2)
                .offset(x: reduceMotion ? 0 : end ? geometry.size.width : -110)
                .animation(reduceMotion ? nil : .easeInOut(duration: 1.5).repeatForever(autoreverses: true), value: end)
                .onAppear { end = !reduceMotion }
                .onChange(of: reduceMotion) { _, value in end = !value }
        }.frame(height: 2).clipped().allowsHitTesting(false).accessibilityHidden(true)
    }
}
