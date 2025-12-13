from __future__ import annotations

import sys
import threading
import time
from dataclasses import dataclass, field
from typing import Dict, List, Optional, Tuple

# ═══════════════════════════════════════════════════════════════════════════════
# ANSI Colors - Catppuccin-inspired palette
# ═══════════════════════════════════════════════════════════════════════════════

_ANSI = {
    # Core palette
    "mauve": "\033[38;5;141m",      # Purple - primary accent
    "peach": "\033[38;5;209m",      # Orange - warnings
    "sky": "\033[38;5;117m",        # Light blue - info
    "teal": "\033[38;5;37m",        # Cyan - success secondary
    "blue": "\033[38;5;75m",        # Blue - links
    "lavender": "\033[38;5;183m",   # Light purple - subtle
    "pink": "\033[38;5;218m",       # Pink - highlights
    "flamingo": "\033[38;5;210m",   # Salmon - warm accent
    "rosewater": "\033[38;5;224m",  # Light pink - soft
    "maroon": "\033[38;5;131m",     # Dark red - errors
    "yellow": "\033[38;5;221m",     # Yellow - caution
    "sapphire": "\033[38;5;74m",    # Deep blue
    
    # Standard
    "green": "\033[38;5;114m",      # Success green
    "red": "\033[38;5;203m",        # Error red  
    "white": "\033[38;5;255m",      # Bright white
    "gray": "\033[38;5;245m",       # Muted gray
    "dim": "\033[38;5;240m",        # Very dim
    
    # Formatting
    "reset": "\033[0m",
    "bold": "\033[1m",
    "italic": "\033[3m",
    "dim_fmt": "\033[2m",
}


def style(text: str, color: str, enabled: bool, *, italic: bool = False, bold: bool = False, dim: bool = False) -> str:
    if not enabled:
        return text
    parts = []
    if bold:
        parts.append(_ANSI["bold"])
    if italic:
        parts.append(_ANSI["italic"])
    if dim:
        parts.append(_ANSI["dim_fmt"])
    parts.append(_ANSI.get(color, ""))
    prefix = "".join(parts)
    if not prefix:
        return text
    return f"{prefix}{text}{_ANSI['reset']}"


def icon(name: str, enabled: bool = True) -> str:
    """Get a styled icon."""
    icons = {
        "check": ("✓", "green"),
        "cross": ("✗", "red"),
        "arrow": ("➤", "mauve"),
        "dot": ("●", "gray"),
        "hollow": ("○", "dim"),
        "spin": ("◐", "mauve"),
        "spark": ("✦", "yellow"),
        "bolt": ("⚡", "yellow"),
        "brain": ("🧠", ""),
        "link": ("🔗", ""),
        "hammer": ("🔨", ""),
        "eye": ("👁", ""),
        "rocket": ("🚀", ""),
        "gear": ("⚙", ""),
        "magnify": ("🔍", ""),
        "fix": ("🔧", ""),
    }
    char, color = icons.get(name, ("?", "white"))
    if color and enabled:
        return style(char, color, enabled)
    return char


# ═══════════════════════════════════════════════════════════════════════════════
# Stage Progress Tracker
# ═══════════════════════════════════════════════════════════════════════════════

@dataclass
class StageInfo:
    """Information about a pipeline stage."""
    name: str
    label: str
    icon_pending: str = "○"
    icon_active: str = "◐"
    icon_done: str = "✓"
    icon_error: str = "✗"
    detail: str = ""
    
    
@dataclass  
class StageProgress:
    """Track progress through pipeline stages."""
    stages: List[StageInfo] = field(default_factory=list)
    current_idx: int = -1
    current_detail: str = ""
    attempt: int = 1
    errors: List[str] = field(default_factory=list)
    fixes: List[str] = field(default_factory=list)
    nl_query: str = ""
    
    def __post_init__(self):
        if not self.stages:
            self.stages = [
                StageInfo("understand", "Understanding", detail="parsing intent & schema"),
                StageInfo("contract", "Contract", detail="building requirements"),
                StageInfo("generate", "Generation", detail="LLM candidate generation"),
                StageInfo("validate", "Validation", detail="checking constraints"),
                StageInfo("repair", "Repair", detail="fixing issues"),
                StageInfo("finalize", "Final", detail="selecting best result"),
            ]
    
    def set_stage(self, name: str, detail: str = "") -> None:
        for idx, stage in enumerate(self.stages):
            if stage.name == name:
                self.current_idx = idx
                self.current_detail = detail or stage.detail
                return
    
    def add_error(self, error: str) -> None:
        if error and error not in self.errors:
            self.errors.append(error)
    
    def add_fix(self, fix: str) -> None:
        if fix and fix not in self.fixes:
            self.fixes.append(fix)
            
    def clear_errors(self) -> None:
        self.errors = []
        self.fixes = []


# ═══════════════════════════════════════════════════════════════════════════════
# Live Spinner with Rich Progress Display
# ═══════════════════════════════════════════════════════════════════════════════

class Spinner:
    """Rich terminal spinner with stage-based progress display."""
    
    _SPIN_FRAMES = ["◜", "◠", "◝", "◞", "◡", "◟"]
    _PULSE_FRAMES = ["░", "▒", "▓", "█", "▓", "▒"]
    
    def __init__(self, enabled: bool = True, color: str = "mauve") -> None:
        self.enabled = enabled and sys.stdout.isatty()
        self.color = color
        self.progress = StageProgress()
        self._text = ""
        self._stop = threading.Event()
        self._thread: Optional[threading.Thread] = None
        self._line_count = 0
        self._first_render = True
        self._lock = threading.Lock()
        
    def start(self, initial: str = "", nl_query: str = "") -> None:
        self._text = initial
        self.progress = StageProgress()
        self.progress.nl_query = nl_query
        self._line_count = 0
        self._first_render = True
        if not self.enabled:
            return
        # Clear screen, scrollback buffer, and move cursor to top for fresh start
        # Use multiple strategies to ensure full clearing
        sys.stdout.write("\n" * 50)  # Push old content up
        sys.stdout.write("\033[2J\033[3J\033[H")  # Clear screen + scrollback + home
        sys.stdout.flush()
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()
    
    def update(self, text: str) -> None:
        """Update spinner with text and auto-detect stage from text content."""
        with self._lock:
            self._text = text
            self._detect_and_set_stage(text)
    
    def set_stage(self, stage: str, detail: str = "") -> None:
        """Explicitly set the current stage."""
        with self._lock:
            self.progress.set_stage(stage, detail)
    
    def set_attempt(self, attempt: int) -> None:
        """Set the current attempt number."""
        with self._lock:
            self.progress.attempt = attempt
            self.progress.clear_errors()
    
    def add_error(self, error: str) -> None:
        """Add an error to display."""
        with self._lock:
            self.progress.add_error(error)
    
    def add_fix(self, fix: str) -> None:
        """Add a fix to display."""
        with self._lock:
            self.progress.add_fix(fix)
    
    def stop(self, final: Optional[str] = None, color: Optional[str] = None) -> None:
        if self.enabled:
            self._stop.set()
            if self._thread:
                self._thread.join(timeout=0.5)
            # Clear screen, scrollback, and reset cursor completely
            sys.stdout.write("\n" * 50)  # Push any remaining content up
            sys.stdout.write("\033[2J\033[3J\033[H")  # Clear everything
            sys.stdout.flush()
        if final:
            c = color or "green"
            if self.enabled:
                print(style(final, c, True, bold=True))
            else:
                print(final)
    
    def _detect_and_set_stage(self, text: str) -> None:
        """Auto-detect stage from update text."""
        lower = text.lower()
        
        # Extract attempt number
        if "[attempt" in lower and "]" in lower:
            try:
                start = lower.find("[attempt") + 8
                end = lower.find("]", start)
                num = int(lower[start:end].strip())
                if num != self.progress.attempt:
                    self.progress.attempt = num
                    self.progress.clear_errors()
            except ValueError:
                pass
        
        # Map text patterns to stages
        stage_map = [
            (["preprocess", "parsing", "understanding"], "understand"),
            (["intent", "link", "schema link"], "understand"),
            (["contract", "requirement", "building req"], "contract"),
            (["generat", "candidate", "llm"], "generate"),
            (["evaluat", "validat", "check", "syntax", "logic"], "validate"),
            (["repair", "fix", "schema repair", "deterministic"], "repair"),
            (["success", "final", "complete", "done"], "finalize"),
        ]
        
        for patterns, stage in stage_map:
            if any(p in lower for p in patterns):
                # Extract detail from text after the stage indicator
                detail = ""
                if "]" in text:
                    detail = text.split("]", 1)[-1].strip()
                self.progress.set_stage(stage, detail[:50] if detail else "")
                return
    
    def _render(self, frame_idx: int) -> List[str]:
        """Render the current display state."""
        lines: List[str] = []
        spin = self._SPIN_FRAMES[frame_idx % len(self._SPIN_FRAMES)]
        
        # Show NL query at the top
        if self.progress.nl_query:
            query_display = self.progress.nl_query
            query_line = style(f'  "{query_display}"', "sky", self.enabled, italic=True)
            lines.append(query_line)
            lines.append("")
        
        # Header with attempt info
        attempt_str = style(f"ATTEMPT {self.progress.attempt}", "mauve", self.enabled, bold=True)
        header = f"  {spin} {attempt_str}"
        lines.append(header)
        
        # Stage progress visualization
        lines.append("")
        for idx, stage in enumerate(self.progress.stages):
            is_current = idx == self.progress.current_idx
            is_done = idx < self.progress.current_idx
            is_future = idx > self.progress.current_idx
            
            # Choose icon and color
            if is_done:
                ico = style("✓", "green", self.enabled)
                label_col = "dim"
            elif is_current:
                # Animated icon for current stage
                pulse = self._PULSE_FRAMES[frame_idx % len(self._PULSE_FRAMES)]
                ico = style(pulse, self.color, self.enabled, bold=True)
                label_col = "white"
            else:
                ico = style("○", "dim", self.enabled)
                label_col = "dim"
            
            # Build the stage line
            prefix = "  │" if idx < len(self.progress.stages) - 1 else "  └"
            label = style(stage.label, label_col, self.enabled, bold=is_current)
            
            if is_current and self.progress.current_detail:
                detail = style(f" · {self.progress.current_detail}", "gray", self.enabled, italic=True)
            else:
                detail = ""
            
            lines.append(f"{prefix} {ico} {label}{detail}")
        
        # Show current errors (if any)
        if self.progress.errors:
            lines.append("")
            err_header = style("  ⚠ Issues detected:", "peach", self.enabled)
            lines.append(err_header)
            for err in self.progress.errors[-3:]:  # Show last 3 errors
                err_short = err[:60] + "..." if len(err) > 60 else err
                err_line = style(f"    · {err_short}", "dim", self.enabled)
                lines.append(err_line)
        
        # Show fixes being applied
        if self.progress.fixes:
            lines.append("")
            fix_header = style("  🔧 Applying fixes:", "teal", self.enabled)
            lines.append(fix_header)
            for fix in self.progress.fixes[-2:]:  # Show last 2 fixes
                fix_short = fix[:60] + "..." if len(fix) > 60 else fix
                fix_line = style(f"    · {fix_short}", "gray", self.enabled, italic=True)
                lines.append(fix_line)
        
        # Footer with raw status text
        if self._text and self.progress.current_idx >= 0:
            lines.append("")
            # Extract just the action part from text like "[attempt 1] evaluating..."
            action = self._text
            if "]" in action:
                action = action.split("]", 1)[-1].strip()
            if action:
                footer = style(f"  {action}", "gray", self.enabled, italic=True, dim=True)
                lines.append(footer)
        
        return lines
    
    def _run(self) -> None:
        """Animation loop."""
        frame = 0
        while not self._stop.is_set():
            with self._lock:
                lines = self._render(frame)
            
            # Move cursor to home position (top-left) and clear from there
            # This ensures we always render at a fixed position
            sys.stdout.write("\033[H")  # Move to home (top-left)
            
            # Draw each line, clearing to end of line
            for i, line in enumerate(lines):
                sys.stdout.write(f"{line}\033[K\n")
            
            # Clear any remaining lines from previous render
            if len(lines) < self._line_count:
                for _ in range(self._line_count - len(lines)):
                    sys.stdout.write("\033[K\n")
            
            sys.stdout.flush()
            self._line_count = len(lines)
            frame += 1
            time.sleep(0.1)


# ═══════════════════════════════════════════════════════════════════════════════
# Compact Progress Bar (alternative minimal display)
# ═══════════════════════════════════════════════════════════════════════════════

class ProgressBar:
    """Minimal single-line progress indicator."""
    
    def __init__(self, enabled: bool = True) -> None:
        self.enabled = enabled and sys.stdout.isatty()
        self.stages = ["PARSE", "LINK", "BUILD", "GEN", "VAL", "FIX", "DONE"]
        self.current = 0
        self._stop = threading.Event()
        self._thread: Optional[threading.Thread] = None
    
    def start(self) -> None:
        if not self.enabled:
            return
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()
    
    def advance(self) -> None:
        self.current = min(self.current + 1, len(self.stages) - 1)
    
    def stop(self) -> None:
        self._stop.set()
        if self._thread:
            self._thread.join(timeout=0.3)
        sys.stdout.write("\r\033[K")
        sys.stdout.flush()
    
    def _run(self) -> None:
        spin = "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏"
        idx = 0
        while not self._stop.is_set():
            parts = []
            for i, stage in enumerate(self.stages):
                if i < self.current:
                    parts.append(style(f"✓{stage}", "green", self.enabled))
                elif i == self.current:
                    parts.append(style(f"{spin[idx % len(spin)]}{stage}", "mauve", self.enabled, bold=True))
                else:
                    parts.append(style(f"·{stage}", "dim", self.enabled))
            
            line = " → ".join(parts)
            sys.stdout.write(f"\r{line}\033[K")
            sys.stdout.flush()
            idx += 1
            time.sleep(0.08)


__all__ = ["Spinner", "ProgressBar", "StageProgress", "style", "icon"]
