"""Keep a large public forked history complete without per-item RPC amplification."""

import json
import time

from codex_history_fixture import write_history
from mock_responses import Model
from terminal import Terminal
from test_codex_contract import Codex


def check_older_windows(case, terminal, model):
    """Read the prefix on demand and keep exact native rewind anchors."""
    pages = terminal.requests("thread/turns/list")
    case.assertEqual(len(pages), 1, "startup reads only the newest complete native window")
    case.assertEqual(pages[0]["params"]["sortDirection"], "desc")
    case.assertEqual(terminal.requests("thread/items/list"), [])
    terminal.rewind("history human 104")
    terminal.send(b"\x1b[A")
    terminal.wait(lambda: terminal.input_is("history human 103"), "exact adjacent paged human")
    terminal.send(b"\x1b")
    terminal.wait(lambda: terminal.input_is(""), "rewind closes before transcript Home")
    for count in (1, 2):
        terminal.send(b"\x1b[H")
        terminal.wait(lambda: len(terminal.daemon_requests("session/history")) == count,
                      "one explicit older native history read")
        request_id = terminal.daemon_requests("session/history")[-1]["id"]
        terminal.wait(lambda: any(message.get("id") == request_id
                      for message in terminal.daemon_messages("server")), "older native page replied")
        response = next(message for message in terminal.daemon_messages("server")
                        if message.get("id") == request_id)
        case.assertIn("result", response)
        case.assertTrue(response["result"]["entries"])

    def oldest_visible():
        if "history human 0" in terminal.text:
            return True
        # The recorder sees a reply before the client applies it. Home stays idempotent.
        terminal.send(b"\x1b[H")
        return False

    terminal.wait(oldest_visible, "the client applies the older page and scrolls to the first human")
    case.assertEqual(len(terminal.requests("thread/turns/list")), 106)
    case.assertEqual(terminal.requests("thread/items/list"), [])
    case.assertEqual(model.requests, [])


def check_history(case, binary, codex, root, socket_mode=False):
    root.mkdir(parents=True)
    model = Model(root, lambda _body: case.fail("History must never call a model"))
    case.addCleanup(model.close)
    source, turns, path = write_history(root, count=40, commands_per_turn=55,
        compact=True, large=True, large_indices=(1,3,29), large_bytes=24*1024*1024, provider="fixture")
    original = path.read_bytes()
    peer = Codex(str(codex),root)
    try:
        peer.call("initialize",{"clientInfo":{"name":"carlos-history-fixture","version":"1"},
            "capabilities":{"experimentalApi":True}})
        peer.send({"method":"initialized"})
        fork = peer.call("thread/fork",{"threadId":source,"beforeTurnId":turns[31],"excludeTurns":True})["thread"]
    finally:
        peer.close()
    (root/"rpc.jsonl").rename(root/"fixture-setup-rpc.jsonl")
    source_after_fork = path.read_bytes()
    case.assertTrue(source_after_fork.startswith(original),"native fork must retain original conversation")
    started = time.monotonic()
    if socket_mode:
        from native_shared_codex import attach_shared
        terminal,server,_endpoint = attach_shared(case,binary,codex,root,resume=fork["id"],width=140)
    else:
        terminal = Terminal(binary, root, resume=fork["id"], codex=codex, width=140)
        case.addCleanup(terminal.close)
    terminal.contains("HISTORY_REPLY:29")
    terminal.wait(lambda:len(terminal.requests("thread/read")) >= 2,"complete atomic rebuild")
    elapsed = time.monotonic() - started
    pages = terminal.requests("thread/turns/list")
    case.assertEqual(len(pages),1)
    case.assertEqual(pages[0]["params"]["itemsView"],"full")
    case.assertEqual(pages[0]["params"]["sortDirection"],"desc")
    case.assertEqual(terminal.requests("thread/items/list"),[])
    response = next(row for row in terminal.messages("server") if row.get("id") == pages[0]["id"] and "method" not in row)
    case.assertGreater(len(json.dumps(response)),8*1024*1024,"exercise a real oversized public history page")
    case.assertEqual([turn["id"] for turn in response["result"]["data"]], [turns[30]])
    case.assertEqual(pages[0]["params"]["limit"], 1)
    projected, response_bytes = check_bounded_prefix(case, terminal, turns)
    case.assertEqual(terminal.requests("thread/fork"),[])
    case.assertEqual(terminal.requests("turn/start"),[])
    case.assertEqual(model.requests,[])
    case.assertLess(elapsed,10,"large forked history must fit the unchanged20s deadline")
    terminal.snapshot("large-native-ancestor-history")
    terminal.close()
    case.assertTrue(path.read_bytes().startswith(source_after_fork),"Carlos must not rewrite ancestor bytes")
    if socket_mode:
        case.assertIsNone(server.poll(),"Carlos must leave the shared native server alive")
        records=[json.loads(line) for line in (root/"rpc.jsonl").read_text().splitlines()]
        case.assertTrue(all(row["connection"] == 1 for row in records),"one exact native socket connection")
    (root/"history-load-proof.json").write_text(json.dumps({"readySeconds":elapsed,
        "turns":len(projected),"items":sum(len(turn["items"]) for turn in projected),
        "responseBytes":response_bytes,"turnPageRequests":len(pages),
        "transport":"unix-websocket" if socket_mode else "stdio"},indent=2))


def check_bounded_prefix(case, terminal, turns):
    """Read every complete ancestor turn without combining their large replies."""
    terminal.rewind("history human 29")
    terminal.send(b"\x1b[A")
    terminal.wait(lambda:terminal.input_is("history human 28"),"canonical adjacent rewind anchor")
    terminal.send(b"\x1b")
    terminal.wait(lambda: terminal.input_is(""), "rewind closes before older history")
    def oldest_visible():
        if "history human 0" in terminal.text:
            return True
        terminal.send(b"\x1b[H")
        return False
    terminal.wait(oldest_visible, "all bounded turn pages retain the complete forked ancestor")
    records = terminal.messages("server")
    pages = terminal.requests("thread/turns/list")
    replies = {row["id"]:row for row in records if "id" in row and "method" not in row}
    projected = [turn for request in reversed(pages) for turn in replies[request["id"]]["result"]["data"]]
    case.assertEqual([turn["id"] for turn in projected], turns[:31])
    case.assertEqual(len([item for turn in projected for item in turn["items"]]),30*57+1)
    response_bytes = sum(len(json.dumps(replies[request["id"]])) for request in pages)
    case.assertGreater(response_bytes,64*1024*1024,"combined history exceeds the unchanged frame cap")
    case.assertTrue(all(request["params"]["limit"] == 1 for request in pages))
    case.assertTrue(all(len(json.dumps(replies[request["id"]])) < 64*1024*1024 for request in pages))
    case.assertNotIn("HISTORY_REPLY:30",json.dumps(projected),"the excluded tail must stay excluded")
    return projected, response_bytes
