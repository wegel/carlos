// Check original bytes, the real Markdown binding and device cache limits without a socket or model call.
import Foundation
import SwiftUI
import UIKit
import XCTest
@testable import Carlos

final class DocumentTests: XCTestCase {
    @MainActor
    func testLongLiteralToolKeepsItsStatusOnOneCompactRowWhileAiLabelsWrap() throws {
        let suite = "tool-layout-" + UUID().uuidString
        let defaults = try XCTUnwrap(UserDefaults(suiteName: suite))
        defer { defaults.removePersistentDomain(forName: suite) }
        let preferences = PhonePreferences(defaults: defaults)
        let host = PhoneHostModel(configuration: PhoneHost(id: "layout-test", name: "Test", address: "ws://127.0.0.1:1", allowsPrivateWebSocket: true))
        func height(_ label: String, source: PhoneLabelSource, output: Bool = false) -> CGFloat {
            let tool = PhoneTool(kind: .command, label: label, labelSource: source, status: .ok,
                                 summary: "12 passed", diff: nil, hasOutput: output)
            let row = PhoneEntry(id: "tool", kind: .tool, format: .plain, filePath: nil,
                                 text: PhoneText(preview: "", sourceBytes: nil, sourceRevision: nil), done: true, tool: tool)
            let view = UIHostingController(rootView: PhoneEntryRow(host: host, row: row, compact: true).environment(preferences))
            return view.sizeThatFits(in: CGSize(width: 358, height: 10000)).height
        }
        XCTAssertEqual(height(String(repeating: "long-command-argument ", count: 30), source: .literal), 26, accuracy: 0.5)
        XCTAssertEqual(height(String(repeating: "long-command-argument ", count: 30), source: .literal, output: true), 26, accuracy: 0.5)
        XCTAssertGreaterThan(height(String(repeating: "Describe the tool work clearly. ", count: 6), source: .ai), 26)
    }

    func testOutputColorsKeepUnicodeNewlinesAndAdjacentPiecesIntact() {
        let pieces = [PhoneOutputPiece(kind: .hunk, text: "@@ -1 +1 @@\n"),
                      PhoneOutputPiece(kind: .deletion, text: "-old 🌍\n"),
                      PhoneOutputPiece(kind: .addition, text: "+new "),
                      PhoneOutputPiece(kind: .addition, text: "🦀\n")]
        let text = PhoneOutputText.format(pieces)
        XCTAssertEqual(String(text.characters), "@@ -1 +1 @@\n-old 🌍\n+new 🦀\n")
        XCTAssertEqual(text.runs.first?.foregroundColor, PhoneTheme.mauve)
        XCTAssertEqual(text.runs.last?.foregroundColor, PhoneTheme.green)
        XCTAssertEqual(text.runs.last?.backgroundColor, PhoneTheme.green.opacity(0.08))
    }

    @MainActor
    func testReasoningFitsTheActualWidthAndEmptyCompletedRowsLeaveNoGap() {
        func row(_ text: String, done: Bool = true, source: UInt64? = nil) -> PhoneEntry {
            PhoneEntry(id: "reasoning", kind: .reasoning, format: .plain, filePath: nil,
                       text: PhoneText(preview: text, sourceBytes: source, sourceRevision: source == nil ? nil : 0), done: done, tool: nil)
        }
        XCTAssertEqual(PhoneEntryLayout.presentation(row(""), width: 358), .hidden)
        XCTAssertEqual(PhoneEntryLayout.presentation(row("", done: false), width: 358), .compact)
        XCTAssertEqual(PhoneEntryLayout.presentation(row("", source: 8000), width: 358), .block)
        XCTAssertEqual(PhoneEntryLayout.presentation(row("Check the native result"), width: 358), .compact)
        XCTAssertEqual(PhoneEntryLayout.presentation(row("Check the native result"), width: 80), .block)
        XCTAssertEqual(PhoneEntryLayout.presentation(row("first\nsecond"), width: 358), .block)
        XCTAssertEqual(PhoneEntryLayout.presentation(row("first\rsecond"), width: 358), .block)
    }

    func testOriginalFilesKeepCapturedPrefixesAndRejectGaps() throws {
        let budget = PhoneBodyBudget()
        let file = try PhoneSourceFile(budget: budget)
        try file.append("a🌍", at: 0)
        try file.append("\u{1b}[31mred\u{1b}[0m\n", at: 5)
        XCTAssertEqual(try file.text(upTo: 5), "a🌍")
        XCTAssertEqual(try file.text(upTo: file.byteCount), "a🌍\u{1b}[31mred\u{1b}[0m\n")
        XCTAssertThrowsError(try file.append("wrong offset", at: 0))
        XCTAssertEqual(try file.text(upTo: 5), "a🌍")
    }

    func testCapacityPreservesAcceptedTextAndClosingTheFileReleasesItsAllowance() throws {
        let budget = PhoneBodyBudget()
        try budget.reserve(PhoneBodyBudget.limit - 4)
        var file: PhoneSourceFile? = try PhoneSourceFile(budget: budget)
        try XCTUnwrap(file).append("🌍", at: 0)
        XCTAssertThrowsError(try XCTUnwrap(file).append("extra", at: 4)) { error in
            guard case PhoneError.Capacity = error else { return XCTFail("Expected typed capacity") }
        }
        XCTAssertEqual(try XCTUnwrap(file).text(upTo: 4), "🌍")
        file = nil
        budget.release(PhoneBodyBudget.limit - 4)
        XCTAssertNoThrow(try budget.reserve(PhoneBodyBudget.limit))
        budget.release(PhoneBodyBudget.limit)
    }

    func testGeneratedMarkdownRecordsPreserveReferencesStylesAndLiteralUserText() throws {
        let document = try PhoneDocument(source: "Read **bold [late][ref]** and `x`.\n\n[ref]: https://example.test/é")
        XCTAssertEqual(document.blockCount(), 1)
        let block = try document.block(index: 0)
        XCTAssertEqual(block.spans.map(\.text).joined(), "Read bold late and x.")
        let link = try XCTUnwrap(block.spans.first { $0.text == "late" })
        XCTAssertEqual(link.style.link, "https://example.test/é")
        XCTAssertTrue(link.style.strong)
        let original = "**literal**\n`code` 🌍"
        let plain = try PhoneDocument.plain(source: original)
        XCTAssertEqual(try plain.block(index: 0).spans.map(\.text).joined(), original)
        XCTAssertFalse(try plain.block(index: 0).spans[0].style.strong)
    }

    @MainActor
    func testDisplayPreferencesStayOnTheDeviceAndRowChoicesSurviveOtherSessions() throws {
        let name = "carlos-test-\(UUID().uuidString)"
        let defaults = try XCTUnwrap(UserDefaults(suiteName: name))
        defer { defaults.removePersistentDomain(forName: name) }
        let preferences = PhonePreferences(defaults: defaults)
        preferences.layout = .gutter
        preferences.toggle(host: "host", session: "first", entry: "tool")
        XCTAssertFalse(preferences.isExpanded(host: "host", session: "second", entry: "tool"))
        let restored = PhonePreferences(defaults: defaults)
        XCTAssertEqual(restored.layout, .gutter)
        XCTAssertTrue(restored.isExpanded(host: "host", session: "first", entry: "tool"))
        restored.toolMode = .full
        XCTAssertTrue(restored.isExpanded(host: "host", session: "second", entry: "tool"))
        restored.toolMode = .compact
        XCTAssertFalse(restored.isExpanded(host: "host", session: "first", entry: "tool"))
    }
}
