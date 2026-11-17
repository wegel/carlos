// Choose the phone palette for grammar roles, off the UI thread and without changing source text.
import SwiftUI

enum PhoneCodeText {
    static func format(_ spans: [PhoneSyntaxSpan]) -> AttributedString {
        var result = AttributedString()
        for span in spans {
            var text = AttributedString(span.text)
            text.foregroundColor = color(span.role)
            result.append(text)
        }
        return result
    }

    private static func color(_ role: PhoneSyntaxRole) -> Color {
        switch role {
        case .plain: PhoneTheme.text
        case .comment: PhoneTheme.overlay2
        case .string: PhoneTheme.green
        case .number: PhoneTheme.peach
        case .keyword: PhoneTheme.mauve
        case .type: PhoneTheme.yellow
        case .function: PhoneTheme.blue
        case .variable: PhoneTheme.flamingo
        case .operator: PhoneTheme.teal
        case .invalid: PhoneTheme.red
        }
    }
}
