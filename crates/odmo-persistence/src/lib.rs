pub mod pg;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Context;
use odmo_application::{
    account::AccountRepository,
    character::{CharacterAccountRepository, CharacterRepository},
    game::{
        DigiCombineRepository, DigiSummonRepository, DigimonAssetRepository, DropCollectionResult,
        EvolutionAssetRepository, ExtraEvolutionRepository, GameRepository, ItemAssetRepository,
        MapDropRepository, MapMobRepository, NpcShopDefinition, NpcShopItem, NpcShopRepository,
        PortalDefinition, PortalRepository, RandomBoxRepository, UnionCombineRepository,
    },
};
use odmo_types::{
    AccessLevel, Account, AccountId, AccountSuspension, CharacterSummary, CombineCeilingEntry,
    DEFAULT_GM_PARTNER_MODEL_ID, DEFAULT_GM_TAMER_MODEL_ID, DEFAULT_PARTNER_MODEL_ID,
    DEFAULT_START_MAP_ID, DEFAULT_START_X, DEFAULT_START_Y, DEFAULT_TAMER_MODEL_ID,
    DetailEnchantEntry, DetailSkillEntry, DigiCombineCatalog, DigiCombineCeil, DigiCombineGroup,
    DigiCombineItem, DigiCombineRank, DigiCombineReward, DigiSummonProduct, DigiSummonReward,
    DigiSummonTicket, DigimonAsset, DropSummary, EvolutionAsset, ExtraEvolutionNpc, ItemAsset,
    ItemRecord, MobSummary, PartnerSlotSnapshot, RandomBoxReward, ServerDescriptor,
    UnionCombineCatalog,
};
use serde::{Deserialize, Serialize};

fn map_key(map_id: i16, channel: u8) -> String {
    format!("{map_id}:{channel}")
}

const EVOLUTION_ASSET_CATALOG_PATH: &str = "data/server-assets/evolution_assets.json";
const ITEM_ASSET_CATALOG_PATH: &str = "data/server-assets/item_assets.json";
const DIGIMON_ASSET_CATALOG_PATH: &str = "data/server-assets/digimon_assets.json";
pub const DEMO_CATALOG_ITEM_A: i32 = 3;
pub const DEMO_CATALOG_ITEM_B: i32 = 4;
pub const DEMO_CATALOG_ITEM_C: i32 = 5;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
        })
}

fn workspace_path(relative_path: &str) -> PathBuf {
    workspace_root().join(relative_path)
}

fn read_json_catalog<T>(relative_path: &str) -> anyhow::Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let path = workspace_path(relative_path);
    let payload =
        fs::read(&path).with_context(|| format!("failed to read catalog '{}'", path.display()))?;
    serde_json::from_slice(&payload)
        .with_context(|| format!("failed to parse catalog '{}'", path.display()))
}

pub(crate) fn load_evolution_asset_catalog() -> anyhow::Result<Vec<EvolutionAsset>> {
    read_json_catalog(EVOLUTION_ASSET_CATALOG_PATH)
}

pub(crate) fn load_item_asset_catalog() -> anyhow::Result<Vec<ItemAsset>> {
    read_json_catalog(ITEM_ASSET_CATALOG_PATH)
}

pub(crate) fn load_digimon_asset_catalog() -> anyhow::Result<Vec<DigimonAsset>> {
    read_json_catalog(DIGIMON_ASSET_CATALOG_PATH)
}

fn active_partner_snapshot(character: &CharacterSummary) -> PartnerSlotSnapshot {
    PartnerSlotSnapshot {
        slot: character.partner_current_slot,
        digimon_type: character.partner_current_type,
        model: character.partner_model,
        level: character.partner_level,
        name: character.partner_name.clone(),
        size: character.partner_size,
        hatch_grade: character.partner_hatch_grade,
        hp: character.partner_hp,
        ds: character.partner_ds,
        current_hp: character.partner_current_hp,
        current_ds: character.partner_current_ds,
        de: character.partner_de,
        at: character.partner_at,
        fs: character.partner_fs,
        ev: character.partner_ev,
        cc: character.partner_cc,
        ms: character.partner_ms,
        as_value: character.partner_as,
        ht: character.partner_ht,
        ar: character.partner_ar,
        bl: character.partner_bl,
        clone_level: character.partner_clone_level,
        clone_at_value: character.partner_clone_at_value,
        clone_bl_value: character.partner_clone_bl_value,
        clone_ct_value: character.partner_clone_ct_value,
        clone_ev_value: character.partner_clone_ev_value,
        clone_hp_value: character.partner_clone_hp_value,
        clone_at_level: character.partner_clone_at_level,
        clone_bl_level: character.partner_clone_bl_level,
        clone_ct_level: character.partner_clone_ct_level,
        clone_ev_level: character.partner_clone_ev_level,
        clone_hp_level: character.partner_clone_hp_level,
        active_buffs: character.partner_active_buffs.clone(),
        evolutions: character
            .partner_slots
            .iter()
            .find(|slot| slot.slot == character.partner_current_slot)
            .map(|slot| slot.evolutions.clone())
            .unwrap_or_default(),
        detail_skills: active_partner_detail_skills(character),
        detail_enchants: active_partner_detail_enchants(character),
    }
}

/// Digiclone attribute rows for the partner-detail enchant grid, in the fixed
/// order the detail view renders them: AT, CT, BL, HP, EV.
fn active_partner_detail_enchants(character: &CharacterSummary) -> Vec<DetailEnchantEntry> {
    vec![
        DetailEnchantEntry {
            kind: 0,
            value: i32::from(character.partner_clone_at_value),
        },
        DetailEnchantEntry {
            kind: 1,
            value: i32::from(character.partner_clone_ct_value),
        },
        DetailEnchantEntry {
            kind: 2,
            value: i32::from(character.partner_clone_bl_value),
        },
        DetailEnchantEntry {
            kind: 3,
            value: i32::from(character.partner_clone_hp_value),
        },
        DetailEnchantEntry {
            kind: 4,
            value: i32::from(character.partner_clone_ev_value),
        },
    ]
}

/// Skill rows for the active evolution, shown in the partner-detail skill grid.
/// Level comes from the evolution's per-slot skill levels; the slot index acts
/// as the skill id until the skill-asset table is wired in.
fn active_partner_detail_skills(character: &CharacterSummary) -> Vec<DetailSkillEntry> {
    character
        .partner_slots
        .iter()
        .find(|slot| slot.slot == character.partner_current_slot)
        .and_then(|slot| {
            slot.evolutions
                .iter()
                .find(|evo| evo.evolution_type == character.partner_current_type)
                .or_else(|| slot.evolutions.first())
        })
        .map(|evo| {
            evo.skill_levels
                .iter()
                .enumerate()
                .map(|(idx, &level)| DetailSkillEntry {
                    skill_id: idx as i32,
                    value: i32::from(level),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn apply_partner_snapshot(character: &mut CharacterSummary, partner: &PartnerSlotSnapshot) {
    character.partner_current_type = partner.digimon_type;
    character.partner_model = partner.model;
    character.partner_level = partner.level;
    character.partner_name = partner.name.clone();
    character.partner_size = partner.size;
    character.partner_hatch_grade = partner.hatch_grade;
    character.partner_hp = partner.hp;
    character.partner_ds = partner.ds;
    character.partner_current_hp = partner.current_hp;
    character.partner_current_ds = partner.current_ds;
    character.partner_de = partner.de;
    character.partner_at = partner.at;
    character.partner_fs = partner.fs;
    character.partner_ev = partner.ev;
    character.partner_cc = partner.cc;
    character.partner_ms = partner.ms;
    character.partner_as = partner.as_value;
    character.partner_ht = partner.ht;
    character.partner_ar = partner.ar;
    character.partner_bl = partner.bl;
    character.partner_clone_level = partner.clone_level;
    character.partner_clone_at_value = partner.clone_at_value;
    character.partner_clone_bl_value = partner.clone_bl_value;
    character.partner_clone_ct_value = partner.clone_ct_value;
    character.partner_clone_ev_value = partner.clone_ev_value;
    character.partner_clone_hp_value = partner.clone_hp_value;
    character.partner_clone_at_level = partner.clone_at_level;
    character.partner_clone_bl_level = partner.clone_bl_level;
    character.partner_clone_ct_level = partner.clone_ct_level;
    character.partner_clone_ev_level = partner.clone_ev_level;
    character.partner_clone_hp_level = partner.clone_hp_level;
    character.partner_active_buffs = partner.active_buffs.clone();
    if let Some(current_slot) = character
        .partner_slots
        .iter_mut()
        .find(|slot| slot.slot == partner.slot)
    {
        current_slot.evolutions = partner.evolutions.clone();
    }
}

/// Persistence backend selection.
pub enum PersistenceBackend {
    /// PostgreSQL — canonical production backend.
    Pg(Arc<pg::PgRepository>),
    /// JSON file — development/testing only. Requires ODMO_DEV_MODE=1.
    Json(Arc<JsonRepository>),
}

/// Initialize the persistence backend.
///
/// Priority:
/// 1. `ODMO_DATABASE_URL` → PostgreSQL (production)
/// 2. `ODMO_DEV_MODE=1` → JSON file (development)
/// 3. Neither set → error
///
/// For PostgreSQL, runs migrations and prepares the server-owned baseline
/// catalogs automatically. Demo world/account seeds are opt-in via
/// `ODMO_SEED_DEMO=1`.
pub async fn initialize_backend() -> anyhow::Result<PersistenceBackend> {
    if let Ok(database_url) = std::env::var("ODMO_DATABASE_URL") {
        tracing::info!("using PostgreSQL persistence");
        let pg = pg::PgRepository::open(&database_url)
            .await
            .context("failed to connect to PostgreSQL")?;
        pg.migrate().await.context("failed to run migrations")?;
        pg.prepare_runtime()
            .await
            .context("failed to prepare PostgreSQL runtime state")?;
        if demo_seed_enabled() {
            pg.seed_demo_world()
                .await
                .context("failed to seed PostgreSQL demo data")?;
        }
        Ok(PersistenceBackend::Pg(Arc::new(pg)))
    } else if std::env::var("ODMO_DEV_MODE").is_ok() {
        tracing::warn!("using JSON file persistence (dev mode — not for production)");
        let repository_path = std::env::var("ODMO_REPOSITORY_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir().join("odmo-data").join("world.json"));
        let repo = JsonRepository::open_or_create(repository_path)
            .context("failed to initialize JSON repository")?;
        Ok(PersistenceBackend::Json(Arc::new(repo)))
    } else {
        anyhow::bail!(
            "no persistence backend configured. \
             Set ODMO_DATABASE_URL for PostgreSQL (production) \
             or ODMO_DEV_MODE=1 for JSON file (development)."
        )
    }
}

fn demo_seed_enabled() -> bool {
    std::env::var("ODMO_SEED_DEMO")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

impl PersistenceBackend {
    /// Extract an `Arc<dyn AccountRepository>` from the backend.
    pub fn account_repository(self: &Arc<Self>) -> Arc<dyn AccountRepository> {
        match self.as_ref() {
            PersistenceBackend::Pg(pg) => pg.clone() as Arc<dyn AccountRepository>,
            PersistenceBackend::Json(json) => json.clone() as Arc<dyn AccountRepository>,
        }
    }

    /// Extract an `Arc<dyn CharacterRepository>` from the backend.
    pub fn character_repository(self: &Arc<Self>) -> Arc<dyn CharacterRepository> {
        match self.as_ref() {
            PersistenceBackend::Pg(pg) => pg.clone() as Arc<dyn CharacterRepository>,
            PersistenceBackend::Json(json) => json.clone() as Arc<dyn CharacterRepository>,
        }
    }

    /// Extract an `Arc<dyn CharacterAccountRepository>` from the backend.
    pub fn character_account_repository(self: &Arc<Self>) -> Arc<dyn CharacterAccountRepository> {
        match self.as_ref() {
            PersistenceBackend::Pg(pg) => pg.clone() as Arc<dyn CharacterAccountRepository>,
            PersistenceBackend::Json(json) => json.clone() as Arc<dyn CharacterAccountRepository>,
        }
    }

    /// Extract an `Arc<dyn GameRepository>` from the backend.
    pub fn game_repository(self: &Arc<Self>) -> Arc<dyn GameRepository> {
        match self.as_ref() {
            PersistenceBackend::Pg(pg) => pg.clone() as Arc<dyn GameRepository>,
            PersistenceBackend::Json(json) => json.clone() as Arc<dyn GameRepository>,
        }
    }
}

#[derive(Debug)]
pub struct JsonRepository {
    path: PathBuf,
    state: RwLock<WorldSnapshot>,
}

impl JsonRepository {
    pub fn open_or_create(path: PathBuf) -> anyhow::Result<Self> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create repository directory {}", parent.display())
            })?;
        }

        let mut snapshot = if path.exists() {
            let payload = fs::read(&path)
                .with_context(|| format!("failed to read repository {}", path.display()))?;
            serde_json::from_slice(&payload)
                .with_context(|| format!("failed to decode repository {}", path.display()))?
        } else {
            let seed = WorldSnapshot::demo();
            fs::write(&path, serde_json::to_vec_pretty(&seed)?)
                .with_context(|| format!("failed to seed repository {}", path.display()))?;
            seed
        };

        if normalize_legacy_snapshot(&mut snapshot) {
            fs::write(&path, serde_json::to_vec_pretty(&snapshot)?).with_context(|| {
                format!("failed to persist normalized repository {}", path.display())
            })?;
        }

        Ok(Self {
            path,
            state: RwLock::new(snapshot),
        })
    }

    fn persist(&self, snapshot: &WorldSnapshot) -> anyhow::Result<()> {
        let payload = serde_json::to_vec_pretty(snapshot)?;
        fs::write(&self.path, payload)
            .with_context(|| format!("failed to persist repository {}", self.path.display()))?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Apply a mutation to the character with the given id and persist the world.
    /// Returns Ok(()) silently if the character is not found, mirroring the legacy
    /// repository behavior.
    fn mutate_character<F>(&self, character_id: u64, f: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut CharacterSummary),
    {
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(character) = characters.iter_mut().find(|c| c.id == character_id) {
                f(character);
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }
}

impl AccountRepository for JsonRepository {
    fn account_by_username(&self, username: &str) -> anyhow::Result<Option<Account>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state.accounts.get(username).cloned())
    }

    fn account_by_id(&self, account_id: AccountId) -> anyhow::Result<Option<Account>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state
            .accounts
            .values()
            .find(|account| account.id == account_id)
            .cloned())
    }

    fn update_secondary_password(
        &self,
        account_id: AccountId,
        password: String,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("repository poisoned");
        if let Some(account) = state
            .accounts
            .values_mut()
            .find(|account| account.id == account_id)
        {
            account.secondary_password = Some(password);
        }
        self.persist(&state)
    }

    fn list_servers(&self) -> anyhow::Result<Vec<ServerDescriptor>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state.servers.clone())
    }

    fn resource_hash_hex(&self) -> anyhow::Result<Option<String>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state.resource_hash_hex.clone())
    }
}

impl CharacterRepository for JsonRepository {
    fn list_characters_by_account(
        &self,
        account_id: AccountId,
    ) -> anyhow::Result<Vec<CharacterSummary>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state
            .characters_by_account
            .get(&account_id)
            .cloned()
            .unwrap_or_default())
    }

    fn character_by_slot(
        &self,
        account_id: AccountId,
        slot: u8,
    ) -> anyhow::Result<Option<CharacterSummary>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state
            .characters_by_account
            .get(&account_id)
            .and_then(|characters| characters.iter().find(|character| character.slot == slot))
            .cloned())
    }

    fn character_by_id(&self, character_id: u64) -> anyhow::Result<Option<CharacterSummary>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state
            .characters_by_account
            .values()
            .flatten()
            .find(|character| character.id == character_id)
            .cloned())
    }

    fn character_by_name(&self, name: &str) -> anyhow::Result<Option<CharacterSummary>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state
            .characters_by_account
            .values()
            .flat_map(|characters| characters.iter())
            .find(|character| character.name.eq_ignore_ascii_case(name))
            .cloned())
    }

    fn is_name_available(&self, name: &str) -> anyhow::Result<bool> {
        let state = self.state.read().expect("repository poisoned");
        Ok(!state
            .characters_by_account
            .values()
            .flatten()
            .any(|character| character.name.eq_ignore_ascii_case(name)))
    }

    fn create_character(
        &self,
        account_id: AccountId,
        slot: u8,
        tamer_name: String,
        tamer_model: i32,
        partner_name: String,
        partner_model: i32,
    ) -> anyhow::Result<CharacterSummary> {
        let mut state = self.state.write().expect("repository poisoned");
        let next_id = state
            .characters_by_account
            .values()
            .flatten()
            .map(|character| character.id)
            .max()
            .unwrap_or(0)
            + 1;

        let character = CharacterSummary {
            id: next_id,
            account_id,
            slot,
            name: tamer_name,
            partner_current_slot: 1,
            partner_current_type: partner_model,
            partner_model,
            partner_name: partner_name.clone(),
            partner_slots: vec![PartnerSlotSnapshot {
                slot: 1,
                digimon_type: partner_model,
                model: partner_model,
                name: partner_name,
                ..PartnerSlotSnapshot::default()
            }],
            general_handler: next_id as u32 + 10_000,
            partner_handler: next_id as u32 + 20_000,
            model: tamer_model,
            ..CharacterSummary::default()
        };

        state
            .characters_by_account
            .entry(account_id)
            .or_default()
            .retain(|existing| existing.slot != slot);
        state
            .characters_by_account
            .entry(account_id)
            .or_default()
            .push(character.clone());
        state
            .characters_by_account
            .get_mut(&account_id)
            .expect("character list must exist")
            .sort_by_key(|entry| entry.slot);
        self.persist(&state)?;
        Ok(character)
    }

    fn delete_character(&self, account_id: AccountId, slot: u8) -> anyhow::Result<bool> {
        let mut state = self.state.write().expect("repository poisoned");
        let Some(characters) = state.characters_by_account.get_mut(&account_id) else {
            return Ok(false);
        };

        let original_len = characters.len();
        characters.retain(|character| character.slot != slot);
        let deleted = characters.len() != original_len;
        if deleted {
            self.persist(&state)?;
        }
        Ok(deleted)
    }

    fn update_character_position(
        &self,
        character_id: u64,
        x: i32,
        y: i32,
        z: f32,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                ch.x = x;
                ch.y = y;
                ch.z = z;
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn update_partner_position(
        &self,
        character_id: u64,
        x: i32,
        y: i32,
        z: f32,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                ch.partner_x = x;
                ch.partner_y = y;
                ch.partner_z = z;
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn switch_partner(
        &self,
        character_id: u64,
        slot: u8,
    ) -> anyhow::Result<Option<CharacterSummary>> {
        let mut state = self.state.write().expect("repository poisoned");
        let mut updated_character = None;
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                let current_slot = ch.partner_current_slot;
                if current_slot == slot {
                    updated_character = Some(ch.clone());
                    break;
                }

                if let Some(current_index) = ch
                    .partner_slots
                    .iter()
                    .position(|partner| partner.slot == current_slot)
                {
                    ch.partner_slots[current_index] = active_partner_snapshot(ch);
                }

                let Some(target_partner) = ch
                    .partner_slots
                    .iter()
                    .find(|partner| partner.slot == slot)
                    .cloned()
                else {
                    return Ok(None);
                };

                apply_partner_snapshot(ch, &target_partner);
                ch.partner_current_slot = slot;
                updated_character = Some(ch.clone());
                break;
            }
        }
        if updated_character.is_some() {
            self.persist(&state)?;
        }
        Ok(updated_character)
    }

    fn update_character_map(
        &self,
        character_id: u64,
        map_id: i16,
        x: i32,
        y: i32,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                ch.map_id = map_id;
                ch.x = x;
                ch.y = y;
                ch.partner_x = x;
                ch.partner_y = y;
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn update_inventory(
        &self,
        character_id: u64,
        inventory: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                ch.inventory = inventory;
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn update_equipment(&self, character_id: u64, equipment: Vec<u8>) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                ch.equipment = equipment;
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn update_digivice(
        &self,
        character_id: u64,
        digivice: odmo_types::DigiviceSnapshot,
    ) -> anyhow::Result<()> {
        let mut digivice = digivice;
        digivice.normalize();
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                ch.digivice = digivice;
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn update_extra_inventory(
        &self,
        character_id: u64,
        extra_inventory: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                ch.extra_inventory = extra_inventory;
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn update_warehouse(
        &self,
        character_id: u64,
        warehouse: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                ch.warehouse = warehouse;
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn update_account_warehouse(
        &self,
        character_id: u64,
        account_warehouse: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        let mut state = self.state.write().expect("repository poisoned");
        for characters in state.characters_by_account.values_mut() {
            if let Some(ch) = characters.iter_mut().find(|c| c.id == character_id) {
                ch.account_warehouse = Some(account_warehouse);
                self.persist(&state)?;
                return Ok(());
            }
        }
        Ok(())
    }

    fn update_character_map_region(
        &self,
        _character_id: u64,
        _map_id: i16,
        _unlocked: bool,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn update_character_state(&self, _character_id: u64, _state: u8) -> anyhow::Result<()> {
        Ok(())
    }
    fn update_welcome_flag(&self, _account_id: AccountId, _welcome: bool) -> anyhow::Result<()> {
        Ok(())
    }
    fn update_partner_type(&self, _character_id: u64, _new_type: i32) -> anyhow::Result<()> {
        Ok(())
    }
    fn update_partner_roster(
        &self,
        character_id: u64,
        partner_current_slot: u8,
        partner_slots: Vec<odmo_types::PartnerSlotSnapshot>,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| {
            c.partner_current_slot = partner_current_slot;
            c.partner_slots = partner_slots;
            if let Some(active_partner) = c
                .partner_slots
                .iter()
                .find(|partner| partner.slot == c.partner_current_slot)
                .cloned()
            {
                apply_partner_snapshot(c, &active_partner);
            }
        })
    }

    // ---- Extended persistence ---------------------------------------------

    fn update_quest_progress(
        &self,
        character_id: u64,
        progress: odmo_types::QuestProgressSnapshot,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.quest_progress = progress)
    }

    fn update_encyclopedia(
        &self,
        character_id: u64,
        encyclopedia: odmo_types::EncyclopediaSnapshot,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.encyclopedia = encyclopedia)
    }

    fn update_friend_list(
        &self,
        character_id: u64,
        friends: Vec<odmo_types::FriendListEntry>,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.friend_list = friends)
    }

    fn update_cash_shop_history(
        &self,
        character_id: u64,
        history: Vec<odmo_types::CashShopHistoryEntry>,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.cash_shop_history = history)
    }

    fn update_digimon_archive(
        &self,
        character_id: u64,
        archive: Vec<odmo_types::DigimonArchiveEntry>,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.digimon_archive = archive)
    }

    fn update_hatch_state(
        &self,
        character_id: u64,
        hatch: odmo_types::HatchState,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.hatch_state = hatch)
    }

    fn update_damage_skin(&self, character_id: u64, skin_id: i32) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.damage_skin_id = skin_id)
    }

    fn update_current_title(&self, character_id: u64, title_id: u16) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.current_title = title_id)
    }

    fn update_owned_titles(&self, character_id: u64, owned: Vec<i16>) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.owned_titles = owned)
    }

    fn update_tamer_model(&self, character_id: u64, model_id: i32) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.model = model_id)
    }

    fn update_tamer_name(&self, character_id: u64, new_name: &str) -> anyhow::Result<()> {
        let new_name = new_name.to_string();
        self.mutate_character(character_id, move |c| c.name = new_name)
    }

    fn update_tamer_resources(
        &self,
        character_id: u64,
        current_hp: i32,
        current_ds: i32,
        current_xgauge: i32,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| {
            c.current_hp = current_hp.clamp(0, c.hp);
            c.current_ds = current_ds.clamp(0, c.ds);
            c.current_xgauge = current_xgauge.max(0);
        })
    }

    fn update_inventory_bits(&self, character_id: u64, bits: i64) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| {
            c.inventory_bits = bits.max(0);
            c.inventory.bits = c.inventory_bits;
        })
    }

    fn update_currencies(&self, character_id: u64, premium: i32, silk: i32) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| {
            c.premium = premium.max(0);
            c.silk = silk.max(0);
        })
    }

    fn update_seal_list(
        &self,
        character_id: u64,
        seal_list: odmo_types::SealListSnapshot,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.seal_list = seal_list)
    }

    fn update_active_buffs(
        &self,
        character_id: u64,
        buffs: Vec<odmo_types::ActiveBuffSnapshot>,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.active_buffs = buffs)
    }

    fn update_deck_buff(&self, character_id: u64, deck_buff_id: i32) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| {
            c.active_deck_buff = deck_buff_id;
            c.deck_buff_id = deck_buff_id;
        })
    }

    fn update_reward_storage(
        &self,
        character_id: u64,
        items: Vec<odmo_types::ItemRecord>,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.reward_storage = items)
    }

    fn update_gift_storage(
        &self,
        character_id: u64,
        items: Vec<odmo_types::ItemRecord>,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.gift_storage = items)
    }

    fn update_npc_repurchase_log(
        &self,
        character_id: u64,
        items: Vec<odmo_types::ItemRecord>,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.npc_repurchase_log = items)
    }

    fn update_tamer_shop(
        &self,
        character_id: u64,
        listings: Vec<odmo_types::ConsignedShopListing>,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.tamer_shop_listings = listings)
    }

    fn update_season_pass(
        &self,
        character_id: u64,
        state: odmo_types::SeasonPassState,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.season_pass = state)
    }

    fn update_partner_resources(
        &self,
        character_id: u64,
        current_hp: i32,
        current_ds: i32,
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| {
            c.partner_current_hp = current_hp.clamp(0, c.partner_hp);
            c.partner_current_ds = current_ds.clamp(0, c.partner_ds);
        })
    }

    fn update_partner_name(&self, character_id: u64, new_name: &str) -> anyhow::Result<()> {
        let new_name = new_name.to_string();
        self.mutate_character(character_id, move |c| {
            c.partner_name = new_name.clone();
            if let Some(slot) = c
                .partner_slots
                .iter_mut()
                .find(|p| p.slot == c.partner_current_slot)
            {
                slot.name = new_name;
            }
        })
    }

    fn update_partner_memory_skills(
        &self,
        character_id: u64,
        skills: [i32; 4],
    ) -> anyhow::Result<()> {
        self.mutate_character(character_id, |c| c.partner_memory_skills = skills)
    }

    fn search_characters_by_name(
        &self,
        name_fragment: &str,
        limit: u32,
    ) -> anyhow::Result<Vec<CharacterSummary>> {
        let state = self.state.read().expect("repository poisoned");
        let needle = name_fragment.to_ascii_lowercase();
        let limit = limit.max(1) as usize;
        Ok(state
            .characters_by_account
            .values()
            .flatten()
            .filter(|c| c.name.to_ascii_lowercase().contains(&needle))
            .take(limit)
            .cloned()
            .collect())
    }
}

impl CharacterAccountRepository for JsonRepository {
    fn account_by_id(&self, account_id: AccountId) -> anyhow::Result<Option<Account>> {
        AccountRepository::account_by_id(self, account_id)
    }
}

impl MapMobRepository for JsonRepository {
    fn mobs_by_map(&self, map_id: i16, channel: u8) -> anyhow::Result<Vec<MobSummary>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state
            .mobs_by_map
            .get(&map_key(map_id, channel))
            .cloned()
            .unwrap_or_default())
    }
}

impl MapDropRepository for JsonRepository {
    fn drops_by_map(&self, map_id: i16, channel: u8) -> anyhow::Result<Vec<DropSummary>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state
            .drops_by_map
            .get(&map_key(map_id, channel))
            .cloned()
            .unwrap_or_default())
    }

    fn collect_drop(
        &self,
        character_id: u64,
        map_id: i16,
        channel: u8,
        drop_handler: u32,
    ) -> anyhow::Result<DropCollectionResult> {
        let mut state = self.state.write().expect("repository poisoned");
        let Some((account_id, character_index)) =
            state
                .characters_by_account
                .iter()
                .find_map(|(account_id, characters)| {
                    characters
                        .iter()
                        .position(|character| character.id == character_id)
                        .map(|index| (*account_id, index))
                })
        else {
            return Ok(DropCollectionResult::Missing);
        };
        let character_snapshot = state
            .characters_by_account
            .get(&account_id)
            .and_then(|characters| characters.get(character_index))
            .cloned()
            .expect("character should exist");
        let Some(drop_index) = state
            .drops_by_map
            .get(&map_key(map_id, channel))
            .and_then(|drops| drops.iter().position(|drop| drop.handler == drop_handler))
        else {
            return Ok(DropCollectionResult::Missing);
        };

        let drop = state
            .drops_by_map
            .get(&map_key(map_id, channel))
            .and_then(|drops| drops.get(drop_index))
            .cloned()
            .map(apply_runtime_drop_state)
            .expect("drop should exist");

        if drop.collected {
            state
                .drops_by_map
                .get_mut(&map_key(map_id, channel))
                .expect("drop map should exist")
                .remove(drop_index);
            self.persist(&state)?;
            return Ok(DropCollectionResult::Missing);
        }

        if map_distance(character_snapshot.x, character_snapshot.y, drop.x, drop.y) >= 18_001 {
            return Ok(DropCollectionResult::TooFarAway);
        }

        if drop.owner_id != 0 && drop.owner_id != character_id && !drop.no_owner {
            return Ok(DropCollectionResult::NotTheOwner);
        }

        if drop.bits_drop {
            let updated_character = {
                let character = state
                    .characters_by_account
                    .get_mut(&account_id)
                    .and_then(|characters| characters.get_mut(character_index))
                    .expect("character should exist");
                character.inventory.bits += i64::from(drop.amount.max(0));
                character.inventory_bits += i64::from(drop.amount.max(0));
                character.clone()
            };
            state
                .drops_by_map
                .get_mut(&map_key(map_id, channel))
                .expect("drop map should exist")
                .remove(drop_index);
            self.persist(&state)?;
            return Ok(DropCollectionResult::BitsCollected {
                drop: drop.clone(),
                amount: drop.amount,
                character: updated_character,
            });
        }

        let updated_character = {
            let character = state
                .characters_by_account
                .get_mut(&account_id)
                .and_then(|characters| characters.get_mut(character_index))
                .expect("character should exist");
            if !add_inventory_item(
                &mut character.inventory.items,
                character.inventory.size,
                &drop,
            ) {
                return Ok(DropCollectionResult::InventoryFull);
            }
            character.clone()
        };

        state
            .drops_by_map
            .get_mut(&map_key(map_id, channel))
            .expect("drop map should exist")
            .remove(drop_index);
        self.persist(&state)?;
        Ok(DropCollectionResult::ItemCollected {
            drop: drop.clone(),
            item_id: drop.item_id,
            amount: drop.amount.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
            character: updated_character,
        })
    }
}

impl PortalRepository for JsonRepository {
    fn portal_by_id(&self, portal_id: i32) -> anyhow::Result<Option<PortalDefinition>> {
        // Workspace-owned portal catalog derived from the game's validated rules.
        let portals = get_portal_definitions();
        Ok(portals.into_iter().find(|p| p.id == portal_id))
    }
}

pub(crate) fn get_portal_definitions() -> Vec<PortalDefinition> {
    // Demo portals constrained to map ids known by the current client smoke build.
    vec![
        PortalDefinition {
            id: 10001,
            is_local: false,
            destination_map_id: 102,
            destination_x: 32615,
            destination_y: 14930,
            source: None,
        },
        PortalDefinition {
            id: 10002,
            is_local: false,
            destination_map_id: 3,
            destination_x: 18086,
            destination_y: 18874,
            source: None,
        },
        PortalDefinition {
            id: 20001,
            is_local: false,
            destination_map_id: DEFAULT_START_MAP_ID,
            destination_x: DEFAULT_START_X,
            destination_y: DEFAULT_START_Y,
            source: None,
        },
        PortalDefinition {
            id: 20002,
            is_local: false,
            destination_map_id: DEFAULT_START_MAP_ID,
            destination_x: DEFAULT_START_X,
            destination_y: DEFAULT_START_Y,
            source: None,
        },
    ]
}

impl NpcShopRepository for JsonRepository {
    fn shop_by_npc(&self, npc_id: i32, map_id: i16) -> anyhow::Result<Option<NpcShopDefinition>> {
        Ok(get_npc_shops()
            .into_iter()
            .find(|s| s.npc_id == npc_id && s.map_id == map_id))
    }
}

impl DigiSummonRepository for JsonRepository {
    fn digi_summon_products(&self) -> anyhow::Result<Vec<DigiSummonProduct>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state.digi_summon_products.clone())
    }
}

impl ExtraEvolutionRepository for JsonRepository {
    fn extra_evolution_npcs(&self) -> anyhow::Result<Vec<ExtraEvolutionNpc>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state.extra_evolution_npcs.clone())
    }
}

impl EvolutionAssetRepository for JsonRepository {
    fn evolution_assets(&self) -> anyhow::Result<Vec<EvolutionAsset>> {
        load_evolution_asset_catalog()
    }
}

impl ItemAssetRepository for JsonRepository {
    fn item_assets(&self) -> anyhow::Result<Vec<ItemAsset>> {
        load_item_asset_catalog()
    }
}

impl DigimonAssetRepository for JsonRepository {
    fn digimon_assets(&self) -> anyhow::Result<Vec<DigimonAsset>> {
        load_digimon_asset_catalog()
    }
}

impl DigiCombineRepository for JsonRepository {
    fn digi_combine_catalog(&self) -> anyhow::Result<DigiCombineCatalog> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state.digi_combine_catalog.clone())
    }
}

impl UnionCombineRepository for JsonRepository {
    fn union_combine_catalog(&self) -> anyhow::Result<UnionCombineCatalog> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state.union_combine_catalog.clone())
    }
}

impl RandomBoxRepository for JsonRepository {
    fn random_box_rewards(&self) -> anyhow::Result<Vec<RandomBoxReward>> {
        let state = self.state.read().expect("repository poisoned");
        Ok(state.random_box_rewards.clone())
    }
}

pub(crate) fn get_npc_shops() -> Vec<NpcShopDefinition> {
    vec![
        NpcShopDefinition {
            npc_id: 1001,
            map_id: DEFAULT_START_MAP_ID,
            items: vec![
                NpcShopItem {
                    item_id: 41001,
                    buy_price: 200,
                    sell_price: 100,
                },
                NpcShopItem {
                    item_id: 41002,
                    buy_price: 200,
                    sell_price: 100,
                },
                NpcShopItem {
                    item_id: 42001,
                    buy_price: 500,
                    sell_price: 250,
                },
                NpcShopItem {
                    item_id: 43001,
                    buy_price: 300,
                    sell_price: 150,
                },
            ],
        },
        NpcShopDefinition {
            npc_id: 1002,
            map_id: DEFAULT_START_MAP_ID,
            items: vec![
                NpcShopItem {
                    item_id: 51001,
                    buy_price: 1000,
                    sell_price: 500,
                },
                NpcShopItem {
                    item_id: 52001,
                    buy_price: 800,
                    sell_price: 400,
                },
            ],
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct WorldSnapshot {
    accounts: HashMap<String, Account>,
    servers: Vec<ServerDescriptor>,
    characters_by_account: HashMap<AccountId, Vec<CharacterSummary>>,
    mobs_by_map: HashMap<String, Vec<MobSummary>>,
    drops_by_map: HashMap<String, Vec<DropSummary>>,
    digi_summon_products: Vec<DigiSummonProduct>,
    extra_evolution_npcs: Vec<ExtraEvolutionNpc>,
    digi_combine_catalog: DigiCombineCatalog,
    union_combine_catalog: UnionCombineCatalog,
    random_box_rewards: Vec<RandomBoxReward>,
    resource_hash_hex: Option<String>,
}

impl WorldSnapshot {
    fn demo() -> Self {
        let now = current_unix_timestamp();
        let mut accounts = HashMap::new();
        accounts.insert(
            "admin".to_string(),
            Account {
                id: 1,
                username: "admin".to_string(),
                password_hash: "admin".to_string(),
                email: "admin@odmo.local".to_string(),
                access_level: AccessLevel::Administrator,
                secondary_password: Some("4321".to_string()),
                suspension: None,
            },
        );
        accounts.insert(
            "gm".to_string(),
            Account {
                id: 2,
                username: "gm".to_string(),
                password_hash: "gm".to_string(),
                email: "gm@odmo.local".to_string(),
                access_level: AccessLevel::GameMaster,
                secondary_password: Some("4321".to_string()),
                suspension: None,
            },
        );
        accounts.insert(
            "banned".to_string(),
            Account {
                id: 3,
                username: "banned".to_string(),
                password_hash: "banned".to_string(),
                email: "banned@odmo.local".to_string(),
                access_level: AccessLevel::Player,
                secondary_password: None,
                suspension: Some(AccountSuspension {
                    remaining_seconds: 3_600,
                    reason: "Policy violation".to_string(),
                }),
            },
        );
        // Default smoke account used by Tools/client-testing/Run-ClientSmokeTest.ps1
        // (its -Username default is "ODMO" / "123456"). Ships with a bound partner
        // so the native auto-login + direct-character-select gate passes.
        accounts.insert(
            "ODMO".to_string(),
            Account {
                id: 4,
                username: "ODMO".to_string(),
                password_hash: "123456".to_string(),
                email: "smoke@odmo.local".to_string(),
                access_level: AccessLevel::Player,
                secondary_password: None,
                suspension: None,
            },
        );

        Self {
            accounts,
            servers: vec![
                ServerDescriptor {
                    id: 1,
                    name: "ODMO Alpha".to_string(),
                    maintenance: false,
                    overloaded: false,
                    is_new: true,
                    character_count: 0,
                },
                ServerDescriptor {
                    id: 2,
                    name: "ODMO Beta".to_string(),
                    maintenance: false,
                    overloaded: false,
                    is_new: false,
                    character_count: 0,
                },
            ],
            characters_by_account: HashMap::from([
                (
                    1,
                    vec![CharacterSummary {
                        id: 100,
                        account_id: 1,
                        slot: 0,
                        name: "AdminTamer".to_string(),
                        partner_current_slot: 1,
                        partner_current_type: DEFAULT_PARTNER_MODEL_ID,
                        partner_name: "Agumon".to_string(),
                        partner_slots: vec![
                            PartnerSlotSnapshot {
                                slot: 1,
                                digimon_type: DEFAULT_PARTNER_MODEL_ID,
                                model: DEFAULT_PARTNER_MODEL_ID,
                                name: "Agumon".to_string(),
                                ..PartnerSlotSnapshot::default()
                            },
                            PartnerSlotSnapshot {
                                slot: 2,
                                digimon_type: 31_002,
                                model: 31_002,
                                name: "Greymon".to_string(),
                                level: 11,
                                hp: 1_400,
                                ds: 1_200,
                                current_hp: 1_400,
                                current_ds: 1_200,
                                at: 150,
                                de: 120,
                                fs: 120,
                                ms: 260,
                                as_value: 950,
                                ..PartnerSlotSnapshot::default()
                            },
                        ],
                        general_handler: 11_000,
                        partner_handler: 21_000,
                        model: DEFAULT_TAMER_MODEL_ID,
                        partner_model: DEFAULT_PARTNER_MODEL_ID,
                        ..CharacterSummary::default()
                    }],
                ),
                (
                    2,
                    vec![CharacterSummary {
                        id: 200,
                        account_id: 2,
                        slot: 0,
                        name: "GmTamer".to_string(),
                        partner_current_slot: 1,
                        partner_current_type: DEFAULT_GM_PARTNER_MODEL_ID,
                        partner_name: "Gabumon".to_string(),
                        partner_slots: vec![PartnerSlotSnapshot {
                            slot: 1,
                            digimon_type: DEFAULT_GM_PARTNER_MODEL_ID,
                            model: DEFAULT_GM_PARTNER_MODEL_ID,
                            name: "Gabumon".to_string(),
                            ..PartnerSlotSnapshot::default()
                        }],
                        general_handler: 12_000,
                        partner_handler: 22_000,
                        model: DEFAULT_GM_TAMER_MODEL_ID,
                        partner_model: DEFAULT_GM_PARTNER_MODEL_ID,
                        ..CharacterSummary::default()
                    }],
                ),
                (
                    4,
                    vec![CharacterSummary {
                        id: 300,
                        account_id: 4,
                        slot: 0,
                        name: "SmokeTamer".to_string(),
                        partner_current_slot: 1,
                        partner_current_type: DEFAULT_PARTNER_MODEL_ID,
                        partner_name: "Agumon".to_string(),
                        partner_slots: vec![PartnerSlotSnapshot {
                            slot: 1,
                            digimon_type: DEFAULT_PARTNER_MODEL_ID,
                            model: DEFAULT_PARTNER_MODEL_ID,
                            name: "Agumon".to_string(),
                            ..PartnerSlotSnapshot::default()
                        }],
                        general_handler: 13_000,
                        partner_handler: 23_000,
                        model: DEFAULT_TAMER_MODEL_ID,
                        partner_model: DEFAULT_PARTNER_MODEL_ID,
                        ..CharacterSummary::default()
                    }],
                ),
            ]),
            mobs_by_map: HashMap::from([(
                map_key(DEFAULT_START_MAP_ID, 0),
                vec![
                    MobSummary {
                        id: 400,
                        map_id: DEFAULT_START_MAP_ID,
                        channel: 0,
                        handler: 34_000,
                        type_id: 51_001,
                        model: 51_001,
                        name: "Goblimon".to_string(),
                        level: 12,
                        x: DEFAULT_START_X + 66,
                        y: DEFAULT_START_Y + 45,
                        previous_x: DEFAULT_START_X + 46,
                        previous_y: DEFAULT_START_Y + 25,
                        ..MobSummary::default()
                    },
                    MobSummary {
                        id: 401,
                        map_id: DEFAULT_START_MAP_ID,
                        channel: 0,
                        handler: 34_001,
                        type_id: 51_002,
                        model: 51_002,
                        name: "Woodmon".to_string(),
                        level: 18,
                        x: DEFAULT_START_X + 416,
                        y: DEFAULT_START_Y + 225,
                        previous_x: DEFAULT_START_X + 386,
                        previous_y: DEFAULT_START_Y + 195,
                        ..MobSummary::default()
                    },
                ],
            )]),
            drops_by_map: HashMap::from([(
                map_key(DEFAULT_START_MAP_ID, 0),
                vec![
                    DropSummary {
                        id: 500,
                        map_id: DEFAULT_START_MAP_ID,
                        channel: 0,
                        handler: 49_200,
                        owner_id: 100,
                        owner_handler: 11_000,
                        item_id: 90600,
                        amount: 123,
                        x: DEFAULT_START_X + 76,
                        y: DEFAULT_START_Y + 55,
                        owner_expires_at_unix: now + 60,
                        expires_at_unix: now + 90,
                        bits_drop: true,
                        ..DropSummary::default()
                    },
                    DropSummary {
                        id: 501,
                        map_id: DEFAULT_START_MAP_ID,
                        channel: 0,
                        handler: 49_201,
                        owner_id: 0,
                        owner_handler: 0,
                        item_id: 5101,
                        amount: 1,
                        x: DEFAULT_START_X + 286,
                        y: DEFAULT_START_Y + 145,
                        owner_expires_at_unix: now.saturating_sub(5),
                        expires_at_unix: now + 30,
                        no_owner: true,
                        ..DropSummary::default()
                    },
                ],
            )]),
            digi_summon_products: vec![DigiSummonProduct {
                product_id: 9001,
                string_id: 10001,
                draw_count: 1,
                rank: 1,
                remaining_daily_limit: 0,
                icon: "digi_summon/sample_box.tga".to_string(),
                name: "Sample DigiSummon Box".to_string(),
                description: "Demo DigiSummon product used by the Rust smoke environment."
                    .to_string(),
                tickets: vec![
                    DigiSummonTicket {
                        item_id: DEMO_CATALOG_ITEM_A,
                        cost: 1,
                    },
                    DigiSummonTicket {
                        item_id: DEMO_CATALOG_ITEM_B,
                        cost: 10,
                    },
                ],
                rewards: vec![
                    DigiSummonReward {
                        item_list_id: 1,
                        item_id: 5101,
                        grade: 1,
                        amount: 1,
                        weight: 80,
                        group: 0,
                        group_code: 0,
                    },
                    DigiSummonReward {
                        item_list_id: 2,
                        item_id: 5102,
                        grade: 2,
                        amount: 1,
                        weight: 20,
                        group: 0,
                        group_code: 0,
                    },
                ],
            }],
            extra_evolution_npcs: vec![ExtraEvolutionNpc {
                npc_id: 91001,
                recipes: vec![
                    odmo_types::ExtraEvolutionRecipe {
                        exchange_type: 1,
                        object_id: 31_004,
                        material_type: 2,
                        need_material_value: 0,
                        price: 500,
                        way_type: 1,
                        main_materials: vec![odmo_types::ExtraEvolutionMaterial {
                            material_id: 81_001,
                            amount: 1,
                        }],
                        sub_materials: vec![odmo_types::ExtraEvolutionMaterial {
                            material_id: 81_002,
                            amount: 1,
                        }],
                    },
                    odmo_types::ExtraEvolutionRecipe {
                        exchange_type: 2,
                        object_id: 81_003,
                        material_type: 1,
                        need_material_value: 10,
                        price: 250,
                        way_type: 1,
                        main_materials: vec![odmo_types::ExtraEvolutionMaterial {
                            material_id: 31_002,
                            amount: 1,
                        }],
                        sub_materials: vec![odmo_types::ExtraEvolutionMaterial {
                            material_id: 81_001,
                            amount: 1,
                        }],
                    },
                ],
            }],
            digi_combine_catalog: demo_combine_catalog(),
            union_combine_catalog: demo_combine_catalog(),
            random_box_rewards: vec![
                RandomBoxReward {
                    item_id: 5201,
                    amount: 1,
                    weight: 70,
                },
                RandomBoxReward {
                    item_id: 5202,
                    amount: 1,
                    weight: 25,
                },
                RandomBoxReward {
                    item_id: 5203,
                    amount: 2,
                    weight: 5,
                },
            ],
            resource_hash_hex: Some("0123456789ABCDEF".to_string()),
        }
    }
}

/// Minimal non-empty combine catalog shared by the Digi Combine and Union
/// Combine demo seeds, which use byte-identical node layouts.
fn demo_combine_catalog() -> DigiCombineCatalog {
    DigiCombineCatalog {
        rank_rows: vec![
            DigiCombineRank {
                ceiling_type: 1,
                weight: 80,
                rewards: vec![DigiCombineReward {
                    item_id: 5101,
                    amount: 1,
                    grade: 1,
                }],
            },
            DigiCombineRank {
                ceiling_type: 1,
                weight: 20,
                rewards: vec![DigiCombineReward {
                    item_id: 5102,
                    amount: 1,
                    grade: 2,
                }],
            },
        ],
        item_list: vec![
            DigiCombineItem {
                item_id: DEMO_CATALOG_ITEM_A,
                group_id: 1,
            },
            DigiCombineItem {
                item_id: DEMO_CATALOG_ITEM_B,
                group_id: 1,
            },
        ],
        item_groups: vec![DigiCombineGroup {
            group_id: 1,
            members: vec![DEMO_CATALOG_ITEM_A, DEMO_CATALOG_ITEM_B],
        }],
        ceil_groups: vec![DigiCombineCeil {
            ceiling_type: 1,
            entries: vec![CombineCeilingEntry {
                tier: 1,
                value_a: 1,
                value_b: 100,
            }],
        }],
    }
}

impl Default for WorldSnapshot {
    fn default() -> Self {
        Self::demo()
    }
}

fn normalize_legacy_snapshot(snapshot: &mut WorldSnapshot) -> bool {
    let mut changed = false;

    for (account_id, characters) in snapshot.characters_by_account.iter_mut() {
        let fallback_model = if *account_id == 2 {
            DEFAULT_GM_TAMER_MODEL_ID
        } else {
            DEFAULT_TAMER_MODEL_ID
        };

        for character in characters.iter_mut() {
            changed |= normalize_legacy_character(character, fallback_model);
        }
    }

    if let std::collections::hash_map::Entry::Vacant(e) =
        snapshot.mobs_by_map.entry(map_key(DEFAULT_START_MAP_ID, 0))
    {
        e.insert(
            WorldSnapshot::demo()
                .mobs_by_map
                .remove(&map_key(DEFAULT_START_MAP_ID, 0))
                .unwrap_or_default(),
        );
        changed = true;
    }

    if let std::collections::hash_map::Entry::Vacant(e) = snapshot
        .drops_by_map
        .entry(map_key(DEFAULT_START_MAP_ID, 0))
    {
        e.insert(
            WorldSnapshot::demo()
                .drops_by_map
                .remove(&map_key(DEFAULT_START_MAP_ID, 0))
                .unwrap_or_default(),
        );
        changed = true;
    }

    changed
}

fn normalize_legacy_character(character: &mut CharacterSummary, fallback_model: i32) -> bool {
    const LEGACY_START_X: i32 = 14_834;
    const LEGACY_START_Y: i32 = 9_895;

    let mut changed = false;

    if character.map_id == 0 || character.map_id == 8101 {
        character.map_id = DEFAULT_START_MAP_ID;
        character.x = DEFAULT_START_X;
        character.y = DEFAULT_START_Y;
        character.z = 0.0;
        character.partner_x = DEFAULT_START_X;
        character.partner_y = DEFAULT_START_Y;
        character.partner_z = 0.0;
        changed = true;
    }

    if character.map_id == DEFAULT_START_MAP_ID
        && matches!(
            (character.x, character.y),
            (LEGACY_START_X, LEGACY_START_Y) | (4_471, 5_432) | (4_800, 5_200)
        )
    {
        character.x = DEFAULT_START_X;
        character.y = DEFAULT_START_Y;
        character.z = 0.0;
        character.partner_x = DEFAULT_START_X;
        character.partner_y = DEFAULT_START_Y;
        character.partner_z = 0.0;
        changed = true;
    }

    if matches!(character.model, 1001..=1003) || character.model < 80_000 {
        character.model = fallback_model;
        changed = true;
    }

    if character.partner_model <= 0 {
        character.partner_model = DEFAULT_PARTNER_MODEL_ID;
        changed = true;
    }

    if character.general_handler == 0 {
        character.general_handler = character.id as u32 + 10_000;
        changed = true;
    }

    if character.partner_handler == 0 {
        character.partner_handler = character.id as u32 + 20_000;
        changed = true;
    }

    changed
}

fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn apply_runtime_drop_state(mut drop: DropSummary) -> DropSummary {
    let now = current_unix_timestamp();
    if drop.expires_at_unix > 0 && now >= drop.expires_at_unix {
        drop.collected = true;
    }
    if !drop.collected && drop.owner_expires_at_unix > 0 && now >= drop.owner_expires_at_unix {
        drop.no_owner = true;
    }
    drop
}

fn map_distance(xa: i32, ya: i32, xb: i32, yb: i32) -> i64 {
    let distance_x = (xb as i64 - xa as i64).pow(2);
    let distance_y = (yb as i64 - ya as i64).pow(2);
    ((distance_x + distance_y) as f64).sqrt() as i64
}

fn add_inventory_item(items: &mut Vec<ItemRecord>, size: u16, drop: &DropSummary) -> bool {
    if let Some(existing) = items.iter_mut().find(|item| item.item_id == drop.item_id) {
        existing.amount = existing.amount.saturating_add(drop.amount.max(0));
        existing.sync_record();
        return true;
    }

    if items.len() >= size as usize {
        return false;
    }

    let record = ItemRecord::new(drop.item_id, drop.amount.max(1));
    items.push(record);
    true
}
