import sys

def generate_instance(N: int) -> str:
    """
    Generates a parameterized instance of the Synapse Cache Coherence
    protocol for N caches in strict .ssmv format.

    Problem Description:
      The Synapse protocol ensures cache coherence across multiple local caches
      connected over a shared broadcast bus. It uses a write-invalidate policy,
      transitioning cache lines between Invalid, Valid, and Dirty (modified) states.

    State Translation and Simplifications:
      - Cache Line Count (LINES): Fixed at 2 for parametric scaling.
      - Instead of complex bitwise shifting operations (1 << line), cache values
        are represented by a flat unsigned variable layout for each cache line index.
      - Sentinel value 255 represents an unset or NIL pointer value.
    """
    assert N >= 2, "Synapse requires at least 2 processes."
    LINES = 2  # Parameterized to 2 lines per cache layout

    def expand_set(var, elements):
        return "(" + " | ".join([f"{var} = {e}" for e in elements]) + ")"

    lines = [
        f"-- synapse_{N}.ssmv",
        f"-- Parameterized Synapse Cache Coherence Protocol with N={N} caches",
        "MODULE main",
        "VAR",
        "  lock : 0..1;",
        "  written_line : 0..255;",
        "  written_value : 0..255;",
        "  read_line : 0..255;",
        "  read_value : 0..255;"
    ]

    # 1. Variable Declarations
    for i in range(N):
        lines.append(f"  app{i} : {{st_app{i}_idle, st_app{i}_wait_read, st_app{i}_wait_write}};")
        cache_st = ["invalid", "i_bus_req", "i_app_read", "i_app_write", "iv1", "iv2", "id1", "set_value",
                    "valid", "v_bus_req", "v_app_read", "v_app_write", "wait_bus_ack",
                    "dirty", "d_bus_req", "d_app_read", "error_st"]
        lines.append(f"  cache{i} : {{" + ", ".join([f"st_cache{i}_{s}" for s in cache_st]) + "};")

        # Explicit scalar trackers for line values to prevent left-shift logic bloat
        for l in range(LINES):
            lines.append(f"  cache{i}_line{l} : 0..1;")

        lines.append(f"  cv{i} : 0..3;")
        lines.append(f"  cm{i} : -1..255;")

    bus_sts = ", ".join([f"st_bus_{s}" for s in ["idle", "send", "wait", "check"]])
    lines.append(f"  bus_st : {{{bus_sts}}}; bus_i : 0..{N}; bus_j : 0..{N}; bus_v : 0..3; bus_m : -1..255;")

    action_vals = []
    for i in range(N):
        for l in range(LINES):
            action_vals.append(f"act_App{i}_R{l}")
            for v in range(2):
                action_vals.append(f"act_App{i}_W{l}_{v}")
        action_vals.extend([f"act_App{i}_End", f"act_Cache{i}_Step", f"act_Bus_Start{i}"])
    action_vals.append("act_Bus_Step")

    lines.append(f"  action : {{{', '.join(action_vals)}}};")
    lines.append("ASSIGN")

    # 2. Initialization Block
    lines.append("  init(lock) := 0;")
    lines.append("  init(written_line) := 255;")
    lines.append("  init(written_value) := 255;")
    lines.append("  init(read_line) := 255;")
    lines.append("  init(read_value) := 255;")
    lines.append("  init(bus_st) := st_bus_idle;")
    lines.append("  init(bus_i) := 0;")
    lines.append("  init(bus_j) := 0;")
    lines.append("  init(bus_v) := 0;")
    lines.append("  init(bus_m) := -1;")

    for i in range(N):
        lines.append(f"  init(app{i}) := st_app{i}_idle;")
        lines.append(f"  init(cache{i}) := st_cache{i}_valid;")
        for l in range(LINES):
            lines.append(f"  init(cache{i}_line{l}) := 0;")
        lines.append(f"  init(cv{i}) := 0;")
        lines.append(f"  init(cm{i}) := -1;")
    lines.append("")

    # 3. Next State Transitions (Flattened and unrolled structural blocks)

    # Global System Communication Lock
    lines.append("  next(lock) := case")
    for i in range(N):
        c_acts = expand_set("action", [f"act_App{i}_R0", f"act_App{i}_R1", f"act_App{i}_W0_0", f"act_App{i}_W0_1", f"act_App{i}_W1_0", f"act_App{i}_W1_1"])
        lines.append(f"    {c_acts} & app{i} = st_app{i}_idle & lock = 0 : 1;")
        lines.append(f"    action = act_App{i}_End : 0;")
    lines.extend([
        "    TRUE : lock;",
        "  esac;",
        ""
    ])

    # Specification Instrumentation Logging
    lines.append("  next(written_line) := case")
    for i in range(N):
        for l in range(LINES):
            c_acts = expand_set("action", [f"act_App{i}_W{l}_0", f"act_App{i}_W{l}_1"])
            lines.append(f"    {c_acts} & app{i} = st_app{i}_idle & lock = 0 : {l};")
    lines.extend([
        "    TRUE : written_line;",
        "  esac;",
        ""
    ])

    lines.append("  next(written_value) := case")
    for i in range(N):
        for l in range(LINES):
            for v in range(2):
                lines.append(f"    action = act_App{i}_W{l}_{v} & app{i} = st_app{i}_idle & lock = 0 : {v};")
    lines.extend([
        "    TRUE : written_value;",
        "  esac;",
        ""
    ])

    lines.append("  next(read_line) := case")
    for i in range(N):
        for l in range(LINES):
            lines.append(f"    action = act_App{i}_R{l} & app{i} = st_app{i}_idle & lock = 0 : {l};")
    lines.extend([
        "    TRUE : read_line;",
        "  esac;",
        ""
    ])

    lines.append("  next(read_value) := case")
    for i in range(N):
        c_acts = expand_set("action", [f"act_App{i}_R0", f"act_App{i}_R1"])
        lines.append(f"    {c_acts} & app{i} = st_app{i}_idle & lock = 0 : 2;")
        lines.append(f"    action = act_App{i}_End & app{i} = st_app{i}_wait_read & read_line = 0 : cache{i}_line0;")
        lines.append(f"    action = act_App{i}_End & app{i} = st_app{i}_wait_read & read_line = 1 : cache{i}_line1;")
    lines.extend([
        "    TRUE : read_value;",
        "  esac;",
        ""
    ])

    # Core Application Threads Coordination
    for i in range(N):
        c_acts_r = expand_set("action", [f"act_App{i}_R0", f"act_App{i}_R1"])
        c_acts_w = expand_set("action", [f"act_App{i}_W0_0", f"act_App{i}_W0_1", f"act_App{i}_W1_0", f"act_App{i}_W1_1"])

        lines.extend([
            f"  next(app{i}) := case",
            f"    {c_acts_r} & app{i} = st_app{i}_idle & lock = 0 : st_app{i}_wait_read;",
            f"    {c_acts_w} & app{i} = st_app{i}_idle & lock = 0 : st_app{i}_wait_write;",
            f"    action = act_App{i}_End & app{i} = st_app{i}_wait_read : st_app{i}_idle;",
            f"    action = act_App{i}_End & app{i} = st_app{i}_wait_write : st_app{i}_idle;",
            f"    TRUE : app{i};",
            "  esac;",
            ""
        ])

        # Unified Cache Protocol State Machine Transitions
        lines.extend([
            f"  next(cache{i}) := case",
            f"    action = act_App{i}_R0 & cache{i} = st_cache{i}_invalid : st_cache{i}_i_app_read;",
            f"    action = act_App{i}_R1 & cache{i} = st_cache{i}_invalid : st_cache{i}_i_app_read;",
            f"    action = act_App{i}_W0_0 & cache{i} = st_cache{i}_invalid : st_cache{i}_i_app_write;",
            f"    action = act_App{i}_W1_0 & cache{i} = st_cache{i}_invalid : st_cache{i}_i_app_write;",
            f"    action = act_Cache{i}_Step & cache{i} = st_cache{i}_i_app_read : st_cache{i}_iv1;",
            f"    action = act_Cache{i}_Step & cache{i} = st_cache{i}_iv1 : st_cache{i}_iv2;",
            f"    action = act_Cache{i}_Step & cache{i} = st_cache{i}_iv2 : st_cache{i}_valid;",
            f"    action = act_Cache{i}_Step & cache{i} = st_cache{i}_i_app_write : st_cache{i}_id1;",
            f"    action = act_Cache{i}_Step & cache{i} = st_cache{i}_id1 : st_cache{i}_set_value;",
            f"    action = act_Cache{i}_Step & cache{i} = st_cache{i}_set_value : st_cache{i}_dirty;",
            f"    action = act_Bus_Step & bus_st = st_bus_send & bus_j = {i} & cache{i} = st_cache{i}_invalid : st_cache{i}_i_bus_req;",
            f"    action = act_Bus_Step & bus_st = st_bus_send & bus_j = {i} & cache{i} = st_cache{i}_valid : st_cache{i}_v_bus_req;",
            f"    action = act_Bus_Step & bus_st = st_bus_send & bus_j = {i} & cache{i} = st_cache{i}_dirty : st_cache{i}_d_bus_req;",
            f"    action = act_Bus_Step & bus_st = st_bus_check & bus_i = {i} & bus_m = 3 : st_cache{i}_error_st;",
            f"    action = act_App{i}_End : st_cache{i}_valid;",
            f"    TRUE : cache{i};",
            "  esac;",
            ""
        ])

        # Cache Storage Cell Assignment Blocks (Scalar assignments replacing dynamic left-shifts)
        for l in range(LINES):
            lines.extend([
                f"  next(cache{i}_line{l}) := case",
                f"    action = act_App{i}_W{l}_0 & cache{i} = st_cache{i}_set_value : 0;",
                f"    action = act_App{i}_W{l}_1 & cache{i} = st_cache{i}_set_value : 1;",
                f"    action = act_Bus_Step & bus_st = st_bus_check & bus_j = {i} & bus_m = 2 : 0;", # Invalidation signal
                f"    TRUE : cache{i}_line{l};",
                "  esac;",
                ""
            ])

        # Intermediate Bus Variable Storage Updates
        lines.extend([
            f"  next(cv{i}) := case",
            f"    action = act_Cache{i}_Step & cache{i} = st_cache{i}_iv1 : bus_v;",
            f"    TRUE : cv{i};",
            "  esac;",
            "",
            f"  next(cm{i}) := case",
            f"    action = act_App{i}_R0 : 0;",
            f"    action = act_App{i}_R1 : 1;",
            f"    action = act_Bus_Step & bus_st = st_bus_send & bus_j = {i} : bus_m;",
            f"    TRUE : cm{i};",
            "  esac;",
            ""
        ])

    # Shared Bus controller automaton (Flattened internal switch constraints)
    lines.extend([
        "  next(bus_st) := case",
        "    action = act_Bus_Step & bus_st = st_bus_idle : st_bus_send;",
        f"   action = act_Bus_Step & bus_st = st_bus_send & bus_j = {N} : st_bus_wait;",
        f"   action = act_Bus_Step & bus_st = st_bus_wait & bus_j = {N} : st_bus_check;",
        "    action = act_Bus_Step & bus_st = st_bus_check : st_bus_idle;",
        "    TRUE : bus_st;",
        "  esac;",
        ""
    ])

    # Strict flattening applied to bus loop increments (No nested structures)
    lines.extend([
        "  next(bus_j) := case",
        f"    action = act_Bus_Step & bus_st = st_bus_send & bus_j < {N} : bus_j + 1;",
        f"    action = act_Bus_Step & bus_st = st_bus_wait & bus_j < {N} : bus_j + 1;",
        "    action = act_Bus_Step & bus_st = st_bus_check : 0;",
        "    TRUE : bus_j;",
        "  esac;",
        ""
    ])

    # 4. Formal Verification Specifications (Derived from AP rules in synapse.xml)
    lines.extend([
        "-- ── Safety Property 1: Cache never encounters an invalid protocol state error ──",
        "CTLSPEC AG (cache0 != st_cache0_error_st);",
        "",
        "-- ── Safety Property 2: Value coherence across written/read barriers ──",
        "CTLSPEC AG (written_line = 1 & written_value = 1 -> EF (read_line = 1 & read_value = 1));",
        ""
    ])

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(3))
