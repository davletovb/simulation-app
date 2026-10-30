"""Offline P2 authored case and headline-evidence checks; no network/paid calls."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "data/england-school-pilot-v1"


def read(name):
    path = ROOT / name
    data = path.read_bytes()
    return json.loads(data), hashlib.sha256(data).hexdigest()


def check():
    obs, obs_hash = read("observations.json")
    authored, authored_hash = read("authored.json")
    assert obs["schema_version"] == authored["schema_version"] == 1
    assert obs["cutoff_utc"] == "2026-10-29T23:59:59Z"
    assert "not full source archive" in obs["capture_method"]
    assert len(obs["indicators"]) == 8
    assert len({x["id"] for x in obs["indicators"]}) == 8
    assert all(x["source"].startswith("https://explore-education-statistics.service.gov.uk/")
               for x in obs["indicators"])
    assert authored["pilot"]["credits"] == 100
    assert authored["pilot"]["GBP_conversion"] is None
    assert authored["board"]["support_votes_required"] == 2
    assert len(authored["board"]["ministers"]) == 3
    p = authored["population"]
    assert p["order"] == ["urban_low", "urban_middle", "urban_high",
                          "rural_low", "rural_middle", "rural_high"]
    assert len(p["variants"]) >= 2
    for variant in p["variants"]:
        weights = variant["weights_ppm"]
        assert len(weights) == 6 and all(type(n) is int and n >= 0 for n in weights)
        assert sum(weights) == 1_000_000
    a = p["affiliations"]
    assert 0 <= a["both_ppm"] <= min(a["teacher_linked_ppm"], a["parent_carer_linked_ppm"])
    assert a["teacher_linked_ppm"] + a["parent_carer_linked_ppm"] - a["both_ppm"] <= 1_000_000
    assert all(sum(c["credits"]) <= 100 for c in authored["menu"] if c["valid"])
    assert all(sum(c["credits"]) > 100 for c in authored["menu"] if not c["valid"])
    for news in authored["news"]:
        assert news["kind"] == "fictional_simulation_news" or news["date"] <= "2026-10-29"
    n = authored["narrative"]
    assert n["source_snapshot_changed"] is False
    assert n["patches"] == [{"field": "repair_capacity_cap", "before": 10, "after": 8}]
    country, _ = read("country-pack.json")
    assert country["schema_version"] == 1 and country["country_iso"] == "GB"
    assert country["simulated_jurisdiction"] == "england"
    assert country["locale"] == "en-GB" and country["cutoff_utc"] == obs["cutoff_utc"]
    assert len(country["evidence"]) == 12
    assert len({e["stable_variable_id"] for e in country["evidence"]}) == 12
    assert sum(e["missing_reason"] is None for e in country["evidence"]) == 8
    expected = {x["id"] for x in obs["indicators"]}
    assert {e["stable_variable_id"] for e in country["evidence"] if e["missing_reason"] is None} == expected
    for e in country["evidence"]:
        assert e["sha256_hex"] == obs_hash
        assert e["source_url"].startswith("https://explore-education-statistics.service.gov.uk/")
        assert e["release_date"] <= obs["cutoff_utc"][:10]
        assert e["retrieval_date"] >= e["release_date"]
        assert "NOT YET CONFIRMED" in e["licence_or_terms"]
        return {"observations_sha256": obs_hash, "authored_sha256": authored_hash,
            "source_downloads_archived": False, "source_reuse_terms_confirmed": False}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    manifest = check()
    path = ROOT / "partial-manifest.json"
    if not args.check:
        path.write_text(json.dumps(manifest, indent=2) + "\n")
    print("P2 offline fixture checks passed (source archive/reuse gates remain open)")
