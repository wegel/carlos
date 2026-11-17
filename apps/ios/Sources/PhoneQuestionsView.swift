// Present native fields in a sheet whose Submit button stays above the keyboard.
import SwiftUI

struct PhoneQuestionsView: View {
    let host: PhoneHostModel
    let card: String
    @Bindable var draft: PhoneQuestionDraft
    @Environment(\.dismiss) private var dismiss

    private var current: PhoneApproval? { host.approvals.first { $0.id == card } }
    private var questions: PhoneQuestions? {
        switch current?.input {
        case .fields(let editor)?, .form(let editor)?: return editor
        default: return nil
        }
    }
    private var enabled: Bool {
        host.connected && host.transcript?.currentSnapshot == true && current?.active == true &&
            current?.submitted == false && !host.controlSubmitting && !host.approvalBusy(card)
    }
    private var canSubmit: Bool { enabled && !draft.checking && !draft.changed }

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    if let current {
                        Text(current.caption.title).font(PhoneTheme.prose(15, weight: .semibold)).foregroundStyle(PhoneTheme.yellow)
                        DisclosureGroup("Request details") {
                            Text(current.caption.details.joined(separator: "\n")).font(PhoneTheme.mono(11.5)).textSelection(.enabled)
                        }.font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.subtext0)
                    }
                    ForEach(draft.fields, id: \.key) { field in fieldEditor(field) }
                    if draft.checking { ProgressView("Checking request").font(PhoneTheme.prose(13)) }
                    if draft.changed {
                        Text("This request changed. Your answers are kept here; review the current request before replying.")
                            .font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.yellow)
                    }
                }.padding(16).disabled(!enabled)
            }
            .scrollDismissesKeyboard(.interactively).background(PhoneTheme.base)
            .safeAreaInset(edge: .bottom, spacing: 0) { actions }
            .navigationTitle(draft.form ? "Complete form" : "Answer questions").navigationBarTitleDisplayMode(.inline)
            .toolbar { ToolbarItem(placement: .confirmationAction) {
                Button("Close") { dismiss() }.accessibilityLabel("Close without submitting").accessibilityIdentifier("questions.close")
            } }
            .task(id: questions.map(ObjectIdentifier.init)) {
                if let questions { await draft.refresh(current: questions) }
                else { draft.checking = false; draft.changed = true }
            }
            .onChange(of: current == nil) { _, gone in if gone { dismiss() } }
            .onChange(of: draft.notes) { _, _ in if host.connected { host.error = nil } }
            .onChange(of: draft.choices) { _, _ in if host.connected { host.error = nil } }
            .onChange(of: draft.included) { _, _ in if host.connected { host.error = nil } }
        }
        .font(PhoneTheme.prose()).foregroundStyle(PhoneTheme.text).tint(PhoneTheme.yellow)
        .presentationDetents([.large]).presentationDragIndicator(.visible).presentationCornerRadius(22)
    }

    private func fieldEditor(_ field: PhoneQuestionField) -> some View {
        VStack(alignment: .leading, spacing: 10) {
            if draft.form && !field.required {
                Toggle("Include \(field.title)", isOn: Binding(
                    get: { draft.included.contains(field.key) },
                    set: { if $0 { draft.included.insert(field.key) } else { draft.included.remove(field.key) } }
                )).font(PhoneTheme.prose(14)).tint(PhoneTheme.yellow).accessibilityIdentifier("form.\(field.key).include")
            }
            if !draft.form || field.required || draft.included.contains(field.key) {
                PhoneQuestionFieldView(field: field,
                    selected: Binding(get: { draft.choices[field.key] ?? [] }, set: { draft.choices[field.key] = $0 }),
                    text: Binding(get: { draft.notes[field.key] ?? "" }, set: { draft.notes[field.key] = $0 }))
            }
        }
    }

    private var actions: some View {
        VStack(alignment: .leading, spacing: 10) {
            if let error = host.error ?? current?.caption.inputError {
                Text(error).font(PhoneTheme.prose(13)).foregroundStyle(PhoneTheme.red).accessibilityIdentifier("questions.error")
            }
            Button { host.answer(draft.editor, card: card, answers: draft.answers()) } label: {
                Text(draft.form ? "Submit form" : "Submit answers").font(PhoneTheme.prose(14, weight: .semibold))
                    .frame(maxWidth: .infinity, minHeight: 44)
                    .background(canSubmit ? PhoneTheme.yellow : PhoneTheme.surface0)
                    .foregroundStyle(canSubmit ? PhoneTheme.crust : PhoneTheme.overlay1)
                    .clipShape(RoundedRectangle(cornerRadius: 12))
            }.buttonStyle(.plain).disabled(!canSubmit).accessibilityIdentifier("questions.submit")
            if let current {
                HStack(spacing: 10) {
                    ForEach(current.choices.filter { $0 == .deny || $0 == .cancel }, id: \.self) { decision in
                        Button { host.approve(current, decision: decision) } label: {
                            Text(decision.label).font(PhoneTheme.prose(13, weight: .semibold))
                                .frame(maxWidth: .infinity, minHeight: 44)
                        }.buttonStyle(.plain).disabled(!enabled)
                            .overlay { RoundedRectangle(cornerRadius: 12).stroke(PhoneTheme.surface1, lineWidth: 1) }
                    }
                }
            }
        }.padding(16).background(PhoneTheme.base)
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
            if field.secret { SecureField("Your answer", text: $text).privacySensitive().focused($editing) }
            else if field.kind == .integer || field.kind == .number {
                TextField("Number", text: $text).keyboardType(.numbersAndPunctuation)
                    .focused($editing).submitLabel(.done).onSubmit { editing = false }
            } else {
                TextField(field.notesOnly ? "Add a note" : "Your answer", text: $text, axis: .vertical)
                    .lineLimit(1...5).focused($editing)
            }
        }
        .font(PhoneTheme.prose(14)).textInputAutocapitalization(.never).autocorrectionDisabled()
        .padding(12).frame(minHeight: 44).background(PhoneTheme.crust)
        .clipShape(RoundedRectangle(cornerRadius: 10))
        .overlay { RoundedRectangle(cornerRadius: 10).stroke(PhoneTheme.surface1, lineWidth: 1).allowsHitTesting(false) }
        .contentShape(RoundedRectangle(cornerRadius: 10)).onTapGesture { editing = true }
        .accessibilityLabel(field.title.isEmpty ? "Your answer" : field.title)
        .accessibilityHint(field.required ? "Required" : "Optional")
        .accessibilityIdentifier("question.\(field.key).text")
        .toolbar {
            if editing {
                ToolbarItem(placement: .topBarLeading) { Button("Done") { editing = false }.accessibilityIdentifier("questions.keyboardDone") }
            }
        }
    }
}
