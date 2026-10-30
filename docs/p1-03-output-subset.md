# P1-03 — Conservative model-output schema candidate

**Purpose:** prepare strict, provider-friendly JSON Schema profiles without making a paid request or promising that an unspecified OpenRouter provider accepts them.

Generated source: `crates/contracts/src/strict_subset.rs`, artifact: `docs/contracts/minister-output-subsets.json`.

Each profile is a **single-stage, single-ministry** schema. First positions are split into `menu` and `new_typed` counterproposal origins; final actions are split into Finance, Education, and Infrastructure to avoid nullable/conditional effort fields. There are nine profiles, not a free-form permissive union.

The selected *local candidate subset* uses only:
- `type` (string, integer, array, object), `enum`;
- `properties`, `required`, `additionalProperties:false` on every object; and
- `items` for arrays.

Deliberately absent: `oneOf`, `anyOf`, `$defs`, `format`, `pattern`, nullable properties, implicit extra keys and other provider-sensitive keywords. All model outputs **must still pass strict Serde decoding and domain validation**: the subset does not express range bounds, total-budget rules, version overflow, authorized roles, proposal versions, commitment spending, or evidence provenance.

Use the matching per-role/per-stage candidate with an exact pinned model/provider/route only after P5-01 checks the provider's actual structured-output parameter support, supported schema keywords, failure modes, and response identity. Disable fallback providers. If the provider rejects any selected keyword or cannot enforce the staged response shape, **fail closed**; change the candidate in a versioned, reviewed PR and rerun the offline fixtures.

This is a passed **offline schema construction** test only after CI confirms the generator and test suite. It is not provider certification, not actual model reliability, and not permission to dispatch a live call. The P1-03 offline schema/Serde item can be checked after passing generation, nine-profile parser tests, and CI. **Provider-route compatibility belongs to P5-01 and remains open.**
