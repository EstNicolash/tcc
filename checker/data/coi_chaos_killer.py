"""
coi_chaos_killer.py
======================
Generates the Ultimate Cone of Influence validation models.

Combines a simple 3-bit deterministic counter (queried by the property)
with a background chaotic Rule 30 cellular automation (ignored by the property).
Tests if the engine can completely purge unreferenced chaotic BDD nodes.
"""

import sys

def generate_instance(n: int) -> str:
    assert n >= 4, "Chaotic background layer requires at least 4 cells."
    lines = [
        f"-- coi_chaos_killer_{n}.ssmv",
        f"-- Foreground: 3-bit counter (Target) | Background: {n}-cell Rule 30 (Chaos Shield)",
        "MODULE main",
        "VAR",
        "  a : boolean;",
        "  b : boolean;",
        "  c : boolean;"
    ]

    lines.extend(f"  bg{i} : boolean;" for i in range(n))
    lines.extend(["", "ASSIGN"])

    lines.extend([
        "  init(a) := FALSE;",
        "  init(b) := FALSE;",
        "  init(c) := FALSE;"
    ])

    for i in range(n):
        val = "TRUE" if i == n // 2 else "FALSE"
        lines.append(f"  init(bg{i}) := {val};")

    lines.append("")
    lines.extend([
        "  next(a) := !a;",
        "  next(b) := case a : !b; TRUE : b; esac;",
        "  next(c) := case (a & b) : !c; TRUE : c; esac;"
    ])

    for i in range(n):
        left = f"bg{(i - 1) % n}"
        center = f"bg{i}"
        right = f"bg{(i + 1) % n}"
        expr_A = left
        expr_B = f"({center} | {right})"
        lines.append(
            f"  next(bg{i}) := ({expr_A} & !{expr_B}) | (!{expr_A} & {expr_B});"
        )

    lines.extend([
        "",
        "-- ── Liveness: Property checks exclusively on the foreground counter ──",
        "CTLSPEC AF (a = TRUE & b = TRUE & c = TRUE);"
    ])

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(12))
