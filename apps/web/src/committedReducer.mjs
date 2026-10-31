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
  const previous = old.operation
  const incoming = next.operation
  let operation = incoming ?? previous
  if (previous && incoming) {
    if (previous.command_id === incoming.command_id &&
        previous.status !== 'accepted' && incoming.status === 'accepted') {
      operation = previous
    } else if (previous.command_id !== incoming.command_id) {
      // An older completed record from a fallback must not erase an
      // outstanding accepted command, nor replace a newer completion.
      if (previous.status === 'accepted' && incoming.status !== 'accepted') {
        operation = previous
      } else if (previous.committed_cursor != null &&
                 incoming.committed_cursor != null &&
                 BigInt(incoming.committed_cursor) < BigInt(previous.committed_cursor)) {
        operation = previous
      }
    }
  }
  // Preserve the incoming object's identity when no operation merge was
  // necessary: callers can distinguish a genuinely new committed boundary.
  if (b > a && operation === next.operation) return next
  return {snapshot:b === a ? old.snapshot : next.snapshot,operation}
}
