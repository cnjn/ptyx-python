"""Type stubs for ptyx - Cross-platform PTY/TTY management library"""

from typing import Optional, Tuple, List

class RawState:
    """Saved terminal state for raw mode restoration."""
    ...

class Console:
    """Console - TTY control interface.

    Provides access to the terminal/console for managing raw mode,
    getting terminal size, and other TTY operations.

    Example:
        >>> with Console() as console:
        ...     print(f"Terminal size: {console.size}")
        ...     if console.is_tty:
        ...         state = console.make_raw()
        ...         # ... do raw mode operations
        ...         console.restore(state)
    """

    def __init__(self) -> None:
        """Create a new Console instance."""
        ...

    @property
    def is_tty(self) -> bool:
        """Check if stdout is a TTY."""
        ...

    @property
    def is_tty_err(self) -> bool:
        """Check if stderr is a TTY."""
        ...

    @property
    def size(self) -> Tuple[int, int]:
        """Get terminal size as (cols, rows)."""
        ...

    def make_raw(self) -> RawState:
        """Enter raw mode. Returns state to restore later."""
        ...

    def restore(self, state: RawState) -> None:
        """Restore terminal state from raw mode."""
        ...

    def enable_vt(self) -> None:
        """Enable virtual terminal processing (ANSI support)."""
        ...

    def close(self) -> None:
        """Close the console."""
        ...

    def __enter__(self) -> "Console":
        ...

    def __exit__(
        self,
        exc_type: Optional[type] = None,
        exc_val: Optional[BaseException] = None,
        exc_tb: Optional[object] = None,
    ) -> bool:
        ...

class Session:
    """Session - PTY process control.

    Manages a process running in a pseudo-terminal, providing
    read/write access to its I/O and process control.

    Example:
        >>> with spawn("bash", args=["-l"]) as session:
        ...     session.write(b"echo hello\\n")
        ...     output = session.read(1024)
        ...     print(output)
    """

    @property
    def pid(self) -> int:
        """Get process ID."""
        ...

    @property
    def is_alive(self) -> bool:
        """Check if process is still alive."""
        ...

    def read(self, max_bytes: int) -> bytes:
        """Read data from PTY (up to max_bytes)."""
        ...

    def read_timeout(self, max_bytes: int, timeout_ms: int = 100) -> bytes:
        """Read data from PTY with timeout (milliseconds). Returns empty bytes on timeout."""
        ...

    def write(self, data: bytes) -> int:
        """Write data to PTY. Returns number of bytes written."""
        ...

    def resize(self, cols: int, rows: int) -> None:
        """Resize the PTY."""
        ...

    def wait(self) -> int:
        """Wait for process to exit. Returns exit code."""
        ...

    def kill(self) -> None:
        """Kill the process."""
        ...

    def close_stdin(self) -> None:
        """Close stdin to signal EOF."""
        ...

    def close(self) -> None:
        """Close the session."""
        ...

    def __enter__(self) -> "Session":
        ...

    def __exit__(
        self,
        exc_type: Optional[type] = None,
        exc_val: Optional[BaseException] = None,
        exc_tb: Optional[object] = None,
    ) -> bool:
        ...

def spawn(
    prog: str,
    args: Optional[List[str]] = None,
    env: Optional[List[Tuple[str, str]]] = None,
    dir: Optional[str] = None,
    cols: int = 80,
    rows: int = 24,
) -> Session:
    """Spawn a process in a PTY.

    Args:
        prog: Program to execute
        args: Command line arguments
        env: Environment variables as list of (key, value) tuples
        dir: Working directory
        cols: Terminal width (default 80)
        rows: Terminal height (default 24)

    Returns:
        Session object for interacting with the process

    Example:
        >>> with spawn("bash", args=["-l"]) as session:
        ...     session.write(b"ls\\n")
        ...     print(session.read(4096))
    """
    ...

def run(
    prog: str,
    args: Optional[List[str]] = None,
    env: Optional[List[Tuple[str, str]]] = None,
    dir: Optional[str] = None,
) -> int:
    """Run a command (non-interactive).

    Args:
        prog: Program to execute
        args: Command line arguments
        env: Environment variables
        dir: Working directory

    Returns:
        Exit code of the process
    """
    ...

def run_interactive(
    prog: str,
    args: Optional[List[str]] = None,
    env: Optional[List[Tuple[str, str]]] = None,
    dir: Optional[str] = None,
) -> int:
    """Run a command interactively (with console I/O bridging).

    This function sets up raw mode, bridges console I/O with the PTY,
    and restores the terminal on exit.

    Args:
        prog: Program to execute
        args: Command line arguments
        env: Environment variables
        dir: Working directory

    Returns:
        Exit code of the process
    """
    ...

# ANSI escape sequence helpers

def csi(seq: str) -> str:
    """Create a CSI (Control Sequence Introducer) sequence."""
    ...

def sgr(codes: List[int]) -> str:
    """Create an SGR (Select Graphic Rendition) sequence."""
    ...

def clear_screen() -> str:
    """Clear screen escape sequence."""
    ...

def cursor_home() -> str:
    """Move cursor to home position."""
    ...

def cursor_to(row: int, col: int) -> str:
    """Move cursor to position."""
    ...

def cursor_hide() -> str:
    """Hide cursor."""
    ...

def cursor_show() -> str:
    """Show cursor."""
    ...
