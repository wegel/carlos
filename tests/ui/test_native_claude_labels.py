"""Prove captured human steer UUIDs with the unmodified CLI and a paused localhost model step."""


class NativeClaudeLabels:
    def wait_label_result(self,terminal,uuid):
        def accepted():
            return next((row for row in reversed(self.results(terminal)) if not row.get("is_error") and
                uuid in row.get("user_message_uuids",[row.get("user_message_uuid")])),None)
        terminal.wait(lambda:accepted() is not None,"native result names the captured human UUID",timeout=25)
        self.wait_native_idle(terminal,0)
        return accepted()

    def classified_history(self,terminal):
        return [record for frame in self.helper_messages(terminal,"server")
            if frame.get("type") == "reply" and frame.get("ok") and isinstance(frame.get("result"),dict)
            for record in frame["result"].get("messages",[]) if record.get("inputKind") == "steer"]

    def test_native_steer_label_survives_paused_work_resume_and_validated_fork(self):
        terminal = self.start("context")
        self.submit(terminal,"Hold the native label fixture first human")
        terminal.wait(lambda:len(self.api.requests) == 2,"native second model step paused",timeout=20)
        first = self.helper_requests(terminal,"prompt")[0]["params"]
        self.assertEqual(first["inputKind"],"prompt")
        self.assertNotIn("you (steer)",terminal.text)
        self.submit(terminal,"Steer the native label fixture é🦊")
        terminal.contains("you (steer)")
        steer = self.helper_requests(terminal,"prompt")[1]["params"]
        self.assertEqual(steer["inputKind"],"steer")
        self.assertEqual(steer["priority"],"now")
        self.assertNotEqual(steer["uuid"],first["uuid"])
        self.api.context_release.set()
        completed = self.wait_label_result(terminal,steer["uuid"])
        session = completed["session_id"]
        terminal.close()
        resumed = self.start(resume=session)
        resumed.contains("you (steer)")
        records = self.classified_history(resumed)
        self.assertTrue(any(row["message"]["uuid"] == steer["uuid"] and row["source"] == "human" for row in records))
        self.assertTrue(all("inputKind" not in row["message"] for row in records))
        self.submit(resumed,"Cut after the retained native steer")
        resumed.wait(lambda:len(self.helper_requests(resumed,"prompt")) == 3,"third native human UUID captured")
        selected = self.helper_requests(resumed,"prompt")[-1]["params"]["uuid"]
        self.wait_label_result(resumed,selected)
        path,before = self.source_bytes(resumed,session)
        resumed.rewind("Cut after the retained native steer")
        edited = "Edit the native tail after its steer"
        resumed.replace_input(edited)
        resumed.send(b"\r")
        resumed.wait(lambda:len(self.helper_requests(resumed,"prompt")) == 4,"edited branch dispatched once",timeout=20)
        edited_uuid = self.helper_requests(resumed,"prompt")[-1]["params"]["uuid"]
        branch = self.wait_label_result(resumed,edited_uuid)
        self.assertNotEqual(branch["session_id"],session)
        self.assertEqual(self.helper_requests(resumed,"sessions.forkBefore")[-1]["params"]["beforeMessageId"],selected)
        copied = self.classified_history(resumed)[-1]
        self.assertNotEqual(copied["message"]["uuid"],steer["uuid"])
        self.assertEqual(copied["source"],"human")
        self.assertEqual(resumed.text.count("you (steer)"),1)
        self.source_unchanged(path,before,edited,branch["result"])
        self.assertLessEqual(len(self.api.requests),6,"bounded local fixture requests only")
        resumed.snapshot("native-steer-resumed-and-remapped")
