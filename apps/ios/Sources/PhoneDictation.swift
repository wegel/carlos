// Record only after a tap and recognize speech on this device; keep all results in the private draft.
import AVFAudio
import Observation
import Speech

@MainActor @Observable
final class PhoneDictation {
    enum Phase { case idle, authorizing, listening, finishing }
    private(set) var phase = Phase.idle
    var message: String?
    private(set) var permissionDenied = false
    var active: Bool { phase == .listening || phase == .finishing }

    @ObservationIgnored private var identity = UUID()
    @ObservationIgnored private var engine: AVAudioEngine?
    @ObservationIgnored private var hasTap = false
    @ObservationIgnored private var request: SFSpeechAudioBufferRecognitionRequest?
    @ObservationIgnored private var recognition: SFSpeechRecognitionTask?
    @ObservationIgnored private var recognizer: SFSpeechRecognizer?
    @ObservationIgnored private var deadline: Task<Void, Never>?
    @ObservationIgnored private var latest = ""
    @ObservationIgnored private var insert: ((String) -> Void)?

    func start(insert: @escaping (String) -> Void) {
        guard phase == .idle else { return }
        message = nil
        permissionDenied = false
        guard let recognizer = SFSpeechRecognizer(locale: .current), recognizer.supportsOnDeviceRecognition else {
            message = "On-device speech recognition is unavailable for this device or language. You can still type your prompt."
            return
        }
        phase = .authorizing
        self.insert = insert
        let attempt = UUID()
        identity = attempt
        Task {
            let speech = await withCheckedContinuation { continuation in
                SFSpeechRecognizer.requestAuthorization { @Sendable status in continuation.resume(returning: status) }
            }
            guard identity == attempt else { return }
            guard speech == .authorized else {
                fail("Allow Speech Recognition for Carlos in Settings to use voice input.", denied: true)
                return
            }
            let microphone = await AVAudioApplication.requestRecordPermission()
            guard identity == attempt else { return }
            guard microphone else {
                fail("Allow Microphone access for Carlos in Settings to use voice input.", denied: true)
                return
            }
            guard recognizer.isAvailable else {
                fail("On-device speech recognition is temporarily unavailable. Your draft is unchanged.")
                return
            }
            do { try record(recognizer: recognizer, attempt: attempt) }
            catch { fail("Voice input could not start. Check your microphone and try again. Your draft is unchanged.") }
        }
    }

    func stop() {
        guard phase == .listening else { return }
        phase = .finishing
        stopAudio()
        request?.endAudio()
        let attempt = identity
        deadline = Task {
            try? await Task.sleep(for: .seconds(3))
            guard !Task.isCancelled, identity == attempt else { return }
            cancel(keepText: true)
        }
    }

    func cancel(keepText: Bool = false) {
        let text = latest
        let complete = insert
        identity = UUID()
        deadline?.cancel()
        deadline = nil
        stopAudio()
        request?.endAudio()
        recognition?.cancel()
        recognition = nil
        recognizer = nil
        request = nil
        engine = nil
        insert = nil
        latest = ""
        phase = .idle
        if keepText, !text.isEmpty { complete?(text) }
    }

    private func record(recognizer: SFSpeechRecognizer, attempt: UUID) throws {
        self.recognizer = recognizer
        let session = AVAudioSession.sharedInstance()
        try session.setCategory(.record, mode: .measurement, options: .duckOthers)
        try session.setActive(true)
        let engine = AVAudioEngine()
        let request = SFSpeechAudioBufferRecognitionRequest()
        request.requiresOnDeviceRecognition = true
        request.shouldReportPartialResults = true
        request.addsPunctuation = true
        self.engine = engine
        self.request = request
        let input = engine.inputNode
        let format = input.outputFormat(forBus: 0)
        guard format.sampleRate > 0, format.channelCount > 0 else {
            throw NSError(domain: "CarlosMicrophone", code: 1)
        }
        let sink = PhoneSpeechAudioSink(request)
        input.installTap(onBus: 0, bufferSize: 1024, format: format) { @Sendable buffer, _ in sink.append(buffer) }
        hasTap = true
        recognition = recognizer.recognitionTask(with: request) { @Sendable [weak self] result, error in
            let text = result?.bestTranscription.formattedString
            let final = result?.isFinal == true
            let failed = error != nil
            Task { @MainActor [weak self] in
                guard let self, self.identity == attempt else { return }
                if let text { self.latest = text }
                if final { self.cancel(keepText: true) }
                else if failed { self.fail("Voice input stopped. Any recognized words were kept in your draft.", keepText: true) }
            }
        }
        engine.prepare()
        try engine.start()
        phase = .listening
    }

    private func stopAudio() {
        if let engine {
            engine.stop()
            if hasTap { engine.inputNode.removeTap(onBus: 0) }
            hasTap = false
            self.engine = nil
            try? AVAudioSession.sharedInstance().setActive(false, options: .notifyOthersOnDeactivation)
        }
    }

    private func fail(_ text: String, denied: Bool = false, keepText: Bool = false) {
        cancel(keepText: keepText)
        permissionDenied = denied
        message = text
    }
}

// Speech's legacy Objective-C request lacks Sendable metadata. Only the audio tap
// appends buffers; the main actor configures it before starting and ends it after
// removing the tap. No Swift UI state crosses this native audio boundary.
private final class PhoneSpeechAudioSink: @unchecked Sendable {
    private let request: SFSpeechAudioBufferRecognitionRequest
    init(_ request: SFSpeechAudioBufferRecognitionRequest) { self.request = request }
    func append(_ buffer: AVAudioPCMBuffer) { request.append(buffer) }
}
