"""Check real screen columns, complete narrow contents, and the streamed source wire."""
from markdown_table_fixture import FIRST, LAST, NARROW


class MarkdownTables:
    def assert_table_grid(self,terminal,columns):
        def aligned():
            rows = []
            for y,line in enumerate(terminal.screen.display):
                if "│" not in line: continue
                positions = tuple(x for x in range(terminal.screen.columns) if terminal.screen.buffer[y][x].data == "│")
                if positions: rows.append(positions)
            return len(rows)>=2 and len(rows[0])==columns+1 and all(row==rows[0] for row in rows)
        terminal.wait(aligned,"table cell continuations share fixed column boundaries")
        terminal.contains("┌")
        terminal.contains("└")

    def test_markdown_table_cells_reflow_while_streaming_and_resizing(self):
        terminal = self.launch(mode="markdown-table",resume=None,width=120)
        terminal.wait(lambda: terminal.input_is(""),"table session ready")
        terminal.paste("Show wrapped table")
        terminal.send(b"\r")
        terminal.contains("CELL_END")
        self.assert_table_grid(terminal,2)
        terminal.snapshot("table-stream-desktop-120")
        for width in [54,36]:
            terminal.resize(width,32)
            terminal.contains("CELL_END")
            self.assert_table_grid(terminal,2)
            terminal.snapshot("table-stream-phone-"+str(width))
        (terminal.root/"finish-table").touch()
        terminal.contains("BETA_DONE")
        self.assert_table_grid(terminal,2)
        terminal.resize(120,32)
        terminal.contains("BETA_DONE")
        terminal.wait(lambda: any("┌" in line and terminal.screen.buffer[y][100].data=="─"
                      for y,line in enumerate(terminal.screen.display)),"table expands back to desktop width")
        self.assert_table_grid(terminal,2)
        terminal.snapshot("table-completed-desktop")
        deltas = [m["params"]["delta"] for m in terminal.messages("server") if m.get("method")=="item/agentMessage/delta"]
        self.assertEqual(deltas,[FIRST,LAST])
        self.assertEqual(len(terminal.requests("turn/start")),1)

    def test_markdown_table_phone_fallback_preserves_every_label_and_value(self):
        terminal = self.launch(mode="markdown-table",resume=None,width=18)
        terminal.wait(lambda: terminal.input_is(""),"narrow table session ready")
        terminal.paste("Show narrow table")
        terminal.send(b"\r")
        for field,value in [("First","VALUE_A"),("Second","VALUE_B"),("Third","VALUE_C"),("Fourth","VALUE_D")]:
            terminal.contains(field+":")
            terminal.contains(value)
        terminal.contains("Row 1")
        self.assertNotIn("┌",terminal.text)
        terminal.snapshot("table-labels-phone-18")
        terminal.resize(100,32)
        self.assert_table_grid(terminal,4)
        terminal.snapshot("table-labels-to-desktop")
        self.assertEqual([m["params"]["delta"] for m in terminal.messages("server") if m.get("method")=="item/agentMessage/delta"],[NARROW])
        self.assertEqual(len(terminal.requests("turn/start")),1)
