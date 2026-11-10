//! Rooms and tilesets (redo 7.12): the manifest's `tilesets` and `rooms`
//! loaded into the World's `RoomRegistry`, each room compiled and its map
//! registered under its ID, after the physics layer names (which a
//! collision layer names) and before the prefabs, so the cross-checks that
//! follow cover each room's tileset sprites and objects as a map's.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use tungsten_core::World;
use tungsten_core::assets::{
    GenerateError, ResolvedManifest, Room, RoomError, RoomGenerators, RoomRegistry, Tileset,
};

/// Whether `manifest` lists a room or a tileset: only then does a load
/// insert the `RoomRegistry`, so a game without rooms holds none.
pub(crate) fn has_rooms(manifest: &ResolvedManifest) -> bool {
    !manifest.rooms.is_empty() || !manifest.tilesets.is_empty()
}

/// Reads and parses the tileset file at `path`.
pub(crate) fn read_tileset(path: &Path) -> Result<Tileset, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    Tileset::from_json(&text).map_err(|e| e.to_string())
}

/// Reads and parses the room file at `path`.
pub(crate) fn read_room(path: &Path) -> Result<Room, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read: {e}"))?;
    Room::from_json(&text).map_err(|e| e.to_string())
}

/// A load plan's tileset unit: tileset `id` read and registered with its
/// file.
pub(crate) fn load_tileset_unit(
    manifest: &ResolvedManifest,
    id: &str,
    world: &mut World,
) -> anyhow::Result<()> {
    let path = &manifest.tilesets[id].path;
    let tileset = read_tileset(path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to load tileset '{id}' from '{}': {e}",
            path.display()
        )
    })?;
    log::info!(
        "Loaded tileset '{id}' ({} tiles) from '{}'",
        tileset.tiles.len(),
        path.display()
    );
    registry(world).insert_tileset_with_path(id, tileset, path.clone());
    Ok(())
}

/// A load plan's room unit: room `id` read, compiled against its tileset
/// and registered, its map under its ID.
pub(crate) fn load_room_unit(
    manifest: &ResolvedManifest,
    id: &str,
    world: &mut World,
) -> anyhow::Result<()> {
    let path = &manifest.rooms[id].path;
    let failed = |e: &dyn std::fmt::Display| {
        anyhow::anyhow!("Failed to load room '{id}' from '{}': {e}", path.display())
    };
    let room = read_room(path).map_err(|e| failed(&e))?;
    world
        .insert_room_with_path(id, room, path.clone())
        .map_err(|e: RoomError| failed(&e))?;
    log::info!("Loaded room '{id}' from '{}'", path.display());
    Ok(())
}

/// The World's `RoomRegistry`, inserted when it has none.
fn registry(world: &mut World) -> &mut RoomRegistry {
    if !world.has_resource::<RoomRegistry>() {
        world.insert_resource(RoomRegistry::new());
    }
    world.resource_mut::<RoomRegistry>()
}

/// `tungsten check`'s rooms: every tileset, then every room, sorted, each
/// failure sent to `error` with its file and its message, which names the
/// tileset or room and the field's path; inserts the registry when the
/// manifests list either.
pub(crate) fn check_rooms(
    manifest: &ResolvedManifest,
    world: &mut World,
    mut error: impl FnMut(&Path, String),
) {
    if !has_rooms(manifest) {
        return;
    }
    registry(world);
    let mut tilesets: Vec<&String> = manifest.tilesets.keys().collect();
    tilesets.sort();
    for id in tilesets {
        let path = &manifest.tilesets[id].path;
        match read_tileset(path) {
            Ok(tileset) => registry(world).insert_tileset_with_path(id, tileset, path.clone()),
            Err(e) => error(path, format!("tileset '{id}': {e}")),
        }
    }
    let mut rooms: Vec<&String> = manifest.rooms.keys().collect();
    rooms.sort();
    for id in rooms {
        let path = &manifest.rooms[id].path;
        let loaded = read_room(path).and_then(|room| {
            world
                .insert_room_with_path(id, room, path.clone())
                .map_err(|e| e.to_string())
        });
        if let Err(e) = loaded {
            error(path, format!("room '{id}': {e}"));
        }
    }
}

/// `tungsten check`'s level rules on every listed room, sorted: core's
/// (spawns, supports, links and budgets, the umbrella's `light` lighting an
/// object), then, with the kit, each route's reach; each finding sent to
/// `error` with its room's file.
pub(crate) fn check_room_rules(
    manifest: &ResolvedManifest,
    world: &World,
    mut error: impl FnMut(&Path, String),
) {
    let mut rooms: Vec<&String> = manifest.rooms.keys().collect();
    rooms.sort();
    for id in rooms {
        let path = &manifest.rooms[id].path;
        for finding in world.room_findings(id, &["light"]) {
            error(path, finding.to_string());
        }
        #[cfg(feature = "kit")]
        for finding in tungsten_kit::route_findings(world, id) {
            error(path, finding.to_string());
        }
    }
}

/// A room object's prefab or override error as the level rules print a
/// finding, `<room>: <id>: objects[<n>]: <message>`, so the check names
/// the object by its ID; any other error as ``room '<id>': <error>``.
pub(crate) fn object_finding(
    world: &World,
    room: &str,
    err: &tungsten_core::assets::PrefabError,
) -> String {
    let entry = world
        .get_resource::<RoomRegistry>()
        .and_then(|rooms| rooms.room(room))
        .and_then(|document| document.object_of(err));
    match entry {
        Some((index, object, message)) => {
            format!("{room}: {}: objects[{index}]: {message}", object.id)
        }
        None => format!("room '{room}': {err}"),
    }
}

/// The seeds `tungsten check` sweeps each room generator over (redo 7.12):
/// `count` of them from `first`, each `first.wrapping_add(i)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CheckSeeds {
    pub(crate) first: u64,
    pub(crate) count: u64,
}

impl CheckSeeds {
    /// A check's seeds when `TUNGSTEN_CHECK_SEEDS` is unset: `0+16`.
    pub(crate) const DEFAULT: Self = Self {
        first: 0,
        count: 16,
    };

    /// `TUNGSTEN_CHECK_SEEDS`'s form, `<first>+<count>`.
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        text.split_once('+')
            .and_then(|(first, count)| {
                Some(Self {
                    first: first.trim().parse().ok()?,
                    count: count.trim().parse().ok()?,
                })
            })
            .ok_or_else(|| {
                format!(
                    "TUNGSTEN_CHECK_SEEDS `{text}`: expected <first>+<count>, two whole numbers"
                )
            })
    }
}

/// `tungsten check`'s sweep of the room generators (redo 7.12): each
/// registered generator, sorted, at each of `seeds`, generates its rooms
/// under its own ID as the prefix (an error is a finding), checks each
/// room's objects against the prefabs, core's level rules and, with the
/// kit, each route's reach, then removes them, none spawned. A finding's
/// file is `generator <id>`, sent to `error` once with every seed that
/// gave its message, the first five then the count; each generator sends
/// `note` one line of what it swept.
pub(crate) fn check_generators(
    world: &mut World,
    seeds: CheckSeeds,
    mut error: impl FnMut(String, String),
    mut note: impl FnMut(String),
) {
    let Some(generators) = world.get_resource::<RoomGenerators>() else {
        return;
    };
    let ids: Vec<(String, u32)> = generators
        .ids()
        .filter_map(|id| Some((id.to_owned(), generators.get(id)?.version())))
        .collect();
    for (id, version) in ids {
        let mut found: Vec<(String, Vec<u64>)> = Vec::new();
        let mut at: BTreeMap<String, usize> = BTreeMap::new();
        let mut rooms = 0_u64;
        for i in 0..seeds.count {
            let seed = seeds.first.wrapping_add(i);
            let mut finding = |message: String| {
                if let Some(&index) = at.get(&message) {
                    found[index].1.push(seed);
                } else {
                    at.insert(message.clone(), found.len());
                    found.push((message, vec![seed]));
                }
            };
            let generated = match world.generate_rooms(&id, seed, &id) {
                Ok(generated) => generated,
                Err(err) => {
                    finding(without_seed(&err));
                    continue;
                }
            };
            for (room, _) in generated.rooms() {
                rooms += 1;
                let objects = world
                    .get_resource::<tungsten_core::TilemapRegistry>()
                    .and_then(|maps| maps.get(room))
                    .map(|map| world.check_map_objects(room, map));
                if let Some(Err(e)) = objects {
                    finding(object_finding(world, room, &e));
                }
                for rule in world.room_findings(room, &["light"]) {
                    finding(rule.to_string());
                }
                #[cfg(feature = "kit")]
                for rule in tungsten_kit::route_findings(world, room) {
                    finding(rule.to_string());
                }
            }
            for (room, _) in generated.rooms() {
                let removed = world.remove_room(room);
                debug_assert!(removed.is_ok(), "a swept room is spawned by none");
            }
        }
        for (message, seeds) in found {
            error(
                format!("generator {id}"),
                format!("{}: {message}", seed_list(&seeds)),
            );
        }
        note(format!(
            "generator `{id}` (version {version}): seeds {}+{}, {} rooms",
            seeds.first,
            seeds.count,
            thousands(rooms)
        ));
    }
}

/// The generation `tungsten level generate` asks the game's build for
/// (redo 7.12), `TUNGSTEN_GENERATE_RUN`'s `<generator>:<seed>:<prefix>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GenerateRequest {
    pub(crate) generator: String,
    pub(crate) seed: u64,
    pub(crate) prefix: String,
}

impl GenerateRequest {
    /// `<generator>:<seed>:<prefix>`, the seed a whole number.
    pub(crate) fn parse(text: &str) -> Result<Self, String> {
        let mut parts = text.splitn(3, ':');
        let parsed = match (parts.next(), parts.next(), parts.next()) {
            (Some(generator), Some(seed), Some(prefix))
                if !generator.is_empty() && !prefix.is_empty() =>
            {
                seed.trim().parse().ok().map(|seed| Self {
                    generator: generator.to_owned(),
                    seed,
                    prefix: prefix.to_owned(),
                })
            }
            _ => None,
        };
        parsed.ok_or_else(|| {
            format!(
                "TUNGSTEN_GENERATE_RUN `{text}`: expected <generator>:<seed>:<prefix>, the seed a whole number"
            )
        })
    }
}

/// The version of the report `tungsten level generate` reads.
pub(crate) const GENERATE_REPORT_VERSION: u32 = 1;

/// `tungsten level generate`'s report (redo 7.12), a JSON object at
/// `version` 1: `errors`, each `{file, message}` (the file relative to the
/// game's folder, empty for none), and with none the `generation` and its
/// `rooms`, each its `id`, its canonical `text`, `replaces`, the counts
/// `generated`, `kept` and `dropped` (the kept and dropped objects' IDs)
/// and `reshaped`, from [`World::author_rooms`] over `world` as `request`
/// asks. A load error, or a request that does not read, generates nothing.
pub(crate) fn generate_report(
    world: &World,
    errors: Vec<(String, String)>,
    request: Result<GenerateRequest, String>,
) -> Value {
    let error = |file: String, message: String| json!({ "file": file, "message": message });
    let mut errors: Vec<Value> = errors
        .into_iter()
        .map(|(file, message)| error(file, message))
        .collect();
    let mut report = json!({ "version": GENERATE_REPORT_VERSION });
    match request {
        Err(message) => errors.push(error(String::new(), message)),
        Ok(_) if !errors.is_empty() => {}
        Ok(request) => {
            match world.author_rooms(&request.generator, request.seed, &request.prefix) {
                Err(err) => errors.push(error(
                    format!("generator {}", request.generator),
                    err.to_string(),
                )),
                Ok(authored) => {
                    let rooms: Vec<Value> = (authored.rooms().iter())
                        .map(|room| {
                            json!({
                                "id": room.id,
                                "text": room.document.to_canonical(),
                                "replaces": room.replaces,
                                "generated": room.generated,
                                "kept": room.kept,
                                "dropped": room.dropped,
                                "reshaped": room.reshaped,
                            })
                        })
                        .collect();
                    report["generation"] = json!(authored.generation());
                    report["rooms"] = Value::Array(rooms);
                }
            }
        }
    }
    report["errors"] = Value::Array(errors);
    report
}

/// `err` without its generator and seed, so equal errors at two seeds
/// merge into one finding.
fn without_seed(err: &GenerateError) -> String {
    match err {
        GenerateError::Failed { message, .. } => message.clone(),
        GenerateError::Template {
            template, message, ..
        } => format!("template `{template}`: {message}"),
        GenerateError::Empty { .. } => "added no room".to_owned(),
        other => other.to_string(),
    }
}

/// `seed 3`, `seeds 3, 17 and 40`, or the first five and the count past
/// them: `seeds 3, 17, 40, 41, 66 and 12 more`.
fn seed_list(seeds: &[u64]) -> String {
    let shown: Vec<String> = seeds.iter().take(5).map(u64::to_string).collect();
    match (seeds.len(), shown.split_last()) {
        (1, _) => format!("seed {}", shown[0]),
        (n, Some(_)) if n > 5 => format!("seeds {} and {} more", shown.join(", "), n - 5),
        (_, Some((last, rest))) => format!("seeds {} and {last}", rest.join(", ")),
        (_, None) => String::new(),
    }
}

/// `n` with a comma every three digits: `1,500`.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// The room whose file is `path`, by the files the registry recorded.
pub(crate) fn room_for_path(world: &World, path: &Path) -> Option<String> {
    let rooms = world.get_resource::<RoomRegistry>()?;
    rooms
        .ids()
        .find(|id| rooms.path(id) == Some(path))
        .map(str::to_owned)
}

/// The tileset whose file is `path`, by the files the registry recorded.
pub(crate) fn tileset_for_path(world: &World, path: &Path) -> Option<String> {
    let rooms = world.get_resource::<RoomRegistry>()?;
    rooms
        .tileset_ids()
        .find(|id| rooms.tileset_path(id) == Some(path))
        .map(str::to_owned)
}

/// Room and tileset files read again (redo 7.12), a hot-reload batch's or
/// a manifest reload's (`what` begins each log line): each tileset patched
/// with the batch's rooms on it, then each other room alone
/// (`World::patch_tileset`, `World::patch_room`), so a tileset and a room
/// saved together that need each other load together. A file that fails to
/// read or to patch keeps the registry's entry and the live room, and its
/// line goes to `error`; a room left behind by its tileset's failure is
/// tried alone.
pub(crate) fn reload_room_files(
    tilesets: &[(String, PathBuf)],
    rooms: &[(String, PathBuf)],
    world: &mut World,
    what: &str,
    error: &mut impl FnMut(String),
) {
    let mut documents: BTreeMap<String, Room> = BTreeMap::new();
    for (id, path) in rooms {
        match read_room(path) {
            Ok(room) => {
                documents.insert(id.clone(), room);
            }
            Err(e) => error(format!("{what} room '{id}': {e}; keeping the live room")),
        }
    }
    for (id, path) in tilesets {
        let tileset = match read_tileset(path) {
            Ok(tileset) => tileset,
            Err(e) => {
                error(format!(
                    "{what} tileset '{id}': {e}; keeping the previous tileset"
                ));
                continue;
            }
        };
        let with: Vec<(String, Room)> = documents
            .iter()
            .filter(|(_, room)| room.tileset.as_deref() == Some(id.as_str()))
            .map(|(room, document)| (room.clone(), document.clone()))
            .collect();
        match world.patch_tileset(id, tileset, with.clone()) {
            Ok(patches) => {
                for (room, _) in &with {
                    documents.remove(room);
                }
                log::info!("{what}: tileset '{id}' read again");
                for patch in &patches {
                    log_patch(what, patch);
                }
            }
            Err(e) => error(format!(
                "{what} tileset '{id}': {e}; keeping the previous tileset and its rooms"
            )),
        }
    }
    for (id, document) in documents {
        match world.patch_room(&id, document) {
            Ok(patch) => log_patch(what, &patch),
            Err(e) => error(format!("{what} room '{id}': {e}; keeping the live room")),
        }
    }
}

/// A patch's log line: info when it changed the room, debug when not.
fn log_patch(what: &str, patch: &tungsten_core::assets::room::RoomPatch) {
    if patch.is_empty() {
        log::debug!("{what}: {patch}");
    } else {
        log::info!("{what}: {patch}");
    }
}

/// The manifest reload's warnings for what `held` registers and
/// `new_manifest` no longer lists: each tileset, and each room but a
/// generated one, which no manifest lists.
pub(crate) fn stale_warnings(held: &RoomRegistry, new_manifest: &ResolvedManifest) -> Vec<String> {
    let tilesets = held
        .tileset_ids()
        .filter(|id| !new_manifest.tilesets.contains_key(*id))
        .map(|id| format!("Manifest reload: tileset '{id}' removed — keeping stale"));
    let rooms = held
        .ids()
        .filter(|id| !new_manifest.rooms.contains_key(*id) && held.generation(id).is_none())
        .map(|id| format!("Manifest reload: room '{id}' removed — keeping stale"));
    tilesets.chain(rooms).collect()
}

/// The manifest reload's rooms and tilesets (redo 7.12), after its prefabs,
/// which their objects name: a new tileset registers; a new room inserts
/// with its map, its objects checked against the prefabs now loaded (a
/// failure logs while its tiles stay); every other listed tileset and room
/// is read again and patched as [`reload_room_files`] patches a batch, a
/// moved file's path recorded; one the manifest no longer lists warns and
/// stays, but for a generated room, which no manifest lists. A manifest
/// without rooms or tilesets, in a World that holds none, does nothing.
pub(crate) fn reload_rooms_from_manifest(new_manifest: &ResolvedManifest, world: &mut World) {
    if !has_rooms(new_manifest) && !world.has_resource::<RoomRegistry>() {
        return;
    }
    for line in stale_warnings(registry(world), new_manifest) {
        log::warn!("{line}");
    }
    let mut tilesets: Vec<(&String, &PathBuf)> = new_manifest
        .tilesets
        .iter()
        .map(|(id, entry)| (id, &entry.path))
        .collect();
    tilesets.sort();
    let mut kept_tilesets = Vec::new();
    for (id, path) in tilesets {
        let rooms = registry(world);
        if rooms.set_tileset_path(id, path.clone()) {
            kept_tilesets.push((id.clone(), path.clone()));
            continue;
        }
        match read_tileset(path) {
            Ok(tileset) => {
                rooms.insert_tileset_with_path(id, tileset, path.clone());
                log::info!("Manifest reload: loaded new tileset '{id}'");
            }
            Err(e) => log::error!("Manifest reload: new tileset '{id}': {e}"),
        }
    }
    let mut rooms: Vec<(&String, &PathBuf)> = new_manifest
        .rooms
        .iter()
        .map(|(id, entry)| (id, &entry.path))
        .collect();
    rooms.sort();
    let mut kept_rooms = Vec::new();
    for (id, path) in rooms {
        if registry(world).set_path(id, path.clone()) {
            kept_rooms.push((id.clone(), path.clone()));
            continue;
        }
        let inserted = read_room(path).and_then(|room| {
            world
                .insert_room_with_path(id, room, path.clone())
                .map_err(|e| e.to_string())
        });
        match inserted {
            Ok(()) => {
                log::info!("Manifest reload: loaded new room '{id}'");
                let checked = world
                    .get_resource::<tungsten_core::TilemapRegistry>()
                    .and_then(|maps| maps.get(id))
                    .map(|map| world.check_map_objects(id, map));
                if let Some(Err(e)) = checked {
                    log::error!("Manifest reload: new room '{id}' keeps its tiles, but {e}");
                }
            }
            Err(e) => log::error!("Manifest reload: new room '{id}': {e}"),
        }
    }
    reload_room_files(
        &kept_tilesets,
        &kept_rooms,
        world,
        "Manifest reload",
        &mut |line| log::error!("{line}"),
    );
}

#[cfg(test)]
#[path = "../tests/rooms.rs"]
pub(crate) mod tests;
