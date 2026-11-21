//! NPC locations configured per server.
//!
//! Where NPCs stand comes from the client's map data, which is not published, so servers
//! record their NPCs and list them in `data/server-assets/npcs.json`:
//!
//! ```json
//! {"npcs": [{"npc_id": 1001, "map_id": 105, "x": 39500, "y": 31000}]}
//! ```
//!
//! A tamer must be on the NPC's map and within [`NPC_USE_RANGE`] of it for any request
//! made at the NPC: shop purchases and sales, hatching, item sockets, quest lists, rare
//! machines, guild creation, spirit exchanges and archive swaps. NPCs not listed are not
//! range checked, so a request naming an unlisted NPC id is not stopped by this check.
//!
//! Strict mode closes that gap once every NPC is recorded: with `"strict": true` a request
//! naming an NPC that is not listed is refused. Ids under `anywhere` are allowed from
//! any position, for features the client offers without an NPC (such as portable items),
//! once the id the client sends for them is known:
//!
//! ```json
//! {"strict": true, "anywhere": [0], "npcs": [{"npc_id": 1001, "map_id": 105, "x": 39500, "y": 31000}]}
//! ```
//!
//! To record an NPC, use it with a game master account: the server logs every request
//! made at an NPC with the NPC id and the tamer's map and position. Standing next to the
//! NPC, `!pos` shows the same position.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use anyhow::{Context, bail};
use serde::Deserialize;

/// Farthest a tamer may stand from a listed NPC and still use it. Loose until checked
/// against a real client: covers the click distance at which the client opens an NPC.
pub const NPC_USE_RANGE: i64 = 1_500;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct NpcEntry {
    npc_id: i32,
    map_id: i16,
    x: i32,
    y: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NpcFile {
    #[serde(default)]
    strict: bool,
    #[serde(default)]
    anywhere: Vec<i32>,
    #[serde(default)]
    npcs: Vec<NpcEntry>,
}

/// Where a listed NPC stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcLocation {
    pub map_id: i16,
    pub x: i32,
    pub y: i32,
}

/// Listed NPCs, by NPC id and map (the same NPC id can stand on several maps).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NpcCatalog {
    by_npc: HashMap<i32, Vec<NpcLocation>>,
    /// Refuse requests naming an NPC that is neither listed nor usable anywhere.
    strict: bool,
    /// NPC ids usable from any position.
    anywhere: HashSet<i32>,
}

impl NpcCatalog {
    /// Parse and validate `npcs.json`.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let file: NpcFile = serde_json::from_str(json).context("invalid npcs")?;
        let mut by_npc: HashMap<i32, Vec<NpcLocation>> = HashMap::new();
        for (index, entry) in file.npcs.iter().enumerate() {
            if entry.npc_id <= 0 {
                bail!(
                    "npcs[{index}]: npc_id must be positive, got {}",
                    entry.npc_id
                );
            }
            if entry.map_id <= 0 {
                bail!(
                    "npcs[{index}]: map_id must be positive, got {}",
                    entry.map_id
                );
            }
            if entry.x < 0 || entry.y < 0 {
                bail!(
                    "npcs[{index}]: x and y must not be negative, got ({}, {})",
                    entry.x,
                    entry.y
                );
            }
            let locations = by_npc.entry(entry.npc_id).or_default();
            if locations
                .iter()
                .any(|location| location.map_id == entry.map_id)
            {
                bail!(
                    "npcs[{index}]: npc {} is listed twice on map {}",
                    entry.npc_id,
                    entry.map_id
                );
            }
            locations.push(NpcLocation {
                map_id: entry.map_id,
                x: entry.x,
                y: entry.y,
            });
        }
        let mut anywhere = HashSet::new();
        for (index, npc_id) in file.anywhere.iter().enumerate() {
            if *npc_id < 0 {
                bail!("anywhere[{index}]: npc id must not be negative, got {npc_id}");
            }
            if by_npc.contains_key(npc_id) {
                bail!("anywhere[{index}]: npc {npc_id} is also listed with a location");
            }
            if !anywhere.insert(*npc_id) {
                bail!("anywhere[{index}]: npc {npc_id} is listed twice");
            }
        }
        Ok(Self {
            by_npc,
            strict: file.strict,
            anywhere,
        })
    }

    /// Load `npcs.json` from a server-assets directory; a missing file means no NPC is
    /// range checked.
    pub fn load_dir(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join("npcs.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Self::from_json(&json).with_context(|| format!("in {}", path.display()))
    }

    pub fn len(&self) -> usize {
        self.by_npc.values().map(Vec::len).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.by_npc.is_empty() && !self.strict && self.anywhere.is_empty()
    }

    /// Whether unlisted NPCs are refused.
    pub fn is_strict(&self) -> bool {
        self.strict
    }

    /// Whether `npc_id` is listed with a location or usable from anywhere.
    pub fn is_listed(&self, npc_id: i32) -> bool {
        self.by_npc.contains_key(&npc_id) || self.anywhere.contains(&npc_id)
    }

    /// Whether a tamer on `map_id` at (`x`, `y`) may use NPC `npc_id`. A listed NPC must
    /// stand on the tamer's map within range; ids under `anywhere` are usable from any
    /// position; other NPCs are refused in strict mode and not checked otherwise.
    pub fn usable_from(&self, npc_id: i32, map_id: i16, x: i32, y: i32) -> bool {
        if self.anywhere.contains(&npc_id) {
            return true;
        }
        let Some(locations) = self.by_npc.get(&npc_id) else {
            return !self.strict;
        };
        locations.iter().any(|location| {
            location.map_id == map_id && distance(location.x, location.y, x, y) <= NPC_USE_RANGE
        })
    }
}

fn distance(xa: i32, ya: i32, xb: i32, yb: i32) -> i64 {
    let dx = i64::from(xb) - i64::from(xa);
    let dy = i64::from(yb) - i64::from(ya);
    ((dx * dx + dy * dy) as f64).sqrt() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_npcs_are_usable_only_nearby_on_their_map() {
        let catalog = NpcCatalog::from_json(
            r#"{"npcs": [
                {"npc_id": 1001, "map_id": 105, "x": 1000, "y": 1000},
                {"npc_id": 1001, "map_id": 3, "x": 50, "y": 50}
            ]}"#,
        )
        .expect("catalog");
        assert_eq!(catalog.len(), 2);
        assert!(catalog.usable_from(1001, 105, 1000, 1000));
        assert!(catalog.usable_from(1001, 105, 1000 + NPC_USE_RANGE as i32, 1000));
        assert!(!catalog.usable_from(1001, 105, 1000 + NPC_USE_RANGE as i32 + 1, 1000));
        assert!(catalog.usable_from(1001, 3, 60, 60));
        assert!(!catalog.usable_from(1001, 7, 1000, 1000), "another map");
        assert!(catalog.usable_from(2002, 7, 90_000, 90_000), "unlisted npc");
    }

    #[test]
    fn strict_mode_refuses_unlisted_npcs_but_not_anywhere_ids() {
        let catalog = NpcCatalog::from_json(
            r#"{"strict": true, "anywhere": [0, 90001],
                "npcs": [{"npc_id": 1001, "map_id": 105, "x": 1000, "y": 1000}]}"#,
        )
        .expect("catalog");
        assert!(catalog.is_strict());
        assert!(catalog.usable_from(1001, 105, 1000, 1000));
        assert!(
            !catalog.usable_from(1001, 105, 90_000, 1000),
            "still range checked"
        );
        assert!(
            !catalog.usable_from(2002, 105, 1000, 1000),
            "unlisted npc refused"
        );
        assert!(catalog.usable_from(0, 7, 90_000, 90_000), "anywhere id");
        assert!(catalog.usable_from(90001, 105, 1, 1), "anywhere id");

        // Without strict mode an unlisted npc is not checked.
        let lenient = NpcCatalog::from_json(
            r#"{"npcs": [{"npc_id": 1001, "map_id": 105, "x": 1000, "y": 1000}]}"#,
        )
        .expect("catalog");
        assert!(!lenient.is_strict());
        assert!(lenient.usable_from(2002, 105, 1000, 1000));

        // Strict mode alone, with no npc listed yet, refuses every npc request.
        let only_strict = NpcCatalog::from_json(r#"{"strict": true}"#).expect("catalog");
        assert!(!only_strict.is_empty());
        assert!(!only_strict.usable_from(1001, 105, 0, 0));
    }

    #[test]
    fn rejects_invalid_npcs() {
        for bad in [
            r#"{"npcs": [{"npc_id": 0, "map_id": 1, "x": 0, "y": 0}]}"#,
            r#"{"npcs": [{"npc_id": 1, "map_id": 0, "x": 0, "y": 0}]}"#,
            r#"{"npcs": [{"npc_id": 1, "map_id": 1, "x": -1, "y": 0}]}"#,
            r#"{"npcs": [{"npc_id": 1, "map_id": 1, "x": 0, "y": 0},
                         {"npc_id": 1, "map_id": 1, "x": 5, "y": 5}]}"#,
            r#"{"npcs": [{"npc_id": 1, "map": 1, "x": 0, "y": 0}]}"#,
            r#"{"shops": []}"#,
            r#"{"strict": "yes"}"#,
            r#"{"anywhere": [-1]}"#,
            r#"{"anywhere": [5, 5]}"#,
            r#"{"anywhere": [1001], "npcs": [{"npc_id": 1001, "map_id": 1, "x": 0, "y": 0}]}"#,
        ] {
            assert!(NpcCatalog::from_json(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn missing_file_means_no_listed_npcs() {
        let dir = std::env::temp_dir().join("odmo-no-npcs-dir");
        let catalog = NpcCatalog::load_dir(&dir).expect("missing file is fine");
        assert!(catalog.is_empty());
        assert!(catalog.usable_from(1001, 105, 0, 0));
    }
}
