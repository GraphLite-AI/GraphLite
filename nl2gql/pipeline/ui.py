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

    _STAGES: Tuple[Tuple[str, Tuple[str, ...]], ...] = (
        ("preprocess", ("preprocess",)),
        ("intent/links", ("planning intent", "reusing intent", "intent", "link")),
        ("generate", ("generating candidates", "generate candidates", "generator")),
        ("validate", ("evaluating", "validation", "scoring", "syntax", "logic")),
        ("finalize", ("success", "finalizing", "finalize")),
    )

    def __init__(self, enabled: bool = True, color: str = "mauve") -> None:
        self.enabled = enabled and sys.stdout.isatty()
        self.color = color
        self._text = ""
        self._parts: Optional[Tuple[int, str]] = None
        self._stage_idx: Optional[int] = None
        self._has_stage_line = False
        self._stop = threading.Event()
        self._thread: Optional[threading.Thread] = None
        self._last_len = 0

    def start(self, initial: str = "") -> None:
        self._text = initial
        self._parts = self._split_attempt(initial)
        self._stage_idx = self._detect_stage(self._parts[1] if self._parts else initial)
        if not self.enabled:
            return
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def update(self, text: str) -> None:
        self._text = text
        self._parts = self._split_attempt(text)
        self._stage_idx = self._detect_stage(self._parts[1] if self._parts else text)

    def stop(self, final: Optional[str] = None, color: Optional[str] = None) -> None:
        if self.enabled:
            self._stop.set()
            if self._thread:
                self._thread.join(timeout=0.5)
            if self._has_stage_line:
                sys.stdout.write("\r\033[K")  # clear spinner line
                sys.stdout.write("\033[F\033[K")  # move up, clear stage line
            else:
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

    def _detect_stage(self, text: str) -> Optional[int]:
        lower = text.lower()
        for idx, (_, tokens) in enumerate(self._STAGES):
            if any(token in lower for token in tokens):
                return idx
        return None

    def _render_stage_line(self) -> str:
        if self._stage_idx is None:
            return ""
        parts = []
        for idx, (label, _) in enumerate(self._STAGES):
            if idx < self._stage_idx:
                icon, col = "✓", "green"
            elif idx == self._stage_idx:
                icon, col = "➤", self.color
            else:
                icon, col = "·", "sky"
            connector = "├"
            piece = f"{connector} {icon} {label}"
            parts.append(style(piece, col, self.enabled))
        return "  ".join(parts)

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
            stage_line = self._render_stage_line()
            base_line = f"{frame_col} {text_col}"
            if stage_line:
                # Draw completion status on its own line, above the italic spinner text.
                sys.stdout.write("\r")
                if self._has_stage_line:
                    sys.stdout.write("\033[F")  # move up to stage line
                sys.stdout.write("\033[K" + stage_line + "\n")  # clear + write stage line
                sys.stdout.write("\033[K" + base_line)  # clear + write spinner line
                self._has_stage_line = True
            else:
                line = f"\r{base_line}"
                self._last_len = max(self._last_len, len(line))
                sys.stdout.write(line + " " * max(0, self._last_len - len(line)))
                self._has_stage_line = False
            sys.stdout.flush()
            idx += 1
            time.sleep(0.08)


__all__ = ["Spinner", "style"]



