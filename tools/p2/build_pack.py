"""Deterministic offline P2 evidence/scenario validator and pack builder.

Requires Python 3.13; no runtime dependencies or network. --check never writes.
The archived release HEADLINES are not official raw CSV source archives.
"""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "data/england-school-pilot-v1"
CUT = "2026-10-29T23:59:59Z"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load(name):
    return json.loads((DATA / name).read_bytes())


def canonical(obj):
    return (json.dumps(obj, ensure_ascii=True, sort_keys=True, indent=2) + "\n").encode()


def build():
    observed, authored = load("observations.json"), load("authored.json")
    assert observed["schema_version"] == authored["schema_version"] == 1
    assert observed["cutoff_utc"] == CUT
    assert authored["pilot"]["credits"] == 100 and authored["pilot"]["GBP_conversion"] is None
    assert authored["board"]["support_votes_required"] == 2
    assert len(authored["board"]["ministers"]) == 3
    assert authored["population"]["order"] == [
        "urban_low", "urban_middle", "urban_high",
        "rural_low", "rural_middle", "rural_high",
    ]
    variants = authored["population"]["variants"]
    assert len(variants) >= 2 and len({v["id"] for v in variants}) == len(variants)
    for variant in variants:
        weights = variant["weights_ppm"]
        assert len(weights) == 6 and all(type(x) is int and x >= 0 for x in weights)
        assert sum(weights) == 1_000_000
    a = authored["population"]["affiliations"]
    assert 0 <= a["both_ppm"] <= min(a["teacher_linked_ppm"], a["parent_carer_linked_ppm"])
    assert a["teacher_linked_ppm"] + a["parent_carer_linked_ppm"] - a["both_ppm"] <= 1_000_000
    assert all(sum(v["credits"]) <= 100 for v in authored["menu"] if v["valid"])
    assert all(sum(v["credits"]) > 100 for v in authored["menu"] if not v["valid"])
    assert all(n["kind"] == "fictional_simulation_news" or n["date"] <= CUT[:10]
               for n in authored["news"])
    for patch in authored["narrative"]["patches"]:
        assert patch["field"] == "repair_capacity_cap"
        assert patch["before"] == authored["pilot"]["capacity"]["repairs"]
    assert authored["narrative"]["source_snapshot_changed"] is False

    sources = {
        "workforce": ("https://explore-education-statistics.service.gov.uk/find-statistics/"
                      "school-workforce-in-england/2025", "November 2025", "2026-06-04"),
        "pupils": ("https://explore-education-statistics.service.gov.uk/find-statistics/"
                   "school-pupils-and-their-characteristics/2025-26", "2025-26", "2026-06-04"),
    }
    ids = set()
    evidence = []
    for row in observed["indicators"]:
        sid = row["source_id"]
        assert sid in sources and row["stable_variable_id"] not in ids
        ids.add(row["stable_variable_id"])
        url, period, released = sources[sid]
        missing = row["raw_value"] is None
        assert missing == (row["normalized_value"] is None)
        assert (row["missing_reason"] is None) != missing
        assert not missing or "missing" in row["quality_flags"]
        evidence.append({
            "stable_variable_id": row["stable_variable_id"],
            "geographic_scope": row["geographic_scope"],
            "definition": row["definition"],
            "source_url": url,
            "publisher": "Department for Education",
            "reference_period": period,
            "release_date": released,
            "retrieval_date": "2026-10-30",
            "raw_unit": row["raw_unit"],
            "normalized_unit": row["normalized_unit"],
            "transformation": row["transformation"],
            "licence_or_terms": "Release-specific source redistribution terms NOT YET CONFIRMED",
            "sha256_hex": sha((DATA / "observations.json").read_bytes()),
            "missing_reason": row["missing_reason"],
        })
    assert len(ids) == 8
    pack = {
        "schema_version": 1, "country_iso": "GB",
        "simulated_jurisdiction": "england",
        "cutoff_utc": CUT, "locale": "en-GB", "evidence": evidence,
    }
    manifest = {
        "schema_version": 1, "pack_id": authored["case_id"],
        "observations_sha256": sha((DATA / "observations.json").read_bytes()),
        "authored_sha256": sha((DATA / "authored.json").read_bytes()),
        "country_pack_sha256": sha(canonical(pack)),
        "source_downloads_archived": False,
        "source_reuse_terms_confirmed": False,
        "coverage": {
            "indicators": len(evidence),
            "reported_headlines": sum(e["missing_reason"] is None for e in evidence),
            "explicit_gaps": sum(e["missing_reason"] is not None for e in evidence),
        },
    }
    return pack, manifest


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--check", action="store_true")
    args = p.parse_args()
    pack, manifest = build()
    for name, obj in [("country-pack.json", pack), ("manifest.json", manifest)]:
        target = DATA / name
        content = canonical(obj)
        if args.check:
            assert target.is_file() and target.read_bytes() == content, name + " drift"
        else:
            target.write_bytes(content)
    print("P2 fixture invariants and content hashes: OK")


if __name__ == "__main__":
    main()
