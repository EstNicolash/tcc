"""
rule30.py
======================
Generates parameterized Rule 30 Chaotic Cellular Automata models.

This architecture introduces high non-linearity (chaos). BDD symbolic
structures are unable to compress these state graphs, making it a fair
worst-case memory stress test for state space exploration loops.
"""

import sys

def generate_instance(n: int) -> str:
    assert n >= 3, "Rule 30 requires at least 3 cells."
    lines = [
        f"-- rule30_{n}.ssmv",
        f"-- {n}-cell Rule 30 Cellular Automaton (Chaotic System Boundary)",
        "MODULE main",
        "VAR"
    ]

    lines.extend(f"  c{i} : boolean;" for i in range(n))
    lines.extend(["", "ASSIGN"])

    for i in range(n):
        val = "TRUE" if i == n // 2 else "FALSE"
        lines.append(f"  init(c{i}) := {val};")

    lines.append("")

    for i in range(n):
        left = f"c{(i - 1) % n}"
        center = f"c{i}"
        right = f"c{(i + 1) % n}"

        expr_A = left
        expr_B = f"({center} | {right})"

        lines.append(
            f"  next(c{i}) := ({expr_A} & !{expr_B}) | (!{expr_A} & {expr_B});"
        )

    lines.extend([
        "",
        "-- ── Liveness: Can all cells become TRUE concurrently? ──",
        "CTLSPEC EF (" + " & ".join(f"c{i} = TRUE" for i in range(n)) + ");",
        "",
        "-- ── Reachability: Can the cellular structure completely die out? ──",
        "CTLSPEC EF (" + " & ".join(f"c{i} = FALSE" for i in range(n)) + ");"
    ])

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(6))
