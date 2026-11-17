// Keep native line numbers beside colored code; preserve every other normalized output class.
import Foundation

struct PhoneFileOutput {
    var text = AttributedString()
    var numbers = ""
    var hasNumbers = false

    static func format(_ pieces: [PhoneOutputPiece], path: String) -> Self {
        var result = Self()
        var index = 0
        while index < pieces.count {
            if pieces[index].kind == .plain {
                var source = ""
                repeat {
                    source += pieces[index].text
                    index += 1
                } while index < pieces.count && pieces[index].kind == .plain
                for line in fileLines(source: source, filePath: path) {
                    result.text.append(PhoneCodeText.format(line.spans))
                    if let number = line.number { result.numbers += number; result.hasNumbers = true }
                    if line.spans.last?.text.hasSuffix("\n") == true { result.numbers += "\n" }
                }
            } else {
                let piece = pieces[index]
                result.text.append(PhoneOutputText.format([piece]))
                result.numbers += String(repeating: "\n", count: piece.text.filter { $0 == "\n" }.count)
                index += 1
            }
        }
        return result
    }
}
