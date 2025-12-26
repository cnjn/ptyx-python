#!/usr/bin/env python3
"""Example usage of ptyx library"""

import ptyx

def main():
    # Console example
    print("=== Console Example ===")
    with ptyx.Console() as console:
        print(f"Is TTY: {console.is_tty}")
        print(f"Terminal size: {console.size}")

    # Spawn example - 使用 read_timeout
    print("\n=== Spawn Example ===")
    with ptyx.spawn("ls", args=["-lha", "."]) as session:
        print(f"PID: {session.pid}")

        # 使用带超时的读取，收集所有输出
        output = b""
        while True:
            chunk = session.read_timeout(4096, timeout_ms=200)
            if not chunk:
                break
            output += chunk

        print(f"Output:\n{output.decode()}")
        exit_code = session.wait()
        print(f"Exit code: {exit_code}")

    # ANSI example
    print("\n=== ANSI Example ===")
    print(f"CSI sequence: {repr(ptyx.csi('2J'))}")
    print(f"SGR (bold red): {ptyx.sgr([1, 31])}Hello{ptyx.sgr([0])}")

if __name__ == "__main__":
    main()
