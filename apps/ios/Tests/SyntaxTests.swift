// Exercise the actual pure Rust grammar through generated Swift and the native color formatter.
import XCTest
@testable import Carlos

final class SyntaxTests: XCTestCase {
    func testGrammarColorsKeepTheExactCodeText() {
        let source = "// hello 🌍\nfn main() { let count = 42; println!(\"hello 🦀\"); }\n"
        let spans = highlightCode(source: source, language: "rust", filePath: nil)
        XCTAssertTrue(spans.contains { $0.role == .comment })
        XCTAssertTrue(spans.contains { $0.role == .keyword })
        XCTAssertTrue(spans.contains { $0.role == .string })
        XCTAssertEqual(String(PhoneCodeText.format(spans).characters), source)
        let plain = highlightCode(source: source, language: "unsupported-language", filePath: nil)
        XCTAssertEqual(plain.count, 1)
        XCTAssertEqual(plain.first?.role, .plain)
        XCTAssertEqual(plain.first?.text, source)
    }
}
