import sys

def generate_instance(N: int) -> str:
    """
    Generates a parameterized instance of Fischer's Real-Time Mutual Exclusion
    Protocol for N processes in the strict .ssmv format.

    Problem Description:
      Fischer's protocol achieves mutual exclusion using discrete real-time bounds
      and a single shared variable (id). Processes check deadlines using localized
      countdown timers (t0..tN). If a process delays setting the ID past K1, but
      waits at least K2 before entering the Critical Section (CS), safety is guaranteed.

    Scaling and Parameters:
      - N: Number of concurrent processes competing for the resource.
      - K1: Maximum delay allowed to register process identity (fixed to 3).
      - K2: Minimum wait time required before entering CS (fixed to 3).
      - Domain bounds: Timers reside in 0..255; id tracks 0..N.

    Implementation Constraints Respected:
      - Strictly flattened assignments (NO nested 'case' structures).
      - Guarded countdown loops to ensure purely positive unsigned subtraction.
      - Unified structure layout with restored academic problem documentation.
    """
    k1: int = 3
    k2: int = 3
    n = N
    assert n >= 2, "Fischer requires at least 2 processes."

    lines = [
        f"-- fischer_{n}.ssmv",
        f"-- Fischer's Mutual Exclusion Protocol ({n} processes)",
        f"-- Parametric Bounds: K1={k1}, K2={k2}",
        "MODULE main",
        "VAR",
        f"  id : 0..{n};"
    ]

    # 1. Variable Declarations
    for i in range(n):
        p_states = ", ".join([f"st_p{i}_{s}" for s in ["NCS", "try", "wait", "CS"]])
        lines.append(f"  p{i} : {{{p_states}}};")
        lines.append(f"  t{i} : 0..255;")

    action_vals = ", ".join([f"act_p{i}" for i in range(n)] + ["timer"])
    lines.append(f"  action : {{{action_vals}}};")
    lines.append("")

    lines.append("ASSIGN")
    lines.append("  init(id) := 0;")
    for i in range(n):
        lines.append(f"  init(p{i}) := st_p{i}_NCS;")
        lines.append(f"  init(t{i}) := 255;")
    lines.append("")

    # 2. Global Protocol Identifier Updates
    lines.append("  next(id) := case")
    for i in range(n):
        lines.append(f"    action = act_p{i} & p{i} = st_p{i}_try : {i + 1};")
        lines.append(f"    action = act_p{i} & p{i} = st_p{i}_CS : 0;")
    lines.append("    TRUE : id;")
    lines.append("  esac;\n")

    # 3. Process State Machine Transitions
    for i in range(n):
        lines.extend([
            f"  next(p{i}) := case",
            f"    action != act_p{i} : p{i};",
            f"    p{i} = st_p{i}_NCS & id = 0 : st_p{i}_try;",
            f"    p{i} = st_p{i}_try : st_p{i}_wait;",
            f"    p{i} = st_p{i}_wait & t{i} = 0 : st_p{i}_wait;",
            f"    p{i} = st_p{i}_wait & t{i} = 255 & id = {i+1} : st_p{i}_CS;",
            f"    p{i} = st_p{i}_wait & t{i} = 255 & id != {i+1} : st_p{i}_NCS;",
            f"    p{i} = st_p{i}_CS : st_p{i}_NCS;",
            f"    TRUE : p{i};",
            f"  esac;"
        ])
    lines.append("")

    # 4. Global Clock-Tick Decrements and Updates (Flattened)
    all_timers_not_zero = " & ".join(f"t{i} != 0" for i in range(n))
    for i in range(n):
        lines.extend([
            f"  next(t{i}) := case",
            # Flattened: Combined countdown conditions to avoid nested case allocations
            f"    action = timer & ({all_timers_not_zero}) & t{i} != 255 & t{i} > 0 : t{i} - 1;",
            f"    action = timer & ({all_timers_not_zero}) & t{i} != 255 & t{i} = 0 : t{i};",
            f"    action = act_p{i} & p{i} = st_p{i}_NCS & id = 0 : {k1};",
            f"    action = act_p{i} & p{i} = st_p{i}_try : {k2};",
            f"    action = act_p{i} & p{i} = st_p{i}_wait & t{i} = 0 : 255;",
            f"    TRUE : t{i};",
            f"  esac;"
        ])
    lines.append("")

    # 5. Formal Verification Specifications (Mmapped from XML Reachability schemas)
    safety_conds = []
    for i in range(n):
        for j in range(i + 1, n):
            safety_conds.append(f"!(p{i} = st_p{i}_CS & p{j} = st_p{j}_CS)")

    lines.append("-- ── Safety: Mutual Exclusion (No two processes share the critical section) ──")
    if safety_conds:
        lines.append("CTLSPEC AG (" + " & ".join(safety_conds) + ");")
    else:
        lines.append("CTLSPEC AG (TRUE);")
    lines.append("")

    lines.append("-- ── Reachability: The critical section can be successfully entered ──")
    reach_conds = " | ".join(f"p{i} = st_p{i}_CS" for i in range(n))
    lines.append(f"CTLSPEC EF ({reach_conds});")
    lines.append("")

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(3))
