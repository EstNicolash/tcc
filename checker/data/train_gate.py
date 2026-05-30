import sys

def generate_instance(N: int) -> str:
    """
    Generates a parameterized instance of the Train-Gate controller protocol
    for N nodes (T = N - 1 trains + 1 gate controller) in strict .ssmv format.

    Problem Description:
      This model is a discrete-time translation of a real-time railway gate
      controller based on an UPPAAL benchmark. It coordinates multiple independent
      trains crossing a shared intersection bridge. A centralized gate monitors
      approaching requests, commands conflicting trains to stop, and leverages a
      FIFO queue to store halted processes, releasing them safely one by one.

    Architectural Decisions & Optimization Strategies:
      - Bounded process counters (T) track individual trains (0..T-1).
      - Replaced the reserved keyword variable array 'next' with 'nxt_track_{i}'
        to bypass structural token collisions in ssmv_parser.rs.
      - Fully flattened nested case selections within clock and pointer mutations.
      - Refactored the dynamic array queue loop into localized concurrent slot
        monitors to optimize the BDD transition relation and maximize OxiDD performance.
    """
    T = N - 1
    assert T >= 1, "Train-Gate requires at least 1 train (N >= 2)."

    lines = [
        f"-- train_gate_{N}.ssmv",
        f"-- Parameterized Train-Gate Bridge Controller with T={T} trains",
        "MODULE main",
        "VAR",
        "  x : 0..31;",
        "  gate : {st_gate_Free, st_gate_S1, st_gate_S2, st_gate_S3, st_gate_S4, st_gate_S5, st_gate_S6, st_gate_Occ, st_gate_Free_Wait};"
    ]

    # 1. Variable Declarations
    for i in range(T):
        lines.append(f"  p{i} : {{st_p{i}_Safe, st_p{i}_Stop, st_p{i}_Cross, st_p{i}_Appr, st_p{i}_Start}};")
        lines.append(f"  mx{i} : 0..31;")

    # Queue buffers for waiting trains (0 means empty slot)
    for i in range(N):
        lines.append(f"  l{i} : 0..{T};")

    lines.extend([
        f"  q_len : 0..{N};",
        "  e : 0..31;"
    ])

    train_actions = ", ".join([f"act_train{i}" for i in range(T)])
    lines.extend([
        f"  action : {{timer, act_gate, {train_actions}}};",
        "",
        "ASSIGN"
    ])

    # 2. Initialization Block
    lines.extend([
        "  init(x) := 0;",
        "  init(gate) := st_gate_Free;",
        "  init(q_len) := 0;",
        "  init(e) := 0;"
    ])
    for i in range(T):
        lines.append(f"  init(p{i}) := st_p{i}_Safe;")
        lines.append(f"  init(mx{i}) := 25;")
    for i in range(N):
        lines.append(f"  init(l{i}) := 0;")
    lines.append("")

    # 3. Next State Transitions (Strictly flattened)

    # Discrete Clock Tracker Updates (No nested case blocks)
    clock_guard = " & ".join([f"x < mx{i}" for i in range(T)])
    lines.append("  next(x) := case")
    for i in range(T):
        tc = f"(action = act_train{i} & ((p{i} = st_p{i}_Safe & (gate = st_gate_S4 | gate = st_gate_Occ)) | (p{i} = st_p{i}_Appr & x >= 10) | (p{i} = st_p{i}_Appr & x <= 10 & gate = st_gate_S6 & e = {i+1}) | (p{i} = st_p{i}_Cross & x >= 3 & gate = st_gate_Occ) | (p{i} = st_p{i}_Stop & gate = st_gate_Free_Wait & e = {i+1}) | (p{i} = st_p{i}_Start & x >= 7)))"
        lines.append(f"    {tc} : 0;")
    lines.extend([
        f"    action = timer & {clock_guard} & x < 25 : x + 1;",
        "    TRUE : x;",
        "  esac;",
        ""
    ])

    # Centralized Railway Gate Automation State Machine
    lines.append("  next(gate) := case")
    lines.append("    action = act_gate & gate = st_gate_Free & q_len = 0 : st_gate_S4;")
    lines.append("    action = act_gate & gate = st_gate_Free & q_len > 0 : st_gate_S5;")
    lines.append("    action = act_gate & gate = st_gate_S5 : st_gate_Free_Wait;")
    lines.append("    action = act_gate & gate = st_gate_S3 : st_gate_Occ;")
    lines.append("    action = act_gate & gate = st_gate_S2 : st_gate_Occ;")
    lines.append("    action = act_gate & gate = st_gate_S1 : st_gate_Free;")
    for i in range(T):
        lines.append(f"    action = act_train{i} & p{i} = st_p{i}_Safe & gate = st_gate_Free : st_gate_S3;")
        lines.append(f"    action = act_train{i} & p{i} = st_p{i}_Safe & gate = st_gate_Occ : st_gate_S6;")
        lines.append(f"    action = act_train{i} & p{i} = st_p{i}_Appr & x <= 10 & gate = st_gate_S6 : st_gate_S2;")
        lines.append(f"    action = act_train{i} & p{i} = st_p{i}_Cross & x >= 3 & gate = st_gate_Occ : st_gate_S1;")
        lines.append(f"    action = act_train{i} & p{i} = st_p{i}_Stop & gate = st_gate_Free_Wait & e = {i+1} : st_gate_Occ;")
    lines.extend([
        "    TRUE : gate;",
        "  esac;",
        ""
    ])

    # Parameterized Train Process States & Upper Clock Bounds
    for i in range(T):
        lines.extend([
            f"  next(p{i}) := case",
            f"    action != act_train{i} : p{i};",
            f"    p{i} = st_p{i}_Safe & gate = st_gate_Free : st_p{i}_Appr;",
            f"    p{i} = st_p{i}_Safe & gate = st_gate_Occ : st_p{i}_Appr;",
            f"    p{i} = st_p{i}_Appr & x >= 10 : st_p{i}_Cross;",
            f"    p{i} = st_p{i}_Appr & x <= 10 & gate = st_gate_S6 : st_p{i}_Stop;",
            f"    p{i} = st_p{i}_Cross & x >= 3 & gate = st_gate_Occ : st_p{i}_Safe;",
            f"    p{i} = st_p{i}_Stop & gate = st_gate_Free_Wait & e = {i+1} : st_p{i}_Start;",
            f"    p{i} = st_p{i}_Start & x >= 7 : st_p{i}_Cross;",
            f"    TRUE : p{i};",
            "  esac;",
            ""
        ])

        lines.extend([
            f"  next(mx{i}) := case",
            f"    action = act_train{i} & p{i} = st_p{i}_Safe & gate = st_gate_Free : 20;",
            f"    action = act_train{i} & p{i} = st_p{i}_Safe & gate = st_gate_Occ : 20;",
            f"    action = act_train{i} & p{i} = st_p{i}_Appr & x >= 10 : 5;",
            f"    action = act_train{i} & p{i} = st_p{i}_Appr & x <= 10 & gate = st_gate_S6 : 25;",
            f"    action = act_train{i} & p{i} = st_p{i}_Cross & x >= 3 & gate = st_gate_Occ : 25;",
            f"    action = act_train{i} & p{i} = st_p{i}_Stop & gate = st_gate_Free_Wait & e = {i+1} : 15;",
            f"    action = act_train{i} & p{i} = st_p{i}_Start & x >= 7 : 5;",
            f"    TRUE : mx{i};",
            "  esac;",
            ""
        ])

    # Concurrent FIFO Queue Mechanics (Flattened buffer increments)

    lines.append("  next(q_len) := case")
    lines.append(f"    action = act_gate & (gate = st_gate_S3 | gate = st_gate_S2) & q_len < {N} : q_len + 1;")
    lines.append("    action = act_gate & gate = st_gate_S1 & q_len > 0 : q_len - 1;")
    lines.extend([
        "    TRUE : q_len;",
        "  esac;",
        ""
    ])

    # Dynamic Shared Target Register Evaluation
    lines.append("  next(e) := case")
    for i in range(T):
        lines.append(f"    action = act_train{i} & p{i} = st_p{i}_Safe & gate = st_gate_Free : {i+1};")
        lines.append(f"    action = act_train{i} & p{i} = st_p{i}_Safe & gate = st_gate_Occ : {i+1};")
        lines.append(f"    action = act_train{i} & p{i} = st_p{i}_Cross & x >= 3 & gate = st_gate_Occ : {i+1};")
    lines.extend([
        "    action = act_gate & gate = st_gate_S5 : l0;",
        "    TRUE : e;",
        "  esac;",
        ""
    ])

    # Statically unrolled storage arrays tracking indices to bypass queue loop bottlenecks
    for b in range(N):
        lines.append(f"  next(l{b}) := case")
        # Shift elements forward on popped channel steps
        if b < N - 1:
            lines.append(f"    action = act_gate & gate = st_gate_S1 : l{b+1};")
        else:
            lines.append(f"    action = act_gate & gate = st_gate_S1 : 0;")
        # Append elements to the end of the current queue track length index
        for i in range(T):
            lines.append(f"    action = act_gate & (gate = st_gate_S3 | gate = st_gate_S2) & q_len = {b} : {i+1};")
        lines.extend([
            f"    TRUE : l{b};",
            "  esac;",
            ""
        ])

    # 4. Formal System Verification Specifications (Derived from schemas in train-gate.xml)
    collision_pairs = []
    for i in range(T):
        for j in range(i + 1, T):
            collision_pairs.append(f"!(p{i} = st_p{i}_Cross & p{j} = st_p{j}_Cross)")

    lines.append("-- ── Safety: Mutual Exclusion (No two trains cross the bridge simultaneously) ──")
    if collision_pairs:
        lines.append("CTLSPEC AG (" + " & ".join(collision_pairs) + ");")
    else:
        lines.append("CTLSPEC AG (TRUE);")
    lines.append("")

    # Reachability checks
    reach_conds = " | ".join([f"p{i} = st_p{i}_Cross" for i in range(T)])
    lines.append("-- ── Reachability: At least one train can successfully cross the bridge ──")
    lines.append(f"CTLSPEC EF ({reach_conds});")
    lines.append("")

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(3))
