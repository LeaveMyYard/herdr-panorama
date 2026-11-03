import test from 'node:test';
import assert from 'node:assert/strict';
import { logical, clampOffset, visibleRows } from '../mods/panorama/viewport-math.js';

test('owned viewport offset clamps after resize/fold and never guesses native scroll state', () => {
  assert.equal(clampOffset(300,1000,500),300);
  assert.equal(clampOffset(900,1000,500),500);
  assert.equal(clampOffset(300,200,500),0);
  assert.equal(clampOffset(-1,1000,500),0);
  assert.equal(clampOffset(NaN,1000,500),0);
});
test('exact visible intersections include partial rows and exclude boundary-adjacent animation', () => {
  const rows=[{height:30,animated:true},{height:26},{height:26,animated:true},{height:30}];
  assert.deepEqual(visibleRows(rows,30,26).map(row=>row.index),[1]);
  assert.deepEqual(visibleRows(rows,29,28).map(row=>row.index),[0,1,2]);
  assert.deepEqual(visibleRows(rows,82,30).map(row=>row.index),[3]);
  assert.deepEqual(visibleRows(rows,0,0),[]);
});
test('native measured coordinate conversion follows current zoom and supports remeasurement', () => {
  assert.equal(logical(780,1),780);assert.equal(logical(936,1.2),780);
  assert.equal(logical(780,1.2),650);assert.equal(logical(0,1.2),0);
});
