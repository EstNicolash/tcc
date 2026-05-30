import sys

def generate_instance(N: int) -> str:
    """
    Generates a parameterized instance of the MCS Queue Lock mutual exclusion
    algorithm for N processes in strict .ssmv format.

    Problem Description:
      The Mellor-Crummey and Scott (MCS) lock is an architectural list-based
      queue lock design that minimizes cache-line bouncing on shared memory systems.
      Processes allocate local nodes and chain them into a distributed queue
      using an atomic Fetch-And-Store operation on a global tail pointer.

    Mathematical Translation (Sentinel Optimization):
      The original DVE model leverages the byte literal 255 as a NIL sentinel.
      To preserve optimal power-of-two BDD allocations, this script translates
      the NIL sentinel to the value N.
      - Process IDs range from: 0 to N-1
      - NIL/Empty pointer is represented by: N
      - Safe Integer boundaries are bounded at 0..N

    Implementation Constraints Respected:
      - Renamed reserved keyword 'next_{i}' to 'nxt_{i}' to prevent frontend parser errors.
      - Strictly flattened assignments (NO nested 'case' structures).
      - Multi-process array updates translated to explicit local variable monitors.
      - Complete line splitting for all case branches to satisfy ssmv_parser.rs.
      - All academic documentation and properties written strictly in English.
    """
    assert N >= 1, "MCS requires at least 1 process."

    # NIL Sentinel is mapped directly to N
    NIL = N

    lines = [
        f"-- mcs_{N}.ssmv",
        f"-- Parameterized MCS List-Based Queue Lock with N={N} processes",
        "MODULE main",
        "VAR",
        f"  tail : 0..{N};"
    ]

    # 1. Variable Declarations
    for i in range(N):
        states = ["NCS", "p2", "p3", "p4", "p5", "p6", "CS", "p9", "p13", "p10"]
        p_states = ", ".join([f"st_p{i}_{s}" for s in states])

        # Padded domains bounded safely to cover the process IDs + the NIL sentinel
        # 'next_{i}' is renamed to 'nxt_{i}' to bypass reserved word restrictions
        lines.append(
            f"  p{i} : {{{p_states}}}; "
            f"nxt_{i} : 0..{N}; "
            f"locked_{i} : 0..1; "
            f"pred_{i} : 0..{N};"
        )

    action_vals = ", ".join([f"act_p{i}" for i in range(N)])
    lines.extend([
        f"  action : {{{action_vals}}};",
        "",
        "ASSIGN"
    ])

    # 2. Initialization Block
    lines.append(f"  init(tail) := {NIL};")
    for i in range(N):
        lines.append(
            f"  init(p{i}) := st_p{i}_NCS; "
            f"init(nxt_{i}) := {NIL}; "
            f"init(locked_{i}) := 0; "
            f"init(pred_{i}) := {NIL};"
        )
    lines.append("")

    # 3. Next State Transitions (Flattened without nested case blocks)

    # Global Queue Tail Updates
    lines.append("  next(tail) := case")
    for i in range(N):
        lines.append(f"    action = act_p{i} & p{i} = st_p{i}_p2 : {i};")
        lines.append(f"    action = act_p{i} & p{i} = st_p{i}_p9 & tail = {i} : {NIL};")
    lines.extend([
        "    TRUE : tail;",
        "  esac;",
        ""
    ])

    # Local Process Automata
    for i in range(N):
        lines.extend([
            f"  next(p{i}) := case",
            f"    action != act_p{i} : p{i};",
            f"    p{i} = st_p{i}_NCS : st_p{i}_p2;",
            f"    p{i} = st_p{i}_p2 : st_p{i}_p3;",
            f"    p{i} = st_p{i}_p3 & pred_{i} = {NIL} : st_p{i}_CS;",
            f"    p{i} = st_p{i}_p3 & pred_{i} != {NIL} : st_p{i}_p4;",
            f"    p{i} = st_p{i}_p4 : st_p{i}_p5;",
            f"    p{i} = st_p{i}_p5 : st_p{i}_p6;",
            f"    p{i} = st_p{i}_p6 & locked_{i} = 0 : st_p{i}_CS;",
            f"    p{i} = st_p{i}_CS & nxt_{i} = {NIL} : st_p{i}_p9;",
            f"    p{i} = st_p{i}_CS & nxt_{i} != {NIL} : st_p{i}_p13;",
            f"    p{i} = st_p{i}_p9 & tail = {i} : st_p{i}_NCS;",
            f"    p{i} = st_p{i}_p9 & tail != {i} : st_p{i}_p10;",
            f"    p{i} = st_p{i}_p10 & nxt_{i} != {NIL} : st_p{i}_p13;",
            f"    p{i} = st_p{i}_p13 : st_p{i}_NCS;",
            f"    TRUE : p{i};",
            "  esac;",
            ""
        ])

    # Queue Link Tracking ('nxt' pointer generation via inverted lookup)
    for i in range(N):
        lines.append(f"  next(nxt_{i}) := case")
        lines.append(f"    action = act_p{i} & p{i} = st_p{i}_NCS : {NIL};")
        for j in range(N):
            if i != j:
                lines.append(f"    action = act_p{j} & p{j} = st_p{j}_p5 & pred_{j} = {i} : {j};")
        lines.extend([
            f"    TRUE : nxt_{i};",
            "  esac;",
            ""
        ])

    # Distributed Explicit Lock Handshaking Variables
    for i in range(N):
        lines.append(f"  next(locked_{i}) := case")
        lines.append(f"    action = act_p{i} & p{i} = st_p{i}_p4 : 1;")
        for j in range(N):
            if i != j:
                lines.append(f"    action = act_p{j} & p{j} = st_p{j}_p13 & nxt_{j} = {i} : 0;")
        lines.extend([
            f"    TRUE : locked_{i};",
            "  esac;",
            ""
        ])

    # Predecessor Reference Registers
    for i in range(N):
        lines.extend([
            f"  next(pred_{i}) := case",
            f"    action = act_p{i} & p{i} = st_p{i}_p2 : tail;",
            f"    TRUE : pred_{i};",
            "  esac;",
            ""
        ])

    # 4. Formal Verification Specifications (Mapped from mcs.xml schemas)
    safety_conds = []
    for i in range(N):
        for j in range(i + 1, N):
            safety_conds.append(f"!(p{i} = st_p{i}_CS & p{j} = st_p{j}_CS)")

    lines.append("-- ── Safety: Mutual Exclusion (No two processes occupy the critical section concurrently) ──")
    if safety_conds:
        lines.append("CTLSPEC AG (" + " & ".join(safety_conds) + ");")
    else:
        lines.append("CTLSPEC AG (TRUE);")
    lines.append("")

    # Reachability check
    reach_conds = " | ".join([f"p{i} = st_p{i}_CS" for i in range(N)])
    lines.append("-- ── Reachability: The critical section can be successfully entered ──")
    lines.append(f"CTLSPEC EF ({reach_conds});")
    lines.append("")

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(4))
