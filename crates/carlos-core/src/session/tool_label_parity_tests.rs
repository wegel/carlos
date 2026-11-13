//! Match the old helper outputs for fixed shell, tool, source and diff inputs.

mod shell {
    //! Enumerate shell parser inputs captured before moving the helpers.

    use serde_json::{json, Value};

    use crate::session::{
        shell_command, shell_directory, shell_read, shell_search, shell_words, tool_shell,
    };

    pub(super) fn cases() -> Vec<Value> {
        let mut out = Vec::new();
        commands(&mut out);
        directories(&mut out);
        searches(&mut out);
        words(&mut out);
        controls(&mut out);
        out
    }

    fn commands(out: &mut Vec<Value>) {
        for text in [
            "",
            "cat a.rs",
            "rtk read a.rs",
            "rtk proxy cat a.rs",
            "env A=1 bash -lc 'cat a.rs'",
            "rtk run -- cat a.rs",
            "bash -lc 'echo x'",
            "cat a.rs | head -3",
            "cat a.rs && cat b.rs",
            "cat 'a b.rs'",
            "sed -n '2,5p' a.rs",
            "ssh -p 22 user@host 'cat a.rs'",
            "unterminated '",
        ] {
            let command = shell_command::analyze_shell_command(text);
            let ssh = tool_shell::parse_ssh_remote_command(text);
            out.push(json!({"input":text,"display":shell_command::display_shell_command(text),
            "analyze":[command.text,command.rtk_native,command.has_rtk],
            "read_paths":shell_read::read_paths(&command),"source":shell_read::single_source_path(text),
            "word":shell_words::next_word(text),"operators":shell_words::has_shell_operators(text),
            "normalize":tool_shell::normalize_shell_command(text),
            "ssh":ssh.map(|ssh| json!([ssh.destination,ssh.remote_command])),
            "summary":tool_shell::command_summary_from_shell_cmd(text,Some("/repo")),
            "pipe":shell_read::single_pipe(text)}));
        }
    }

    fn directories(out: &mut Vec<Value>) {
        for text in [
            "cd /repo && cat a.rs",
            "cd './a b' && python3 process.py",
            "cd -- -dash && true",
            "cd $HOME && cat a",
            "cd ../a && true",
            "cd /a; true",
            "bash -lc 'cd /a && true'",
            "bash -lc '$HOME'",
            "cd /a &&",
            "cd /a &&\ncat a.rs",
        ] {
            let prefix = shell_directory::directory_prefix(text);
            out.push(
                json!({"input":text,"directory":prefix.map(|p|json!([p.directory,p.body])),
            "literal":shell_directory::literal_shell_payload(text)}),
            );
        }
    }

    fn searches(out: &mut Vec<Value>) {
        for words in [
            vec!["rg", "-n", "needle", "src"],
            vec!["grep", "-E", "pat", "a"],
            vec!["rg", "-e", "pat", "--", "-file"],
            vec!["rg", "-f", "patterns", "src"],
            vec!["rg", "--regexp=pat", "a"],
            vec!["rg", "-e"],
            vec![],
        ] {
            let words: Vec<String> = words.into_iter().map(str::to_owned).collect();
            out.push(json!({"input":words,"search":shell_search::search_operands(&words)}));
        }
    }

    fn words(out: &mut Vec<Value>) {
        for word in [
            "",
            "A=1",
            "_A=",
            "1A=x",
            "A-B=x",
            "abc",
            "/bin/cat",
            "a/",
            "'quoted'",
            "\"quoted\"",
        ] {
            out.push(json!({"input":word,"assignment":shell_words::is_assignment(word),
            "program":shell_words::program_name(word),"unquote":tool_shell::strip_shell_quotes(word),
            "path":tool_shell::compact_command_path(word,Some("/repo"))}));
        }
    }

    fn controls(out: &mut Vec<Value>) {
        for text in [
            "1,20p",
            "p",
            "20p",
            "1,$p",
            "1,20d",
            "1,20p;3p",
            "plain\r\ntext",
            "\u{1b}[31mred\u{1b}[0m\u{1b}[2J",
            "\u{1b}]0;title\u{7}ok",
            "\u{009b}31mred\u{009b}0m",
            "nul\0tab\ttext",
        ] {
            out.push(json!({"input":text,"range":shell_read::is_print_range(text),
            "plain":tool_shell::strip_terminal_controls(text),"sgr":tool_shell::strip_terminal_controls_preserving_sgr(text)}));
        }
    }
}

mod items {
    //! Enumerate plain tool facts captured before moving the helpers.

    use std::path::Path;

    use serde_json::{json, Value};

    use crate::session::models::{EntryDraft, MessageKind, Role};
    use crate::session::transcript::Transcript;
    use crate::session::{
        read_summary, summary_request, tool_command_view::ToolCommandView, tool_diff,
        tool_diff_summary, tool_format, tool_source, tools,
    };

    pub(super) fn cases() -> Vec<Value> {
        let mut out = Vec::new();
        items(&mut out);
        values(&mut out);
        kinds(&mut out);
        reads(&mut out);
        diffs(&mut out);
        out
    }

    fn items(out: &mut Vec<Value>) {
        for item in [
            json!(null),
            json!({}),
            json!({"type":"commandExecution","command":["bash","-lc","cat a.rs"],"output":"body","exitCode":0}),
            json!({"type":"commandExecution","command":["echo","a b"],"exit_code":2}),
            json!({"type":"commandExecution","command":"cd /repo/scripts && python3 process.py","cwd":"/repo","aggregatedOutput":"x"}),
            json!({"type":"toolCall","tool":"Read","input":{"file_path":"a.rs","offset":2,"limit":4}}),
            json!({"type":"toolCall","tool":"Read","path":"a.rs","isError":true}),
            json!({"type":"commandExecution","command":"cat a.rs","stderr":"warning"}),
            json!({"type":"commandExecution","command":"cat a.rs","commandActions":[{"type":"read","path":"b.rs"}]}),
            json!({"type":"commandExecution","command":"cat a.rs","commandActions":[{"type":"read","path":"b.rs"},{"type":"read","path":"c.rs"}]}),
            json!({"type":"toolCall","tool":"Search","input":{"pattern":"needle","path":"src"},"description":"find symbols","reasoning":"look here"}),
            json!({"type":"toolOutput","diff":"--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n","filePath":"a.rs"}),
            json!({"type":"function_call","call_id":"x","name":"exec_command","arguments":"{\"cmd\":\"printf ok\"}"}),
            json!({"type":"function_call_output","call_id":"x","output":"@@ -1 +1 @@\n-a\n+b"}),
            json!({"content":[{"type":"text","text":"one"},{"type":"image","text":"skip"},{"type":"output_text","text":"two"}]}),
        ] {
            out.push(item_case(item));
        }
    }

    fn values(out: &mut Vec<Value>) {
        for value in [
            Value::Null,
            json!(""),
            json!("bad json"),
            json!("{\"x\":1}"),
            json!({"x":1,"empty":"","bool":false}),
            json!([1, 2, 3]),
            json!(7),
        ] {
            let object = value.as_object();
            out.push(json!({"input":value,"arguments":tools::parse_arguments_value(&value),
            "compact":tool_format::compact_json_summary(&value,12),"inline":tool_format::inline_value(&value),
            "brackets":object.and_then(|o|tool_format::format_input_brackets(o,&["empty"])),
            "diff_path":object.and_then(tool_diff::infer_file_path_from_object)}));
        }
    }

    fn kinds(out: &mut Vec<Value>) {
        for kind in [
            "commandExecution",
            "toolCall",
            "tool_call",
            "mcpToolCall",
            "functionCallOutput",
            "toolOutput",
            "tool_result",
            "mcpToolResult",
            "future",
        ] {
            out.push(json!({"input":kind,"call":tools::is_tool_call_type(kind),"output":tools::is_tool_output_type(kind),
            "role":tools::role_for_tool_type(kind).map(|r|format!("{r:?}")),
            "title":tool_format::titlecase_tool_name(kind),"icon":tool_format::tool_icon(kind)}));
        }
    }

    fn reads(out: &mut Vec<Value>) {
        for text in [
            "",
            "→ Read",
            " → Read a.rs [offset=2] ",
            "→ Read a.rs ×0",
            "→ Read a.rs ×3",
            "→ Read a.rs ×bad",
            "→ Read a.rs (in dir [a]) ×2",
            "read a.rs",
        ] {
            out.push(json!({"input":text,"path":read_summary::read_summary_path(text),"read":read_summary::parse_read_summary(text),
            "one":read_summary::format_read_summary_with_count(text,1),"three":read_summary::format_read_summary_with_count(text,3)}));
        }
    }

    fn diffs(out: &mut Vec<Value>) {
        for (path, text) in [
            (None, ""),
            (None, "--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-old\n+new\n"),
            (Some("named.rs"), "+a\n-b\n"),
            (None, "--- a/gone.rs\n+++ /dev/null\n-old\n"),
            (None, "diff --git a/a b/a\n+a\ndiff --git a/b b/b\n-b\n"),
        ] {
            let mut transcript = Transcript::new();
            let mut draft = EntryDraft::plain(Role::ToolOutput, text);
            draft.kind = MessageKind::Diff;
            draft.file_path = path.map(str::to_owned);
            transcript.append(draft);
            out.push(json!({"input":[path,text],"diff":tool_diff_summary::diff_summary(&transcript[0]),
            "split":tool_diff_summary::split_file_diffs(text),"probably":tool_diff::is_probably_diff_text(text)}));
        }
    }

    fn item_case(item: Value) -> Value {
        let input = summary_request::summary_input(&item, Path::new("/repo"));
        let view = ToolCommandView::from_item(&item, Path::new("/repo"));
        let diffs = tool_diff::extract_diff_blocks(&item)
            .into_iter()
            .map(|d| json!([d.file_path, d.diff]))
            .collect::<Vec<_>>();
        json!({"input":item,"command":tools::tool_command(&item),"name":tools::tool_name(&item),
        "reason":tools::tool_reasoning(&item),"description":tools::tool_description(&item),"output":tools::tool_output_text(&item),
        "text":tools::item_text_from_content(&item),"parts":item["content"].as_array().map(|c|tools::collect_text_parts(c)),
        "path":tools::value_at_path(&item,&["input","path"]),"string":tools::first_string_at_paths(&item,&[&["path"],&["input","file_path"]]),
        "integer":tools::first_i64_at_paths(&item,&[&["exitCode"],&["exit_code"]]),
        "action":crate::session::tool_shell::command_execution_action_command(&item),
        "call":tool_format::format_tool_item(&item,Role::ToolCall),"result":tool_format::format_tool_item(&item,Role::ToolOutput),
        "input_summary":tool_format::tool_input_summary(&item),"input_object":tool_format::tool_input_object(&item),
        "inline_call":tool_format::format_tool_call_inline(&item,"Read"),"parsed":tool_format::command_summary_from_parsed_cmd(&item),
        "raw_call":tools::raw_function_call_to_tool_item(&item),"raw_output":tools::raw_function_call_output_to_tool_item(&item),
        "source":tool_source::source_path_from_item(&item),"diffs":diffs,"command_diff":tool_diff::command_execution_diff_output(&item),
        "has_command":summary_request::has_command(&item),
        "summary":input.map(|i|json!([i.key,i.command,i.location.cwd,i.location.read_root,i.location.directory,i.file_based])),
        "view":view.map(|v|json!([v.can_classify(),v.label("→ Read a.rs".into()),v.command,v.cwd,v.read_root,v.directory]))})
    }
}

#[test]
fn plain_helpers_match_previous_commit_cases() {
    let expected: Vec<serde_json::Value> = serde_json::from_str(include_str!(
        "../../../../src/tests/fixtures/tool_labels_previous.json"
    ))
    .expect("previous helper outputs");
    let actual: Vec<_> = shell::cases().into_iter().chain(items::cases()).collect();
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(
            actual, expected,
            "helper case {index}, input: {}",
            actual["input"]
        );
    }
}
