//! Regenerate checked-in TypeScript and JSON Schema from the Serde wire DTOs.
//! --check fails on drift, --write regenerates from the Rust source.
use myth_contracts::checkpoint::{CountryPack, EventRecord, EvidenceRecord, PrivateCheckpoint};
use myth_contracts::{
    Allocation, CapacityCaps, CommandEnvelope, Condition, Counterproposal, Family, ForecastLine,
    ForecastResponse, MinisterAction, Ministry, PlayerAction, PreviewCalculator, PreviewKind,
    PublicCommitment, PublicRoleView, ScriptedSeatVote, ScriptedVoteKind, ScriptedVotes, Stage,
    Vote,
};
use schemars::schema_for;
use std::{
    env, fs,
    path::{Path, PathBuf},
};
use ts_rs::TS;

fn generated_types() -> String {
    let config = ts_rs::Config::default();
    let declarations = [
        <Allocation as TS>::decl(&config),
        <CapacityCaps as TS>::decl(&config),
        <Ministry as TS>::decl(&config),
        <Family as TS>::decl(&config),
        <Vote as TS>::decl(&config),
        <Condition as TS>::decl(&config),
        <PlayerAction as TS>::decl(&config),
        <CommandEnvelope as TS>::decl(&config),
        <Stage as TS>::decl(&config),
        <Counterproposal as TS>::decl(&config),
        <MinisterAction as TS>::decl(&config),
        <PublicCommitment as TS>::decl(&config),
        <PublicRoleView as TS>::decl(&config),
        <PreviewKind as TS>::decl(&config),
        <PreviewCalculator as TS>::decl(&config),
        <ForecastLine as TS>::decl(&config),
        <ForecastResponse as TS>::decl(&config),
        <ScriptedVoteKind as TS>::decl(&config),
        <ScriptedSeatVote as TS>::decl(&config),
        <ScriptedVotes as TS>::decl(&config),
        <EvidenceRecord as TS>::decl(&config),
        <CountryPack as TS>::decl(&config),
    ];
    format!(
        "// GENERATED from myth-contracts. DO NOT EDIT.\n// All *_version and expected_world_version fields are decimal strings.\n\n{}\n",
        declarations
            .into_iter()
            .map(|decl| format!("export {decl}"))
            .collect::<Vec<_>>()
            .join("\n\n")
    )
}
fn generated_schema<T: schemars::JsonSchema>() -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(&schema_for!(T)).expect("schema serialization")
    )
}
fn put(root: &Path, relative: &str, content: &str, mode: &str) {
    let path = root.join(relative);
    match mode {
        "--check" => {
            let old = fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Missing generated artifact {}: {e}", path.display()));
            assert_eq!(
                old,
                content,
                "Generated contract drift in {}",
                path.display()
            );
        }
        "--write" => {
            fs::create_dir_all(path.parent().expect("generated path")).expect("create directory");
            fs::write(&path, content).expect("write generated artifact");
        }
        other => panic!("unknown mode: {other}"),
    }
}
fn main() {
    let mode = env::args().nth(1).unwrap_or_else(|| "--check".into());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    put(
        &root,
        "apps/web/src/generated/transport.ts",
        &generated_types(),
        &mode,
    );
    put(
        &root,
        "docs/contracts/command.schema.json",
        &generated_schema::<CommandEnvelope>(),
        &mode,
    );
    put(
        &root,
        "docs/contracts/minister-action.schema.json",
        &generated_schema::<MinisterAction>(),
        &mode,
    );
    put(
        &root,
        "docs/contracts/public-role-view.schema.json",
        &generated_schema::<PublicRoleView>(),
        &mode,
    );
    put(
        &root,
        "docs/contracts/preview-response.schema.json",
        &generated_schema::<ForecastResponse>(),
        &mode,
    );
    put(
        &root,
        "docs/contracts/scripted-votes.schema.json",
        &generated_schema::<ScriptedVotes>(),
        &mode,
    );
    put(
        &root,
        "docs/contracts/country-pack.schema.json",
        &generated_schema::<CountryPack>(),
        &mode,
    );
    put(
        &root,
        "docs/contracts/private-checkpoint.schema.json",
        &generated_schema::<PrivateCheckpoint>(),
        &mode,
    );
    put(
        &root,
        "docs/contracts/event-record.schema.json",
        &generated_schema::<EventRecord>(),
        &mode,
    );
    put(
        &root,
        "docs/contracts/minister-output-subsets.json",
        &format!("{}\n", serde_json::to_string_pretty(
            &myth_contracts::strict_subset::profiles()).expect("profile schema")),
        &mode,
    );
}
