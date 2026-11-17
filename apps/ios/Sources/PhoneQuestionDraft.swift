// Retain private form edits when its native sheet closes; the original Rust editor keeps its reply target.
import Foundation
import Observation

@MainActor @Observable
final class PhoneQuestionDraft: Identifiable {
    nonisolated let id = UUID()
    let card: String
    let editor: PhoneQuestions
    let form: Bool
    var fields: [PhoneQuestionField] = []
    var choices: [String: [UInt32]] = [:]
    var notes: [String: String] = [:]
    var included: Set<String> = []
    var checking = true
    var changed = false

    init(card: String, editor: PhoneQuestions) {
        self.card = card
        self.editor = editor
        form = editor.isForm()
    }

    func refresh(current: PhoneQuestions) async {
        checking = true
        let editor = editor
        let result = await Task.detached(priority: .userInitiated) {
            (editor.fields(), editor.sameQuestions(other: current))
        }.value
        guard !Task.isCancelled else { return }
        fields = result.0
        changed = !result.1
        checking = false
    }

    func answers() -> [PhoneQuestionAnswer] {
        fields.filter { !form || $0.required || included.contains($0.key) }
            .map { PhoneQuestionAnswer(key: $0.key, selected: choices[$0.key] ?? [], text: notes[$0.key]) }
    }
}
