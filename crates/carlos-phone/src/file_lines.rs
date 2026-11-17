//! Separate existing read gutters from code without inventing a file's starting line.

use carlos_client::source::split_read_gutter;

use crate::{highlight_code, PhoneSyntaxRole, PhoneSyntaxSpan};

/// One read line preserves its original prefix separately from the colored source.
#[derive(uniffi::Record)]
pub struct PhoneFileLine {
    pub prefix: String,
    pub number: Option<String>,
    pub spans: Vec<PhoneSyntaxSpan>,
}

/// Format the bounded visible read window; unnumbered lines remain unnumbered.
#[uniffi::export]
pub fn file_lines(source: String, file_path: String) -> Vec<PhoneFileLine> {
    if source.len() > 256 * 1024 {
        return vec![PhoneFileLine {
            prefix: String::new(),
            number: None,
            spans: vec![PhoneSyntaxSpan {
                text: source,
                role: PhoneSyntaxRole::Plain,
            }],
        }];
    }
    let originals: Vec<_> = source.split_inclusive('\n').collect();
    let mut lines = Vec::new();
    let mut code = String::new();
    for line in &originals {
        let (prefix, body) = split_read_gutter(line);
        let number = (!prefix.is_empty()).then(|| {
            prefix
                .trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .collect()
        });
        code.push_str(body);
        lines.push(PhoneFileLine {
            prefix: prefix.into(),
            number,
            spans: Vec::new(),
        });
    }
    let mut index = 0;
    for span in highlight_code(code, None, Some(file_path)) {
        for text in span.text.split_inclusive('\n') {
            if let Some(line) = lines.get_mut(index) {
                line.spans.push(PhoneSyntaxSpan {
                    text: text.into(),
                    role: span.role,
                });
            }
            if text.ends_with('\n') {
                index += 1;
            }
        }
    }
    lines
}
