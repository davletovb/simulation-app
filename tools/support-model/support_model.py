#!/usr/bin/env python3
"""Replica of the engine's public-support model, for tuning "stay in power" without touching Rust.

Source of truth: crates/domain/src/reducer.rs (`cohort_response_scaled`, quarter advance).
`--check` replays ten strategies that were played against the real service
(captured_runs.json) and must match the engine's reported support to the ppm.
Without arguments it prints the election table for today's numbers and the
proposed calibration. Standard library only.

    python3 tools/support-model/support_model.py --check
    python3 tools/support-model/support_model.py
"""
import json, pathlib, sys

HERE = pathlib.Path(__file__).parent
W = [260000, 320000, 220000, 70000, 80000, 50000]            # urban low/mid/high, rural low/mid/high
LABEL = ['urbL', 'urbM', 'urbH', 'rurL', 'rurM', 'rurH']
GEO = [[100, 100, 100, 100], [80, 130, 140, 100]]            # per channel: staff, repairs, access, cost
INC = [[120, 90, 130, 130], [100, 100, 100, 100], [80, 110, 70, 150]]
TODAY = dict(base=[500, 400, 750, 400], mag=100, clamp=8000)
PROPOSED = dict(base=[800, 700, 750, 500], mag=400, clamp=40000)


def response(ix, d, base, mag, clamp):
    """One quarter's delivery -> change in one group's support (ppm), as the engine computes it."""
    g, i = ix // 3, ix % 3
    t = [base[c] * GEO[g][c] * INC[i][c] // 10000 for c in range(4)]
    raw = t[0] * d[0] + t[1] * d[1] + t[2] * d[2] - t[3] * sum(d)
    return max(-clamp, min(clamp, raw * mag // 100))


def play(cums, base, mag, clamp, lag=1, settle=False):
    """Support after the last quarter. `settle` also lands the final quarter's response (election night)."""
    sup, queue, prev = [500000] * 6, [], [0, 0, 0]
    history = []
    for q, cum in enumerate(cums, start=1):
        d = [cum[k] - prev[k] for k in range(3)]
        prev = list(cum)
        for _, ed in [e for e in queue if e[0] == q]:
            sup = [max(0, min(10**6, s + response(ix, ed, base, mag, clamp))) for ix, s in enumerate(sup)]
        queue = [e for e in queue if e[0] != q]
        if sum(d) > 0:
            queue.append((q + lag, d))
        history.append(sum(w * s for w, s in zip(W, sup)) // 10**6)
    if settle and sum(d) > 0:
        sup = [max(0, min(10**6, s + response(ix, d, base, mag, clamp))) for ix, s in enumerate(sup)]
    return sup, sum(w * s for w, s in zip(W, sup)) // 10**6, history


def check(runs):
    bad = 0
    for name, rows in runs.items():
        got = play([r['cum'] for r in rows], **TODAY)[2]
        bad += sum(g != r['support'] for g, r in zip(got, rows))
    total = sum(len(v) for v in runs.values())
    print(f"replica vs engine: {total - bad}/{total} quarter-values identical across {len(runs)} strategies")
    return bad == 0


def table(runs, title, params, bar=None):
    print(f"\n== {title}")
    print(f"{'strategy':32}{'work':>5}{'support':>9}{'voters held':>12}  " + ' '.join(f'{l:>5}' for l in LABEL) + ('  result' if bar else ''))
    for name, rows in runs.items():
        sup, w, _ = play([r['cum'] for r in rows], settle=True, **params)
        held = sum(W[i] for i in range(6) if sup[i] > 500000) / 1e4
        res = ''
        if bar:
            res = '  WIN' if w / 1e4 >= bar and held >= 60 else '  lose'
        print(f"{name:32}{sum(rows[-1]['cum']):5}{w / 1e4:8.2f}%{held:11.0f}%  " + ' '.join(f'{s / 1e4:5.1f}' for s in sup) + res)


if __name__ == '__main__':
    runs = json.loads((HERE / 'captured_runs.json').read_text())
    ok = check(runs)
    if '--check' in sys.argv:
        sys.exit(0 if ok else 1)
    table(runs, 'today: election night counts the final quarter, no bar', TODAY)
    table(runs, 'proposed calibration, win = support >= 55% and blocs worth >= 60% of voters', PROPOSED, bar=55.0)
