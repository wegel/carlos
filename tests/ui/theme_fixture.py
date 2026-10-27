"""Fixed transcript and unfinished reasoning for the 3c terminal checks."""


def theme_turn(extra_read=False):
    turn = {"id": "theme-turn", "status": "completed", "items": [
        {"type": "userMessage", "id": "theme-user-1", "content": [
            {"type": "text", "text": "Please inspect the theme"}]},
        {"type": "userMessage", "id": "theme-user-2", "content": [
            {"type": "text", "text": "Keep this second user message together"}]},
        {"type": "reasoning", "id": "theme-reasoning", "summary": [
            {"type": "summary_text", "text": "**Checking the palette**\nChecking the spacing"}]},
        {"type": "agentMessage", "id": "theme-commentary", "phase": "commentary",
         "text": "I will check the tool colors."},
        {"type": "commandExecution", "id": "theme-read", "command": "cat palette.rs",
         "aggregatedOutput": "\n".join(f"let tone_{i} = {i};" for i in range(8)), "exitCode": 0},
        {"type": "commandExecution", "id": "theme-search", "command": "rg palette src",
         "aggregatedOutput": "src/theme.rs:1:palette", "exitCode": 0},
        {"type": "toolOutput", "id": "theme-diff", "files": [
            {"filePath": "palette.rs", "diff": "@@ -1 +1 @@\n-old\n+new\n"}]},
        {"type": "commandExecution", "id": "theme-failed", "command": "theme-check",
         "aggregatedOutput": "check failed detail", "exitCode": 2},
        {"type": "commandExecution", "id": "theme-long", "command": "printf " + "界e\u0301" * 60,
         "aggregatedOutput": "long command detail", "exitCode": 0},
        {"type": "agentMessage", "id": "theme-answer", "phase": "final_answer",
         "text": "The colors are ready.\n\nThe details remain available."},
    ]}

    if extra_read:
        turn["items"].insert(5, {"type": "commandExecution", "id": "theme-read-second",
                               "command": "cat second.rs", "aggregatedOutput": "let second = 2;", "exitCode": 0})
        diff = next(i for i, item in enumerate(turn["items"]) if item["id"] == "theme-diff")
        turn["items"].insert(diff, {"type": "reasoning", "id": "theme-inline-reasoning", "summary": [
            {"type": "summary_text", "text": "**Checking results**"}]})
    return turn


def spacing_turn():
    turn = theme_turn(extra_read=True)
    items = turn["items"]
    commentary = next(item for item in items if item["id"] == "theme-commentary")
    items.remove(commentary)
    diff = next(i for i, item in enumerate(items) if item["id"] == "theme-diff")
    items.insert(diff, {"type": "reasoning", "id": "spacing-short-reasoning", "summary": [
        {"type": "summary_text", "text": "**Checking matches**"}]})
    items[-1:-1] = [
        {"type": "reasoning", "id": "spacing-long-reasoning", "summary": [
            {"type": "summary_text", "text": "First detailed thought\nSecond detailed thought"}]},
        commentary,
        {"type": "agentMessage", "id": "spacing-commentary", "phase": "commentary",
         "text": "Here is a second update."},
    ]
    return turn


def start_theme_step(server, request, params):
    thread_id = params["threadId"]
    turn = {"id": "theme-active-turn", "status": "inProgress", "items": [
        {"type": "userMessage", "id": "theme-active-user", "content": params["input"]},
        {"type": "reasoning", "id": "theme-active-reasoning", "summary": []},
    ]}
    server.threads[thread_id]["turns"].append(turn)
    server.reply(request, {"turn": {"id": turn["id"], "status": "inProgress"}})
    server.notify("turn/started", {"threadId": thread_id, "turn": turn})
    for item in turn["items"]:
        server.notify("item/started", {"threadId": thread_id, "turnId": turn["id"], "item": item})
        if item["type"] == "userMessage":
            server.notify("item/completed", {"threadId": thread_id, "turnId": turn["id"], "item": item})
    server.notify("item/reasoning/summaryTextDelta", {
        "threadId": thread_id, "turnId": turn["id"], "itemId": "theme-active-reasoning",
        "delta": "Analyzing key config files"})
