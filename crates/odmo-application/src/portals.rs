//! Portal catalog configured per server.
//!
//! Portal ids, their destinations and where they stand come from the client's map data,
//! which is not published, so servers record them and list them in
//! `data/server-assets/portals.json`:
//!
//! ```json
//! {
//!   "portals": [
//!     {
//!       "id": 10001,
//!       "is_local": false,
//!       "destination": {"map_id": 102, "x": 32615, "y": 14930},
//!       "source": {"map_id": 105, "x": 39400, "y": 30900}
//!     }
//!   ]
//! }
//! ```
//!
//! `source` is optional: with it a tamer must stand within
//! [`crate::game::PORTAL_USE_RANGE`] of the portal to use it, without it the portal is
//! not range checked. A portal listed here replaces the repository's portal with the
//! same id; portals not listed still come from the repository.
//!
//! To record a portal, walk into it with a game master account: the server logs every
//! warp request with the portal id and the tamer's map and position. The `!pos` chat
//! command shows the current position, for destinations.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, bail};
use serde::Deserialize;

use crate::game::{PortalDefinition, PortalSource};

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct Location {
    map_id: i16,
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortalEntry {
    id: i32,
    #[serde(default)]
    is_local: bool,
    destination: Location,
    #[serde(default)]
    source: Option<Location>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PortalFile {
    #[serde(default)]
    portals: Vec<PortalEntry>,
}

/// Portals configured for this server, by id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PortalCatalog {
    by_id: HashMap<i32, PortalDefinition>,
}

fn check_location(index: usize, field: &str, location: &Location) -> anyhow::Result<()> {
    if location.map_id <= 0 {
        bail!(
            "portals[{index}].{field}: map_id must be positive, got {}",
            location.map_id
        );
    }
    if location.x < 0 || location.y < 0 {
        bail!(
            "portals[{index}].{field}: x and y must not be negative, got ({}, {})",
            location.x,
            location.y
        );
    }
    Ok(())
}

impl PortalCatalog {
    /// Parse and validate `portals.json`.
    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        let file: PortalFile = serde_json::from_str(json).context("invalid portals")?;
        let mut by_id = HashMap::new();
        for (index, entry) in file.portals.iter().enumerate() {
            if entry.id <= 0 {
                bail!("portals[{index}]: id must be positive, got {}", entry.id);
            }
            check_location(index, "destination", &entry.destination)?;
            if let Some(source) = &entry.source {
                check_location(index, "source", source)?;
                if entry.is_local && source.map_id != entry.destination.map_id {
                    bail!(
                        "portals[{index}]: a local portal must lead to its own map ({} -> {})",
                        source.map_id,
                        entry.destination.map_id
                    );
                }
            }
            let portal = PortalDefinition {
                id: entry.id,
                is_local: entry.is_local,
                destination_map_id: entry.destination.map_id,
                destination_x: entry.destination.x,
                destination_y: entry.destination.y,
                source: entry.source.map(|source| PortalSource {
                    map_id: source.map_id,
                    x: source.x,
                    y: source.y,
                }),
            };
            if by_id.insert(entry.id, portal).is_some() {
                bail!("portals[{index}]: id {} is listed twice", entry.id);
            }
        }
        Ok(Self { by_id })
    }

    /// Load `portals.json` from a server-assets directory; a missing file means every
    /// portal comes from the repository.
    pub fn load_dir(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join("portals.json");
        if !path.exists() {
            return Ok(Self::default());
        }
        let json = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        Self::from_json(&json).with_context(|| format!("in {}", path.display()))
    }

    pub fn get(&self, id: i32) -> Option<&PortalDefinition> {
        self.by_id.get(&id)
    }

    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// How many configured portals have a known source location.
    pub fn with_source(&self) -> usize {
        self.by_id
            .values()
            .filter(|portal| portal.source.is_some())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_portals_with_and_without_a_source() {
        let catalog = PortalCatalog::from_json(
            r#"{"portals": [
                {"id": 10001, "destination": {"map_id": 102, "x": 32615, "y": 14930},
                 "source": {"map_id": 105, "x": 39400, "y": 30900}},
                {"id": 10002, "is_local": true, "destination": {"map_id": 3, "x": 1, "y": 2}}
            ]}"#,
        )
        .expect("catalog");
        assert_eq!(catalog.len(), 2);
        assert_eq!(catalog.with_source(), 1);
        let first = catalog.get(10001).expect("10001");
        assert!(!first.is_local);
        assert_eq!(
            (
                first.destination_map_id,
                first.destination_x,
                first.destination_y
            ),
            (102, 32615, 14930)
        );
        assert_eq!(
            first.source,
            Some(PortalSource {
                map_id: 105,
                x: 39400,
                y: 30900
            })
        );
        let second = catalog.get(10002).expect("10002");
        assert!(second.is_local);
        assert_eq!(second.source, None);
        assert!(catalog.get(1).is_none());
    }

    #[test]
    fn rejects_invalid_portals() {
        for bad in [
            r#"{"portals": [{"id": 0, "destination": {"map_id": 1, "x": 0, "y": 0}}]}"#,
            r#"{"portals": [{"id": 1, "destination": {"map_id": 0, "x": 0, "y": 0}}]}"#,
            r#"{"portals": [{"id": 1, "destination": {"map_id": 1, "x": -1, "y": 0}}]}"#,
            r#"{"portals": [{"id": 1, "destination": {"map_id": 1, "x": 0, "y": 0},
                             "source": {"map_id": 1, "x": 0, "y": -5}}]}"#,
            r#"{"portals": [{"id": 1, "is_local": true, "destination": {"map_id": 1, "x": 0, "y": 0},
                             "source": {"map_id": 2, "x": 0, "y": 0}}]}"#,
            r#"{"portals": [{"id": 1, "destination": {"map_id": 1, "x": 0, "y": 0}},
                            {"id": 1, "destination": {"map_id": 2, "x": 0, "y": 0}}]}"#,
            r#"{"portals": [{"id": 1, "target": {"map_id": 1, "x": 0, "y": 0}}]}"#,
            r#"{"gates": []}"#,
        ] {
            assert!(PortalCatalog::from_json(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn missing_file_means_no_configured_portals() {
        let dir = std::env::temp_dir().join("odmo-no-portals-dir");
        let catalog = PortalCatalog::load_dir(&dir).expect("missing file is fine");
        assert!(catalog.is_empty());
    }
}
