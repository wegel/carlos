// Measure the real generated Rust boundary and grammar work in the native test process.
import Foundation
import XCTest
@testable import Carlos

final class BridgePerformanceTests: XCTestCase {
    func testThousandBoundedMarkdownReads() throws {
        let source = (0..<1000).map { "Paragraph \($0) has **native** text and `code` 🌍." }.joined(separator: "\n\n")
        let document = try PhoneDocument(source: source)
        XCTAssertEqual(document.blockCount(), 1000)
        let options = XCTMeasureOptions()
        options.iterationCount = 5
        measure(metrics: [XCTClockMetric(), XCTMemoryMetric()], options: options) {
            for index: UInt32 in 0..<1000 {
                do {
                    let block = try document.block(index: index)
                    XCTAssertFalse(block.spans.isEmpty)
                } catch { XCTFail("A checked block must remain readable: \(error)") }
            }
        }
    }

    func testFiveHundredLinesOfActualRustGrammar() {
        let source = String(repeating: "fn main() { let count = 42; } // 🌍\n", count: 500)
        // Compile the grammar once, as the app does on its first visible code block.
        XCTAssertTrue(highlightCode(source: source, language: "rust", filePath: nil).contains { $0.role == .keyword })
        let options = XCTMeasureOptions()
        options.iterationCount = 5
        measure(metrics: [XCTClockMetric(), XCTMemoryMetric()], options: options) {
            let spans = highlightCode(source: source, language: "rust", filePath: nil)
            XCTAssertEqual(spans.map(\.text).joined(), source)
        }
    }
}
