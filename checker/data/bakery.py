def generate_instance(N: int) -> str:
    assert N >= 2, "Bakery algorithm requires at least 2 processes."
    MAX = 10
    lines = [f"-- bakery_{N}.ssmv", "MODULE main", "VAR"]
    for i in range(N):
        p_states = ", ".join([f"st_p{i}_{s}" for s in ["NCS", "choose", "for_loop", "wait", "CS"]])
        lines.append(f"  p{i} : {{{p_states}}}; choosing{i} : 0..1; number{i} : 0..{MAX}; j{i} : 0..{N}; max{i} : 0..{MAX};")
    action_vals = ", ".join([f"act_p{i}" for i in range(N)])
    lines.append(f"  action : {{{action_vals}}};")
    lines.append("ASSIGN")
    for i in range(N):
        lines.append(f"  init(p{i}) := st_p{i}_NCS; init(choosing{i}) := 0; init(number{i}) := 0; init(j{i}) := 0; init(max{i}) := 0;")
    for i in range(N):
        lines.append(f"  next(p{i}) := case action != act_p{i} : p{i}; p{i} = st_p{i}_NCS : st_p{i}_choose; p{i} = st_p{i}_choose & j{i} < {N} : st_p{i}_choose; p{i} = st_p{i}_choose & j{i} = {N} & max{i} < {MAX} : st_p{i}_for_loop;")
        wait_cond = " | ".join([f"(j{i} = {k} & choosing{k} = 0)" for k in range(N)])
        lines.append(f"    p{i} = st_p{i}_for_loop & j{i} < {N} & ({wait_cond}) : st_p{i}_wait;")
        lines.append(f"    p{i} = st_p{i}_for_loop & j{i} = {N} : st_p{i}_CS;")
        ok_cond = " | ".join([f"(j{i} = {k} & (number{k} = 0 | number{k} > number{i} | (number{k} = number{i} & {i} <= {k})))" for k in range(N)])
        lines.append(f"    p{i} = st_p{i}_wait & ({ok_cond}) : st_p{i}_for_loop;")
        lines.append(f"    p{i} = st_p{i}_CS : st_p{i}_NCS; TRUE : p{i}; esac;")
        lines.append(f"  next(choosing{i}) := case action = act_p{i} & p{i} = st_p{i}_NCS : 1; action = act_p{i} & p{i} = st_p{i}_choose & j{i} = {N} & max{i} < {MAX} : 0; TRUE : choosing{i}; esac;")
        lines.append(f"  next(number{i}) := case action = act_p{i} & p{i} = st_p{i}_choose & j{i} = {N} & max{i} < {MAX} : case max{i} < {MAX} : max{i} + 1; TRUE : max{i}; esac; action = act_p{i} & p{i} = st_p{i}_CS : 0; TRUE : number{i}; esac;")
        lines.append(f"  next(j{i}) := case action != act_p{i} : j{i}; p{i} = st_p{i}_NCS : 0; p{i} = st_p{i}_choose & j{i} < {N} : case j{i} < {N} : j{i} + 1; TRUE : j{i}; esac; p{i} = st_p{i}_choose & j{i} = {N} & max{i} < {MAX} : 0; p{i} = st_p{i}_wait & ({ok_cond}) : case j{i} < {N} : j{i} + 1; TRUE : j{i}; esac; TRUE : j{i}; esac;")
        lines.append(f"  next(max{i}) := case action != act_p{i} : max{i}; p{i} = st_p{i}_NCS : 0;")
        for k in range(N): lines.append(f"    p{i} = st_p{i}_choose & j{i} = {k} & number{k} > max{i} : number{k};")
        lines.append(f"    TRUE : max{i}; esac;")
    safety = " & ".join([f"!(p{i} = st_p{i}_CS & p{k} = st_p{k}_CS)" for i in range(N) for k in range(i+1, N)])
    lines.append(f"CTLSPEC AG ({safety if safety else 'TRUE'});")
    return "\n".join(lines)
