#!/usr/bin/env python3
"""Does the second election bar ("voters in groups backing you >= 60%") ever decide a win? (see decision 0004)

Plays every opening plan, and every mid-term change of course, through the engine's support model
(`support_model.py`, which reproduces the engine to the ppm) and asks one question: is there a play that clears the
55% overall bar but is stopped only by the 60% coalition bar? In today's content there is not one, so the second bar
changes no outcome.

What is and is not modelled, so the result is read for what it is:
  * Delivery is the best case (every minister at full effort) from the engine's own lags and caps (Staffing starts one
    quarter after approval, Repairs two, Access at once; 12, 10 and 4 units a quarter, 5 for Access from Q2 once 10
    credits are funded). Balanced reproduces the engine's pinned election night exactly (asserted below).
  * A change of course re-plans the undelivered work in quarter k: the old plan stops, the new one is funded with what is
    left and its delays count from k. This is an approximation of the engine's re-plan, not the engine.
  * One term only, plans in steps of 5 (opening) or 10 (change of course).

    python3 tools/support-model/coalition_sweep.py
"""
import sys, itertools, collections
import pathlib
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import support_model as sm
P = sm.CALIBRATIONS['v2']
CAP = {'s': 12, 'r': 10}
def deliver(plan, quarters=6):
    """Cumulative delivery by quarter for ONE plan approved in Q1, best case (full effort), as the preview computes it."""
    s, r, a = plan
    cums, cs, cr, ca = [], 0, 0, 0
    for q in range(1, quarters + 1):
        cs += min(s - cs, CAP['s']) if q >= 2 else 0                   # staffing starts one quarter after approval
        cr += min(r - cr, CAP['r']) if q >= 3 else 0                   # repairs two quarters after
        cap_a = 4 if q == 1 else (5 if a >= 10 else 4)                 # access bonus from Q2 when >= 10 funded
        ca += min(a - ca, cap_a)
        cums.append([cs, cr, ca])
    return cums
def result(plan):
    sup, w, _ = sm.play(deliver(plan), settle=True, **P)
    held = sum(sm.W[i] for i in range(6) if sup[i] > 500000)
    return sup, w, held
# 1. sanity: Balanced must reproduce the engine's pinned numbers
sup, w, held = result((50, 40, 10))
print('Balanced', sup, w, held, '(engine: [571800, 602000, 472200, 579240, 615600, 491960] 559584 730000)')
assert sup == [571800, 602000, 472200, 579240, 615600, 491960] and w == 559584 and held == 730000
# 2. sweep every plan in steps of 5 that fits the opening 100 credits
rows = []
for s in range(0, 101, 5):
    for r in range(0, 101 - s, 5):
        for a in range(0, 101 - s - r, 5):
            sup, w, held = result((s, r, a)); rows.append(((s, r, a), sup, w, held))
n = len(rows)
over = [x for x in rows if x[2] >= 550000]
blocs = [x for x in rows if x[3] >= 600000]
both = [x for x in rows if x[2] >= 550000 and x[3] >= 600000]
print(f'plans tried {n}; overall>=55%: {len(over)}; blocs>=60%: {len(blocs)}; both: {len(both)}')
print('overall >=55% but blocs <60% (the second bar is the only thing stopping a win):', len([x for x in over if x[3] < 600000]))
print('blocs >=60% but overall <55% (the first bar alone stops it):', len([x for x in blocs if x[2] < 550000]))
for plan, sup, w, held in sorted([x for x in over if x[3] < 600000], key=lambda x: -x[2])[:8]:
    print('  blocked only by blocs:', plan, f'overall {w/1e4:.2f}% held {held/1e4:.0f}%', [round(v/1e4, 1) for v in sup])
print('highest overall plans:')
for plan, sup, w, held in sorted(rows, key=lambda x: -x[2])[:6]:
    print('  ', plan, f'overall {w/1e4:.2f}% held {held/1e4:.0f}%', [round(v/1e4, 1) for v in sup])
# 3. which sets of groups end up above 50% among the winning plans, and among all plans
sets = collections.Counter(tuple(sm.LABEL[i] for i in range(6) if x[1][i] > 500000) for x in rows)
print('group sets above 50% across all plans (top):')
for k, v in sets.most_common(8): print('  ', v, k)

# ---- 4. Changing course: plan A from Q1, then at quarter k the player re-plans (undelivered work is replaced,
# delivery delays restart from k). Best-case delivery, approximated from the engine's lags and caps.
def deliver2(A, k, B, quarters=6):
    cs = cr = ca = 0; cums = []
    sA, rA, aA = A; sB, rB, aB = B
    done = [0, 0, 0]; remB = [sB, rB, aB]
    for q in range(1, quarters + 1):
        if q < k:        # still on plan A
            ds = min(sA - done[0], CAP['s']) if q >= 2 else 0
            dr = min(rA - done[1], CAP['r']) if q >= 3 else 0
            da = min(aA - done[2], 4 if q == 1 else (5 if aA >= 10 else 4))
        else:            # plan B, its delays counted from k
            rel = q - k + 1
            ds = min(remB[0], CAP['s']) if rel >= 2 else 0
            dr = min(remB[1], CAP['r']) if rel >= 3 else 0
            da = min(remB[2], 4 if rel == 1 else (5 if aB >= 10 else 4))
            remB = [remB[0] - ds, remB[1] - dr, remB[2] - da]
        done = [done[0] + ds, done[1] + dr, done[2] + da] if q < k else done
        cs += ds; cr += dr; ca += da
        cums.append([cs, cr, ca])
    return cums
def delivered_before(A, k):
    c = deliver(A, k - 1)
    return list(c[-1]) if c else [0, 0, 0]
rows2 = []
for s in range(0, 101, 10):
    for r in range(0, 101 - s, 10):
        for a in range(0, 101 - s - r, 10):
            A = (s, r, a)
            for k in (2, 3, 4, 5):
                got = delivered_before(A, k); fundable = 100 - sum(got)
                for s2 in range(0, fundable + 1, 10):
                    for r2 in range(0, fundable - s2 + 1, 10):
                        for a2 in range(0, fundable - s2 - r2 + 1, 10):
                            B = (s2, r2, a2)
                            sup, w, _ = sm.play(deliver2(A, k, B), settle=True, **P)
                            held = sum(sm.W[i] for i in range(6) if sup[i] > 500000)
                            rows2.append((A, k, B, sup, w, held))
over2 = [x for x in rows2 if x[4] >= 550000]
print(f'\nCOURSE CHANGES: plays tried {len(rows2)}; overall>=55%: {len(over2)}; of those blocked only by the coalition bar: {len([x for x in over2 if x[5] < 600000])}')
print('  coalition (groups above 50%) across course-change plays that win overall:',
      collections.Counter(tuple(sm.LABEL[i] for i in range(6) if x[3][i] > 500000) for x in over2).most_common(4))
