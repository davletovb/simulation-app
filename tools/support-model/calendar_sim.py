#!/usr/bin/env python3
"""Draft news calendar against the support model: what would it do to a player who stops playing, and to one who keeps going?

This is a DESIGN SKETCH on the replica (`support_model.py`, which matches the engine to the ppm), not the engine: nothing
here is built yet (see docs/news-calendar.md). It exists so the proposal's numbers can be re-run and the calendar tuned before
any Rust is written.

Model of one game, term after term (six quarters each; the first election needs 55% overall support):
  * Players: `idle` plays Balanced in quarter 1 and then does nothing; `repeat` plays Balanced at the start of every term
    (the fresh 100 credits, all delivered, as the engine's scripted run does); `smart` is `repeat` plus the proposed paid
    response (below).
  * Events: per term, `per_term` headlines land in distinct quarters 2..6. 70% are bad news, 30% good. Each draws a group
    pattern from a small deck. Bad news can be (a) FLAT: a fixed number of points, or (b) CUSHION-SCALED: a small base plus a
    share of how far above 50% each group currently stands, so a big lead is not a safe lead.
  * Response (`smart`): on a bad headline costing at least 1 point of overall support, while overall support is under 64%
    and at most twice a term, spend 10% of that term's Staffing and Repairs delivery to cut the hit by 60%.
  * Shocks land before queued responses, as in the engine (`Campaign::advance`); election night also counts the queued
    last-quarter response, as the engine does.

    python3 tools/support-model/calendar_sim.py
"""
import pathlib, random, statistics, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import support_model as sm

P, W, BAR = sm.CALIBRATIONS['v2'], sm.W, 550_000
# Shock in POINTS per group (urban low, middle, high, rural low, middle, high). 1 point = 10,000 ppm; the engine caps one
# shock at 4 points (SHOCK_CAP_PPM).
DECK = {
    'Teacher shortage headlines': [-2.0, -2.0, -2.0, -1.0, -1.0, -1.0],
    'School roof scare':          [-1.0, -1.0, -1.0, -3.0, -3.0, -3.0],
    'Funding row in Parliament':  [-2.0, -2.0, -2.0, -2.0, -2.0, -2.0],
    'Rural bus cuts':             [0.0, 0.0, 0.0, -4.0, -4.0, -4.0],
    'Cost-of-living squeeze':     [-3.0, -1.5, 0.0, -3.0, -1.5, 0.0],
    'Exam results row':           [0.0, -2.5, -3.0, 0.0, -1.5, -2.0],
    'Inspectorate praises you':   [1.5, 1.5, 1.5, 1.5, 1.5, 1.5],
    'Volunteer surge':            [0.0, 0.0, 0.0, 2.5, 2.5, 2.5],
    'Attendance rises':           [2.0, 1.0, 0.0, 2.0, 1.0, 0.0],
}
GOOD = {'Inspectorate praises you', 'Volunteer surge', 'Attendance rises'}
BAL = {'s': [0, 12, 12, 12, 12, 2], 'r': [0, 0, 10, 10, 10, 10], 'a': [4, 5, 1, 0, 0, 0]}   # Balanced, best case, per term quarter


def response(ix, d):
    return sm.response(ix, d, base=P['base'], mag=P['mag'], clamp=P['clamp'])


def overall(sup):
    return sum(w * s for w, s in zip(W, sup)) // 10**6


def clamp(v):
    return max(0, min(10**6, v))


def game(rng, strategy, terms, per_term, mode, k_pct=35, base_pts=0.5, first_term_scale=0.5, escalation=0.35, flat_scale=1.0,
         quiet_first_term=False):
    sup, queue, results = [500_000] * 6, {}, []
    for term in range(1, terms + 1):
        headlines = {}
        if per_term:
            for q in rng.sample([2, 3, 4, 5, 6], per_term):
                good = rng.random() < 0.3
                name = rng.choice([n for n in DECK if (n in GOOD) == good])
                headlines[q] = DECK[name]
        plays = strategy != 'idle' or term == 1
        relief = 0
        for qt in range(1, 7):
            q = (term - 1) * 6 + qt
            d = [BAL['s'][qt - 1], BAL['r'][qt - 1], BAL['a'][qt - 1]] if plays else [0, 0, 0]
            if qt in headlines:
                scale = first_term_scale if term == 1 else min(2.5, 1 + escalation * (term - 2))
                shock = []
                for g, v in enumerate(headlines[qt]):
                    if v >= 0 or (quiet_first_term and term == 1):
                        shock.append(int(max(v, 0) * 10_000))
                    elif mode == 'flat':
                        shock.append(-min(40_000, int(-v * 10_000 * flat_scale)))
                    else:   # cushion-scaled: exposure is |v|/4 of the worst case
                        gap = max(0, sup[g] - 500_000)
                        shock.append(-min(40_000, int((-v / 4.0) * (base_pts * 10_000 * scale + gap * k_pct / 100))))
                lost = -sum(w * x for w, x in zip(W, shock) if x < 0) / 1e10
                if strategy == 'smart' and lost >= 1.0 and overall(sup) < 640_000 and relief < 2:
                    shock = [int(x * 40 / 100) if x < 0 else x for x in shock]          # cut the hit by 60%
                    d = [d[0] * 90 // 100, d[1] * 90 // 100, d[2]]                      # 10 credits did not become delivered work
                    relief += 1
                sup = [clamp(s + v) for s, v in zip(sup, shock)]
            for due in queue.pop(q, []):
                sup = [clamp(s + response(ix, due)) for ix, s in enumerate(sup)]
            if sum(d) > 0:
                queue.setdefault(q + 1, []).append(d)
        settled = list(sup)
        for due in queue.get((term - 1) * 6 + 7, []):
            settled = [clamp(s + response(ix, due)) for ix, s in enumerate(settled)]
        results.append(overall(settled))
        if results[-1] < BAR:
            break
    return results


def table(label, strategies, n=2000, terms=8, **kw):
    print(label)
    for strategy in strategies:
        reach, margins, last = [0] * (terms + 1), [], []
        for seed in range(n):
            r = game(random.Random(seed), strategy, terms, **kw)
            for t in range(1, sum(1 for x in r if x >= BAR) + 1):
                reach[t] += 1
            margins += [(x - BAR) / 1e4 for x in r if x >= BAR]
            last.append(r)
        median = f'{statistics.median(margins):.1f}' if margins else '-'
        print(f'   {strategy:7}', ' '.join(f'{100 * reach[t] / n:4.0f}%' for t in range(1, terms + 1)), f'| median margin over the bar when won: {median} pts')


if __name__ == '__main__':
    print('Share of games still going after each election (term 1 ... 8). A game ends at the first lost election.\n')
    table('TODAY: no calendar', ['idle', 'repeat'], per_term=0, mode='flat')
    table('FLAT shocks, 3 headlines a term, full strength from term 1', ['idle', 'repeat'], per_term=3, mode='flat', n=3000)
    table('CUSHION-SCALED, 4 headlines a term, half strength in term 1, +35% a term after, bad news takes 35% of the cushion',
          ['idle', 'repeat', 'smart'], per_term=4, mode='cushion', k_pct=35)
    table('CUSHION-SCALED, same but bad news takes 50% of the cushion', ['idle', 'repeat', 'smart'], per_term=4, mode='cushion', k_pct=50)
    table('CUSHION-SCALED 35%, and term 1 has no bad news at all (the first election is a fair test of the opening plan)',
          ['idle', 'repeat', 'smart'], per_term=4, mode='cushion', k_pct=35, quiet_first_term=True)
