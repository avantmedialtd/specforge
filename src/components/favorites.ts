/// Favorite-change helpers shared by the workspace tree — kept out of
/// WorkspaceTree.tsx so the pure ordering logic is unit-testable without
/// pulling in the React component (see favorites.test.ts).

/// Favorite-toggle wiring a favoritable row passes down to `Row` — the
/// current state plus the toggle callback, already bound to the change's
/// position-independent favorite key.
export interface RowFavorite {
    active: boolean
    onToggle: () => void
}

/// Stable partition for a group's change list: favorited items first, the
/// backend's name order preserved within each half (*Favorite-First Change
/// Ordering*). Returns the input array itself when the partition would be a
/// no-op, so memoized subtrees keep their identity.
export function partitionFavorites<T>(
    items: T[],
    favorites: Set<string>,
    keyOf: (item: T) => string,
): T[] {
    if (favorites.size === 0) return items
    const starred: T[] = []
    const rest: T[] = []
    for (const item of items) {
        if (favorites.has(keyOf(item))) starred.push(item)
        else rest.push(item)
    }
    return starred.length === 0 ? items : starred.concat(rest)
}
