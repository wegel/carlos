"""Send many assistant replies to one human prompt before an interrupt."""

import uuid

PROMPT = "We got https://example.test/review"
LATEST = "The functional suite failed in test_enroll_keys_provisioned."


def start(host, message):
    if message["message"]["content"] != PROMPT:
        return False
    prompt = message["uuid"]
    host.sessions.prompt(message)
    host.active = prompt
    host.state("running")
    replies = [f"Suite progress {index}: checking the next test." for index in range(60)]
    for index, text in enumerate([*replies, LATEST]):
        record = {"type": "assistant", "session_id": host.SESSION,
                  "uuid": str(uuid.uuid4()), "user_message_uuid": prompt,
                  "message": {"id": f"progress-{index}", "role": "assistant",
                              "content": [{"type": "text", "text": text}]}}
        host.sessions.append(record)
        host.send(record)
    return True
