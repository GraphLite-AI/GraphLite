from __future__ import annotations

import sys
import threading
import time
from typing import Optional, Tuple

_ANSI_COLORS = {
    "mauve": "\033[38;5;141m",
    "peach": "\033[38;5;209m",
    "sky": "\033[38;5;117m",
    "teal": "\033[38;5;37m",
    "blue": "\033[34m",
    "white": "\033[37m",
    "green": "\033[32m",
    "red": "\033[31m",
    "reset": "\033[0m",
    "italic": "\033[3m",
}


def style(text: str, color: str, enabled: bool, *, italic: bool = False) -> str:
    if not enabled:
        return text
    code = _ANSI_COLORS.get("blue" if italic else color, "")
    italic_code = _ANSI_COLORS["italic"] if italic else ""
    if not (code or italic_code):
        return text
    return f"{italic_code}{code}{text}{_ANSI_COLORS['reset']}"


class Spinner:
    """Lightweight terminal spinner for live status updates."""

    def __init__(self, enabled: bool = True, color: str = "mauve") -> None:
        self.enabled = enabled and sys.stdout.isatty()
        self.color = color
        self._text = ""
        self._parts: Optional[Tuple[int, str]] = None
        self._stop = threading.Event()
        self._thread: Optional[threading.Thread] = None
        self._last_len = 0

    def start(self, initial: str = "") -> None:
        self._text = initial
        self._parts = self._split_attempt(initial)
        if not self.enabled:
            return
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def update(self, text: str) -> None:
        self._text = text
        self._parts = self._split_attempt(text)

    def stop(self, final: Optional[str] = None, color: Optional[str] = None) -> None:
        if self.enabled:
            self._stop.set()
            if self._thread:
                self._thread.join(timeout=0.5)
            sys.stdout.write("\r" + " " * self._last_len + "\r")
            sys.stdout.flush()
        if final:
            if self.enabled and color:
                print(style(final, color, True))
            elif self.enabled:
                print(style(final, "green", True))
            else:
                print(final)

    @staticmethod
    def _split_attempt(text: str) -> Optional[Tuple[int, str]]:
        if text.startswith("[attempt") and "]" in text:
            end = text.find("]") + 1
            prefix = text[:end]
            rest = text[end:].lstrip()
            try:
                num_part = prefix.strip("[]").split()[1]
                num = int(num_part)
            except Exception:
                return None
            return num, rest
        return None

    def _run(self) -> None:
        frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]
        idx = 0
        while not self._stop.is_set():
            frame = frames[idx % len(frames)]
            frame_col = style(frame, self.color, self.enabled)
            if self._parts:
                _, rest = self._parts
                rest_col = style(rest, "white", self.enabled, italic=True)
                text_col = rest_col
            else:
                text_col = style(self._text, "white", self.enabled, italic=True)
            line = f"\r{frame_col} {text_col}"
            self._last_len = max(self._last_len, len(line))
            sys.stdout.write(line + " " * max(0, self._last_len - len(line)))
            sys.stdout.flush()
            idx += 1
            time.sleep(0.08)


__all__ = ["Spinner", "style"]


