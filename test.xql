SELECT Power FILTER Enemy, <MinionOf(*) AS enemy
FOLLOW Wields(#006) [ SELECT Weapon FILTER PoweredUp AS weapon ]
FOLLOW Targets [
    SELECT mut Health, LocalTransform
    FILTER Player, LooksAt(enemy), !Resists(weapon)
    AS player
    FOLLOW IsAttacking(enemy) [ WITH Shield ]
]
