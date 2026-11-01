#!/usr/bin/env python3
"""How much does a one- or two-quarter capacity cut move an election? (decision 0011)

The Balanced plan, approved in quarter 1 of a term and delivered at full effort under the engine's lags and capacities
(Staffing from the second quarter, 12 a quarter; Repairs from the third, 10; Access at once, 4 and 5 in the second quarter), with
one kind of work cut to a share of its capacity for one or two quarters starting in quarter k. The table is the change in
overall support at that term's election, in points, against the same plan with no cut. No headlines, no mood: the cut alone.

It is why a disruption is a hybrid (a story that shakes opinion and also cuts delivery): on its own it is a weak challenge.

    python3 tools/support-model/disruption_sim.py
"""
import pathlib, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import support_model as sm
P = sm.CALIBRATIONS['v2']
CAP = {'s': 12, 'r': 10, 'a': 4}
LAG = {'s': 1, 'r': 2, 'a': 0}
PLAN = {'s': 50, 'r': 40, 'a': 10}
def deliver(cuts):
    """cuts: {(family, quarter): fraction kept (0..1)} -> cumulative [s, r, a] for 6 quarters."""
    left = dict(PLAN); cum = {'s': 0, 'r': 0, 'a': 0}; out = []
    for q in range(1, 7):
        for f in 'sra':
            if q <= LAG[f]: continue
            cap = CAP[f] + (1 if f == 'a' and PLAN['a'] >= 10 and q >= 2 else 0)
            cap = int(cap * cuts.get((f, q), 1.0))
            d = min(left[f], cap); left[f] -= d; cum[f] += d
        out.append([cum['s'], cum['r'], cum['a']])
    return out
def final(cuts):
    sup, w, _ = sm.play(deliver(cuts), settle=True, **P)
    return w
base = final({})
print(f'baseline settled overall: {base/1e4:.2f}%   (units delivered by Q6: {deliver({})[-1]})')
print('change in points of overall support at the election, for a cut starting in quarter k (rows) to family f:')
for label, keep, dur in [('halved, 1 quarter', 0.5, 1), ('stopped, 1 quarter', 0.0, 1), ('halved, 2 quarters', 0.5, 2), ('stopped, 2 quarters', 0.0, 2)]:
    print(f'  {label:20}', end='')
    for f in 'sra':
        row = []
        for k in range(2, 7):
            cuts = {(f, k + j): keep for j in range(dur) if k + j <= 6}
            row.append(f'{(final(cuts) - base) / 1e4:+.2f}')
        print(f'  {f}: ' + ' '.join(row), end='')
    print()
print('(columns are start quarter 2..6)')
