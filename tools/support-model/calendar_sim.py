#!/usr/bin/env python3
"""The news calendar against the support model: how hard should it hit, and how does that grow with each term?

A sketch on the replica (`support_model.py`, which matches the engine to the ppm), used to choose the numbers that are now
built into the engine (decision 0010, `crates/domain/src/news.rs`). It is not the engine: delivery is best-case, and the
players are simple on purpose.

Rules sketched (the owner's choices of 2026-11-01):
  * Term 1 has no bad news: two good headlines. Every later term has four: 30% good, 70% bad.
  * A third of headlines are surprises (nobody is told in advance and nothing can be done); the rest are announced a
    quarter ahead and can be answered with a paid response that cuts the hit by 60% and costs credits in proportion to the
    damage it was expected to do (`RELIEF_CREDITS_PER_POINT` credits for every point of overall support the headline would
    take), so protecting votes is a real trade against delivering work.
  * Bad news takes, from each group, its exposure (a share of the worst case) times a small base plus a share of the group's
    lead over 50%. Both the base and the share grow with the term, so doing well is never the end of the problem.
  * Shocks land before queued responses, as in the engine (`Campaign::advance`); election night also counts the queued
    last-quarter response, as the engine does.

Players: `idle` plays Balanced once and then does nothing; `repeat` plays Balanced every term; `smart` is `repeat` plus the
paid response, used when the lead over the bar is thin compared with what the headline would take.

    python3 tools/support-model/calendar_sim.py
"""
import pathlib, random, statistics, sys
sys.path.insert(0, str(pathlib.Path(__file__).parent))
import support_model as sm

P, W, BAR = sm.CALIBRATIONS['v2'], sm.W, 550_000
BAL = {'s': [0, 12, 12, 12, 12, 2], 'r': [0, 0, 10, 10, 10, 10], 'a': [4, 5, 1, 0, 0, 0]}   # Balanced, best case, per term quarter

# (id, exposure in % of the worst case per group: urban low, middle, high, rural low, middle, high)
BAD = [
    ('teacher_shortage',  [50, 50, 50, 25, 25, 25]),
    ('roof_scare',        [25, 25, 25, 75, 75, 75]),
    ('funding_row',       [50, 50, 50, 50, 50, 50]),
    ('rural_bus_cuts',    [0, 0, 0, 100, 100, 100]),
    ('cost_of_living',    [75, 40, 0, 75, 40, 0]),
    ('exam_results_row',  [0, 60, 75, 0, 40, 50]),
    ('agency_scandal',    [60, 60, 30, 30, 30, 15]),
    ('broadband_outage',  [10, 10, 10, 80, 80, 80]),
]
# (id, support gained per group, in ppm)
GOOD = [
    ('inspectorate_praise', [15_000] * 6),
    ('volunteer_surge',     [0, 0, 0, 25_000, 25_000, 25_000]),
    ('attendance_rises',    [20_000, 10_000, 0, 20_000, 10_000, 0]),
    ('recruitment_up',      [12_000] * 6),
]
SHOCK_CAP = 40_000
RELIEF_CREDITS_PER_POINT = 10
RELIEF_CUT_PCT = 60


def response(ix, d):
    return sm.response(ix, d, base=P['base'], mag=P['mag'], clamp=P['clamp'])


def overall(sup):
    return sum(w * s for w, s in zip(W, sup)) // 10**6


def clamp(v):
    return max(0, min(10**6, v))


def severity(term, base_ppm=5_000, base_step=50, share=40, share_step=8, share_cap=90, base_cap=400):
    """(base in ppm, share of the lead in %) for a term: both grow with every term after the second."""
    n = max(0, term - 2)
    return base_ppm * min(base_cap, 100 + base_step * n) // 100, min(share_cap, share + share_step * n)


def hit(exposure, lead, base, share):
    return min(SHOCK_CAP, exposure * (base + lead * share // 100) // 100)


def game(rng, strategy, terms, surprise_in=3, **sev):
    sup, queue, results = [500_000] * 6, {}, []
    for term in range(1, terms + 1):
        count = 2 if term == 1 else 4
        slots = sorted(rng.sample([2, 3, 4, 5, 6], count))
        plan = []
        for q in slots:
            good = term == 1 or rng.random() < 0.3
            item = rng.choice(GOOD if good else BAD)
            plan.append((q, good, item, rng.randrange(surprise_in) == 0))
        headline = {q: (good, item, surprise) for q, good, item, surprise in plan}
        plays = strategy != 'idle' or term == 1
        relief = 0
        base, share = severity(term, **sev)
        for qt in range(1, 7):
            q = (term - 1) * 6 + qt
            d = [BAL['s'][qt - 1], BAL['r'][qt - 1], BAL['a'][qt - 1]] if plays else [0, 0, 0]
            if qt in headline:
                good, (_, vec), surprise = headline[qt]
                if good:
                    shock = list(vec)
                else:
                    shock = [-hit(e, max(0, sup[g] - 500_000), base, share) for g, e in enumerate(vec)]
                    lost = -sum(w * x for w, x in zip(W, shock)) / 1e10
                    lead = (overall(sup) - BAR) / 1e4
                    cost = max(1, int(lost * RELIEF_CREDITS_PER_POINT + 0.999))
                    if strategy == 'smart' and not surprise and lost >= 1.0 and lead < 2 * lost + 4 and relief + cost <= 40:
                        shock = [x * (100 - RELIEF_CUT_PCT) // 100 for x in shock]
                        d = [d[0] * (100 - cost) // 100, d[1] * (100 - cost) // 100, d[2]]   # these credits did not become delivered work
                        relief += cost
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


def table(label, strategies, n=2000, terms=10, **sev):
    print(label)
    print('   ' + ' ' * 7, ' '.join(f'{"T" + str(t):>5}' for t in range(1, terms + 1)))
    for strategy in strategies:
        reach, margins = [0] * (terms + 1), []
        for seed in range(n):
            r = game(random.Random(seed), strategy, terms, **sev)
            for t in range(1, sum(1 for x in r if x >= BAR) + 1):
                reach[t] += 1
            margins += [(x - BAR) / 1e4 for x in r if x >= BAR]
        median = f'{statistics.median(margins):.1f}' if margins else '-'
        print(f'   {strategy:7}', ' '.join(f'{100 * reach[t] / n:4.0f}%' for t in range(1, terms + 1)), f'| median margin when won {median} pts')


if __name__ == '__main__':
    print('Share of games still going after each election. A game ends at the first lost one.\n')
    table('CHOSEN: bad news takes a base of 0.5 pt (+50% a term after term 2, to x4) plus 40% of the lead (+8 points a term, to 90%)',
          ['idle', 'repeat', 'smart'])
    table('GENTLER: base +35% a term (to x3), 30% of the lead (+5 a term, to 60%)', ['idle', 'repeat', 'smart'],
          base_step=35, base_cap=300, share=30, share_step=5, share_cap=60)
    table('HARDER: base +75% a term (to x5), 50% of the lead (+10 a term, to 100%)', ['idle', 'repeat', 'smart'],
          base_step=75, base_cap=500, share=50, share_step=10, share_cap=100)
