"""
dining.py
======================
Generates parameterized Dining Philosophers benchmark models.

Models N philosophers handling shared resources with a non-deterministic
scheduler action block. Used to study safety invariants and non-fair liveness.
"""

import sys

def generate_instance(n: int) -> str:
    assert n >= 2, "Dining philosophers require at least 2 seats."

    lines = [
        f"-- dining_{n}.ssmv",
        f"-- Dining Philosophers: {n} philosophers with non-deterministic scheduling",
        "MODULE main",
        "VAR"
    ]

    for i in range(n):
        lines.append(f"  phil{i} : {{thinking, hungry, eating}};")

    action_vals = ", ".join(f"act{i}" for i in range(n))
    lines.extend([
        f"  action : {{{action_vals}}};",
        "",
        "ASSIGN"
    ])

    lines.extend(f"  init(phil{i}) := thinking;" for i in range(n))
    lines.append("")

    for i in range(n):
        left = (i - 1) % n
        right = (i + 1) % n
        lines.extend([
            f"  next(phil{i}) := case",
            f"    action != act{i} : phil{i};",
            f"    phil{i} = eating : thinking;",
            f"    phil{i} = hungry & phil{left} != eating & phil{right} != eating : eating;",
            f"    phil{i} = thinking : {{thinking, hungry}};",
            f"    TRUE : phil{i};",
            "  esac;",
            ""
        ])

    # Verification invariants
    lines.append("-- ── Safety: Adjacent processes can never eat concurrently ──")
    for i in range(n):
        lines.append(f"CTLSPEC AG !(phil{i} = eating & phil{(i + 1) % n} = eating);")

    lines.append("\n-- ── Liveness: Hungry philosophers are guaranteed to eat (False without fairness) ──")
    for i in range(n):
        lines.append(f"CTLSPEC AG (phil{i} = hungry -> AF phil{i} = eating);")

    lines.append("\n-- ── Deadlock Freedom: The eating state remains always reachable ──")
    for i in range(n):
        lines.append(f"CTLSPEC AG EF phil{i} = eating;")

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(4))
