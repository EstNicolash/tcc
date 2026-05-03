"""
generate_examples.py
======================
Generates parameterized SSMV benchmark models for model-checking experiments.

Models:
  ring_N.smv      — 1 integer variable, N states, fully deterministic
  dining_N.smv    — N enum variables + scheduler, non-deterministic
  peterson.smv    — 2-process Peterson mutex, boolean flags

All models use only the SSMV subset:
  - Single MODULE main
  - Types: boolean, range (integer), enum
  - case expressions, Set literals for non-determinism
  - No multiple modules, no LTLSPEC
"""

import os


def write(path: str, content: str) -> None:
    os.makedirs(os.path.dirname(path) or ".", exist_ok=True)
    with open(path, "w") as f:
        f.write(content)
    print(f"  wrote {path}  ({len(content.splitlines())} lines)")




# ─── Cone of Influence (COI) Killer ───────────────────────────────────────────

def generate_coi_killer(n: int) -> str:
    """
    The Monolithic System Killer (Cone of Influence).
    Generates N 3-bit systems (counters) running in parallel, but
    completely disconnected from each other.

    The CTL property checks ONLY counter 0.

    NuSMV: Detects that variables 1 to N-1 are irrelevant, reduces the
           model to just 3 boolean variables, and solves it in 0.01s.
    SSMV: Attempts to build the Monolithic Transition Relation for 3*N variables.
          For large N, memory blows up during the global AND operations.
    """
    assert n >= 2
    lines = [
        f"-- coi_killer_{n}.smv",
        f"-- {n} Independent 3-bit counters (Variables: {n * 3})",
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
        "-- NuSMV will ignore the other N-1 counters instantly.",
        f"CTLSPEC AF (a0 = TRUE & b0 = TRUE & c0 = TRUE)"
    ])

    return "\n".join(lines) + "\n"



# ─── N-Bit Binary Counter ─────────────────────────────────────────────────────

def generate_counter(n: int) -> str:
    """
    N-bit synchronous binary counter.
    State space is exactly 2^N.
    Explicit Labelling -> O(2^N) time/memory (Will die around N=20).
    Symbolic BDD -> O(N) nodes (Can handle N=100 in milliseconds).
    """
    assert n >= 2

    lines = [
        f"-- counter_{n}.smv",
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
            lines.append(f"  next(b0) := !b0;")
        else:
            cond = " & ".join(f"b{j}" for j in range(i))
            lines.extend([
                f"  next(b{i}) := case",
                f"    {cond} : !b{i};",
                f"    TRUE : b{i};",
                f"  esac;",
                ""
            ])

    lines.extend([
        "-- ── Liveness: O bit mais significativo vai virar TRUE eventualmente ──",
        f"CTLSPEC AF (b{n-1} = TRUE)",
        "",
        "-- ── Safety: O estado onde todos são TRUE é alcançável ──",
        f"CTLSPEC EF (" + " & ".join(f"b{i} = TRUE" for i in range(n)) + ")"
    ])

    return "\n".join(lines) + "\n"

# ─── Bad Variable Order Killer ───────────────────────────────────────────────

def generate_bad_order(n: int) -> str:
    """
    The BDD Sifting Vindicator.
    Declares variables in block order: A_0...A_N, then B_0...B_N.
    Static order (SSMV) yields O(2^N) BDD size.
    Dynamic order (NuSMV) interleaves to A_0, B_0... yielding O(N) size.
    """
    assert n >= 5
    lines = [
        f"-- bad_order_{n}.smv",
        f"-- Tests static vs dynamic variable ordering",
        "MODULE main",
        "VAR"
    ]

    lines.extend(f"  a{i} : boolean;" for i in range(n))
    lines.extend(f"  b{i} : boolean;" for i in range(n))

    lines.extend(["", "ASSIGN"])

    for i in range(n):
        lines.extend([
            f"  init(a{i}) := FALSE; next(a{i}) := !a{i};",
            f"  init(b{i}) := FALSE; next(b{i}) := !b{i};"
        ])

    lines.extend([
        "",
        "-- ── Safety: Are 'a' and 'b' bitwise equal? ──",
        f"CTLSPEC AG (" + " & ".join(f"a{i} = b{i}" for i in range(n)) + ")"
    ])

    return "\n".join(lines) + "\n"

def generate_rule30(n: int) -> str:
    """
    Rule 30 Cellular Automaton.
    Generates pseudorandom patterns (chaos).
    BDDs cannot compress chaotic logic, causing an explosion in the graph size.
    Labelling suffers from the 2^N state space.
    A true "fair" stress test for both engines.
    """
    assert n >= 3
    lines = [
        f"-- rule30_{n}.smv",
        f"-- {n}-cell Rule 30 Cellular Automaton (Chaotic System)",
        "MODULE main",
        "VAR"
    ]

    # N interconnected boolean cells
    lines.extend(f"  c{i} : boolean;" for i in range(n))
    lines.extend(["", "ASSIGN"])

    # Init: Only one true cell in the center
    for i in range(n):
        val = "TRUE" if i == n // 2 else "FALSE"
        lines.append(f"  init(c{i}) := {val};")

    lines.append("")

    # Next state: Left XOR (Center OR Right)
    # In pure boolean: XOR(A, B) = (A & !B) | (!A & B)
    for i in range(n):
        left = f"c{(i - 1) % n}"
        center = f"c{i}"
        right = f"c{(i + 1) % n}"

        # A = Left, B = (Center | Right)
        expr_A = left
        expr_B = f"({center} | {right})"

        lines.append(
            f"  next(c{i}) := ({expr_A} & !{expr_B}) | (!{expr_A} & {expr_B});"
        )

    lines.extend([
        "",
        "-- ── Liveness: Can all cells be TRUE simultaneously? ──",
        f"CTLSPEC EF (" + " & ".join(f"c{i} = TRUE" for i in range(n)) + ")",
        "",
        "-- ── Reachability: Does the system die (all FALSE)? ──",
        f"CTLSPEC EF (" + " & ".join(f"c{i} = FALSE" for i in range(n)) + ")"
    ])

    return "\n".join(lines) + "\n"

# ─── Dining philosophers ──────────────────────────────────────────────────────

def generate_dining(n: int) -> str:
    """
    N philosophers at a round table.

    Encoding:
      phil_i : {thinking, hungry, eating}
      action  : {act0, ..., act_{N-1}}  — non-deterministic scheduler
                (no init/next → free input, chosen by environment each step)

    Transition for philosopher i (when action = act_i):
      eating   → thinking          (release forks)
      hungry   → eating            (if both neighbours ≠ eating)
      thinking → {thinking, hungry}(non-deterministically gets hungry)

    Note on liveness:
      Without fairness constraints, the non-deterministic scheduler can
      starve a hungry philosopher forever → AF specs may be FALSE.
      This is intentional: it demonstrates the difference between
      safety (always TRUE) and liveness (scheduler-dependent).

    Scaling:
      States ≈ 3^N × N   (exponential)
      Interesting range: N = 3, 4, 5, 6
    """
    assert n >= 2

    phil_vars   = "\n".join(f"  phil{i} : {{thinking, hungry, eating}};" for i in range(n))
    action_vals = ", ".join(f"act{i}" for i in range(n))
    inits       = "\n".join(f"  init(phil{i}) := thinking;" for i in range(n))

    def next_phil(i: int) -> str:
        left  = (i - 1) % n
        right = (i + 1) % n
        return "\n".join([
            f"  next(phil{i}) := case",
            f"    action != act{i}                                                 : phil{i};",
            f"    phil{i} = eating                                                 : thinking;",
            f"    phil{i} = hungry & phil{left} != eating & phil{right} != eating  : eating;",
            f"    phil{i} = thinking                                               : {{thinking, hungry}};",
            f"    TRUE                                                             : phil{i};",
            f"  esac;",
        ])

    nexts = "\n\n".join(next_phil(i) for i in range(n))

    safety = "\n".join(
        f"CTLSPEC AG !(phil{i} = eating & phil{(i + 1) % n} = eating)"
        for i in range(n)
    )
    liveness = "\n".join(
        f"CTLSPEC AG (phil{i} = hungry -> AF phil{i} = eating)"
        for i in range(n)
    )
    deadlock = "\n".join(
        f"CTLSPEC AG EF phil{i} = eating"
        for i in range(n)
    )

    return "\n".join([
        f"-- dining_{n}.smv",
        f"-- Dining Philosophers: {n} philosophers, non-deterministic scheduler",
        "--",
        "-- action has no init/next → it is a free environment variable.",
        "-- Safety properties (mutual exclusion) should be TRUE.",
        "-- Liveness properties may be FALSE without fairness (intentional).",
        "MODULE main",
        "VAR",
        phil_vars,
        f"  action : {{{action_vals}}};",
        "",
        "ASSIGN",
        inits,
        "",
        nexts,
        "",
        "-- ── Safety: no two adjacent philosophers eat simultaneously ──────────",
        safety,
        "",
        "-- ── Liveness: every hungry philosopher eventually eats ───────────────",
        "-- NOTE: may be FALSE without fairness — this is expected and informative",
        liveness,
        "",
        "-- ── Deadlock freedom: eating is always reachable for each philosopher ─",
        deadlock,
    ]) + "\n"

# ─── Ultimate Cone of Influence Killer (Chaos Background) ─────────────────────

def generate_coi_chaos_killer(n: int) -> str:
    """
    The Ultimate COI Test.
    Combines a simple 3-bit counter with a chaotic Rule 30 system running in parallel.
    The property ONLY queries the 3-bit counter.

    NuSMV: Uses COI to delete the chaotic system entirely. Solves in 0.01s.
    SSMV: Monolithic transition relation tries to compile the chaotic system. Explodes.
    """
    assert n >= 10
    lines = [
        f"-- coi_chaos_killer_{n}.smv",
        f"-- Foreground: 3-bit counter (Queried)",
        f"-- Background: {n}-cell Rule 30 Chaotic Automaton (Ignored by COI)",
        "MODULE main",
        "VAR",
        "  -- Foreground system",
        "  a : boolean;",
        "  b : boolean;",
        "  c : boolean;",
        "  -- Background chaotic system"
    ]

    lines.extend(f"  bg{i} : boolean;" for i in range(n))
    lines.extend(["", "ASSIGN"])

    # Init foreground
    lines.extend([
        "  init(a) := FALSE;",
        "  init(b) := FALSE;",
        "  init(c) := FALSE;"
    ])

    # Init background (Rule 30)
    for i in range(n):
        val = "TRUE" if i == n // 2 else "FALSE"
        lines.append(f"  init(bg{i}) := {val};")

    lines.append("")

    # Next foreground
    lines.extend([
        "  next(a) := !a;",
        "  next(b) := case a : !b; TRUE : b; esac;",
        "  next(c) := case (a & b) : !c; TRUE : c; esac;"
    ])

    # Next background (Rule 30)
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
        "-- ── Liveness: Checks ONLY the foreground counter ──",
        "-- NuSMV ignores the chaotic background thanks to COI.",
        "-- Monolithic BDDs will attempt to compile the chaos and hang.",
        f"CTLSPEC AF (a = TRUE & b = TRUE & c = TRUE)"
    ])

    return "\n".join(lines) + "\n"
# ─── Peterson mutex ───────────────────────────────────────────────────────────

def generate_peterson() -> str:
    """
    Peterson's mutual exclusion algorithm for 2 processes.

    Synchronous encoding (both processes advance simultaneously each step).
    Peterson's algorithm:
      idle → try  : raise flag
      try  → wait : set turn = other  (yield priority)
      wait → cs   : if ¬flag_other OR turn = me
      cs   → idle : lower flag

    turn = FALSE → process 0 yielded (process 1 has priority in CS)
    turn = TRUE  → process 1 yielded (process 0 has priority in CS)

    When both are in 'try' simultaneously, the first case arm wins
    (pc0 = try → turn := TRUE), giving process 1 priority that step.

    Expected results:
      Mutual exclusion    → TRUE  (safety, invariant of the algorithm)
      Liveness            → TRUE  (synchronous model ensures progress)
      Deadlock freedom    → TRUE
      CS reachability     → TRUE

    Scaling:
      States ≤ 4×4×2×2×2 = 128 (reachable ~50)
      Trivial for both BDD and labelling — used to verify correctness.
    """
    return "\n".join([
        "-- peterson.smv",
        "-- Peterson's Mutual Exclusion Algorithm (2 processes, synchronous)",
        "MODULE main",
        "VAR",
        "  pc0   : {idle, try, wait, cs};",
        "  pc1   : {idle, try, wait, cs};",
        "  flag0 : boolean;",
        "  flag1 : boolean;",
        "  -- turn = FALSE: p0 yielded (p1 has priority)",
        "  -- turn = TRUE:  p1 yielded (p0 has priority)",
        "  turn  : boolean;",
        "",
        "ASSIGN",
        "  init(pc0)   := idle;",
        "  init(pc1)   := idle;",
        "  init(flag0) := FALSE;",
        "  init(flag1) := FALSE;",
        "  init(turn)  := FALSE;",
        "",
        "  -- Process 0",
        "  next(pc0) := case",
        "    pc0 = idle                    : try;",
        "    pc0 = try                     : wait;",
        "    pc0 = wait & (!flag1 | !turn) : cs;",
        "    pc0 = wait                    : wait;",
        "    pc0 = cs                      : idle;",
        "    TRUE                          : pc0;",
        "  esac;",
        "",
        "  -- Process 1",
        "  next(pc1) := case",
        "    pc1 = idle                   : try;",
        "    pc1 = try                    : wait;",
        "    pc1 = wait & (!flag0 | turn) : cs;",
        "    pc1 = wait                   : wait;",
        "    pc1 = cs                     : idle;",
        "    TRUE                         : pc1;",
        "  esac;",
        "",
        "  -- Flags: raised when starting, lowered when leaving CS",
        "  next(flag0) := case",
        "    pc0 = idle : TRUE;",
        "    pc0 = cs   : FALSE;",
        "    TRUE       : flag0;",
        "  esac;",
        "",
        "  next(flag1) := case",
        "    pc1 = idle : TRUE;",
        "    pc1 = cs   : FALSE;",
        "    TRUE       : flag1;",
        "  esac;",
        "",
        "  -- Turn: a process yields to the other when entering try",
        "  -- If both in try simultaneously, first arm wins (p0 yields → p1 priority)",
        "  next(turn) := case",
        "    pc0 = try : TRUE;",
        "    pc1 = try : FALSE;",
        "    TRUE      : turn;",
        "  esac;",
        "",
        "-- ── Safety ────────────────────────────────────────────────────────────",
        "-- Mutual exclusion: never both in CS simultaneously (MUST be TRUE)",
        "CTLSPEC AG !(pc0 = cs & pc1 = cs)",
        "",
        "-- ── Liveness ──────────────────────────────────────────────────────────",
        "-- A process that tries will eventually enter CS (MUST be TRUE)",
        "CTLSPEC AG (pc0 = try -> AF pc0 = cs)",
        "CTLSPEC AG (pc1 = try -> AF pc1 = cs)",
        "",
        "-- A waiting process is never starved (MUST be TRUE)",
        "CTLSPEC AG (pc0 = wait -> AF pc0 = cs)",
        "CTLSPEC AG (pc1 = wait -> AF pc1 = cs)",
        "",
        "-- ── Deadlock freedom ──────────────────────────────────────────────────",
        "-- From any reachable state, CS is always eventually reachable (MUST be TRUE)",
        "CTLSPEC AG EF pc0 = cs",
        "CTLSPEC AG EF pc1 = cs",
        "",
        "-- ── Reachability ──────────────────────────────────────────────────────",
        "-- CS is reachable from the initial state (MUST be TRUE)",
        "CTLSPEC EF pc0 = cs",
        "CTLSPEC EF pc1 = cs",
    ]) + "\n"


# ─── CLI ──────────────────────────────────────────────────────────────────────

def main() -> None:
    out = "benchmarks"
    print(f"Generating benchmarks in ./{out}/\n")


    print("N-Bit Counter:")
    for n in [10, 15, 20, 30]:
        write(f"{out}/counter_{n}.smv", generate_counter(n))

    print("\nDining philosophers:")
    for n in [8, 10, 12, 14]:
        write(f"{out}/dining_{n}.smv", generate_dining(n))

    print("\nRule 30 Cellular Automaton:")
    for n in [10, 15, 20, 25]:
        write(f"{out}/rule30_{n}.smv", generate_rule30(n))

    print("\nCone of Influence Killer:")
    for n in [10, 50, 100, 200]:
        write(f"{out}/coi_killer_{n}.smv", generate_coi_killer(n))

    print("\nUltimate COI Killer (Simple Counter + Chaotic Background):")
    for n in [20, 25, 30]:
        write(f"{out}/coi_chaos_killer_{n}.smv", generate_coi_chaos_killer(n))

    for n in [20, 30, 40]:
        write(f"{out}/bad_order_{n}.smv", generate_bad_order(n))

    print("\nPeterson mutex:")
    write(f"{out}/peterson.smv", generate_peterson())

    print(f"""
Done.

Expected results summary:
  ring_N.smv    — all CTLSPEC TRUE (trivially correct, tests scaling)
  dining_N.smv  — safety TRUE, liveness may be FALSE (scheduler fairness)
  peterson.smv  — all CTLSPEC TRUE (correct mutual exclusion algorithm)

Suggested NuSMV commands:
  nusmv -bdd_stats benchmarks/ring_1000.smv
  nusmv -bdd_stats benchmarks/dining_5.smv
  nusmv -bdd_stats benchmarks/peterson.smv
""")


if __name__ == "__main__":
    main()
