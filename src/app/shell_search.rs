//! Find search patterns and paths without treating option values as filenames.

/// Extract a search pattern and file operands from grep or ripgrep arguments.
pub(super) fn search_operands(words: &[String]) -> Option<(Option<String>, Vec<String>)> {
    let grep = super::shell_words::program_name(words.first()?) == "grep";
    let mut pattern = None;
    let mut pattern_file = false;
    let mut paths = Vec::new();
    let mut options = true;
    let mut index = 1;
    while let Some(word) = words.get(index) {
        index += 1;
        if options && word == "--" {
            options = false;
            continue;
        }
        if options && matches!(word.as_str(), "-e" | "--regexp") {
            pattern = Some(words.get(index)?.clone());
            index += 1;
            continue;
        }
        if options && matches!(word.as_str(), "-f" | "--file") {
            words.get(index)?;
            pattern_file = true;
            index += 1;
            continue;
        }
        if options && word.starts_with("--regexp=") {
            pattern = word.strip_prefix("--regexp=").map(str::to_string);
            continue;
        }
        if options && word.starts_with("--file=") {
            pattern_file = true;
            continue;
        }
        if options && word.starts_with("-e") && !word.starts_with("--") && word.len() > 2 {
            pattern = Some(word[2..].to_string());
            continue;
        }
        if options && word.starts_with("-f") && !word.starts_with("--") && word.len() > 2 {
            pattern_file = true;
            continue;
        }
        if options && word.starts_with('-') {
            if option_takes_value(word, grep) {
                words.get(index)?;
                index += 1;
            }
            continue;
        }
        if pattern.is_none() && !pattern_file {
            pattern = Some(word.clone());
        } else {
            paths.push(word.clone());
        }
    }
    Some((pattern, paths))
}

fn option_takes_value(word: &str, grep: bool) -> bool {
    if grep && matches!(word, "-E" | "-r" | "-R" | "-T") {
        return false;
    }
    matches!(
        word,
        "-g" | "--glob"
            | "--iglob"
            | "-t"
            | "--type"
            | "-T"
            | "--type-not"
            | "--ignore-file"
            | "-m"
            | "--max-count"
            | "-M"
            | "--max-columns"
            | "--max-filesize"
            | "-j"
            | "--threads"
            | "-A"
            | "--after-context"
            | "-B"
            | "--before-context"
            | "-C"
            | "--context"
            | "-E"
            | "--encoding"
            | "--color"
            | "--colors"
            | "-r"
            | "--replace"
            | "--path-separator"
            | "--include"
            | "--exclude"
            | "--exclude-dir"
            | "--sort"
            | "--sortr"
            | "-d"
            | "--directories"
            | "-D"
            | "--devices"
    )
}
