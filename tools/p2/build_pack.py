"""P2 frozen England case: deterministic source-to-pack compiler (stdlib only).

--write is an explicit authoring operation. --check never writes or downloads.
Raw sources: gzip-compressed original published DfE release HTML captured once,
each checked by its ORIGINAL byte hash. P1 CountryPack remains provenance-only;
values.json is joined by stable_variable_id and contains typed value strings.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import html
import json
import re
from decimal import Decimal, InvalidOperation
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "data/england-school-pilot-v1"
CUTOFF = "2026-10-29T23:59:59Z"
OGL = "https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/"
SOURCE_METADATA = {
    "workforce": ("2026-06-04", "November 2025", "17 August 2026"),
    "pupils": ("2026-06-04", "January 2026 census / academic year 2025/26", "16 October 2026"),
    "funding": ("2026-01-29", "2026-27 financial-year allocations", "29 July 2026"),
}
SOURCE_FOR_INDICATOR = {
    "teacher_fte": "workforce",
    "pupil_teacher_ratio": "workforce",
    "teacher_vacancy_rate": "workforce",
    "teaching_assistant_fte": "workforce",
    "fsm_eligibility_rate": "pupils",
    "school_count": "pupils",
    "allocated_funding_per_pupil_2026_27": "funding",
    "allocated_funding_total_2026_27": "funding",
}
MISSING_VARIABLES = {
    "beneficiary_income_residence_joint": ("pupils", "Joint distribution of school-service beneficiaries by residence and income; unsupported"),
    "teacher_parent_overlap": ("pupils", "Teacher-linked and parent-linked beneficiary overlap; unsupported"),
    "school_repair_backlog": ("workforce", "Fictional programme repair-unit demand has no official measure"),
    "school_access_gaps": ("pupils", "Fictional programme access-unit demand has no official measure"),
}
ORDER = ["urban_low", "urban_middle", "urban_high", "rural_low", "rural_middle", "rural_high"]


def need(condition: bool, explanation: str) -> None:
    if not condition:
        raise ValueError(explanation)


def raw(name: str) -> bytes:
    return (ROOT / name).read_bytes()


def load(name: str):
    return json.loads(raw(name))


def canonical(obj) -> bytes:
    return (json.dumps(obj, sort_keys=True, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def normalize(display: str, transformation: str) -> str:
    text = display.replace(",", "").replace("£", "").strip()
    if text.endswith(" billion"):
        text = text.removesuffix(" billion")
    if text.endswith("%"):
        text = text[:-1]
    try:
        number = Decimal(text)
    except InvalidOperation as exc:
        raise ValueError(f"invalid displayed decimal: {display!r}") from exc
    factor = {
        "identity": Decimal(1),
        "percent * 10000": Decimal(10000),
        "GBP billions * 1000000000": Decimal(1_000_000_000),
    }.get(transformation)
    need(factor is not None, f"unsupported transformation: {transformation}")
    result = number * factor
    return format(result, "f")


def source_bytes():
    manifest = load("raw-source-manifest.json")
    need(manifest["version"] == 1, "unversioned source manifest")
    need(manifest["evidence_cutoff_utc"] == CUTOFF, "source cutoff drift")
    need(manifest["captured_utc"] > CUTOFF, "misrepresented capture date")
    records = {r["source_id"]: r for r in manifest["sources"]}
    need(set(records) == set(SOURCE_METADATA), "missing or unexpected source")
    extracted = {}
    for sid in sorted(records):
        source = records[sid]
        published, period, updated = SOURCE_METADATA[sid]
        need(source["expected_last_update"] == updated, f"release history changed: {sid}")
        need(published <= CUTOFF[:10], f"source release beyond cutoff: {sid}")
        need(source["url"].startswith("https://explore-education-statistics.service.gov.uk/find-statistics/"), "unapproved source")
        path = ROOT / source["file"]
        need(path.parent == ROOT / "raw", "source must be in archived raw directory")
        zipped = path.read_bytes()
        need(digest(zipped) == source["archived_gzip_sha256"], f"gzip digest mismatch: {sid}")
        decompressed = gzip.decompress(zipped)
        need(len(decompressed) == source["uncompressed_length"], f"source byte length mismatch: {sid}")
        need(digest(decompressed) == source["source_bytes_sha256"], f"source byte digest mismatch: {sid}")
        body = html.unescape(decompressed.decode("utf-8"))
        need(updated in body, f"page no longer matches declared last-updated date: {sid}")
        need(len(decompressed) < 2_000_000, "source exceeds bounded archival limit")
        extracted[sid] = (source, body, published, period)
    return manifest, extracted


def authoring_checks(authored):
    need(authored["schema_version"] == 1, "scenario version")
    need(authored["pilot"]["credits"] == 100 and authored["pilot"]["GBP_conversion"] is None, "fake GBP conversion")
    need(authored["quarters"] == 6 and authored["start"] == "2027-Q1", "time frame")
    need(authored["board"]["support_votes_required"] == 2, "two-thirds vote")
    need(len(authored["board"]["ministers"]) == 3, "not three ministers")
    need(authored["board"]["initial_commitments"] == [], "initial commitments must not be fabricated")
    pop = authored["population"]
    need(pop["order"] == ORDER, "cell order drift")
    baseline = pop["variants"][0]["weights_ppm"]
    residence = (sum(baseline[:3]), sum(baseline[3:]))
    income = tuple(baseline[i] + baseline[i + 3] for i in range(3))
    need(residence == (800000, 200000) and income == (330000, 400000, 270000), "authored marginals")
    need(len(pop["variants"]) >= 3 and len({v["id"] for v in pop["variants"]}) == len(pop["variants"]), "variants")
    membership = pop["affiliations"]
    both, teacher, parent = (membership[k] for k in ("both_ppm", "teacher_linked_ppm", "parent_carer_linked_ppm"))
    need(0 <= both <= min(teacher, parent) and teacher + parent - both <= 1000000, "overlap invalid")
    for variant in pop["variants"]:
        w = variant["weights_ppm"]
        need(len(w) == 6 and all(type(i) is int and i >= 0 for i in w), "invalid population cells")
        need(sum(w) == 1_000_000, "population total not conserved")
        need((sum(w[:3]), sum(w[3:])) == residence, "authored residence margin drift")
        need(tuple(w[i] + w[i+3] for i in range(3)) == income, "authored income margin drift")
        # Because the chosen ppm cells are multiples of 10,000, these four
        # subcategories are exact integers and partition each cell.
        for cell in w:
            counts = [cell * (teacher-both)//1000000, cell*(parent-both)//1000000,
                      cell*both//1000000]
            need(sum(counts) <= cell, "affiliation double-counting")
            need(all(cell*x%1000000 == 0 for x in (teacher-both,parent-both,both)), "unallocated residual")
    menu = authored["menu"]
    need({p["id"] for p in menu} >= {"balanced", "staff_first", "access_first", "oversubscribed", "defer"}, "menu incomplete")
    for m in menu:
        funded = sum(m["credits"])
        need(all(type(v) is int and v >= 0 for v in m["credits"]), "invalid credit")
        need((funded <= 100) == m["valid"], "menu feasibility label mismatch")
    need(authored["narrative"]["source_snapshot_changed"] is False, "overlay overwrites raw data")
    need(authored["narrative"]["patches"] == [{"field":"repair_capacity_cap","before":10,"after":8}], "overlay diff drift")
    for n in authored["news"]:
        need(n["kind"] == "fictional_simulation_news" or n["date"] <= CUTOFF[:10], "future real news")
        need(n["url"] is None if n["kind"] == "fictional_simulation_news" else n["url"].startswith("https://"), "news kind/url mismatch")


def compile_pack():
    source_manifest, sources = source_bytes()
    obs = load("observations.json")
    authored = load("authored.json")
    values = load("values.json")
    authoring_checks(authored)
    need(obs["cutoff_utc"] == values["cutoff_utc"] == CUTOFF, "conflicting cutoffs")
    need(obs["schema_version"] == authored["schema_version"] == values["schema_version"] == 1, "input schema")
    observed = {r["id"]: r for r in obs["indicators"]}
    measured = {r["stable_variable_id"]: r for r in values["records"]}
    need(set(observed) == set(measured) == set(SOURCE_FOR_INDICATOR), "indicator set drift")
    need(len(values["records"]) == 8 and len(observed) == 8, "duplicates in value table")
    records = []
    for row in values["records"]:
        vid = row["stable_variable_id"]
        spec = observed[vid]
        sid = SOURCE_FOR_INDICATOR[vid]
        source, original, published, period = sources[sid]
        need(spec["source"] == source["url"], f"source URL mismatch: {vid}")
        # Exact publisher-displayed token is required in the immutable release bytes.
        # It is NOT inferred from other series or later online tables.
        token = row["raw_value"]
        need(token in original, f"recorded source value absent from frozen page: {vid}: {token}")
        number = normalize(token, row["transformation"])
        need(number == row["normalized_value"], f"numeric transformation mismatch: {vid}")
        if row["transformation"] == "identity":
            need(spec["value"] == number, f"transcription mismatch: {vid}")
        need(bool(row["quality_flags"]) and bool(row["normalized_unit"]), "missing quality/unit")
        records.append({
            "stable_variable_id": vid,
            "geographic_scope": ("England state-funded schools" if sid == "workforce"
                                  else "England school census (state-funded and independent scope varies)" if sid == "pupils"
                                  else "England state-funded school revenue allocations for pupils aged 5-16"),
            "definition": {
                "teacher_fte":"FTE of all teachers in state-funded schools, qualified and unqualified",
                "teaching_assistant_fte":"FTE teaching assistants in state-funded schools",
                "pupil_teacher_ratio":"FTE pupils divided by FTE teachers, cross-census PTR",
                "teacher_vacancy_rate":"Full and part-time teacher vacancies as proportion of teachers in service",
                "fsm_eligibility_rate":"Share of eligible pupils for FSM; NOT an income-band measure",
                "school_count":"Number of all schools (state-funded and independent included)",
                "allocated_funding_per_pupil_2026_27":"Revenue funding allocated per pupil, cash terms; estimates and no capital",
                "allocated_funding_total_2026_27":"Total revenue funding allocated, cash terms; estimates and no capital",
            }[vid],
            "source_url": source["url"], "publisher":"Department for Education",
            "reference_period": period, "release_date": published,
            "retrieval_date": source_manifest["captured_utc"][:10],
            "raw_unit": row["raw_unit"], "normalized_unit": row["normalized_unit"],
            "transformation": row["transformation"],
            "licence_or_terms": "UK Open Government Licence v3.0, except excluded rights; "+OGL,
            "sha256_hex": source["source_bytes_sha256"], "missing_reason": None
        })
    for vid, (sid, reason) in MISSING_VARIABLES.items():
        source, original, published, period = sources[sid]
        records.append({
            "stable_variable_id":vid, "geographic_scope":"England fictional school-service beneficiary pilot",
            "definition":reason, "source_url":source["url"], "publisher":"Department for Education",
            "reference_period":period, "release_date":published,
            "retrieval_date":source_manifest["captured_utc"][:10],
            "raw_unit":"unavailable", "normalized_unit":"unavailable",
            "transformation":"No inference or imputation; authored case assumption separately",
            "licence_or_terms":"UK Open Government Licence v3.0, except excluded rights; "+OGL,
            "sha256_hex":source["source_bytes_sha256"], "missing_reason":reason
        })
    pack = {
        "schema_version": 1, "country_iso":"GB", "simulated_jurisdiction":"england",
        "cutoff_utc": CUTOFF, "locale":"en-GB",
        "evidence": sorted(records, key=lambda e: e["stable_variable_id"])
    }
    # Explicitly hash every immutable input; never depend on a remote call
    # or on mutable release pages when rebuilding.
    manifest = {
        "schema_version":1, "pack_id":authored["case_id"], "cutoff_utc":CUTOFF,
        "snapshot_capture_utc":source_manifest["captured_utc"],
        "raw_original_pages_archived":True, "underlying_csv_datasets_archived":False,
        "source_reuse_terms":"OGL-v3-except-third-party-material",
        "source_manifest_sha256":digest(raw("raw-source-manifest.json")),
        "observations_sha256":digest(raw("observations.json")),
        "values_sha256":digest(raw("values.json")),
        "authored_sha256":digest(raw("authored.json")),
        "country_pack_sha256":digest(canonical(pack)),
        "coverage":{"sourced_headline_indicators":8, "explicit_missing_variables":4,
                    "joint_distribution_empirically_supported":False},
        "scope":"Fictional England data twin; no empirical policy forecast or GBP mapping"
    }
    return {"country-pack.json":canonical(pack), "manifest.json":canonical(manifest)}


def main():
    ap = argparse.ArgumentParser()
    group = ap.add_mutually_exclusive_group(required=True)
    group.add_argument("--check", action="store_true")
    group.add_argument("--write", action="store_true")
    args = ap.parse_args()
    built = compile_pack()
    for path, body in built.items():
        dest = ROOT / path
        if args.check:
            need(dest.is_file() and dest.read_bytes() == body, f"frozen artifact drift: {path}")
        else:
            dest.write_bytes(body)
    print("P2: frozen official-source hashes, provenance, authored margins and derived outputs verified")


if __name__ == "__main__":
    main()
