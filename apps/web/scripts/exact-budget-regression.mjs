import test from 'node:test'
import assert from 'node:assert/strict'
import { parseExactBudget } from '../src/exactBudget.mjs'
test('exact integer zero-to-budget values round-trip without rounding',()=>{
  for (let i=0;i<=100;i++) assert.equal(parseExactBudget(String(i),100),i)
})
test('disallows negative, decimal, exponent, grouped, unicode or overflow inputs',()=>{
  for (const s of ['',' ','01','-1','+2','1.0','1e2','１','١','10,0','1000',
    '9007199254740993','101','NaN','Infinity','2 ','0x10','0.1']) {
    assert.equal(parseExactBudget(s,100),null,s)
  }
})
test('bound respects remaining treasury, not initial allocation',()=>{
  assert.equal(parseExactBudget('40',39),null)
  assert.equal(parseExactBudget('39',39),39)
  assert.equal(parseExactBudget('0',0),0)
  assert.equal(parseExactBudget('1',0),null)
})
