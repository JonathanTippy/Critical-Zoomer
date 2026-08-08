Order of preference of tiles when deciding which to prune:

r[depends cz.system.tile-manager-protect-current-lookahead+1]

1. current stencil members (any part is a member? -> member)
2. lookahead tiles (deeper? -> less preferred)
3. hoarded tiles (containing stencil (closser to mouse? more preferred))
4. unrelated hoarded tiles

r[cz.system.max-homotheties+1]

Tile manager also enforces the 8-homothety limit.

r[cz.int.memory-bump+1]

memory policy: evict hoarded work to stay within memory limit. If the screen itself and its lookaheds take up more than the limit, bump the limit. One channel from tile manager to headgroup to achieve this on the workgroup, and not necessary on the headgroup. The code which does this must be part of the tile collection manager, shared code between the headgroup and workgoup which ensures the collection of tiles considered in play is the same between the groups. There is no size difference as both groups store answers.

Even though its not true, assume the workgroup collection is cpu memory only.
The headgroup collections is gpu memory only.

The tile manager in the workgroup is residen to the publisher and the publisher owns the channel to the headgroup for memory bumps.
