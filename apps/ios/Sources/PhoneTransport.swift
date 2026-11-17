// Keep Rust calls and socket writes ordered off the main thread; Swift owns no wire decoder.
import Foundation

enum PhoneLinkState: Sendable {
    case connecting
    case connected
    case disconnected(String?)
}

struct PhoneCallbacks: Sendable {
    let received: @MainActor @Sendable (PhoneBatch) -> Void
    let link: @MainActor @Sendable (PhoneLinkState) -> Void
}

// Every mutable field and Rust call runs on queue. Queue jobs retain self; deinit
// runs only after those jobs finish, and weak socket callbacks cannot resurrect it.
final class PhoneTransport: @unchecked Sendable {
    private let queue = DispatchQueue(label: "app.carlos.phone.transport", qos: .userInitiated)
    private let client = PhoneClient()
    private let session = URLSession(configuration: .ephemeral, delegate: SocketPolicy(), delegateQueue: nil)
    private var socket: URLSessionWebSocketTask?
    private var generation: UInt64?
    private var callbacks: PhoneCallbacks?
    private var writes: [PhoneFrame] = []
    private var writing = false
    private var greeted = false
    private var greetingDeadline: DispatchWorkItem?

    deinit { session.invalidateAndCancel() }

    func connect(url: URL, token: String, callbacks: PhoneCallbacks) {
        queue.async {
            self.close()
            self.callbacks = callbacks
            do {
                let greeting = try self.client.begin(name: "Carlos iPhone", token: token)
                let socket = self.session.webSocketTask(with: url)
                socket.maximumMessageSize = Int(self.client.maximumMessageBytes())
                self.socket = socket
                self.generation = greeting.generation
                self.emit(.connecting)
                socket.resume()
                try self.enqueue([greeting])
                self.receive(socket, generation: greeting.generation)
                let deadline = DispatchWorkItem { [weak self] in
                    guard let self, self.generation == greeting.generation, !self.greeted else { return }
                    self.close(message: "The daemon did not respond. Check its address and network, then try again.")
                }
                self.greetingDeadline = deadline
                self.queue.asyncAfter(deadline: .now() + 15, execute: deadline)
            } catch {
                self.close(message: PhoneFailureMessage.describe(error))
            }
        }
    }

    func disconnect() {
        queue.async { self.close() }
    }

    func read<Value: Sendable>(
        _ call: @escaping @Sendable (PhoneClient) throws -> Value,
        completion: @escaping @MainActor @Sendable (Result<Value, any Error>) -> Void
    ) {
        queue.async {
            let result = Result { try call(self.client) }
            DispatchQueue.main.async { completion(result) }
        }
    }

    func command(
        _ call: @escaping @Sendable (PhoneClient) throws -> PhoneFrame,
        completion: @escaping @MainActor @Sendable (Result<Void, any Error>) -> Void
    ) {
        queue.async {
            let result = Result { try self.admit(call(self.client)) }
            DispatchQueue.main.async { completion(result) }
        }
    }

    func batch(
        _ call: @escaping @Sendable (PhoneClient) throws -> PhoneBatch,
        completion: @escaping @MainActor @Sendable (Result<Void, any Error>) -> Void
    ) {
        queue.async {
            let result = Result {
                let batch = try call(self.client)
                try self.enqueue(batch.outgoing)
                if let callbacks = self.callbacks { DispatchQueue.main.async { callbacks.received(batch) } }
            }
            DispatchQueue.main.async { completion(result) }
        }
    }

    func submit(
        _ call: @escaping @Sendable (PhoneClient) throws -> PhoneSubmission,
        completion: @escaping @MainActor @Sendable (Result<PhoneOperation, any Error>) -> Void
    ) {
        queue.async {
            let result = Result {
                let submission = try call(self.client)
                try self.admit(submission.frame)
                return submission.operation
            }
            DispatchQueue.main.async { completion(result) }
        }
    }

    private func admit(_ frame: PhoneFrame) throws {
        do { try enqueue([frame]) }
        catch {
            failed(frame, possiblyWritten: false)
            throw error
        }
    }

    private func enqueue(_ frames: [PhoneFrame]) throws {
        guard !frames.isEmpty else { return }
        guard frames.allSatisfy({ $0.generation == generation }), socket != nil,
              frames.count <= 256 - writes.count else {
            throw PhoneError.Capacity(message: "Socket writes are unavailable; private text is retained")
        }
        writes.append(contentsOf: frames)
        sendNext()
    }

    private func sendNext() {
        guard !writing, !writes.isEmpty, let socket else { return }
        let frame = writes.removeFirst()
        writing = true
        socket.send(.string(frame.payload)) { [weak self] error in
            guard let self else { return }
            self.queue.async {
                guard self.generation == frame.generation else { return }
                self.writing = false
                if error != nil { self.failed(frame, possiblyWritten: true) }
                else { self.sendNext() }
            }
        }
    }

    private func receive(_ socket: URLSessionWebSocketTask, generation: UInt64) {
        socket.receive { [weak self] result in
            guard let self else { return }
            self.queue.async { self.received(result, socket: socket, generation: generation) }
        }
    }

    private func received(
        _ result: Result<URLSessionWebSocketTask.Message, any Error>,
        socket: URLSessionWebSocketTask, generation: UInt64
    ) {
        guard self.generation == generation else { return }
        do {
            let data: Data
            switch try result.get() {
            case .string(let text): data = Data(text.utf8)
            case .data(let bytes): data = bytes
            @unknown default: throw PhoneError.InvalidMessage(message: "Unsupported WebSocket message")
            }
            let batch = try client.receive(generation: generation, message: data)
            try enqueue(batch.outgoing)
            if batch.ready && !greeted {
                greetingDeadline?.cancel()
                greetingDeadline = nil
                greeted = true
                emit(.connected)
            }
            let rejected = !batch.ready && !batch.failures.isEmpty
            guard let callbacks else { return }
            DispatchQueue.main.async {
                callbacks.received(batch)
                self.queue.async {
                    guard self.generation == generation else { return }
                    if rejected { self.close(message: "Daemon rejected this connection") }
                    else { self.receive(socket, generation: generation) }
                }
            }
        } catch {
            close(message: PhoneFailureMessage.describe(error))
        }
    }

    private func failed(_ frame: PhoneFrame, possiblyWritten: Bool) {
        if let batch = try? client.writeFailed(
            generation: frame.generation, requestId: frame.requestId, possiblyWritten: possiblyWritten
        ), let callbacks {
            DispatchQueue.main.async { callbacks.received(batch) }
        }
        close(message: "Connection lost while sending; submitted text is retained")
    }

    private func close(message: String? = nil) {
        greetingDeadline?.cancel()
        greetingDeadline = nil
        if let generation { try? client.disconnect(generation: generation) }
        socket?.cancel(with: .goingAway, reason: nil)
        socket = nil
        generation = nil
        writes.removeAll(keepingCapacity: true)
        writing = false
        greeted = false
        emit(.disconnected(message))
    }

    private func emit(_ state: PhoneLinkState) {
        guard let callbacks else { return }
        DispatchQueue.main.async { callbacks.link(state) }
    }
}

// A redirect must not send a provisioned device token to a different endpoint.
private final class SocketPolicy: NSObject, URLSessionTaskDelegate {
    func urlSession(
        _ session: URLSession, task: URLSessionTask,
        willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest,
        completionHandler: @escaping @Sendable (URLRequest?) -> Void
    ) {
        completionHandler(nil)
    }
}
