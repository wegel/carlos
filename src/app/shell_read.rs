//! Identify file operands in read commands without guessing from unrelated flags.

use super::shell_command::{analyze_shell_command, ShellCommand};
use super::shell_words::{has_shell_operators, next_word, program_name};

/// Recognize one or more explicitly named source files, including native RTK reads.
pub(super) fn read_paths(command: &ShellCommand) -> Option<Vec<String>> {
    if has_shell_operators(&command.text) {
        return None;
    }
    let words = shlex::split(&command.text)?;
    let program = program_name(words.first()?);
    let native_read = command.rtk_native && program == "read";
    if !native_read && !matches!(program, "cat" | "nl" | "bat" | "head" | "tail" | "sed") {
        return None;
    }
    let mut paths = Vec::new();
    let mut operands = false;
    let mut sed_script = program != "sed";
    let mut index = 1;
    while let Some(word) = words.get(index) {
        index += 1;
        if !operands && word == "--" {
            operands = true;
            continue;
        }
        if !operands && matches!(word.as_str(), "-h" | "--help" | "--version" | "-V") {
            return None;
        }
        if !operands && word.starts_with('-') {
            if option_takes_value(program, word, native_read) {
                words.get(index)?;
                index += 1;
            } else if native_read && !rtk_read_flag(word) {
                return None;
            }
            continue;
        }
        if !sed_script {
            if !is_print_range(word) {
                return None;
            }
            sed_script = true;
            continue;
        }
        if word == "-" || word.contains(['$', '`', '*', '?', '[', ']']) {
            return None;
        }
        paths.push(word.clone());
    }
    (!paths.is_empty()).then_some(paths)
}

/// Infer a language only when the command reads exactly one file.
pub(super) fn single_source_path(raw: &str) -> Option<String> {
    let command = analyze_shell_command(raw);
    if let Some(mut paths) = read_paths(&command) {
        return (paths.len() == 1).then(|| paths.remove(0));
    }
    let (left, right) = single_pipe(&command.text)?;
    let right = analyze_shell_command(right);
    if has_shell_operators(&right.text) {
        return None;
    }
    let words = shlex::split(&right.text)?;
    if program_name(words.first()?) != "sed"
        || words.len() != 3
        || words[1] != "-n"
        || !is_print_range(&words[2])
    {
        return None;
    }
    let mut paths = read_paths(&analyze_shell_command(left))?;
    (paths.len() == 1).then(|| paths.remove(0))
}

fn option_takes_value(program: &str, word: &str, native_read: bool) -> bool {
    if native_read {
        return matches!(
            word,
            "-l" | "--level" | "-m" | "--max-lines" | "--tail-lines"
        );
    }
    match program {
        "head" | "tail" => matches!(word, "-n" | "--lines" | "-c" | "--bytes"),
        "nl" => matches!(
            word,
            "-b" | "--body-numbering"
                | "-s"
                | "--number-separator"
                | "-w"
                | "--number-width"
                | "-v"
                | "--starting-line-number"
                | "-i"
                | "--line-increment"
                | "-n"
                | "--number-format"
        ),
        "bat" => matches!(
            word,
            "-l" | "--language" | "--theme" | "--paging" | "--style" | "-r" | "--line-range"
        ),
        _ => false,
    }
}

fn rtk_read_flag(word: &str) -> bool {
    matches!(
        word,
        "-n" | "--line-numbers" | "--ultra-compact" | "--skip-env"
    ) || ((word.starts_with("-l") || word.starts_with("-m"))
        && !word.starts_with("--")
        && word.len() > 2)
        || ["--level=", "--max-lines=", "--tail-lines="]
            .iter()
            .any(|prefix| word.starts_with(prefix))
}

pub(super) fn is_print_range(script: &str) -> bool {
    script.strip_suffix('p').is_some_and(|range| {
        !range.is_empty()
            && range.split(',').count() <= 2
            && range.split(',').all(|number| {
                number == "$"
                    || (!number.is_empty() && number.bytes().all(|ch| ch.is_ascii_digit()))
            })
    })
}

/// Split one pipe outside quotes, rejecting other shell operators.
pub(super) fn single_pipe(text: &str) -> Option<(&str, &str)> {
    let mut rest = text.trim_start();
    while !rest.is_empty() {
        if let Some(right) = rest.strip_prefix('|') {
            let left = text[..text.len() - rest.len()].trim();
            return (!has_shell_operators(left) && !has_shell_operators(right))
                .then_some((left, right.trim()));
        }
        rest = next_word(rest)?.1;
    }
    None
}
