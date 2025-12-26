"""ptyx - Cross-platform PTY/TTY management library

A simple, cross-platform API for managing pseudo-terminals (PTY) and terminal TTYs.
"""

from .ptyx import (
    # Classes
    Console,
    Session,
    RawState,
    # Functions
    spawn,
    run,
    run_interactive,
    # ANSI helpers
    csi,
    sgr,
    clear_screen,
    cursor_home,
    cursor_to,
    cursor_hide,
    cursor_show,
)

__all__ = [
    # Classes
    "Console",
    "Session",
    "RawState",
    # Functions
    "spawn",
    "run",
    "run_interactive",
    # ANSI helpers
    "csi",
    "sgr",
    "clear_screen",
    "cursor_home",
    "cursor_to",
    "cursor_hide",
    "cursor_show",
]

__version__ = "0.1.0"
