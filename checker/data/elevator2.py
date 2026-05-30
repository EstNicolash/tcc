import sys

def generate_instance(N: int) -> str:
    """
    Generates a parameterized instance of the Elevator2 benchmark for N floors
    in the strict .ssmv format.

    Problem Description:
      This model represents a clever elevator controller serving N floors, based
      on the Lego elevator model from the Paradise laboratory. The controller
      sweeps floors in the current direction of movement (ldir), reversing
      direction when it reaches the top or bottom boundaries.

    Mathematical Translation (Negative Elimination):
      The original DVE model allows the target floor variable 't' to drop to -1
      to signal a boundary condition. To make this model compatible with our
      purely natural (unsigned) BDD ALU, an offset of +1 is applied to 't' and 'p'.
      - Original floor -1 becomes Natural 0.
      - Original floor 0 becomes Natural 1.
      - Original floor N becomes Natural N + 1.

    Implementation Constraints Respected:
      - Strictly scalar assignments (no nested 'case' expressions).
      - Domain range padded to safe power-of-two boundaries.
      - Grouped INIT and NEXT assignments in unified structures.
      - Restored documentation and clean formatting.
    """
    assert N >= 2, "Elevator2 requires at least 2 floors."

    # We pad the boundaries to powers of two matching the domain layout for safety
    lines = [
        f"-- elevator2_{N}.ssmv",
        f"-- Elevator controller with {N} floors (Natural Offset Translation)",
        "MODULE main",
        "VAR"
    ]

    # 1. Variable Declarations
    for i in range(N):
        lines.append(f"  req_{i} : 0..1;")

    lines.extend([
        f"  t : 0..{N+2};",      # Mapped from original -1..N
        f"  p : 0..{N+2};",      # Mapped from original 0..N
        "  v : 0..1;",
        "  ldir : 0..1;",
        "  cabin : {st_cabin_idle, st_cabin_mov, st_cabin_open};",
        "  ctrl : {st_ctrl_wait, st_ctrl_work, st_ctrl_done};",
        ""
    ])

    env_actions = ", ".join([f"act_env_{i}" for i in range(N)])
    lines.append(f"  action : {{act_cabin, act_ctrl, {env_actions}}};")
    lines.append("")

    lines.append("ASSIGN")

    # 2. Initialization Block
    for i in range(N):
        lines.append(f"  init(req_{i}) := 0;")
    lines.extend([
        "  init(t) := 1;",       # Mapped from original 0 (0 + 1 = 1)
        "  init(p) := 1;",       # Mapped from original 0 (0 + 1 = 1)
        "  init(v) := 0;",
        "  init(ldir) := 0;",
        "  init(cabin) := st_cabin_idle;",
        "  init(ctrl) := st_ctrl_wait;",
        ""
    ])

    # 3. Next State Transitions (Flattened without nested case blocks)

    # Position 'p' transitions (translated floor bounds)
    lines.extend([
        "  next(p) := case",
        "    action = act_cabin & cabin = st_cabin_mov & t < p & p > 1 : p - 1;",
        f"   action = act_cabin & cabin = st_cabin_mov & t > p & p < {N} : p + 1;",
        "    TRUE : p;",
        "  esac;",
        ""
    ])

    # Target 't' transitions (translated floor bounds)
    lines.extend([
        "  next(t) := case",
        "    action = act_ctrl & ctrl = st_ctrl_wait & v = 0 & ldir = 0 & t > 0 : t - 1;",
        f"   action = act_ctrl & ctrl = st_ctrl_wait & v = 0 & ldir = 1 & t < {N+1} : t + 1;",
    ])
    for i in range(N):
        # We transform the evaluation check for floor i to its translated coordinate (i + 1)
        lines.extend([
            f"    action = act_ctrl & ctrl = st_ctrl_work & t = {i+1} & req_{i} = 0 & ldir = 0 & t > 0 : t - 1;",
            f"    action = act_ctrl & ctrl = st_ctrl_work & t = {i+1} & req_{i} = 0 & ldir = 1 & t < {N+1} : t + 1;",
        ])
    lines.extend([
        "    TRUE : t;",
        "  esac;",
        ""
    ])

    # Control 'v' transitions
    lines.extend([
        "  next(v) := case",
        "    action = act_ctrl & ctrl = st_ctrl_done : 1;",
        "    action = act_cabin & cabin = st_cabin_open : 0;",
        "    TRUE : v;",
        "  esac;",
        ""
    ])

    # Direction 'ldir' transitions (original bounds checking: t < 0 became t = 0, t = N became t = N+1)
    lines.extend([
        "  next(ldir) := case",
        f"   action = act_ctrl & ctrl = st_ctrl_work & (t = 0 | t = {N+1}) & ldir = 0 : 1;",
        f"   action = act_ctrl & ctrl = st_ctrl_work & (t = 0 | t = {N+1}) & ldir = 1 : 0;",
        "    TRUE : ldir;",
        "  esac;",
        ""
    ])

    # Cabin state transitions
    lines.extend([
        "  next(cabin) := case",
        "    action = act_cabin & cabin = st_cabin_idle & v > 0 : st_cabin_mov;",
        "    action = act_cabin & cabin = st_cabin_mov & t = p : st_cabin_open;",
        "    action = act_cabin & cabin = st_cabin_open : st_cabin_idle;",
        "    TRUE : cabin;",
        "  esac;",
        ""
    ])

    # Controller state transitions
    lines.extend([
        "  next(ctrl) := case",
        "    action = act_ctrl & ctrl = st_ctrl_wait & v = 0 : st_ctrl_work;",
        f"   action = act_ctrl & ctrl = st_ctrl_work & (t = 0 | t = {N+1}) : st_ctrl_wait;"
    ])
    for i in range(N):
        lines.append(f"    action = act_ctrl & ctrl = st_ctrl_work & t = {i+1} & req_{i} = 1 : st_ctrl_done;")
    lines.extend([
        "    action = act_ctrl & ctrl = st_ctrl_done : st_ctrl_wait;",
        "    TRUE : ctrl;",
        "  esac;",
        ""
    ])

    # Request floor environments
    for i in range(N):
        lines.append(f"  next(req_{i}) := case action = act_cabin & cabin = st_cabin_open & p = {i+1} : 0; action = act_env_{i} & req_{i} = 0 : 1; TRUE : req_{i}; esac;")
    lines.append("")

    # 4. Specifications Mapped to the Translated Context
    lines.append("-- ── Safety: Doors only open at target floor ──")
    lines.append("CTLSPEC AG (cabin = st_cabin_open -> t = p);")
    lines.append("")
    lines.append("-- ── Reachability: The highest floor can be served ──")
    lines.append(f"CTLSPEC EF (p = {N} & cabin = st_cabin_open);")
    lines.append("")

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(4))
