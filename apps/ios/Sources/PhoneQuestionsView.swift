// Keep private answers tied to the original question object while Swift draws native controls.
import SwiftUI

struct PhoneQuestionsView: View {
    let host: PhoneHostModel
    let current: PhoneQuestions
    let enabled: Bool
    @State private var editor: PhoneQuestions
    @State private var fields: [PhoneQuestionField] = []
    @State private var choices: [String: [UInt32]] = [:]
    @State private var notes: [String: String] = [:]
    @State private var checking = true
    @State private var changed = false

    init(host: PhoneHostModel, questions: PhoneQuestions, enabled: Bool) {
        self.host = host
        current = questions
        self.enabled = enabled
        _editor = State(initialValue: questions)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 18) {
            ForEach(fields, id: \.key) { field in
                PhoneQuestionFieldView(field: field,
                    selected: Binding(get: { choices[field.key] ?? [] }, set: { choices[field.key] = $0 }),
                    text: Binding(get: { notes[field.key] ?? "" }, set: { notes[field.key] = $0 }))
            }
            if checking { ProgressView("Checking questions").font(PhoneTheme.prose(13)) }
            if changed {
                Text("These questions changed. Your answers are kept here; review the current request before replying.")
                    .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.yellow)
            }
            Button {
                let answers = fields.map { PhoneQuestionAnswer(key: $0.key, selected: choices[$0.key] ?? [], text: notes[$0.key]) }
                host.answer(editor, answers: answers)
            } label: {
                Text("Submit answers").font(PhoneTheme.prose(13, weight: .semibold))
                    .frame(maxWidth: .infinity, minHeight: 44)
                    .background(PhoneTheme.yellow).foregroundStyle(PhoneTheme.crust)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            }
            .buttonStyle(.plain).disabled(!enabled || checking || changed)
            .accessibilityIdentifier("questions.submit")
        }
        .disabled(!enabled)
        .task(id: ObjectIdentifier(current)) {
            checking = true
            let editor = editor, current = current
            let result = await Task.detached(priority: .userInitiated) {
                (editor.fields(), editor.sameQuestions(other: current))
            }.value
            guard !Task.isCancelled else { return }
            fields = result.0
            changed = !result.1
            checking = false
        }
    }
}

private struct PhoneQuestionFieldView: View {
    let field: PhoneQuestionField
    @Binding var selected: [UInt32]
    @Binding var text: String
    @FocusState private var editing: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text((field.title.isEmpty ? "Question" : field.title) + (field.required ? " *" : ""))
                .font(PhoneTheme.prose(14, weight: .semibold))
            if !field.description.isEmpty {
                Text(field.description).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.subtext0)
            }
            ForEach(field.options.indices, id: \.self) { index in option(index) }
            if field.other { input }
        }
    }

    private func option(_ index: Int) -> some View {
        let option = field.options[index]
        let chosen = selected.contains(UInt32(index))
        return Button {
            if chosen { selected.removeAll { $0 == UInt32(index) } }
            else if field.multiple { selected.append(UInt32(index)) }
            else { selected = [UInt32(index)] }
        } label: {
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: chosen ? "checkmark.circle.fill" : "circle")
                    .foregroundStyle(chosen ? PhoneTheme.yellow : PhoneTheme.overlay1)
                VStack(alignment: .leading, spacing: 4) {
                    Text(option.label).font(PhoneTheme.prose(14, weight: .medium))
                    if !option.description.isEmpty {
                        Text(option.description).font(PhoneTheme.prose(12)).foregroundStyle(PhoneTheme.subtext0)
                    }
                }.frame(maxWidth: .infinity, alignment: .leading)
            }.padding(12).frame(minHeight: 44).background(chosen ? PhoneTheme.yellow.opacity(0.08) : PhoneTheme.crust)
                .clipShape(RoundedRectangle(cornerRadius: 10))
                .overlay { RoundedRectangle(cornerRadius: 10).stroke(chosen ? PhoneTheme.yellow.opacity(0.4) : PhoneTheme.surface0, lineWidth: 1) }
        }
        .buttonStyle(.plain).foregroundStyle(PhoneTheme.text)
        .accessibilityIdentifier("question.\(field.key).option.\(index)")
        .accessibilityValue(chosen ? "Selected" : "Not selected")
    }

    private var input: some View {
        Group {
            if field.secret { SecureField("Your answer", text: $text).privacySensitive() }
            else { TextField(field.notesOnly ? "Add a note" : "Your answer", text: $text, axis: .vertical).lineLimit(1...5) }
        }
        .font(PhoneTheme.prose(14)).textInputAutocapitalization(.never).autocorrectionDisabled()
        .focused($editing).padding(12).frame(minHeight: 44).background(PhoneTheme.crust)
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .overlay { RoundedRectangle(cornerRadius: 10).stroke(PhoneTheme.surface1, lineWidth: 1) }
        .accessibilityIdentifier("question.\(field.key).text")
        .toolbar {
            if editing {
                ToolbarItemGroup(placement: .keyboard) { Spacer(); Button("Done") { editing = false } }
            }
        }
    }
}
