// Bound each native text layout without dropping lines or changing the original output reader.
import Foundation

struct PhoneOutputChunk: Identifiable {
    let id: Int
    let text: AttributedString
    let numbers: String?

    static func split(_ output: PhoneFileOutput) -> [Self] {
        var chunks: [Self] = []
        let text = output.text
        let numbers = output.numbers.split(separator: "\n", omittingEmptySubsequences: false)
        var start = text.startIndex
        var cursor = start
        var lines = 0
        var numberStart = 0
        var characters = 0
        while cursor < text.endIndex {
            let newline = text.characters[cursor] == "\n"
            let next = text.characters.index(after: cursor)
            characters += 1
            if newline { lines += 1 }
            if newline && (lines - numberStart >= 64 || characters >= 8192) {
                chunks.append(Self(id: chunks.count, text: AttributedString(text[start..<cursor]),
                    numbers: output.hasNumbers ? numbers[numberStart..<min(lines, numbers.count)].joined(separator: "\n") : nil))
                start = next
                numberStart = lines
                characters = 0
            }
            cursor = next
        }
        if start < text.endIndex {
            chunks.append(Self(id: chunks.count, text: AttributedString(text[start..<text.endIndex]),
                numbers: output.hasNumbers ? numbers[numberStart..<numbers.count].joined(separator: "\n") : nil))
        }
        return chunks
    }
}
