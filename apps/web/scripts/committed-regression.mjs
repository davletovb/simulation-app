import test from 'node:test'
import assert from 'node:assert/strict'
import {newer} from '../src/committedReducer.mjs'

const view = (cursor, version, operation=null) => ({
  snapshot:{cursor,view_version:version},operation,
})

test('SSE duplicates and older frames cannot reapply committed state',()=>{
  const first=view('9007199254740993','41')
  const newerEvent=view('9007199254740994','42')
  const accepted=newer(first,newerEvent)
  assert.equal(accepted,newerEvent)
  assert.equal(newer(accepted,first),accepted)
  assert.equal(newer(accepted,newerEvent).snapshot,newerEvent.snapshot)
})
test('Same-cursor conflicting view fails closed',()=>{
  assert.throws(()=>newer(view('10','40'),view('10','41')),/Conflicting/)
})
test('Same boundary may refresh operation, never swap snapshot',()=>{
  const before=view('7','9',{status:'accepted'})
  const after=view('7','9',{status:'completed'})
  const accepted=newer(before,after)
  assert.equal(accepted.snapshot,before.snapshot)
  assert.equal(accepted.operation.status,'completed')
})
test('SSE gap after disconnect uses verified fresh snapshot without rounding',()=>{
  const current=view('18446744073709551613','81')
  const fallback=view('18446744073709551615','83')
  assert.equal(newer(current,fallback),fallback)
  assert.equal(newer(fallback,view('18446744073709551614','82')),fallback)
})
