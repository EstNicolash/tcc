"""
counter.py
======================
Generates parameterized N-bit Synchronous Binary Counter benchmark models.

State space scales exponentially as 2^N. Symbolic BDD algorithms should
handle large instances (N=100) using a linear number of nodes O(N).
"""

import sys

def generate_instance(n: int) -> str:
    assert n >= 2, "Counter requires at least 2 bits."

    lines = [
        f"-- counter_{n}.ssmv",
        f"-- {n}-bit Binary Counter (State Space: 2^{n})",
        "MODULE main",
        "VAR"
    ]

    lines.extend(f"  b{i} : boolean;" for i in range(n))
    lines.extend(["", "ASSIGN"])

    lines.extend(f"  init(b{i}) := FALSE;" for i in range(n))
    lines.append("")

    for i in range(n):
        if i == 0:
            lines.append("  next(b0) := !b0;")
        else:
            cond = " & ".join(f"b{j}" for j in range(i))
            lines.extend([
                f"  next(b{i}) := case",
                f"    {cond} : !b{i};",
                f"    TRUE : b{i};",
                "  esac;"
            ])

    lines.extend([
        "",
        "-- ── Liveness: The most significant bit will eventually turn TRUE ──",
        f"CTLSPEC AF (b{n-1} = TRUE);",
        "",
        "-- ── Safety: The state where all bits are TRUE is reachable ──",
        "CTLSPEC EF (" + " & ".join(f"b{i} = TRUE" for i in range(n)) + ");"
    ])

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(8))
