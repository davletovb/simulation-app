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
from datetime import datetime
from html.parser import HTMLParser
from decimal import Decimal, InvalidOperation
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "data/england-school-pilot-v1"
CUTOFF = "2026-10-29T23:59:59Z"
OGL = "https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/"
# All source slugs, publication and update dates live in source-catalog.json.
# Both capture and offline reconstruction consume exactly this record.
def source_catalog():
    catalog = load("source-catalog.json")
    need(catalog["schema_version"] == 1 and catalog["cutoff_utc"] == CUTOFF, "catalog version/cutoff")
    sources = {x["id"]: x for x in catalog["sources"]}
    need(len(sources) == len(catalog["sources"]) == 3, "duplicate/missing source")
    return sources


class _VisibleText(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.chunks = []
        self.hidden = 0

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self.hidden += 1

    def handle_endtag(self, tag):
        if tag in ("script", "style"):
            self.hidden -= 1

    def handle_data(self, data):
        if not self.hidden:
            self.chunks.append(data)


def visible_text(markup):
    parser = _VisibleText()
    parser.feed(markup)
    return " ".join(" ".join(parser.chunks).split())


def parsed_release_dates(text):
    def field(label):
        matches = re.findall(r"\b" + label + r"\s+(\d{1,2} [A-Za-z]+ \d{4})\b", text, flags=re.I)
        need(bool(matches), f"published-page {label} missing")
        need(len(set(matches)) == 1, f"ambiguous published-page {label}")
        return datetime.strptime(matches[0], "%d %B %Y").date()
    return field("Published"), field("Last updated")


# Each source token is read from a labelled headline position, never matched
# anywhere in the entire page. The first relevant labelled observation wins.
# Numeric boundaries prevent '17.9' from matching '117.9' or '17.95'.
HEADLINE_PATTERNS = {
    "teacher_fte": r"\bFTE of all school teachers\b\s+([\d,]+)\b",
    "teaching_assistant_fte": r"\bFTE of teaching assistants\b\s+([\d,]+)\b",
    "pupil_teacher_ratio": r"\bPupil to teacher ratio\b\s+(\d+(?:\.\d+)?)(?![\d.,])",
    "teacher_vacancy_rate": r"\bTeacher vacancy rate\b\s+(\d+(?:\.\d+)?%)(?![\d.,])",
    "fsm_eligibility_rate": r"(\d+(?:\.\d+)?%)\s+of pupils are eligible for free school meals\b",
    "school_count": r"\bnumber of schools has increased slightly, by\s+[\d,]+\s+to\s+([\d,]+)\b",
    "allocated_funding_per_pupil_2026_27": r"\bOn a per-pupil basis.{0,200}?in 2026-27 was\s+(£\d{1,3}(?:,\d{3})*)(?![\d.])",
    "allocated_funding_total_2026_27": r"\btotal funding allocated to schools through the grants covered in this report is\s+(£[\d,.]+ billion)\b",
}


def headline_value(vid, text):
    pattern = HEADLINE_PATTERNS[vid]
    matched = re.search(pattern, text, flags=re.I)
    need(matched is not None, f"headline label/value absent: {vid}")
    return matched.group(1)


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
# Missing inputs are authored gap metadata only, never fake DfE observations.
MISSING_VARIABLES = {
    "beneficiary_income_residence_joint": "Joint distribution of school-service beneficiaries by residence and income; unsupported",
    "teacher_parent_overlap": "Teacher-linked and parent-linked beneficiary overlap; unsupported",
    "school_repair_backlog": "Fictional programme repair-unit demand has no official measure",
    "school_access_gaps": "Fictional programme access-unit demand has no official measure",
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
    return format(result.quantize(Decimal(1)) if result == result.to_integral_value() else result, "f")


def source_bytes():
    manifest = load("raw-source-manifest.json")
    catalog = source_catalog()
    need(manifest["version"] == 1, "unversioned source manifest")
    need(manifest["evidence_cutoff_utc"] == CUTOFF, "source cutoff drift")
    capture = datetime.strptime(manifest["captured_utc"], "%Y-%m-%dT%H:%M:%SZ").date()
    records = {r["source_id"]: r for r in manifest["sources"]}
    need(len(records) == len(manifest["sources"]) and set(records) == set(catalog),
         "missing/duplicate/unexpected source")
    extracted = {}
    for sid in sorted(records):
        source = records[sid]
        entry = catalog[sid]
        need(source["expected_last_update"] == entry["updated"], f"manifest update differs: {sid}")
        expected_url = "https://explore-education-statistics.service.gov.uk/find-statistics/" + entry["slug"]
        need(source["url"] == expected_url, f"source release URL differs: {sid}")
        path = ROOT / source["file"]
        need(path.parent == ROOT / "raw" and path.name == sid + ".html.gz", "archived source path")
        zipped = path.read_bytes()
        need(digest(zipped) == source["archived_gzip_sha256"], f"gzip digest mismatch: {sid}")
        decompressed = gzip.decompress(zipped)
        need(len(decompressed) == source["uncompressed_length"], f"source length mismatch: {sid}")
        need(len(decompressed) < 2_000_000, "source exceeds archival size limit")
        need(digest(decompressed) == source["source_bytes_sha256"], f"source bytes mismatch: {sid}")
        page = html.unescape(decompressed.decode("utf-8"))
        rendered = visible_text(page)
        published, updated = parsed_release_dates(rendered)
        declared_published = datetime.strptime(entry["published"], "%Y-%m-%d").date()
        declared_updated = datetime.strptime(entry["updated"], "%d %B %Y").date()
        cutoff_day = datetime.strptime(CUTOFF[:10], "%Y-%m-%d").date()
        need(published == declared_published and updated == declared_updated,
             f"frozen page publication/update metadata mismatches catalog: {sid}")
        need(published <= updated <= cutoff_day, f"after-cutoff release/update: {sid}")
        need(capture >= published, f"capture predates page publication: {sid}")
        need("Open Government Licence" in rendered, f"OGL notice absent: {sid}")
        extracted[sid] = (source, rendered, entry["published"], entry["reference_period"])
    return manifest, extracted


def authoring_checks(authored):
    need(authored["schema_version"] == 1, "scenario version")
    need(authored["pilot"]["credits"] == 100 and authored["pilot"]["GBP_conversion"] is None, "fake GBP conversion")
    need(authored["quarters"] == 6 and authored["start"] == "2027-Q1", "time frame")
    need(authored["board"]["support_votes_required"] == 2, "two-thirds vote")
    need(len(authored["board"]["ministers"]) == 3, "not three ministers")
    board = authored["board"]
    need(board["initial_commitments"] == [], "initial commitments must not be fabricated")
    need("initial_enforceable_commitments" not in board and "initial_political_promises" not in board,
         "duplicated opening commitment fields forbidden")
    ledger = board["commitment_ledger"]
    need(set(ledger) == {"honored_relationship_delta", "broken_relationship_delta"},
         "legacy duplicate registry fields")
    need(board["opening_statements_are_not_registry_commitments"] is True,
         "opening statements must not create a registry entry")
    need(len(board["opening_statements"]) == 2, "expected fictional nonbinding statements")
    pop = authored["population"]
    need(pop["order"] == ORDER, "cell order drift")
    baseline = pop["variants"][0]["weights_ppm"]
    residence = (sum(baseline[:3]), sum(baseline[3:]))
    income = tuple(baseline[i] + baseline[i + 3] for i in range(3))
    residence_declared = pop["authored_residence_marginals_ppm"]
    income_declared = pop["authored_income_marginals_ppm"]
    need(residence == (residence_declared["urban"], residence_declared["rural"]),
         "authored residence margins")
    need(income == tuple(income_declared[k] for k in ("low", "middle", "high")),
         "authored income margins")
    need(sum(residence) == sum(income) == 1_000_000, "authored margin totals")
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
    patch = authored["narrative"]["patches"]
    need(len(patch) == 1 and patch[0]["field"] == "repair_capacity_cap"
         and patch[0]["before"] == authored["pilot"]["capacity"]["repairs"]
         and patch[0]["after"] == 8, "overlay diff drift")
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
    need(len(values["records"]) == len(observed) == len(SOURCE_FOR_INDICATOR) == 8,
         "duplicate observation or value IDs")
    records = []
    for row in values["records"]:
        vid = row["stable_variable_id"]
        spec = observed[vid]
        sid = SOURCE_FOR_INDICATOR[vid]
        source, original, published, period = sources[sid]
        need(spec["source"] == source["url"], f"source URL mismatch: {vid}")
        token = headline_value(vid, original)
        need(token == row["raw_value"], f"labelled source headline changed: {vid}")
        number = normalize(token, row["transformation"])
        need(number == row["normalized_value"], f"normalization mismatch: {vid}")
        # observations.json may use a raw percent with 'percent' as its unit,
        # but funding billion values are normalized to whole GBP. Compare ALL
        # eight observations against the correct representation, not just identity.
        observed_value = normalize(token, "identity") if token.endswith("%") else number
        observed_unit = "percent" if token.endswith("%") else row["normalized_unit"]
        need(Decimal(spec["value"]) == Decimal(observed_value)
             and spec["unit"] == observed_unit, f"observed value/unit mismatch: {vid}")
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
        "source_catalog_sha256":digest(raw("source-catalog.json")),
        "missing_variables":[{"stable_variable_id":vid, "origin":"authored_unobserved_gap",
                              "missing_reason":reason, "source_url":None}
                             for vid, reason in sorted(MISSING_VARIABLES.items())],
        "observations_sha256":digest(raw("observations.json")),
        "values_sha256":digest(raw("values.json")),
        "authored_sha256":digest(raw("authored.json")),
        "country_pack_sha256":digest(canonical(pack)),
        "coverage":{"sourced_headline_indicators":8, "explicit_missing_variables":len(MISSING_VARIABLES),
                    "joint_distribution_empirically_supported":False},
        "scope":"Fictional England data twin; no empirical policy forecast or GBP mapping"
    }
    news = {"schema_version":1, "cutoff_utc":CUTOFF, "items":authored["news"],
            "label":"Real DfE release metadata versus explicitly fictional scenario headlines"}
    diff = {"schema_version":1, "overlay_id":authored["narrative"]["id"],
            "applies_quarter":authored["narrative"]["quarter"],
            "baseline_pack_sha256":digest(canonical(pack)),
            "baseline_scenario_sha256":digest(raw("authored.json")),
            "baseline_repair_cap":authored["pilot"]["capacity"]["repairs"],
            "overlay_repair_cap":authored["narrative"]["patches"][0]["after"],
            "evidence_modified":False,
            "apply_rule":"Copy baseline state; for Q2 set repair cap to 8; never mutate raw pack or baseline"}
    manifest["news_bundle_sha256"] = digest(canonical(news))
    manifest["narrative_diff_sha256"] = digest(canonical(diff))
    return {"country-pack.json":canonical(pack), "news-bundle.json":canonical(news),
            "narrative-diff.json":canonical(diff), "manifest.json":canonical(manifest)}


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
