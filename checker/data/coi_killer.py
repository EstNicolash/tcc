"""
coi_killer.py
======================
Generates parameterized COI (Cone of Influence) Killer benchmark models.

This script creates N independent 3-bit counters running in parallel,
completely disconnected from one another. The verification property
queries only counter 0, forcing a smart model checker to prune the rest.
"""

import sys

def generate_instance(n: int) -> str:
    assert n >= 2, "COI Killer requires at least 2 counters."
    lines = [
        f"-- coi_killer_{n}.ssmv",
        f"-- {n} Independent 3-bit counters (Total variables: {n * 3})",
        "MODULE main",
        "VAR"
    ]

    for i in range(n):
        lines.extend([
            f"  a{i} : boolean;",
            f"  b{i} : boolean;",
            f"  c{i} : boolean;"
        ])

    lines.extend(["", "ASSIGN"])

    for i in range(n):
        lines.extend([
            f"  init(a{i}) := FALSE;",
            f"  init(b{i}) := FALSE;",
            f"  init(c{i}) := FALSE;"
        ])

    lines.append("")

    for i in range(n):
        lines.extend([
            f"  next(a{i}) := !a{i};",
            f"  next(b{i}) := case a{i} : !b{i}; TRUE : b{i}; esac;",
            f"  next(c{i}) := case (a{i} & b{i}) : !c{i}; TRUE : c{i}; esac;"
        ])

    lines.extend([
        "",
        "-- ── Liveness: Checks ONLY counter ZERO ──",
        "CTLSPEC AF (a0 = TRUE & b0 = TRUE & c0 = TRUE);"
    ])

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(10))
