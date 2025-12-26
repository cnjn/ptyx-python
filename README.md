# ptyx

A simple, cross-platform PTY/TTY library for Python, written in Rust.

**ATTENTION: THIS PROJ IS TOTALLY WRITTEN BY CLAUDE 4.5 OPUS THINKING!!**

## Features

- **Cross-platform**: Linux, macOS, FreeBSD, NetBSD, OpenBSD, Windows
- **Zero Python dependencies**: Pure Rust core with PyO3 bindings
- **Pythonic API**: Context managers, properties, type hints

## Installation

```bash
pip install ptyx-python
```

## Usage

### Spawn a process in PTY

```python
import ptyx

with ptyx.spawn("bash", args=["-l"]) as session:
    session.write(b"echo hello\n")

    output = b""
    while True:
        chunk = session.read_timeout(4096, timeout_ms=200)
        if not chunk:
            break
        output += chunk

    print(output.decode())
    session.wait()
```

### Console control

```python
with ptyx.Console() as console:
    print(f"Is TTY: {console.is_tty}")
    print(f"Size: {console.size}")  # (cols, rows)

    # Raw mode
    state = console.make_raw()
    # ... do raw mode operations
    console.restore(state)
```

### Convenience functions

```python
# Run and wait
exit_code = ptyx.run("ls", args=["-la"])

# Interactive session
ptyx.run_interactive("bash")
```

### ANSI helpers

```python
print(ptyx.sgr([1, 31]) + "Bold Red" + ptyx.sgr([0]))
print(ptyx.clear_screen())
print(ptyx.cursor_to(10, 5))
```

## API Reference

### Session

| Method | Description |
|--------|-------------|
| `read(n)` | Blocking read up to n bytes |
| `read_timeout(n, ms)` | Read with timeout, returns empty on timeout |
| `write(data)` | Write bytes to PTY |
| `resize(cols, rows)` | Resize PTY |
| `wait()` | Wait for process exit, returns exit code |
| `kill()` | Kill the process |
| `pid` | Process ID (property) |
| `is_alive` | Check if process is running (property) |

### Console

| Method | Description |
|--------|-------------|
| `is_tty` | Check if stdout is a TTY (property) |
| `size` | Terminal size as (cols, rows) (property) |
| `make_raw()` | Enter raw mode, returns state |
| `restore(state)` | Restore terminal state |
| `enable_vt()` | Enable ANSI support (Windows) |

## License

MIT
