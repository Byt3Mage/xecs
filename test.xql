SELECT Power AS enemy FILTER Enemy, <MinionOf(*)
FOLLOW Wields(#006) [ SELECT Weapon AS weapon FILTER PoweredUp ]
FOLLOW Targets [
    SELECT mut Health, LocalTransform AS player
    FILTER Player, LooksAt(enemy), !Resists(weapon)
    FOLLOW IsAttacking(enemy) [ WITH Shield ]
]
