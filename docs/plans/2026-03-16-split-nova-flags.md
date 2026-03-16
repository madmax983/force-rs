# Split Nova Flags Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the broad `nova` feature in `force` with narrower experimental flags: `schema`, `data_utility`, and `composite_graph`, while folding query-plan support into plain `rest` and moving `SoqlMassOp` to `composite`.

**Architecture:** Update the feature graph in `crates/force/Cargo.toml`, then retarget every `#[cfg(feature = "nova")]` gate to the narrower feature that actually owns that code. Finally, update example metadata/docs and verify the new feature combinations compile independently.

**Tech Stack:** Rust 2024, Cargo features, cfg-gated modules/examples, `cargo check`, `cargo test`, `cargo clippy`.

---

### Task 1: Define the replacement feature graph

**Files:**
- Modify: `crates/force/Cargo.toml`

**Step 1: Write the failing test**

Run:
```bash
cargo check -p force --example data_dictionary --features schema
```

Expected: FAIL because `schema` does not exist yet.

**Step 2: Run test to verify it fails**

Run:
```bash
cargo check -p force --example query_plan --features rest
```

Expected: FAIL because the example still requires `nova`.

**Step 3: Write minimal implementation**

Add `schema`, `data_utility`, and `composite_graph` features; remove `nova`; update example `required-features` entries to the narrow flags.

**Step 4: Run test to verify it passes**

Run:
```bash
cargo check -p force --example data_dictionary --features schema
cargo check -p force --example query_plan --features rest
```

Expected: PASS.

### Task 2: Retarget module gates to their real owners

**Files:**
- Modify: `crates/force/src/api/rest/mod.rs`
- Modify: `crates/force/src/api/composite/mod.rs`
- Modify: `crates/force/src/experimental/mod.rs`
- Modify: `crates/force/src/experimental/type_generator.rs`

**Step 1: Write the failing test**

Run:
```bash
cargo check -p force --features "schema"
cargo check -p force --features "data_utility"
cargo check -p force --features "composite composite_graph"
```

Expected: FAIL until cfg gates match the new feature graph.

**Step 2: Run test to verify it fails**

Observe compile errors caused by stale `nova` gates or missing modules.

**Step 3: Write minimal implementation**

Map gates as follows:
- `schema`: schema graph/diff/analyzer/visualizer/changelog, `DataDictionary`, `StructGenerator`
- `data_utility`: `DataFaker`, `sql_exporter`
- `composite_graph`: composite graph API
- `composite`: `SoqlMassOp`
- `rest`: query-plan `explain`

**Step 4: Run test to verify it passes**

Run the same `cargo check` commands again.

### Task 3: Update examples and docs

**Files:**
- Modify: `crates/force/examples/query_plan.rs`
- Modify: `crates/force/examples/generate_struct.rs`
- Modify: `crates/force/examples/soql_mass_op.rs`
- Modify: `crates/force/examples/README.md`

**Step 1: Write the failing test**

Run:
```bash
rg -n "nova" crates/force
```

Expected: stale docs/comments/example guards still mention `nova`.

**Step 2: Run test to verify it fails**

Confirm the remaining hits are outdated references, not unrelated prose.

**Step 3: Write minimal implementation**

Update example comments, `#[cfg]` guards, and README run commands to the new feature names.

**Step 4: Run test to verify it passes**

Run:
```bash
rg -n "nova" crates/force
```

Expected: no remaining references to `nova`.

### Task 4: Verify the split

**Files:**
- No additional code changes expected

**Step 1: Run focused verification**

Run:
```bash
cargo check -p force --example data_dictionary --features schema
cargo check -p force --example generate_struct --features schema
cargo check -p force --example query_plan --features rest
cargo check -p force --example soql_mass_op --features composite
cargo check -p force --features data_utility
```

Expected: PASS.

**Step 2: Run broader verification**

Run:
```bash
cargo clippy -p force --all-features --all-targets -- -D warnings
```

Expected: PASS.
