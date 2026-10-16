# Phase 6 milestone 61: core data assets (W13f) — draft

- **status:** done
- **goal:** A `data` manifest section of shared definitions by kind and ID, held in a core `DataRegistry` that plugins register kinds into and named from prefabs and from other entries through `DataRef<T>`, by ID or written in place; and a two-phase load in which every prefab and data entry is read and parsed, put in place, then checked against the World, so an entry may name any other, with reloads staged and kept only when every check passes.
- **non-goals:** The kit's kinds and scene components (`tags` onward, W13g to W13k); prefab variants and nested prefabs (1.x); a JSON Schema export (catalogue Q8); inline entries in the manifest; nearest-name suggestions outside prefab and data files; patching live entities on a reload; the CLI's `check` (W14b); any render, schedule, system or ECS change.
- **files to touch:** Step 1: `crates/tungsten-core/src/assets/data.rs` (new), `crates/tungsten-core/src/tests/assets/data.rs` (new), `crates/tungsten-core/src/assets/{mod,manifest,prefab,scene}.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/assets/{manifest,prefab}.rs`, `api/tungsten-core.txt`. Step 2: `crates/tungsten/src/asset_loader/{mod,prefab,reload}.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/prefabs.rs`, `api/tungsten.txt`. Step 3: `crates/tungsten-core/tests/manifests.rs`, `scripts/check-repo.py`, `scripts/test-check-repo.py`. Step 4: `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/assets.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/w04-api-freeze.md`, `templates/basic/AGENTS.md`. Step 5: `CHANGELOG.md`, `DESIGN.md`, `Cargo.toml`, `Cargo.lock`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/README.md`, `docs/plans/1.0/w13-kit.md`, this plan (moved to `docs/plans/archive/1.0/`). The plan session: this plan, the register row's plan column, the README's owned fields.
- **ordered steps:** (1) Core: the `data` section, `DataRegistry`, `DataRef<T>` and the two-phase `StagedLoad`; `just api`, then the step 1 API critique. (2) Umbrella: the startup load, the file and manifest reloads, the data files' hot-reload route. (3) Layer 1 and the asset coverage check. (4) The decision, then design, docs, ledger, index and the template's rule. (5) Release 0.81.
- **done-when:** every step's evidence row quoted; workflow §5 closed; the release checks, `just physics-release` and `just visual` pass after the cut; the preflight prints the command block and the post-merge block.

Candidate W13f ([implementation plan](../../1.0/implementation-plan.md) §4 card, level C), milestone M61, release 0.81. Written 2026-10-16 at `18576d9` (0.80). Runs under the [tungsten-milestone skill](../../../../.claude/skills/tungsten-milestone/SKILL.md) and [workflow](../../1.0/workflow.md) §4–§6; this plan records only where it differs.

## Context digest

W13f is the kit catalogue's F2 ([w13-kit](../../1.0/w13-kit.md) §7), next after W13e (M60, 0.80, `D-170`). The catalogue's review found that assets cannot name each other at load (R2): `load_prefabs` checks each prefab against the World before the registry it builds is inserted, so no prefab can name a prefab, and nothing names shared definitions. Every later kit milestone needs both: W13g registers the `tags` kind and scene components that name prefabs (`spawner`); W13h `stats` and `cue_rules`; W13j `loot_tables` and projectiles' prefabs; W13k `effects`, `abilities`, `curves` and `offers` (catalogue §3). A game registers its own kinds the same way.

It stands on prefab files and the asset-checked form (`D-157`), the component registry and `serde_path_to_error` field paths (`D-153`), manifest composition (`D-017`, `D-052`, `D-089`) and the reload matrix (`D-053`). One decision lands, amending `D-157`'s load and reload clauses and extending `D-017` and `D-052` with the section. Level C (`D-150`, `D-151`): the API sketch (§2) is the deliverable, critiqued here and again on step 1's API diff. No capture: nothing per frame changes (§8).

## 1. Audit

Read at `18576d9`.

| # | Finding | Evidence |
| --- | --- | --- |
| A1 | `load_prefabs` loads in sorted ID order into a local `PrefabRegistry` and inserts it after the loop, checking each prefab with `PrefabData::load` against the World's old registry: a component that checks a prefab ID fails whatever the order | `crates/tungsten/src/asset_loader/prefab.rs:19-32` |
| A2 | `PrefabData::load` reads, parses and checks in one call; `validate` runs the fields' checks, then `check_in` per component against the World | `crates/tungsten-core/src/assets/prefab.rs:101-130`, `:136-149` |
| A3 | The manifest reload's prefab block loads each new prefab alone in sorted order, skips an existing ID even when its file moved, and logs and skips a failure while the rest load | `crates/tungsten/src/asset_loader/prefab.rs:56-96` |
| A4 | `reload_prefab` reloads one file against the World and keeps the previous prefab on failure | `crates/tungsten/src/asset_loader/prefab.rs:37-48` |
| A5 | `load_all` loads tilemaps, particle meshes and particle configs before the prefabs, then the cross-checks, then every map's objects | `crates/tungsten/src/asset_loader/mod.rs:423-492` (`:435-438`, `:491`) |
| A6 | `RawManifest` has no `deny_unknown_fields`, so a `data` key is ignored today; each section goes through `unique_ids`; `ManifestError` is not `#[non_exhaustive]`; `merge_entries` refuses a repeated ID per section | `crates/tungsten-core/src/assets/manifest.rs:63-95`, `:164-197`, `:9-59`, `:699-789` |
| A7 | `MissingAsset { field, kind: &'static str, id }` is `#[non_exhaustive]` with `new`; `ComponentError::MissingAsset`'s fields are exhaustive in patterns, so adding one breaks; the engine's checks name `"tilemap"` and `"particle config"`, and none names a prefab or a data entry | `crates/tungsten-core/src/component_registry.rs:462-484`, `:515-525`; `crates/tungsten/src/asset_loader/scene.rs:189-220` |
| A8 | `PrefabRegistry::insert_with_path` keeps a replaced ID's old path in `path_to_id`, so a moved file's old path still routes to the ID, and a file two prefabs share routes to the last one inserted; the manifest lets two prefabs name one file | `crates/tungsten-core/src/assets/prefab.rs:155-209` (`:176-179`); `crates/tungsten-core/src/assets/manifest.rs:577-589` |
| A9 | `strict_components` is crate-visible and `StrictValue`, which refuses a key repeated at any depth, is private | `crates/tungsten-core/src/assets/scene.rs:330-436` |
| A10 | `App::process_hot_reload` dispatches a drained batch path by path; its `json` arm routes theme, animation, particle and prefab files by `id_for_path`, the prefab branch last and exclusive; the drain returns without a `Renderer`, so no headless test reaches the arm | `crates/tungsten/src/app.rs:633-653`, `:659`, `:709-755` (`:745`) |
| A11 | `App::new` inserts the `PrefabRegistry` and the `ComponentRegistry`, then builds the plugins; the merged manifest loads in `resumed`, after every plugin has built | `crates/tungsten/src/app.rs:284`, `:325`, `:371`, `:2218-2228` |
| A12 | The asset coverage check reads `path`, `normal_map` and `emissive_mask` one level under each section, so nested `data` entries would read as unlisted files | `scripts/check-repo.py:74-114` |
| A13 | Layer 1 parses every shipped prefab file; nothing reads data files | `crates/tungsten-core/tests/manifests.rs:120-156` |
| A14 | `docs/LLM_INDEX.md` holds 12,280 of its 12,288 bytes; `just ctx` enforces it | `scripts/check-agent-context.py:22-26` |
| A15 | `load_prefabs`' callers: the template's harness, example 01's test on a bare World, eight umbrella tests; `reload_new_prefabs`' two test callers | `templates/basic/tests/game.rs:40`; `examples/01_platformer/src/tests/main.rs:250-266`; `crates/tungsten/src/tests/prefabs.rs:142-497`, `:264`, `:294` |
| A16 | The reload matrix's Prefab row; the M51 design section; the status line | `DESIGN.md:230`, `:469-477`, `:5` |
| A17 | Prefab items are re-exported at `tungsten_core::assets` and the root, so the core snapshot lists each type three times and each `World` method twice | `crates/tungsten-core/src/assets/mod.rs:31`; `crates/tungsten-core/src/lib.rs:27-39`; `api/tungsten-core.txt:1254`, `:2882`, `:15381`, `:5276`, `:17168` |
| A18 | No shipped manifest has a `data` key, and only example 01's and the template's have `prefabs`; the benchmarks load the root and bench manifests | `jq keys` over the seven `manifest.json` files; `docs/perf/benchmarks.md:23` |
| A19 | The kit names a prefab only in Rust (`Spawner::new`); `animation` is its one scene component | `crates/tungsten-kit/src/spawner.rs:85`; `crates/tungsten-kit/src/animation.rs:72` |
| A20 | Clippy is pedantic across the workspace (`must_use_candidate` among its lints); the kit denies `missing_docs`, core does not | `Cargo.toml:159-175`; `crates/tungsten-kit/src/lib.rs:67` |

## 2. Design

### 2.1 The manifest's `data` section (core, `assets/manifest.rs`)

```jsonc
"data": {
  "effects": { "burning": { "path": "data/effects/burning.json" } },
  "items":   { "potion":  { "path": "data/items/potion.json" } }
}
```

- `RawManifest.data: BTreeMap<String, BTreeMap<String, DataEntry>>`; a kind or an ID repeated inside one manifest is a parse error, as `unique_ids` refuses a repeated ID (`D-017`). `DataEntry { path }` is `#[non_exhaustive]`, as `PrefabEntry` is (Q1).
- `ResolvedManifest.data: BTreeMap<String, BTreeMap<String, ResolvedDataEntry>>`, `ResolvedDataEntry { path: PathBuf }` (`#[non_exhaustive]`, with `new`), canonical when it resolves; a missing file is `ManifestError::MissingDataFile { kind, id, path }`.
- Merge: an ID is unique within its kind across roots, a repeat failing as `ManifestError::DuplicateId` naming `data.<kind>.<id>`; two kinds may share an ID, as a sprite and a prefab may.
- One file backs one entry: a file listed under two `prefabs` or `data` entries, in one root or across roots, fails the merged graph's check (`D-089`'s place) as `ManifestError::SharedFile { path, first, second }`, the smallest pair named, since a file edit routes to one entry (A8; §9, finding 2).
- The manifest stays World-free: a kind no plugin registered fails at the load (§2.4), not the parse.

### 2.2 `DataRegistry` (core, new `assets/data.rs`)

```rust
pub struct DataRegistry { /* kinds by name, entries by kind and ID, their files */ }
impl DataRegistry {
    pub fn new() -> Self;
    pub fn register<T: DeserializeOwned + 'static>(&mut self, kind: &'static str) -> bool;
    pub fn register_with_assets<T: DeserializeOwned + 'static>(
        &mut self,
        kind: &'static str,
        assets: impl Fn(&T, &World) -> Result<(), MissingAsset> + 'static,
    ) -> bool;
    pub fn contains_kind(&self, kind: &str) -> bool;
    pub fn kinds(&self) -> impl Iterator<Item = &'static str>; // sorted
    pub fn kind_of<T: 'static>(&self) -> Option<&'static str>;
    pub fn get<T: 'static>(&self, id: &str) -> Option<&T>;
    pub fn ids(&self, kind: &str) -> impl Iterator<Item = &str>; // sorted; none for an unknown kind
    pub fn entry_for_path(&self, path: &Path) -> Option<(&'static str, &str)>;
    pub fn insert<T: 'static>(&mut self, id: impl Into<String>, value: T);
    pub fn len(&self) -> usize; // entries
    pub fn is_empty(&self) -> bool;
}
impl World {
    pub fn register_data_kind<T: DeserializeOwned + 'static>(&mut self, kind: &'static str) -> bool;
    pub fn register_data_kind_with_assets<T: DeserializeOwned + 'static>(
        &mut self,
        kind: &'static str,
        assets: impl Fn(&T, &World) -> Result<(), MissingAsset> + 'static,
    ) -> bool;
    pub fn check_prefab_id(&self, field: &str, id: &str) -> Result<(), MissingAsset>;
}
```

- Registration follows `ComponentRegistry`'s rules: a name matches `[a-z][a-z0-9_]*`; one type per kind and one kind per type, so `DataRef<T>` finds its kind by `T`; the same pair again is a no-op returning `false`; a name taken by another type, or a type under a second name, panics (`#[track_caller]`), so a game's kind that collides with the kit's fails startup (catalogue §8). The World doors insert the registry when the World has none.
- Kind names are `&'static str` because `MissingAsset.kind` is (A7, Q3).
- Entries are type-erased (`Box<dyn Any>`) and read back typed by `get::<T>`; neither the registry nor `StagedLoad` is `Send` or `Sync`, as `ComponentRegistry` is not (`D-131`: no bound). `Debug` prints the kinds and their IDs. `insert` takes a value as given, as `PrefabRegistry::insert` does (Q10), drops the file the ID had, and panics when no kind has `T`'s type.
- `check_prefab_id` is the asset check of a field that names a prefab: `MissingAsset { field, kind: "prefab", id }` when the World's `PrefabRegistry` lacks `id`. With `DataRef::check` these are the card's `MissingAsset` kinds `prefab` and `data`.
- `data.rs` opens with `#![deny(missing_docs)]`, so `just lint` proves its rustdoc.

### 2.3 `DataRef<T>`

```rust
pub struct DataRef<T>(/* private: an ID, or a boxed T */);
impl<T> DataRef<T> {
    pub fn by_id(id: impl Into<String>) -> Self;
    pub fn inline(definition: T) -> Self;
    pub fn id(&self) -> Option<&str>;
    pub fn as_inline(&self) -> Option<&T>;
}
impl<T: 'static> DataRef<T> {
    pub fn resolve<'a>(&'a self, data: &'a DataRegistry) -> Option<&'a T>;
    pub fn check(&self, world: &World, field: &str) -> Result<(), MissingAsset>;
}
// Clone, Debug and PartialEq when T has them; Deserialize; Serialize.
```

- Deserialize, a visitor over `deserialize_any`: a JSON string is an ID; an object or an array is `T`, read from the same deserializer through `MapAccessDeserializer` or `SeqAccessDeserializer`, so `serde_path_to_error` keeps `T`'s own path (`on_hit.duration`) and never prints an untagged enum's "did not match any variant" (catalogue §3); another JSON value fails with "a data entry's ID or its definition". A `T` written as a string cannot be inline (Q6).
- Serialize: the ID as a string, an inline definition as `T` through `serde_json::to_value`, so a game's `SaveData` holds it (catalogue §2 rule 14); an inline definition whose form is not an object or an array fails to serialize, since it would read back as an ID or not at all (§9, finding 3).
- `resolve`: the inline definition, or the entry the registry holds under the ID now, so a reload shows on the next read; a reader that copies at spawn keeps its copy.
- `check`: an ID the registry lacks is `MissingAsset { field, kind: <T's kind>, id }`; an inline definition runs its kind's check, each `field` it reports prefixed with `field.`; a `T` no kind registered fails naming `type_name::<T>()`.

How a component names definitions (illustrative):

```rust
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProjectileDef { on_hit: DataRef<EffectDef>, spark: String }
world.register_data_kind_with_assets::<EffectDef>("effects", |e, w| w.check_prefab_id("trail", &e.trail));
world.register_component_with_assets(
    "projectile",
    |d: &ProjectileDef, w| { d.on_hit.check(w, "on_hit")?; w.check_prefab_id("spark", &d.spark) },
    |d, _| Projectile::from(d),
);
```

### 2.4 The two-phase load: `StagedLoad`

```rust
pub struct StagedLoad { /* staged prefabs and data entries with their files */ }
impl StagedLoad {
    pub fn new() -> Self;
    pub fn add_prefab(&mut self, id: impl Into<String>, path: impl Into<PathBuf>) -> Result<(), PrefabError>;
    pub fn add_data(
        &mut self,
        world: &World,
        kind: &str,
        id: impl Into<String>,
        path: impl Into<PathBuf>,
    ) -> Result<(), DataError>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn commit(self, world: &mut World) -> Result<(), StagedLoadError>;
}
#[non_exhaustive] pub enum StagedLoadError { Prefab { id: String, source: PrefabError }, Data(DataError) }
#[non_exhaustive] pub enum DataError { UnknownKind { .. }, Io { .. }, Parse { .. }, Invalid { .. }, MissingAsset { .. } }
```

- **Phase one.** `add_prefab` reads and parses as `PrefabData::load` does, with no World check (`PrefabError::{Io, Parse}`). `add_data` reads the file through `StrictValue` (A9, made crate-visible), so a key repeated at any depth fails as in scenes and prefabs, then deserializes it as the kind's type with `serde_path_to_error`: `DataError::UnknownKind` (with the registered kinds), `Io`, `Parse`, `Invalid` (with the field). A second add of an ID replaces the first.
- **Phase two.** `commit` puts every staged entry in place of the World's entry of that ID, inserting the `DataRegistry` and `PrefabRegistry` a World lacks, then checks every entry the two registries hold from a file, staged or not, against the World with the staging in place: a changed definition can break an unchanged one that reads it, as a tags entry that drops a declaration would a prefab's tag filter (W13g; §9, finding 1). Data entries go by kind name, then ID, through their kind's check (`DataError::MissingAsset`, naming the file, the field, the missing kind and ID), then prefabs by ID through the checks `PrefabData::load` runs (`StagedLoadError::Prefab` with `PrefabError::Validation`). Each staged entry takes its file: an entry of either registry that held that file loses it, so the file routes to the staged entry alone (§9, finding 2.2). The first failure restores every staged entry's previous state, a replaced entry with its path, an added one removed and a file's former holder with its file, and returns, naming the entry that failed; otherwise all stay. Entries without a file, inserted in code or left by a lost file, are taken as given (Q10); a map's objects keep M51's checks at the map's own load and reload, since a map whose objects fail is kept with its tiles (`asset_loader/prefab.rs:85-95`). Live entities are never touched.
- `PrefabData::load` keeps its signature and behaviour; its parse and check halves become crate-private functions that `StagedLoad` calls. `PrefabRegistry::insert_with_path` drops the old path of an ID it replaces (A8), and `PrefabRegistry::insert` and `DataRegistry::insert` drop the file the ID had, so a replacement built in code is never routed or checked (§9, finding 2.3); `commit` restores through crate-private replace and remove.
- Each `DataError` and `PrefabError::Validation` message names the file and the field. A `MissingAsset` of kind `prefab` or of a registered data kind, and an unknown kind, end with "; did you mean `<name>`?" when one is near (catalogue §5.2, Q7): a Levenshtein distance over chars in core, at most max(1, length / 3) edits, ties to the first in sorted order, no dependency. Scenes, component names, particle configs and tilemaps print none (Follow-ups).

### 2.5 The umbrella (`asset_loader/`, `app.rs`)

- `load_prefabs(manifest, world)` keeps its signature and becomes the two-phase load of both sections (Q4): it stages every data entry (by kind, then ID), then every prefab (by ID), and commits, adding over the World's registries in place of a new `PrefabRegistry` (Q5). The first failure is "Failed to load prefab '<id>': …", as today, or "Failed to load data `<kind>` entry `<id>`: …". `load_all` calls it where it does today, after the tilemaps and particle configs (A5).
- `reload_prefab(id, path, world)` keeps its signature: one prefab staged and committed, a failure logged with the previous kept.
- `reload_data(kind, id, path, world) -> anyhow::Result<()>`, new and public: the same for one data entry.
- The manifest reload's block, `reload_new_prefabs` renamed `reload_prefabs_and_data` (crate-private): it stages every data entry and prefab of the new manifest, each file read again, so new, moved and edited files load together (Q9), commits once, and on a failure logs "Manifest reload: prefabs and data keep their previous entries: …" and keeps every previous entry (catalogue F2, Q2); removed entries warn and stay. The new maps' object checks follow it, as today (`reload.rs:617-619`).
- Routing: the `json` arm (A10) collects the batch's JSON paths, and crate-private `asset_loader::reload_json_files(paths, world)` in `reload.rs` reloads themes, animations and particle configs per path, with their log lines kept, then stages every prefab and data file of the batch (`id_for_path`, `DataRegistry::entry_for_path`) in one `StagedLoad`, so edits saved together that need each other load together; a batch whose manifest reloaded skips that commit, since the manifest's block read every file (§9, finding 2.1). A failure's log names the entry and says that saving the manifest reloads every prefab and data file together, the way out for two edits saved apart that need each other. `App::new` inserts a `DataRegistry` beside the `PrefabRegistry` (A11). A test drives a data file's edit by path.
- No `App::register_data_kind`: kinds register in a plugin's `build` through the World, as the template registers its components.

### 2.6 The seam and what stays

Core holds every type and the two-phase rule, and no render type (`D-007`, `D-016`); the umbrella reads the manifest's files and routes edits. Nothing per frame changes: no system, stage, schedule or extract (`D-018`), so `DefaultPlugins`' resolved schedule stands. No dependency: `serde_path_to_error` is in core (`D-153`). The template gains one rule line (Q8) and no code: it ships no kind before W13g.

## 3. Steps

Shared checks. **List** `<pkg> <prefix>` counts a crate's library tests under a prefix: `python3 -c 'import subprocess,sys; o=subprocess.run(["cargo","test","-p",sys.argv[1],"--lib","--locked","--","--list"],capture_output=True,text=True).stdout; print(sum(l.startswith(sys.argv[2]) and l.endswith(": test") for l in o.splitlines()))' <pkg> <prefix>`. **Snapshot** `<file>` counts its lines added and removed against `HEAD`: `python3 -c 'import collections as c,subprocess as s,sys; f=sys.argv[1]; o=c.Counter(s.run(["git","show","HEAD:"+f],capture_output=True,text=True).stdout.splitlines()); n=c.Counter(open(f).read().splitlines()); print("added",sum((n-o).values()),"removed",sum((o-n).values()))' <file>`. Counts below assume the names and re-exports of §2; a rename the API critique accepts renames its checks, recorded in §9.

### Step 1: Core data model and the two-phase stage

- **Files:** `crates/tungsten-core/src/assets/data.rs` (new), `crates/tungsten-core/src/tests/assets/data.rs` (new, `#[path]` from `data.rs`), `crates/tungsten-core/src/assets/mod.rs`, `crates/tungsten-core/src/assets/manifest.rs`, `crates/tungsten-core/src/assets/prefab.rs`, `crates/tungsten-core/src/assets/scene.rs`, `crates/tungsten-core/src/lib.rs`, `crates/tungsten-core/src/tests/assets/manifest.rs`, `crates/tungsten-core/src/tests/assets/prefab.rs`, `api/tungsten-core.txt`
- **Change:** in order: the `data` section (§2.1); `data.rs` with `DataRegistry`, the `World` doors and `check_prefab_id` (§2.2), `DataRef<T>` (§2.3), `DataError`, `StagedLoad` and `StagedLoadError` with the nearest-name helper (§2.4); `prefab.rs`'s crate-private parse and check halves, the suggestion in its validation message and the `insert_with_path` fix; `StrictValue` crate-visible; `pub mod data` and the re-exports at `tungsten_core::assets` and the root. Every new public item has rustdoc, the registry, `DataRef` and `StagedLoad` a doctest; getters and constructors take `#[must_use]` (A20). Tests, in `tests/assets/data.rs`: the registration rules and their panics; `get`, `ids`, `kinds`, `kind_of`, `insert`, `entry_for_path`; `DataRef` from a string, an object and an array, a number refused, an inline mistake at `T`'s field path with no "variant" in the message, both forms round-tripped, an inline string or number failing to serialize; `resolve` inline, by ID, missing and after a replace; `check` for a missing ID, an inline definition's prefixed field and an unregistered type; `check_prefab_id` present, absent and with no registry; `StagedLoad` with a prefab and an entry naming each other in either add order, a failure restoring every staged entry (a replaced one with its path, an added one gone) and returning the first in kind, ID, then prefab order, a staged entry that breaks an unchanged file-backed one refused with the unchanged one named, an entry inserted in code left unchecked, a loaded prefab replaced through `insert` left unchecked and unrouted at an unrelated commit, a data entry taking a removed prefab's file so the file routes to it alone and a failure giving the file back, a repeated key as `Parse`, a bad value as `Invalid` with its field, an unknown kind with the registered kinds and the nearest, a second add replacing the first, a commit on a bare World; the nearest name within its bound, none beyond it, ties in sorted order. In `tests/assets/manifest.rs`: the section parses with resolved paths; a kind or an ID repeated in one manifest; `MissingDataFile`; `merge` refusing one kind and ID in two roots and taking one ID under two kinds; `SharedFile` for one file under two entries, in one root and across two; no `data` key gives an empty section. In `tests/assets/prefab.rs`: a replaced path no longer routes; a validation message suggesting the nearest prefab ID. Then `just api`, and the API critique (§9, `C.<n>`) before step 2.
- **Done-when:** `cargo fmt --all -- --check` → exit 0; `cargo test -p tungsten-core --locked` → passes (unit, integration and doctests); List `tungsten-core assets::data::tests::` → at least 12; List `tungsten-core assets::manifest::tests::` → at least 52 (49 at `18576d9`); List `tungsten-core assets::prefab::tests::` → at least 24 (23); `cargo check --workspace --all-targets --locked` → exit 0; `just lint` → exit 0; after `just api`, Snapshot `api/tungsten-core.txt` → `removed 0` and `added` above 0; `rg -c '^pub struct tungsten_core::(assets::(data::)?)?(DataRegistry|DataRef<T>|StagedLoad)$' api/tungsten-core.txt` → 9; `rg -c '^#\[non_exhaustive\] pub enum tungsten_core::(assets::(data::)?)?(DataError|StagedLoadError)$' api/tungsten-core.txt` → 6; `rg -c '^pub fn tungsten_core::World::(register_data_kind|register_data_kind_with_assets|check_prefab_id)[<(]' api/tungsten-core.txt` → 6; `rg -c 'ManifestError::(MissingDataFile|SharedFile)$' api/tungsten-core.txt` → 6; `rg -c '^pub tungsten_core::assets::manifest::RawManifest::data:' api/tungsten-core.txt` → 1; `git diff --quiet HEAD -- api/tungsten.txt api/tungsten-kit.txt api/tungsten-render.txt` → exit 0; `rg -U --count-matches '///[^\n]*\n(\s*#\[[^\n]*\n)*\s*pub struct (DataEntry|ResolvedDataEntry)\b' crates/tungsten-core/src/assets/manifest.rs` → 2
- **Moves:** `api/tungsten-core.txt` grows by additions only

### Step 2: The umbrella's loads, reloads and routing

- **Files:** `crates/tungsten/src/asset_loader/prefab.rs`, `crates/tungsten/src/asset_loader/mod.rs`, `crates/tungsten/src/asset_loader/reload.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/prefabs.rs`, `api/tungsten.txt`
- **Change:** §2.5 in order: `load_prefabs` on `StagedLoad` with its log lines per entry; `reload_prefab` staged; `reload_data`, re-exported beside it in `mod.rs`; `reload_prefabs_and_data` in place of `reload_new_prefabs`, called from `reload_manifest` and from the two tests that called the old name; `reload_json_files` in `reload.rs`, called once per batch with the paths the `json` arm collects; `App::new`'s `DataRegistry`; `load_all`'s and the module docs. Tests, in `tests/prefabs.rs` on an `App`'s World (`reload_manifest` needs a `Renderer`, so its block is driven directly, as M51's tests do): a prefab naming a missing prefab ID fails `load_prefabs` with its file, the field and the suggestion; a prefab naming a missing data ID, and a data entry naming a missing prefab, fail with their files and fields; prefabs and entries naming each other load in any ID order; a data file's edit through `reload_json_files` replaces the entry for later reads, and a bad edit keeps the previous; a data edit that breaks an unchanged prefab is refused, the log naming the prefab, and the previous entry kept; a data edit and a prefab edit that need each other load in one batch, fail one at a time, and then load from the manifest's block; a theme or animation path in the same batch still reloads; a manifest edit adding a prefab and an entry that name each other loads both; a manifest edit whose staged entry fails keeps every previous entry and loads none, and the next good edit loads them; a prefab whose manifest path moved reloads from the new file; a removed prefab's file that the manifest gives a new data entry routes to the entry; an unregistered kind fails the load naming the registered kinds.
- **Done-when:** `cargo fmt --all -- --check` → exit 0; `cargo test -p tungsten --lib --locked asset_loader::` → passes; List `tungsten asset_loader::prefab::tests::` → at least 17 (11 at `18576d9`); `cargo test --workspace --locked` → passes (the template's and example 01's harnesses included); `just lint` → exit 0; after `just api`, Snapshot `api/tungsten.txt` → `removed 0`; `rg -c '^pub fn tungsten::asset_loader::reload_data\(' api/tungsten.txt` → 1; `rg -l 'reload_new_prefabs' crates` → no match (exit 1); `git diff --quiet HEAD -- Cargo.lock crates/tungsten-core/Cargo.toml crates/tungsten/Cargo.toml` → exit 0; `just smoke` → passes
- **Moves:** `api/tungsten.txt` grows by one function; a manifest reload's prefab block becomes all or nothing over every prefab and data file, read again, and a batch's prefab and data edits commit together (Q2, Q9)

### Step 3: Layer 1 and the asset coverage check

- **Files:** `crates/tungsten-core/tests/manifests.rs`, `scripts/check-repo.py`, `scripts/test-check-repo.py`
- **Change:** Layer 1 gains `every_data_file_parses`, the twin of `every_prefab_file_parses`: each `data` entry's file of every example and template manifest parses as JSON; kinds need the game's plugins, which its load checks (no shipped manifest has one yet, A18). `check_assets` reads the `data` section two levels deep, each entry's `path` covering its file, a missing file erroring as `data.<kind>.<id>.path: missing file …` and a kind that is not an object erroring. Three regression tests: a data entry covers its file while an unlisted file under `assets/data/` is reported; the missing-file error; the kind-shape error.
- **Done-when:** `rtk proxy cargo test -p tungsten-core --test manifests --locked` → `test result: ok. 5 passed`; `python3 -B scripts/test-check-repo.py` → `Ran 51 tests` and `OK` (48 at `18576d9`); `just repo-check` → exit 0
- **Moves:** none

### Step 4: Decision, design, docs, ledger, index and template

- **Files:** `DECISIONS.md`, `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/assets.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/w04-api-freeze.md`, `templates/basic/AGENTS.md`
- **Change:** first the decision, with [tungsten-decision](../../../../.claude/skills/tungsten-decision/SKILL.md) (§5): its entry, its row at the end of the index's Assets / Rendering section, and marker lines on `D-017`, `D-052` and `D-157` with "Amended by" notes on their rows. Then, citing it: `DESIGN.md` gains `### Data Entries and the Two-Phase Load — M61` before `### Performance and Profiling` (§2.1–§2.6 in prose), a `Data entry` row in the reload matrix, and the Prefab row's staging, both citing the new decision; `docs/assets.md` a `Data entry` row (`assets/data/`, a folder per kind by convention; the section's shape; checked with the prefabs at load; reloaded live) and its ID note per kind; three ledger rows, `W13f (M61 …)`: the manifest types gain the section (fields, the `MissingDataFile` and `SharedFile` arms, a `data` value that was ignored now parsing, two prefabs sharing one file now failing the load), the two-phase load and reloads (a behaviour change: both sections in `load_prefabs`, adding over the registries, every file-backed entry checked at each commit, a batch's prefab and data edits committed together, a manifest reload reading every prefab and data file again and keeping all or none, `insert` and `insert_with_path` dropping an ID's old file, a suggestion in a prefab's validation message), and the additions for the freeze; `docs/LLM_INDEX.md`'s rows 38 and 39 merged into one, task `Manifest load, data entries, load path, GPU upload, registry, IDs, composition` with its five IDs and the new one, files `core/assets/manifest.rs`, `core/assets/data.rs`, `core/assets/registry.rs`, `core/assets/mod.rs`, `tungsten/asset_loader/` (12,283 bytes on the 0.80 tree); a Recorded limits bullet in `docs/known-issues.md`, `**Data entries and the two-phase load**` with the new ID and M61 (edits that need each other load only when saved in one batch or followed by a manifest save; a failed manifest reload retries only at the next manifest save, since a file that never loaded is not routed; a reader's copy outlives a reload; a prefab or data reload does not check maps' objects again, as before; Layer 1 parses data files as JSON only; suggestions in prefab and data files only); one rule line in `templates/basic/AGENTS.md` after the prefab rule (Q8): shared definitions that several prefabs name are data entries, registered with `world.register_data_kind::<T>("<kind>")` in the plugin's `build`, listed under the manifest's `data.<kind>` and named from a component's field as a `DataRef<T>`, by ID or in place, every ID checked at load.
- **Done-when:** `rg -c '^## D-17[1] — ' DECISIONS.md` → 1; `rg -c '^\*\*Amended by D-17[1]:\*\*' DECISIONS.md` → 3; `rg -c '^\| `D-17[1]` ' docs/DECISION_INDEX.md` → 1; `rg -c '`D-17[1]`' docs/DECISION_INDEX.md` → 4 (its row and the rows of `D-017`, `D-052` and `D-157`); `rg -c '^### Data Entries and the Two-Phase Load — M61$' DESIGN.md` → 1; `rg -c '^\| (Prefab|Data entry) \(.*D-17[1]' DESIGN.md` → 2; `rg -c '^\| Data entry \| `assets/data/' docs/assets.md` → 1; `rg -c 'W13f \(M61' docs/plans/1.0/w04-api-freeze.md` → 3; `rg -c 'core/assets/data\.rs' docs/LLM_INDEX.md` → 1; `rg -c '^\| Load path, GPU upload bridge' docs/LLM_INDEX.md` → no match (exit 1); `python3 -c 'import pathlib; print(len(pathlib.Path("docs/LLM_INDEX.md").read_bytes()))'` → at most 12288; `rg -c '^- \*\*Data entries and the two-phase load\*\* \(`D-17[1]`, M61\)' docs/known-issues.md` → 1; `rg -c 'register_data_kind' templates/basic/AGENTS.md` → 1; `just ctx` → exit 0; `just repo-check` → exit 0
- **Moves:** none

### Step 5: Release 0.81

- **Files:** `CHANGELOG.md`, `DESIGN.md` (the status line; the M61 section is step 4's), `Cargo.toml`, `Cargo.lock`, `docs/plans/1.0/implementation-plan.md` (the register row), `docs/plans/1.0/README.md` (the four owned Now fields, `roadmap.py summary`), `docs/plans/1.0/w13-kit.md` (the revision line; a "From M61" follow-up list after the M52 list, line 486, above the fragment at line 488), this plan (moved to `docs/plans/archive/1.0/`)
- **Change:** close workflow §5 (§8) and this step's evidence row with `status: done`: the register row `Cut <date> as v0.81.0` with the decision; the status line gains "a `data` manifest section of shared definitions checked with the prefabs in a two-phase load"; one `CHANGELOG.md` line naming the decision, its three amendments and the unchanged invariants (§6). Before the cut: `git status --short` and `rg -n '^## D-1' DECISIONS.md | tail -3` for another session's files or IDs (§7); files another session changed ship with the commit (`D-105`) and the report lists them apart, never reverted (at plan time, the Phase 7 rewrite's edits to the implementation plan's §2 and §4 and to `w10-closing-phases.md`); `just checks` → each recommendation matched to a check a step or this step runs. Then [tungsten-finalize](../../../../.claude/skills/tungsten-finalize/SKILL.md) and [tungsten-release](../../../../.claude/skills/tungsten-release/SKILL.md) ([releases](../../../releases.md)): the plan archived, `just release-cut 0.81.0` once, every release check once after it with `just physics-release` and `just visual` (§8 line 2), then the preflight with `--message`
- **Done-when:** [releases](../../../releases.md) step 1's command chain → every check passes, quoted in the report (the plan is archived before the cut); `just physics-release` → passes, the determinism and containment hashes unchanged; `just visual` → passes (its one `load_shaders 'fade'` error line is expected); `just release-preflight 0.81.0 --repo JosephGibson/Tungsten --message 'Update 0.81: M61 core data assets'` → passes and prints the command block and the post-merge block
- **Ends with:** the changed-file list, the command block, the post-merge block, then the plan prompt for W13g: `Use the tungsten-milestone skill. Write the milestone plan for W13g: read docs/plans/1.0/workflow.md, its card in docs/plans/1.0/implementation-plan.md and docs/plans/1.0/w13-kit.md; audit only the code it touches; name it docs/plans/1.0/phase6-milestone-62-kit-foundations.md; critique it last as the skill's step 7 says; end with the run prompt. Don't commit: leave the changes in the tree for the release commit.`

The release needs steps 1, 2 and 4. Step 3 may fall to a follow-up: then the asset coverage misses data files (none ships), step 4 records that as a known issue and words `docs/assets.md`'s coverage sentence to match, and the release chain's `just script-test` still runs the checker's suite.

## 4. Open questions

| # | Question | Default |
| --- | --- | --- |
| Q1 | The manifest entry's shape: `{ "path": … }` or a bare string, as the catalogue's non-binding sketch writes it? | `{ "path": … }`, as every file section has it: one shape for authors and agents, the coverage check's `path` field, and room for a field under `D-103` |
| Q2 | A manifest reload's staged set: all or nothing, or per entry as today? | All or nothing (catalogue F2): an entry kept while one it names failed would fail at its first spawn; the failure logs and the next save retries |
| Q3 | Kind names `&'static str` or `&str`, as component names are? | `&'static str`: `MissingAsset.kind` is static and widening it breaks `ComponentError::MissingAsset`'s patterns (A7); kinds are code-declared |
| Q4 | Keep `load_prefabs` for both sections, or add a name for the pair and deprecate it? | Keep it: the template, example 01 and every umbrella test call it to seed a World (A15), and a prefab check needs the entries it names loaded with it; the freeze review (W4b) may rename it with a ledger row |
| Q5 | The startup load: add over the registries, or replace them as `load_prefabs` replaces the `PrefabRegistry` today? | Add over: the `DataRegistry` holds the kinds plugins registered, a failed commit restores the previous state, and an entry inserted in code survives unless the manifest names its ID; a ledger row records it |
| Q6 | `DataRef<T>`'s inline form: objects and arrays only? | Yes: a string is always an ID, so a definition written as a string has no inline form, and serializing one fails rather than saving an ID (§2.3); no definition the catalogue lists is a string |
| Q7 | Nearest-name suggestions in F2? | Yes, for prefab IDs, data IDs and kinds in prefab and data files (catalogue §5.2 assigns them to every milestone); `ComponentError::MissingAsset` has no field for one, so scenes, component names, particle configs and tilemaps wait (Follow-ups) |
| Q8 | A rule line in the template's `AGENTS.md`? | Yes, one line: workflow §5 line 6 wants the template current when a way to write a game arrives, and the acceptance game reads those rules; no template code until W13g (catalogue §5.7) |
| Q9 | What does a manifest reload stage? | Every prefab and data entry it lists, each file read again: "every entry the reload adds or changes" (catalogue F2) includes moved and edited files, and a full read is how edits saved apart that need each other recover (§9, finding 2.1); today a moved prefab keeps its old file (A3) |
| Q10 | Check an entry without a file? | No, as `PrefabRegistry::insert`: an entry inserted in code, or one whose file another entry took, is taken as given, a reader resolves it, and `commit`'s pass skips it, so it never blocks a reload; `insert` drops the file an ID had (§2.4) |

Each question takes its stated default; the owner reviews the plan on their own time (`D-150`).

## 5. Decisions expected

171: W13f landed (M61): the manifest's `data` section and a core `DataRegistry` of kinds that plugins register, `DataRef<T>` by ID or inline, `World::check_prefab_id`, and a two-phase load of prefabs and data entries (`StagedLoad`) whose commits check every file-backed entry and whose reloads stay only when every check passes. Amends `D-017` (an ID is unique within its kind across roots, and a file backs one prefab or data entry), `D-052` (the merged graph carries the section, loaded with the prefabs) and `D-157` (its load and reload clauses); `D-053`'s matrix gains the row, and `D-103` covers the section's format. The next free ID at `18576d9`; checked again when step 4 runs.

## 6. Invariants

- **May move:** `api/tungsten-core.txt` (step 1) and `api/tungsten.txt` (step 2), by additions only, reviewed and recorded in the evidence log and the `CHANGELOG.md` line.
- **Must not move:** the determinism hash `0x088e_c07a_73c1_b168` and the containment hash `0xaee2_72e0_1ffc_4e4c` (`just physics-release`, release); the row digests, which no step can reach: no benchmark loads a prefab or a data entry (A18) and nothing per frame changes; `gpu-visual.png` and the post, transition and UI overlay references (`just visual`, release); `DefaultPlugins`' resolved schedule (`default_plugins_resolve_to_the_snapshot` in `just check`); `api/tungsten-kit.txt` and `api/tungsten-render.txt`; the kit on core alone; no new dependency (`Cargo.lock` unchanged before the cut); two background threads; scene and prefab files' fields (`D-153`, `D-157`); every shipped manifest loading as before.

## 7. Stop conditions

- A done-when check fails: restore the step's files from their copies, mark the step skipped, and continue with steps that don't depend on it.
- A step needs a system, a stage, an ECS type or a per-frame path to change: stop the step; this plan has no capture.
- An existing test fails for a behaviour the plan does not name (§2.5, Q2, Q5, Q9): stop the step and record it; a test that §2 names a change for is updated in its step.
- The API critique finds a shape tied to one genre, or a contract that installed plugins cannot follow: change step 1 where its files allow, else record an open question with its default and continue (w13-kit §14).
- ID 171 is taken when step 4 runs: take the next free ID, change every reference this plan's steps write, and note it in the report.
- A file outside §files to touch needs a change: stop the step and record it.
- The 5h window is above about 65% when a critique is due: queue it behind `resetsAt` with a background waiter and run the steps that do not depend on it; a code-changing finding then reworks them before the release.

## 8. What the milestone owes

| Workflow §5 line | Step | Note |
| --- | --- | --- |
| 1 Evidence | every | |
| 2 Gates by change | 1, 2, 3, release | The validation matrix's asset rows: crate tests and Layer 1 (1–3), `just repo-check` (3, 4), `just smoke` (2); the release step runs the release checks once after the cut, plus `just physics-release` and `just visual`, which no step runs. No perf capture: no system, stage, ECS type or per-frame path changes, the load runs before the first measured frame, the reload arm runs on a watcher event only, and the card names no row |
| 3 Invariants | release | §6; the digests by construction (A18), the rest by their checks |
| 4 Decisions | 4 | One entry before the docs that cite it |
| 5 API ([break ledger](../../1.0/w04-api-freeze.md#break-ledger), rustdoc, `just api` snapshot) | 1, 2, 4 | `just api` in steps 1 and 2; `#![deny(missing_docs)]` on `data.rs`; three ledger rows in step 4; `just api-check` in the release chain |
| 6 Template and guide | 4 | One `AGENTS.md` rule line (Q8); the guide waits for W9b |
| 7 Routes (`docs/LLM_INDEX.md`, `just ctx`) | 4 | One merged row routes `core/assets/data.rs` within the budget |
| 8 Design | 4 | The M61 section and the reload matrix |
| 9 Records (known issues, backlog, gap log, register, workstream file, README's four owned Now fields) | 4, release | A known-issues limit (4); the register row, `w13-kit.md`'s revision line and follow-ups, the owned fields from `python3 -B scripts/roadmap.py summary` (release); no backlog row (nothing cut from the card); no gap row (the gap log is empty) |
| 10 Close (one `CHANGELOG.md` line, `status: done`, the plan archived) | release | Then the cut, the checks and the preflight; the session ends with both blocks and the next prompt |

## 9. Critique

Round 1: the `critique` skill with `--repo`, `gpt-6-astra`, 2026-10-16, reading the tree at `18576d9` plus this plan, uncommitted; the artifact cut with `sed '/^## 9\./,/^## Fol/{/^## Fol/!d}'`. Effort max (level C's first round), context 872,000, output cap 4,000, 471.5 s, 723,418 tokens in (642,816 cached), 12,655 out (8,897 reasoning), 67 commands, 265.6 KB output (largest 33.1 KB), 5h window 0% → 21%, week 42% → 46%. Three findings, each checked against the lines it cites, all accepted. They changed step 1's done-when (the `SharedFile` count), so round 2 runs.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 1 | [MEDIUM] Reload validation misses unchanged dependents: `commit` checks only staged entries, while a check reads other definitions through `&World`; with W13g's tag filters checked against tag declarations, a changed entry could pass while an unchanged prefab breaks and then fails at its spawn | `docs/plans/1.0/w13-kit.md:267` (tags checked at load); `crates/tungsten-core/src/component_registry.rs:296-299` (`prepare` runs the asset check again) | Accepted | §2.4 phase two checks every file-backed entry with the staging in place and names the one that fails; Q10 (code-built entries skipped); maps keep M51's own checks; tests in steps 1 and 2 |
| 2 | [MEDIUM] The path lookup cannot reload every entry sharing a file: `entry_for_path` gives one entry, nothing stops two IDs naming one file, so an edit reloads one of them | `crates/tungsten-core/src/assets/manifest.rs:577-589` (prefab paths unchecked for sharing); `crates/tungsten-core/src/assets/prefab.rs:177` (the reverse map keeps the last) | Accepted, by refusing the shared file: one rule, no multi-valued maps, no shipped manifest shares one | §2.1's `SharedFile` rule; A8; step 1's test and API count; step 4's ledger row and decision text (§5) |
| 3 | [MEDIUM] Inline serialization can turn a definition into an ID: `inline` takes any `T`, so an inline string saves and reads back as an ID, and an inline number saves and then fails to load | `crates/tungsten-core/src/save.rs:243-247` (a slot is plain `serde_json` output); §2.3's deserializer | Accepted | §2.3: an inline definition serializes through `serde_json::to_value` and fails unless it is an object or an array; Q6; step 1's test |

Round 2: the same reviewer on the revised plan, 2026-10-16, the artifact cut as in round 1, with a focus note naming round 1's changes (none rejected). Effort xhigh, context 872,000, output cap 4,000, 295.2 s, 342,253 tokens in (281,216 cached), 8,011 out (4,841 reasoning), 17 commands, 203.2 KB output (largest 28.9 KB), 5h window 21% → 34%, week 46% → 48%. Three findings, each checked against the lines it cites, all accepted; the fixes stay inside the steps' files. There is no third round.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| 2.1 | [MEDIUM] Edits that need each other cannot reload together: with round 1's full check, renaming a declared tag and updating the prefab that names it fail one at a time against each other's old file, and a manifest save stages only missing or moved entries | `crates/tungsten/src/app.rs:659` (one path at a time) | Accepted | §2.5: one `StagedLoad` per drained batch for its prefab and data files, and a manifest reload that reads every prefab and data file again, the logged way out for edits saved apart; Q9; step 2's tests |
| 2.2 | [MEDIUM] `SharedFile` ignores retained entries: a removed prefab keeps its file's route, so a new data entry given that file never reloads, the prefab branch coming first | `crates/tungsten/src/asset_loader/prefab.rs:65-69` (removed entries kept); `crates/tungsten/src/app.rs:745` (the exclusive prefab branch) | Accepted, by moving the file at the commit, with no new API | §2.4: a staged entry takes its file from whichever entry held it, which becomes an entry without a file; a failure gives it back; tests in steps 1 and 2 |
| 2.3 | [MEDIUM] Q10 misses a file-loaded prefab replaced through `insert`, which keeps its path and would enter the full check as file-backed | `crates/tungsten-core/src/assets/prefab.rs:170-172` | Accepted | §2.2, §2.4: `insert` drops the file an ID had in both registries; Q10; step 1's test |

Step 1 API critique: `critique --raw --repo --force` on the `api/` and `templates/` diff (526 added lines, no template change), `gpt-6-astra`, 2026-10-16, xhigh, context 872,000, output cap 4,000, 179.1 s, 552,138 tokens in (447,104 cached), 4,596 out (2,407 reasoning), 19 commands, 174.2 KB output (largest 23.5 KB), 5h window 34% → 51%, week 48% → 50%. Two findings, each checked against the lines it cites, both accepted; the fixes stay inside step 1's files and change no done-when check.

| # | Finding | Checked against | Verdict | Change or reason |
| --- | --- | --- | --- | --- |
| C.1 | [MEDIUM] Staging against one World and committing to another succeeds silently: a kind the destination lacks drops the value, and a kind it holds for another type stores a value whose check's downcast passes and that `get` cannot read | `crates/tungsten-core/src/assets/data.rs` `put_entry` (returns on a missing kind) and `register_with_assets`' check (`Ok` on a failed downcast) | Accepted | `commit` first checks each staged entry's kind against the World's registry, by `TypeId`, before anything changes: a missing kind is `DataError::UnknownKind`, another type the new `DataError::MismatchedKind` (`DataError` is `#[non_exhaustive]` and new); test `a_commit_refuses_a_kind_the_world_lacks_or_holds_for_another_type` |
| C.2 | [MEDIUM] `add_prefab` and `add_data` keep the path as given, while the app routes canonical watcher paths, so a relative staging path never hot-reloads | `crates/tungsten/src/app.rs:660` (`canonicalize`), `:729` (`id_for_path(&canon)`) | Accepted | Both keep the file's canonical path when it resolves, as the manifest does; test `a_staged_file_routes_by_its_canonical_path`; the core tests' file helper returns canonical paths, since the Windows CI job's `canonicalize` adds a `\\?\` prefix |

## Evidence log

| Step | Date | Verdict | Key numbers | Paths |
| --- | --- | --- | --- | --- |
| 1 | 2026-10-16 | pass | `cargo fmt --all -- --check` exit 0; `cargo test -p tungsten-core --locked` 1,143 passed, 0 failed (doctests included); List `assets::data::tests::` 24, `assets::manifest::tests::` 55, `assets::prefab::tests::` 25; `cargo check --workspace --all-targets --locked` exit 0; `just lint` exit 0; Snapshot `api/tungsten-core.txt` added 526, removed 0; the five `rg -c` counts 9, 6, 6, 6, 1; `git diff --quiet HEAD -- api/tungsten.txt api/tungsten-kit.txt api/tungsten-render.txt` exit 0; the rustdoc count 2. The struct count first read 6: a tuple struct renders as `DataRef<T>(_)`, so `DataRef` took a named private field. `a_repeated_prefab_id_fails_in_one_section_and_across_manifests` gave `bat` a file of its own, `SharedFile` (§2.1) refusing its shared one. `StagedLoadError`'s sources are boxed (`result_large_err`). C.1 and C.2 applied, then every check rerun. `just api`'s first run hit a full disk; `target/debug/incremental` (150 GB) was removed (workflow §6.5) | `crates/tungsten-core/src/assets/data.rs`, `crates/tungsten-core/src/tests/assets/data.rs`, `api/tungsten-core.txt`, §9 C.1–C.2 |
| 2 | 2026-10-16 | pass | `cargo fmt --all -- --check` exit 0; `cargo test -p tungsten --lib --locked asset_loader::` 50 passed; List `asset_loader::prefab::tests::` 23; `cargo test --workspace --locked` 40 suites, 2,215 passed, 0 failed; `just lint` exit 0; Snapshot `api/tungsten.txt` added 1, removed 0; `rg -c '^pub fn tungsten::asset_loader::reload_data\('` 1; `rg -l 'reload_new_prefabs' crates` exit 1; `git diff --quiet HEAD -- Cargo.lock crates/tungsten-core/Cargo.toml crates/tungsten/Cargo.toml` exit 0; `just smoke` exit 0 (examples 5/5, template 1/1, benchmarks 15/15, every row OK). `reload_json_files` returns the error lines it logs, so a test reads the line naming the prefab; `load_prefabs_loads_each_prefab_with_its_path` looks its file up by the canonical path (C.2) | `crates/tungsten/src/asset_loader/{prefab,reload,mod}.rs`, `crates/tungsten/src/app.rs`, `crates/tungsten/src/tests/prefabs.rs`, `api/tungsten.txt` |
| 3 | 2026-10-16 | pass | `rtk proxy cargo test -p tungsten-core --test manifests --locked` → `test result: ok. 5 passed`; `python3 -B scripts/test-check-repo.py` → `Ran 51 tests`, `OK`; `just repo-check` exit 0 | `crates/tungsten-core/tests/manifests.rs`, `scripts/check-repo.py`, `scripts/test-check-repo.py` |
| 4 | 2026-10-16 | pass | ID 171 free (`D-170` the last); the counts in order 1, 3, 1, 4, 1, 2, 1, 3, 1; `rg -c '^\| Load path, GPU upload bridge'` exit 1; `docs/LLM_INDEX.md` 12,283 bytes; the known-issues and template counts 1, 1; `just ctx` exit 0; `just repo-check` exit 0 | `DECISIONS.md` (`D-171`, markers on `D-017`, `D-052`, `D-157`), `docs/DECISION_INDEX.md`, `DESIGN.md`, `docs/assets.md`, `docs/LLM_INDEX.md`, `docs/known-issues.md`, `docs/plans/1.0/w04-api-freeze.md`, `templates/basic/AGENTS.md` |
| 5 | 2026-10-16 | pass (records); the release checks run once after the cut, the plan archived before it, so their output is in the session report | Workflow §5: (1) rows 1–5; (2) steps 1–3's crate tests, Layer 1, `just repo-check`, `just smoke`, then the release chain with `just physics-release` and `just visual` after the cut, no capture (§8); (3) §6's invariants by their checks after the cut, `Cargo.lock` unchanged before it (`git diff --quiet HEAD -- Cargo.lock` exit 0), the kit and render snapshots unchanged; (4) `D-171`; (5) three ledger rows, `#![deny(missing_docs)]` on `data.rs`, `just api` in steps 1 and 2; (6) the template's rule line; (7) the merged LLM index row, `just ctx`; (8) `DESIGN.md`'s M61 section, matrix rows and status line; (9) the known-issues limit, the register row, `w13-kit.md`'s revision line and "From M61" follow-ups, the README's owned fields from the cut; (10) one `[Unreleased]` entry, `status: done`, archived. Before the cut: `git status --short` showed the Phase 7 rewrite's `docs/plans/1.0/w10-closing-phases.md` and its edits to `implementation-plan.md` and `README.md` beside this plan's files, which ship with the commit; `rg -n '^## D-1' DECISIONS.md \| tail -3` → `D-169`, `D-170`, `D-171`; `just checks` → `cargo test -p tungsten-core`, `cargo test -p tungsten`, `just ctx`, `just repo-check`, `just api-check`, `just script-test`, `just check`, `git diff --check`, `just smoke`, each run by a step or the release chain; its perf review item is §8's no capture | `CHANGELOG.md`, `DESIGN.md`, `docs/plans/1.0/implementation-plan.md`, `docs/plans/1.0/w13-kit.md`, this plan |

## Follow-ups

- **Suggestions everywhere.** Nearest names for scenes, component names, particle configs and tilemaps (catalogue §5.2): `ComponentError::MissingAsset` needs a field or the loaders a shared helper; W13g's errors or the Phase 6 QA pass. The release step copies this to `w13-kit.md`'s follow-ups.
- **The index budget.** After step 4, `docs/LLM_INDEX.md` sits 5 bytes under its budget; W13g's domain subfolders need a trim or a budget change (`D-106`).
- **`w13-kit.md`'s tail.** Lines 488–511 hold a fragment of the archived catalogue's critique and evidence log under a heading that a `sed` expression's remains broke; left for the owner, since its intended text is not in this plan's sources.
- **The CLI.** W14b's `tungsten check` can drive `StagedLoad` against a World with the game's plugins built, and report every failure rather than the first.
- **Routes of other removed assets.** A removed theme, animation or particle config keeps its file's route, and the `json` arm tries those first (A10), so a manifest that gives such a file to a prefab or data entry reloads the old asset; `SharedFile` and the commit's file move cover the two staged sections only. The Phase 6 QA pass, with the known issues' manifest-path row.
