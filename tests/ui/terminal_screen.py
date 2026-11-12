"""Render overwritten wide glyphs as erased cells in the pinned terminal emulator."""

import pyte
from wcwidth import wcwidth


class TerminalScreen(pyte.Screen):
    @property
    def display(self):
        return [self.render_line(self.buffer[y]) for y in range(self.lines)]

    def render_line(self, line):
        cells = []
        skip_stub = False
        for x in range(self.columns):
            if skip_stub:
                skip_stub = False
                continue
            text = line[x].data
            if not text:
                cells.append(" ")
                continue
            if len(text) > 1:
                assert sum(map(wcwidth, text[1:])) == 0
            width = wcwidth(text[0])
            if width == 0:
                text = " "
            elif width == 2:
                trailing = line[x + 1].data if x + 1 < self.columns else " "
                if not trailing or all(wcwidth(char) == 0 for char in trailing):
                    text += trailing
                    skip_stub = True
                else:
                    text = " "
            cells.append(text)
        return "".join(cells)
