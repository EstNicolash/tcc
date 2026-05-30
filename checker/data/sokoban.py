import sys

def generate_instance(N: int) -> str:
    """
    Generates a parameterized instance of the Sokoban sliding block puzzle
    for N boxes in strict .ssmv format.

    Problem Description:
      Sokoban is a transport puzzle where the player (the warehouse worker) pushes
      boxes around a maze grid. The worker can only push boxes forward into empty
      spaces—boxes cannot be pulled, and more than one box cannot be pushed at a time.
      The puzzle is solved when all boxes reside on designated goal squares.

    Grid Parameterization:
      - N: Number of boxes/stones to be pushed.
      - Width (W): 2 * N + 3 (Provides moving room and dedicated goal slots).
      - Height (H): 5 (Fixed floor layouts with central track rows).
      - Cells follow an explicit 'cell_{y}_{x}' token schema to keep the ssmv parser clean.
      - Cell Mapping: 0 = Free/Empty Space, 1 = Static Wall, 2 = Box/Stone.

    Implementation Constraints Respected:
      - Replaced ambiguous grid tokens with the explicit schema 'cell_{y}_{x}'.
      - Omitted 'next()' relations for static boundary walls to avoid BDD bloat.
      - Flattened conditional statements to comply with standard case rules.
      - Separated all INIT and NEXT assignments onto clean, separate lines.
      - Written entirely with English text documentation and property comments.
    """
    if N < 1:
        N = 1

    W = 2 * N + 3
    H = 5

    lines = [
        f"-- sokoban_{N}.ssmv",
        f"-- Parameterized Sokoban sliding block puzzle with N={N} boxes",
        "MODULE main",
        "VAR",
        f"  px : 0..{W-1};",
        f"  py : 0..{H-1};",
        "  done : 0..1;"
    ]

    # 1. Variable Declarations
    for y in range(H):
        for x in range(W):
            lines.append(f"  cell_{y}_{x} : 0..2;")

    lines.extend([
        "  action : {act_up, act_down, act_left, act_right, act_check};",
        "",
        "ASSIGN"
    ])

    # 2. Initialization Block (Cleanly split line-by-line)
    lines.append("  init(px) := 1;")
    lines.append("  init(py) := 2;")
    lines.append("  init(done) := 0;")

    for y in range(H):
        for x in range(W):
            if (y == 0 or y == H - 1 or x == 0 or x == W - 1):
                val = 1  # Boundary Wall
            elif (y == 2 and 2 <= x <= N + 1):
                val = 2  # Initial Box Placement
            else:
                val = 0  # Empty Floor Space
            lines.append(f"  init(cell_{y}_{x}) := {val};")
    lines.append("")

    # 3. Next State Transitions (Flattened and split safely)

    # Player Coordinate X transitions
    lines.append("  next(px) := case")
    lines.append("    done = 1 : px;")
    for act, dx in [("act_left", -1), ("act_right", 1)]:
        for y in range(1, H - 1):
            for x in range(1, W - 1):
                tx = x + dx
                bx = x + 2 * dx
                if 0 < tx < W - 1:
                    cond = f"action = {act} & px = {x} & py = {y} & (cell_{y}_{tx} = 0"
                    if 0 < bx < W - 1:
                        cond += f" | (cell_{y}_{tx} = 2 & cell_{y}_{bx} = 0)"
                    lines.append(f"    {cond}) : {tx};")
    lines.extend([
        "    TRUE : px;",
        "  esac;",
        ""
    ])

    # Player Coordinate Y transitions
    lines.append("  next(py) := case")
    lines.append("    done = 1 : py;")
    for act, dy in [("act_up", -1), ("act_down", 1)]:
        for y in range(1, H - 1):
            for x in range(1, W - 1):
                ty = y + dy
                by = y + 2 * dy
                if 0 < ty < H - 1:
                    cond = f"action = {act} & px = {x} & py = {y} & (cell_{ty}_{x} = 0"
                    if 0 < by < H - 1:
                        cond += f" | (cell_{ty}_{x} = 2 & cell_{by}_{x} = 0)"
                    lines.append(f"    {cond}) : {ty};")
    lines.extend([
        "    TRUE : py;",
        "  esac;",
        ""
    ])

    # Board Grid Cell Contents transitions
    for y in range(H):
        for x in range(W):
            # Omit 'next' statements for border walls to optimize BDD transition compilation
            if (y == 0 or y == H - 1 or x == 0 or x == W - 1):
                continue
            else:
                lines.append(f"  next(cell_{y}_{x}) := case")
                lines.append(f"    done = 1 : cell_{y}_{x};")

                # Dynamic transitions for box moves into empty cells
                if x >= 2:
                    lines.append(f"    action = act_right & px = {x-2} & py = {y} & cell_{y}_{x-1} = 2 & cell_{y}_{x} = 0 : 2;")
                if x <= W - 3:
                    lines.append(f"    action = act_left  & px = {x+2} & py = {y} & cell_{y}_{x+1} = 2 & cell_{y}_{x} = 0 : 2;")
                if y >= 2:
                    lines.append(f"    action = act_down  & px = {x} & py = {y-2} & cell_{y-1}_{x} = 2 & cell_{y}_{x} = 0 : 2;")
                if y <= H - 3:
                    lines.append(f"    action = act_up    & px = {x} & py = {y+2} & cell_{y+1}_{x} = 2 & cell_{y}_{x} = 0 : 2;")

                # Dynamic transitions for box moves out of occupied cells
                if x <= W - 3:
                    lines.append(f"    action = act_right & px = {x-1} & py = {y} & cell_{y}_{x} = 2 & cell_{y}_{x+1} = 0 : 0;")
                if x >= 2:
                    lines.append(f"    action = act_left  & px = {x+1} & py = {y} & cell_{y}_{x} = 2 & cell_{y}_{x-1} = 0 : 0;")
                if y <= H - 3:
                    lines.append(f"    action = act_down  & px = {x} & py = {y-1} & cell_{y}_{x} = 2 & cell_{y+1}_{x} = 0 : 0;")
                if y >= 2:
                    lines.append(f"    action = act_up    & px = {x} & py = {y+1} & cell_{y}_{x} = 2 & cell_{y-1}_{x} = 0 : 0;")

                lines.append(f"    TRUE : cell_{y}_{x};")
                lines.append("  esac;")
                lines.append("")

    # Global Puzzle Accomplishment / Termination Flag
    goal_cond = " & ".join(f"cell_2_{x} = 2" for x in range(N + 2, 2 * N + 2))
    lines.extend([
        "  next(done) := case",
        "    done = 1 : 1;",
        f"    action = act_check & {goal_cond} : 1;",
        "    TRUE : done;",
        "  esac;",
        ""
    ])

    # 4. Formal Verification Specifications (Mapped from schemas in sokoban.xml)
    lines.append("-- ── Reachability Specification: The sliding puzzle can be solved ──")
    lines.append("CTLSPEC EF (done = 1);")
    lines.append("")

    return "\n".join(lines)

if __name__ == "__main__":
    if len(sys.argv) > 1:
        print(generate_instance(int(sys.argv[1])))
    else:
        print(generate_instance(3))
