"""Prove native goal persistence without inference and explicit activation with localhost responses."""

from mock_responses import answer, outputs, tool


class NativeCodexGoals:
    def native_goal_panel(self,terminal):
        terminal.send(b"\x1bg")
        terminal.contains("Codex goal")
        terminal.wait(lambda:"Goal state: Known" in terminal.text,"native root goal snapshot")

    def native_goal_result(self,terminal,index=-1):
        request=terminal.requests("thread/goal/set")[index]
        terminal.wait(lambda:self.native_reply(terminal,request) is not None,"native goal API acknowledgement")
        response=self.native_reply(terminal,request)
        self.assertNotIn("error",response)
        return request,response["result"]["goal"]

    def test_native_codex_paused_goal_persists_and_blank_budget_keeps_native_choice_without_model_call(self):
        terminal,model=self.launch(lambda _body:self.fail("Paused goal controls must not invoke the model"))
        self.native_goal_panel(terminal)
        self.assertEqual(terminal.requests("thread/goal/set"),[])
        terminal.send(b"e")
        terminal.paste("Preserve this isolated paused goal 日本語")
        terminal.send(b"\x13")
        terminal.wait(lambda:bool(terminal.requests("thread/goal/set")),"explicit paused goal create")
        request,created=self.native_goal_result(terminal)
        self.assertEqual(request["params"]["origin"],"user")
        self.assertEqual(request["params"]["status"],"paused")
        self.assertNotIn("tokenBudget",request["params"])
        self.assertEqual(created["status"],"paused")
        self.assertEqual(created["tokensUsed"],0)
        thread=request["params"]["threadId"]
        terminal.contains("Status: paused")
        terminal.send(b"e\t")
        terminal.paste("4096")
        terminal.send(b"\x13")
        terminal.wait(lambda:len(terminal.requests("thread/goal/set"))==2,"explicit native budget edit")
        _,budgeted=self.native_goal_result(terminal)
        self.assertEqual(budgeted["tokenBudget"],4096)
        terminal.close()
        root=terminal.root/"resumed"
        root.mkdir()
        (root/"codex-home").symlink_to(terminal.root/"codex-home",target_is_directory=True)
        resumed=self.resume_native_goal(root,thread)
        self.native_goal_panel(resumed)
        resumed.contains("Preserve this isolated paused goal 日本語")
        resumed.contains("Token budget: 4096")
        self.assertEqual(resumed.requests("thread/goal/set"),[])
        resumed.send(b"e")
        resumed.paste(" revised")
        resumed.send(b"\x13")
        resumed.wait(lambda:bool(resumed.requests("thread/goal/set")),"explicit native objective edit")
        request,edited=self.native_goal_result(resumed)
        self.assertNotIn("tokenBudget",request["params"])
        self.assertEqual(request["params"]["status"],"paused")
        self.assertEqual(edited["tokenBudget"],4096)
        self.assertEqual(edited["tokensUsed"],budgeted["tokensUsed"])
        self.assertEqual(edited["createdAt"],created["createdAt"])
        self.assertEqual(model.requests,[])
        self.assertEqual(terminal.requests("turn/start"),[])
        self.assertEqual(resumed.requests("turn/start"),[])
        resumed.snapshot("native-paused-goal-persists-no-model")

    def test_native_codex_goal_activation_starts_native_work_only_after_user_confirmation(self):
        def reply(body):
            self.assertLessEqual(len(model.requests),3,"native fixture must complete without a retry loop")
            if "goal-mark-complete" in outputs(body):
                return [answer("NATIVE_GOAL_COMPLETED")]
            return [tool("goal-mark-complete","functions","update_goal",{"status":"complete"})]

        terminal,model=self.launch(reply)
        self.native_goal_panel(terminal)
        terminal.send(b"e")
        terminal.paste("Mark this isolated local fixture complete.")
        terminal.send(b"\t\t\x1b[C\x13")
        terminal.contains("may start model work immediately")
        self.assertEqual(terminal.requests("thread/goal/set"),[])
        self.assertEqual(model.requests,[])
        terminal.send(b"y")
        terminal.wait(lambda:bool(terminal.requests("thread/goal/set")),"confirmed native activation")
        request,active=self.native_goal_result(terminal)
        self.assertEqual(request["params"]["status"],"active")
        self.assertEqual(request["params"]["origin"],"user")
        terminal.contains("Status: complete")
        self.assertGreaterEqual(len(model.requests),1)
        self.assertEqual(terminal.requests("turn/start"),[])
        self.assertEqual(terminal.requests("turn/steer"),[])
        self.assertTrue(any(message.get("method")=="thread/goal/updated" and
            message["params"]["goal"]["status"]=="complete" for message in terminal.messages("server")))
        terminal.send(b"\x1b")
        terminal.contains("NATIVE_GOAL_COMPLETED")
        terminal.snapshot("native-confirmed-goal-owned-work")
