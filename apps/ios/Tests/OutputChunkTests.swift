// Keep Unicode, native numbers and colors intact when a large tool result crosses layout blocks.
import Foundation
import SwiftUI
import XCTest
@testable import Carlos

final class OutputChunkTests: XCTestCase {
    func testLargeColoredOutputKeepsEveryLineAndItsNativeNumber() {
        let source = (0..<10000).map { "line \($0) 🦀" }.joined(separator: "\n")
        var text = AttributedString(source)
        text.foregroundColor = PhoneTheme.green
        let numbers = (40000..<50000).map(String.init).joined(separator: "\n")
        let chunks = PhoneOutputChunk.split(PhoneFileOutput(text: text, numbers: numbers, hasNumbers: true))
        XCTAssertGreaterThan(chunks.count, 100)
        XCTAssertEqual(chunks.map { String($0.text.characters) }.joined(separator: "\n"), source)
        XCTAssertEqual(chunks.compactMap(\.numbers).joined(separator: "\n"), numbers)
        for chunk in chunks {
            XCTAssertLessThanOrEqual(chunk.text.characters.filter { $0 == "\n" }.count, 63)
            XCTAssertEqual(chunk.text.runs.first?.foregroundColor, PhoneTheme.green)
            XCTAssertEqual(chunk.text.runs.last?.foregroundColor, PhoneTheme.green)
        }
    }
}
