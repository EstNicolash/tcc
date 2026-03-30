# 1) If the player opens the chest while trapped, they must die in the next step.
AG(is_alive & !is_locked & is_trapped -> AX !is_alive)

# 2) If the player dies, the chest is locked again for the next attempt.
AG(!is_alive -> AX is_locked)

# 3) Impossible to have the treasure if the chest is still locked.
!EF(has_treasure & is_locked)

# 4) HP never increases (monotonicity).
AG(hp3 -> !EF(!hp3 & !hp2 & !hp1 & !hp0))
AG(hp2 -> !EF hp3)
AG(hp1 -> !EF(hp3 | hp2))

# 5) Game Over is definitive.
AG(is_lost -> AG is_lost)

# 6) The game is not impossible.
EF is_solved

# 7) Possible to win even with 1 HP.
EF(hp1 & EF is_solved)

# 8) Respawn logic: If dead and has HP, will eventually be alive.
AG(!is_alive & !hp0 -> AX is_alive)

# 9) Cannot carry items after dying.
AG(!is_alive -> AX has_no_items)

# 10) Trap stays disarmed forever once handled.
AG(!is_trapped -> AG !is_trapped)

# 11) The player lives until they open a trapped chest (or forever).
E[is_alive U (!is_locked & is_trapped)] | AG is_alive

# 12) Can you win with 0 HP? (Should be false based on logic)
EF(is_solved & hp0)
