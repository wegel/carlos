// Temporary opt-in diagnostic: record only Carlos while collecting numeric scroll facts.
import ReplayKit
import UIKit

@MainActor
final class PhoneScrollRecording: NSObject {
    static let shared = PhoneScrollRecording()
    private var started = false
    private var displayLink: CADisplayLink?
    private weak var transcript: UICollectionView?
    private var frames: [[String: Any]] = []
    private var start: CFTimeInterval = 0
    private var directory: URL?
    private var identities: [String: Int] = [:]

    func startIfRequested() {
        guard !started, ProcessInfo.processInfo.arguments.contains("--carlos-scroll-record") else { return }
        started = true
        Task {
            try? await Task.sleep(for: .seconds(3))
            await record()
        }
    }

    private func record() async {
        let root = URL.cachesDirectory.appending(path: "ScrollMotion", directoryHint: .isDirectory)
        let folder = root.appending(path: UUID().uuidString, directoryHint: .isDirectory)
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            directory = folder
            try Data(folder.lastPathComponent.utf8).write(to: root.appending(path: "current"), options: .atomic)
            writeStatus("waiting-for-ios-consent")
            let recorder = RPScreenRecorder.shared()
            recorder.isMicrophoneEnabled = false
            recorder.isCameraEnabled = false
            try await withCheckedThrowingContinuation { (completion: CheckedContinuation<Void, any Error>) in
                recorder.startRecording { error in
                    if let error { completion.resume(throwing: error) }
                    else { completion.resume() }
                }
            }
            start = CACurrentMediaTime()
            writeStatus("recording")
            let link = CADisplayLink(target: self, selector: #selector(sample(_:)))
            link.preferredFrameRateRange = CAFrameRateRange(minimum: 60, maximum: 120, preferred: 120)
            displayLink = link
            link.add(to: .main, forMode: .common)
            try await Task.sleep(for: .seconds(30))
            displayLink?.invalidate()
            displayLink = nil
            writeStatus("saving")
            try await recorder.stopRecording(withOutput: folder.appending(path: "scroll.mov"))
            try JSONSerialization.data(withJSONObject: ["clock": "seconds from recording-start callback", "frames": frames])
                .write(to: folder.appending(path: "frames.json"), options: .atomic)
            writeStatus("complete")
        } catch {
            displayLink?.invalidate()
            displayLink = nil
            writeStatus("failed", error: error.localizedDescription)
        }
    }

    @objc private func sample(_ link: CADisplayLink) {
        if transcript == nil {
            for scene in UIApplication.shared.connectedScenes {
                guard let scene = scene as? UIWindowScene else { continue }
                for window in scene.windows {
                    if let found = find(in: window) { transcript = found; break }
                }
                if transcript != nil { break }
            }
        }
        guard let transcript else { return }
        let cells = transcript.indexPathsForVisibleItems.sorted().prefix(12).compactMap { path -> [String: Any]? in
            guard let cell = transcript.cellForItem(at: path) else { return nil }
            let key = cell.accessibilityIdentifier ?? "unknown"
            if identities[key] == nil { identities[key] = identities.count }
            return ["id": identities[key]!, "y": cell.frame.minY - transcript.contentOffset.y,
                    "height": cell.frame.height]
        }
        frames.append(["time": link.timestamp - start, "next": link.targetTimestamp - start,
                       "offset": transcript.contentOffset.y, "pan": transcript.panGestureRecognizer.translation(in: transcript).y,
                       "dragging": transcript.isDragging, "decelerating": transcript.isDecelerating,
                       "contentHeight": transcript.contentSize.height, "cells": cells])
    }

    private func find(in view: UIView) -> UICollectionView? {
        if let collection = view as? UICollectionView, collection.accessibilityIdentifier == "transcript" { return collection }
        for child in view.subviews { if let found = find(in: child) { return found } }
        return nil
    }

    private func writeStatus(_ state: String, error: String? = nil) {
        guard let directory else { return }
        let value: [String: Any] = ["state": state, "error": error ?? "", "frameCount": frames.count,
                                    "microphone": false, "camera": false]
        if let data = try? JSONSerialization.data(withJSONObject: value, options: .sortedKeys) {
            try? data.write(to: directory.appending(path: "status.json"), options: .atomic)
        }
    }
}
