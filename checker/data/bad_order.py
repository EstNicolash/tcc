"""
bad_order.py
======================
Generates benchmark models tailored to test Variable Ordering heuristics.

Variables are declared in a separated block layout (A_0..A_N then B_0..B_N).
Static orderings yield O(2^N) BDD structures, while dynamic sifting or
interleaved ordering yields a compact linear size of O(N) nodes.
"""

import sys

def generate_instance(n: int) -> str:
    assert n >= 5, "Bad variable order test requires at least 5 layers."
    lines = [
        f"-- bad_order_{n}.ssmv",
        "-- Evaluates the resilience of the engine against suboptimal variable ordering",
        "MODULE main",
        "VAR"
    ]

    lines.extend(f"  a{i} : boolean;" for i in range(n))
    lines.extend(f"  b{i} : boolean;" for i in range(n))

    lines.extend(["", "ASSIGN"])

    for i in range(n):
        lines.extend([
            f"  init(a{i}) := FALSE;",
            f"  init(b{i}) := FALSE;"
        ])

    lines.append("")
    for i in range(n):
        lines.extend([
            f"  next(a{i}) := !a{i};",
            f"  next(b{i}) := !b{i};"
        ])

    lines.extend([
        "",
        "-- ── Safety: Check if 'a' and 'b' remain bitwise equivalent ──",
        "CTLSPEC AG (" + " & ".join(f"a{i} = b{i}" for i in range(n)) + ");"
    ])

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(10))
