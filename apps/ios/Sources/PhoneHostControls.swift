// Submit explicit native controls and settle their private replies without inventing session facts.
import Foundation

extension PhoneHostModel {
    func interrupt() {
        guard let selected, connected, !controlSubmitting else { return }
        submitControl { try $0.interrupt(session: selected) }
    }

    func approve(_ card: PhoneApproval, decision: PhoneDecision) {
        guard let selected, connected, !controlSubmitting, !approvalBusy(card.id) else { return }
        followRequest = UUID()
        submitApproval(card.id) { try $0.approve(session: selected, approval: card.id, decision: decision) }
    }

    func answer(_ questions: PhoneQuestions, card: String, answers: [PhoneQuestionAnswer]) {
        guard connected, !controlSubmitting, !approvalBusy(card) else { return }
        followRequest = UUID()
        submitApproval(card) { try $0.answerQuestions(questions: questions, answers: answers) }
    }

    func approvalBusy(_ card: String) -> Bool {
        guard let request = approvalRequests[card] else { return false }
        switch request {
        case .writing: return true
        case .operation(let key):
            switch operations[key]?.state {
            case .rejected?, .uncertain?: return false
            default: return true
            }
        }
    }

    private func submitApproval(_ card: String, _ call: @escaping @Sendable (PhoneClient) throws -> PhoneSubmission) {
        error = nil
        approvalRequests[card] = .writing
        controlSubmitting = true
        transport.submit(call) { [weak self] result in
            guard let self else { return }
            controlSubmitting = false
            switch result {
            case .success(let operation):
                approvalRequests[card] = .operation(operation.key)
                accept(operation)
            case .failure(let failure):
                approvalRequests.removeValue(forKey: card)
                error = PhoneFailureMessage.describe(failure)
            }
        }
    }

    func reopen() {
        guard let selected, connected, !controlSubmitting else { return }
        error = nil
        controlSubmitting = true
        transport.submit({ try $0.reopen(session: selected) }) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let operation):
                reopening = operation.key
                accept(operation)
            case .failure(let failure):
                controlSubmitting = false
                error = PhoneFailureMessage.describe(failure)
            }
        }
    }

    func completeReopen(_ operation: PhoneOperation) {
        guard reopening == operation.key else { return }
        switch operation.state {
        case .accepted:
            reopening = nil
            controlSubmitting = false
            guard let session = operation.session, selected == session, connected else { return }
            transport.command({ try $0.subscribe(session: session) }) { [weak self] result in
                if case .failure(let failure) = result { self?.error = PhoneFailureMessage.describe(failure) }
            }
        case .rejected, .uncertain:
            reopening = nil
            controlSubmitting = false
        default: break
        }
    }

    func setRalph(_ on: Bool) {
        guard let selected, connected, !controlSubmitting else { return }
        controlSubmitting = true
        transport.submit({ try $0.setRalph(session: selected, on: on) }) { [weak self] result in
            guard let self else { return }
            controlSubmitting = false
            switch result {
            case .success(let operation):
                desiredRalph = on
                ralphRequest = operation
                accept(operation)
            case .failure(let failure): error = PhoneFailureMessage.describe(failure)
            }
        }
    }

    private func submitControl(_ call: @escaping @Sendable (PhoneClient) throws -> PhoneSubmission) {
        controlSubmitting = true
        transport.submit(call) { [weak self] result in
            guard let self else { return }
            controlSubmitting = false
            switch result {
            case .success(let operation): accept(operation)
            case .failure(let failure): error = PhoneFailureMessage.describe(failure)
            }
        }
    }

}

enum ApprovalSubmission {
    case writing
    case operation(String)
}
