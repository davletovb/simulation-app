/**
 * Pure committed-boundary reducer. Cursors are unsigned decimal strings,
 * not JavaScript Numbers; an older or duplicated SSE frame cannot rewind the
 * world or apply a command twice. A conflicting same-cursor state is fatal.
 */
export function newer(old, next) {
  if (!old) return next
  const a = BigInt(old.snapshot.cursor)
  const b = BigInt(next.snapshot.cursor)
  if (b < a) return old
  if (b === a && old.snapshot.view_version !== next.snapshot.view_version) {
    throw new Error('Conflicting view version at one committed cursor')
  }
  if (b === a) {
    const previous = old.operation
    const incoming = next.operation
    // A duplicated or out-of-order SSE frame must not regress a durable
    // operation from completed/failed back to accepted at one boundary.
    const operation = previous && incoming &&
      previous.command_id === incoming.command_id &&
      previous.status !== 'accepted' && incoming.status === 'accepted'
      ? previous : incoming ?? previous
    return {snapshot:old.snapshot,operation}
  }
  return next
}
