"""Publish actual native Markdown code so XCTest can inspect its colors and original copy."""

SOURCE = 'Native code example.\n\n```rust\n// hello 🌍\nfn main() {\n    let count = 42;\n    println!("hello 🦀 {count}");\n}\n```\n\n```unknown-language\nkeep literal 🦀\n```\n'


def configure_syntax(native):
    original = native.prompt
    original_history = native.history

    def history():
        thread = original_history()
        thread["turns"] = [{"id": "previous-turn", "status": "completed", "items": [
            {"type": "agentMessage", "id": "previous-answer", "text": "Earlier native answer.", "phase": "final_answer"}]}]
        return thread

    native.history = history

    async def prompt(link, request):
        if request["params"]["input"][0]["text"] != "phone syntax":
            await original(link, request)
            return
        native.prompts += 1
        await native.notify(link, "turn/started", turn={"id": "live"})
        await native.notify(link, "item/completed", turnId="live", item={
            "type": "commandExecution", "id": "phone-file-source", "command": "cat src/demo.rs",
            "commandActions": [{"type": "read", "path": "src/demo.rs", "name": "demo.rs"}],
            "aggregatedOutput": ' 40→fn main() {\n 41→    let count = 42;\n 42→    println!("hello 🦀");\n 43→}\n',
            "exitCode": 0, "status": "completed"})
        await native.notify(link, "item/agentMessage/delta", turnId="live",
                            itemId="phone-code-answer", delta=SOURCE)
        await native.notify(link, "turn/completed", turn={"id": "live", "status": "completed"})

    native.prompt = prompt
    return {"syntaxSource": SOURCE}
