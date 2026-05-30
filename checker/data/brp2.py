import sys

def generate_instance(N: int) -> str:
    """
    Generates an instance of the Bounded Retransmission Protocol (BRP) with timing
    for N retransmissions in the strict .ssmv format.

    Problem Description:
      Based on the alternating bit protocol, BRP is used in Philips products.
      It allows only a bounded number of retransmissions (MAX) for each frame.
      This model represents a discrete-time simulation of the real-time protocol.

    Scaling and Parameters:
      - N (MAX): Maximal number of retransmissions.
      - num_frames (n): Fixed to 3 frames per file transmission.
      - TD: Transmission delay (fixed to 2).
      - T1: Sender timeout (fixed to 5).
      - TR: Sync timeout (computed as 2 * MAX * T1 + 3 * TD).

    Implementation Constraints Respected:
      - Strictly scalar assignments (no nested 'case' expressions).
      - No Python operators (like '==' or 'in') leaked into SSMV guards.
      - Isolated enum scopes (e.g., st_K_start, st_L_start).
      - Pure numeric ranges explicitly aligned with bit-blasting boundaries.
      - Restored CTL Properties based on brp2.xml reachability specifications.
    """
    max_retrans = N
    num_frames = 3
    td = 2
    t1 = 2 * td + 1 # 5
    tr = 2 * max_retrans * t1 + 3 * td
    maxtime = tr + 1

    lines = [
        f"-- brp2_{N}.ssmv",
        f"-- Bounded Retransmission Protocol (N={N} retransmissions)",
        f"-- Parameters: MAX={max_retrans}, n={num_frames}, TD={td}, T1={t1}, TR={tr}",
        "MODULE main",
        "VAR",
        # Extended bounds to prevent bit overflow out-of-bounds panics
        f"  clk_X : 0..{maxtime+1};",
        f"  clk_U : 0..{maxtime+1};",
        f"  clk_V : 0..{maxtime+1};",
        f"  clk_W : 0..{maxtime+1};",
        f"  clk_Z : 0..{maxtime+1};",
        "  File : {st_File_SAME, st_File_OTHER};",
        "  SClient_state : {st_SClient_ok, st_SClient_dk, st_SClient_nok, st_SClient_send_req, st_SClient_file_req};",
        "  RClient_state : {st_RClient_ok, st_RClient_inc, st_RClient_nok};",
        "  Sender_state : {st_Sender_init, st_Sender_idle, st_Sender_next_frame, st_Sender_wait_ack, st_Sender_success, st_Sender_error};",
        "  Sender_ab : 0..2;",
        f"  Sender_i : 1..{num_frames+1};",
        f"  Sender_rc : 0..{max_retrans+1};",
        "  Receiver_state : {st_Receiver_new_file, st_Receiver_first_safe_frame, st_Receiver_frame_received, st_Receiver_frame_reported, st_Receiver_idle};",
        "  Receiver_exp_ab : 0..2;",
        "  Receiver_triple : 0..8;",
        "  K_state : {st_K_start, st_K_in_transit, st_K_BAD};",
        "  K_triple : 0..8;",
        "  L_state : {st_L_start, st_L_in_transit, st_L_BAD};",
        "  action : {tick, act_SClient_internal, act_Sender_internal, act_Receiver_internal, act_K_loss, act_L_loss, act_Sin, act_Sout_OK, act_Sout_DK, act_Sout_NOK, act_F, act_G, act_A, act_B, act_Rout_OK, act_Rout_INC, act_Rout_FST, act_Rout_NOK};",
        ""
    ]

    # Global Timing Invariant (Prevents time advancing beyond strict deadlines)
    invariant = (
        f"!(Receiver_state = st_Receiver_first_safe_frame | Receiver_state = st_Receiver_frame_received | Receiver_state = st_Receiver_frame_reported | Sender_state = st_Sender_next_frame | Sender_state = st_Sender_success) "
        f"& (Sender_state = st_Sender_wait_ack -> clk_X < {t1}) "
        f"& (Sender_state = st_Sender_error -> clk_X < {tr}) "
        f"& (K_state = st_K_in_transit -> clk_U < {td}) "
        f"& (L_state = st_L_in_transit -> clk_V < {td}) "
        f"& (Receiver_state = st_Receiver_idle -> clk_Z < {tr})"
    )

    lines.append("ASSIGN")

    # Initialization Block (Corrected variable names with clk_ prefix)
    for c in ['clk_X', 'clk_U', 'clk_V', 'clk_W', 'clk_Z']:
        lines.append(f"  init({c}) := 0;")

    lines.extend([
        "  init(File) := st_File_SAME;",
        "  init(SClient_state) := st_SClient_ok;",
        "  init(RClient_state) := st_RClient_ok;",
        "  init(Sender_state) := st_Sender_init;",
        "  init(Sender_ab) := 0;",
        "  init(Sender_i) := 1;",
        "  init(Sender_rc) := 0;",
        "  init(Receiver_state) := st_Receiver_new_file;",
        "  init(Receiver_exp_ab) := 0;",
        "  init(Receiver_triple) := 0;",
        "  init(K_state) := st_K_start;",
        "  init(K_triple) := 0;",
        "  init(L_state) := st_L_start;",
        ""
    ])

    # Clock Advances
    for c in ['clk_X', 'clk_U', 'clk_V', 'clk_W', 'clk_Z']:
        lines.append(f"  next({c}) := case")
        lines.append(f"    action = tick & ({invariant}) & {c} < {maxtime} : {c} + 1;")
        if c == 'clk_X':
            lines.extend([
                "    action = act_Sin : 0;",
                "    action = act_F & Sender_state = st_Sender_wait_ack : 0;",
                "    action = act_B : 0;",
                "    action = act_Sout_DK | action = act_Sout_NOK : 0;"
            ])
        elif c == 'clk_U':
            lines.append("    action = act_F : 0;")
        elif c == 'clk_V':
            lines.append("    action = act_A : 0;")
        elif c == 'clk_W':
            lines.append("    action = act_G : 0;")
        elif c == 'clk_Z':
            lines.extend([
                "    action = act_G & Receiver_state = st_Receiver_new_file : 0;",
                "    action = act_A & Receiver_state = st_Receiver_frame_reported : 0;"
            ])
        lines.append(f"    TRUE : {c};")
        lines.append("  esac;")

    lines.append("")

    # State Transitions (Flattened and Cleaned)
    lines.extend([
        "  next(File) := case",
        "    action = act_SClient_internal : st_File_OTHER;",
        "    action = act_Rout_FST | action = act_Rout_OK : st_File_SAME;",
        "    TRUE : File;",
        "  esac;",

        "  next(SClient_state) := case",
        "    action = act_SClient_internal & (SClient_state = st_SClient_ok | SClient_state = st_SClient_dk | SClient_state = st_SClient_nok) : st_SClient_send_req;",
        "    action = act_Sin & SClient_state = st_SClient_send_req : st_SClient_file_req;",
        "    action = act_Sout_OK & SClient_state = st_SClient_file_req : st_SClient_ok;",
        "    action = act_Sout_DK & SClient_state = st_SClient_file_req : st_SClient_dk;",
        "    action = act_Sout_NOK & SClient_state = st_SClient_file_req : st_SClient_nok;",
        "    TRUE : SClient_state;",
        "  esac;",

        "  next(RClient_state) := case",
        "    (action = act_Rout_FST | action = act_Rout_OK) & (RClient_state = st_RClient_ok | RClient_state = st_RClient_nok) : st_RClient_inc;",
        "    action = act_Rout_INC & RClient_state = st_RClient_inc : st_RClient_inc;",
        "    action = act_Rout_OK & RClient_state = st_RClient_inc : st_RClient_ok;",
        "    action = act_Rout_NOK & RClient_state = st_RClient_inc : st_RClient_nok;",
        "    action = act_Rout_OK & RClient_state = st_RClient_nok : st_RClient_ok;",
        "    TRUE : RClient_state;",
        "  esac;",

        "  next(Sender_state) := case",
        "    action = act_Sender_internal & Sender_state = st_Sender_init : st_Sender_idle;",
        "    action = act_Sin & Sender_state = st_Sender_idle : st_Sender_next_frame;",
        "    action = act_F & Sender_state = st_Sender_next_frame & K_state = st_K_start : st_Sender_wait_ack;",
        f"   action = act_F & Sender_state = st_Sender_wait_ack & Sender_rc < {max_retrans} & clk_X = {t1} & K_state = st_K_start : st_Sender_wait_ack;",
        f"   action = act_B & Sender_state = st_Sender_wait_ack & clk_X < {t1} & L_state = st_L_in_transit & clk_V > 0 & clk_V <= {td} : st_Sender_success;",
        f"   action = act_Sout_DK & Sender_state = st_Sender_wait_ack & Sender_rc = {max_retrans} & Sender_i = {num_frames} & clk_X = {t1} : st_Sender_error;",
        f"   action = act_Sout_NOK & Sender_state = st_Sender_wait_ack & Sender_rc = {max_retrans} & Sender_i < {num_frames} & clk_X = {t1} : st_Sender_error;",
        f"   action = act_Sender_internal & Sender_state = st_Sender_success & Sender_i < {num_frames} : st_Sender_next_frame;",
        f"   action = act_Sout_OK & Sender_state = st_Sender_success & Sender_i = {num_frames} : st_Sender_idle;",
        f"   action = act_Sender_internal & Sender_state = st_Sender_error & clk_X = {tr} : st_Sender_idle;",
        "    TRUE : Sender_state;",
        "  esac;"
    ])

    # Flattened Variables (Removing nested cases to avoid panics in AST)
    lines.extend([
        "  next(Sender_ab) := case",
        "    action = act_Sender_internal & Sender_state = st_Sender_init : 0;",
        f"   action = act_Sender_internal & Sender_state = st_Sender_error & clk_X = {tr} : 0;",
        f"   action = act_B & Sender_state = st_Sender_wait_ack & clk_X < {t1} & L_state = st_L_in_transit & clk_V > 0 & clk_V <= {td} & Sender_ab = 0 : 1;",
        f"   action = act_B & Sender_state = st_Sender_wait_ack & clk_X < {t1} & L_state = st_L_in_transit & clk_V > 0 & clk_V <= {td} & Sender_ab = 1 : 0;",
        "    TRUE : Sender_ab;",
        "  esac;",

        "  next(Sender_i) := case",
        "    action = act_Sin & Sender_state = st_Sender_idle : 1;",
        f"   action = act_Sender_internal & Sender_state = st_Sender_success & Sender_i < {num_frames} : Sender_i + 1;",
        "    TRUE : Sender_i;",
        "  esac;",

        "  next(Sender_rc) := case",
        "    action = act_F & Sender_state = st_Sender_next_frame & K_state = st_K_start : 0;",
        f"   action = act_F & Sender_state = st_Sender_wait_ack & Sender_rc < {max_retrans} & clk_X = {t1} & K_state = st_K_start : Sender_rc + 1;",
        "    TRUE : Sender_rc;",
        "  esac;"
    ])

    # Receiver Guard Macros (flattened logic for clarity without defining macro vars)
    exp_ab_match = "(Receiver_triple = 1 | Receiver_triple = 3 | Receiver_triple = 5 | Receiver_triple = 7) & Receiver_exp_ab = 1 | (Receiver_triple = 0 | Receiver_triple = 2 | Receiver_triple = 4 | Receiver_triple = 6) & Receiver_exp_ab = 0"
    exp_ab_diff = "!((Receiver_triple = 1 | Receiver_triple = 3 | Receiver_triple = 5 | Receiver_triple = 7) & Receiver_exp_ab = 1 | (Receiver_triple = 0 | Receiver_triple = 2 | Receiver_triple = 4 | Receiver_triple = 6) & Receiver_exp_ab = 0)"

    lines.extend([
        "  next(Receiver_state) := case",
        f"   action = act_G & Receiver_state = st_Receiver_new_file & K_state = st_K_in_transit & clk_U > 0 & clk_U <= {td} : st_Receiver_first_safe_frame;",
        "    action = act_Receiver_internal & Receiver_state = st_Receiver_first_safe_frame : st_Receiver_frame_received;",
        f"   action = act_Rout_OK & Receiver_state = st_Receiver_frame_received & ({exp_ab_match}) & (Receiver_triple = 2 | Receiver_triple = 3 | Receiver_triple = 6 | Receiver_triple = 7) : st_Receiver_frame_reported;",
        f"   action = act_Rout_INC & Receiver_state = st_Receiver_frame_received & ({exp_ab_match}) & (Receiver_triple = 0 | Receiver_triple = 1) : st_Receiver_frame_reported;",
        f"   action = act_Rout_FST & Receiver_state = st_Receiver_frame_received & ({exp_ab_match}) & (Receiver_triple = 4 | Receiver_triple = 5 | Receiver_triple = 6 | Receiver_triple = 7) : st_Receiver_frame_reported;",
        f"   action = act_A & Receiver_state = st_Receiver_frame_received & {exp_ab_diff} & L_state = st_L_start : st_Receiver_idle;",
        "    action = act_A & Receiver_state = st_Receiver_frame_reported & L_state = st_L_start : st_Receiver_idle;",
        f"   action = act_Receiver_internal & Receiver_state = st_Receiver_idle & clk_Z = {tr} : st_Receiver_new_file;",
        f"   action = act_Rout_NOK & Receiver_state = st_Receiver_idle & clk_Z = {tr} : st_Receiver_new_file;",
        f"   action = act_G & Receiver_state = st_Receiver_idle & clk_Z < {tr} & K_state = st_K_in_transit & clk_U > 0 & clk_U <= {td} : st_Receiver_frame_received;",
        "    TRUE : Receiver_state;",
        "  esac;",

        "  next(Receiver_exp_ab) := case",
        "    action = act_Receiver_internal & Receiver_state = st_Receiver_first_safe_frame & (Receiver_triple = 1 | Receiver_triple = 3 | Receiver_triple = 5 | Receiver_triple = 7) : 1;",
        "    action = act_Receiver_internal & Receiver_state = st_Receiver_first_safe_frame & !(Receiver_triple = 1 | Receiver_triple = 3 | Receiver_triple = 5 | Receiver_triple = 7) : 0;",
        "    action = act_A & Receiver_state = st_Receiver_frame_reported & L_state = st_L_start & Receiver_exp_ab = 0 : 1;",
        "    action = act_A & Receiver_state = st_Receiver_frame_reported & L_state = st_L_start & Receiver_exp_ab = 1 : 0;",
        "    TRUE : Receiver_exp_ab;",
        "  esac;",

        "  next(Receiver_triple) := case",
        f"   action = act_G & K_state = st_K_in_transit & clk_U > 0 & clk_U <= {td} : K_triple;",
        "    TRUE : Receiver_triple;",
        "  esac;",

        "  next(K_state) := case",
        "    action = act_F & K_state = st_K_start : st_K_in_transit;",
        f"   action = act_K_loss & K_state = st_K_in_transit & clk_U > 0 & clk_U <= {td} : st_K_start;",
        f"   action = act_G & K_state = st_K_in_transit & clk_U > 0 & clk_U <= {td} : st_K_start;",
        "    TRUE : K_state;",
        "  esac;",

        "  next(K_triple) := case",
        "    action = act_F & K_state = st_K_start & Sender_i = 1 & Sender_ab = 0 : 4;",
        "    action = act_F & K_state = st_K_start & Sender_i = 1 & Sender_ab = 1 : 5;",
        f"   action = act_F & K_state = st_K_start & Sender_i = {num_frames} & Sender_ab = 0 : 2;",
        f"   action = act_F & K_state = st_K_start & Sender_i = {num_frames} & Sender_ab = 1 : 3;",
        f"   action = act_F & K_state = st_K_start & Sender_i != 1 & Sender_i != {num_frames} & Sender_ab = 0 : 0;",
        f"   action = act_F & K_state = st_K_start & Sender_i != 1 & Sender_i != {num_frames} & Sender_ab = 1 : 1;",
        "    TRUE : K_triple;",
        "  esac;",

        "  next(L_state) := case",
        "    action = act_A & L_state = st_L_start : st_L_in_transit;",
        f"   action = act_L_loss & L_state = st_L_in_transit & clk_V > 0 & clk_V <= {td} : st_L_start;",
        f"   action = act_B & L_state = st_L_in_transit & clk_V > 0 & clk_V <= {td} : st_L_start;",
        "    TRUE : L_state;",
        "  esac;",
        ""
    ])

    # Restoring the explicit CTL Specifications translated directly from the XML document
    lines.extend([
        "-- ── Property 1: Receiver's time limit is not sufficient ──",
        "CTLSPEC EF (Receiver_state = st_Receiver_first_safe_frame & !(Receiver_triple = 4 | Receiver_triple = 5));",
        "",
        "-- ── Property 2: Sender starts a new transmission before Receiver's reaction ──",
        f"CTLSPEC EF (Sender_state = st_Sender_error & clk_X = {tr} & !(Receiver_state = st_Receiver_new_file));",
        "",
        "-- ── Property 3: Discrepancy between status of Sender/Receiver clients ──",
        "CTLSPEC EF (File = st_File_SAME & ((SClient_state = st_SClient_ok & RClient_state = st_RClient_nok) | (RClient_state = st_RClient_ok & SClient_state = st_SClient_nok)));",
        ""
    ])

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(2))
