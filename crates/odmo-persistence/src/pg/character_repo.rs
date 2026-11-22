use odmo_application::account::AccountRepository;
use odmo_application::character::{CharacterAccountRepository, CharacterRepository};
use odmo_types::{
    Account, AccountId, ActiveBuffSnapshot, AttendanceStatus, ChannelAvailability,
    CharacterSummary, DEFAULT_START_MAP_ID, DEFAULT_START_X, DEFAULT_START_Y, DailyRewardStatus,
    DigiviceItemSnapshot, DigiviceSnapshot, EncyclopediaSnapshot, GuildSnapshot, InventorySnapshot,
    ItemRecord, PartnerSlotSnapshot, QuestProgressSnapshot, RelationEntry, SealListSnapshot,
    UnionHackSlotRow, XaiSnapshot,
};

use super::PgRepository;
use crate::{active_partner_snapshot, apply_partner_snapshot};

const LEGACY_START_X: i32 = 14_834;
const LEGACY_START_Y: i32 = 9_895;

/// Database row struct mapping all columns from the characters table.
#[derive(Debug, sqlx::FromRow)]
pub(crate) struct CharacterDb {
    pub id: i64,
    pub account_id: i64,
    pub slot: i16,
    pub name: String,
    pub model: i32,
    pub level: i16,
    pub current_x: i32,
    pub current_y: i32,
    pub current_map_id: i16,
    pub partner_current_x: i32,
    pub partner_current_y: i32,
    pub partner_current_slot: i16,
    pub channel: i16,
    pub current_condition: i16,
    pub general_handler: i32,
    pub partner_handler: i32,
    pub partner_name: String,
    pub partner_model: i32,
    pub bits: i64,
    pub xgauge: i16,
    pub xcrystals: i16,
    pub inventory: serde_json::Value,
    pub warehouse: serde_json::Value,
    pub extra_inventory: serde_json::Value,
    pub account_warehouse: Option<serde_json::Value>,
    pub seal_list: serde_json::Value,
    pub guild_snapshot: Option<serde_json::Value>,
    pub xai_snapshot: Option<serde_json::Value>,
    pub active_buffs: serde_json::Value,
    pub friends: serde_json::Value,
    pub foes: serde_json::Value,
    pub friended_character_ids: serde_json::Value,
    pub map_regions: serde_json::Value,
    pub equipment: serde_json::Value,
    pub digivice: serde_json::Value,
    pub daily_reward: serde_json::Value,
    pub attendance: serde_json::Value,
    pub available_channels: serde_json::Value,
    pub partner_slots: serde_json::Value,
    pub encyclopedia: serde_json::Value,
    pub union_hack_slots: serde_json::Value,
    pub deck_buff_id: i32,
    pub server_experience: i32,
    pub premium: i32,
    pub silk: i32,
    pub membership_seconds: i64,
    pub quest_progress: serde_json::Value,
    pub npc_repurchase_log: serde_json::Value,
}

const SELECT_COLS: &str = "\
    id, account_id, slot, name, model, level, \
    current_x, current_y, current_map_id, \
    partner_current_x, partner_current_y, \
    partner_current_slot, channel, current_condition, general_handler, partner_handler, \
    partner_name, partner_model, bits, xgauge, xcrystals, \
    inventory, warehouse, extra_inventory, account_warehouse, \
    seal_list, guild_snapshot, xai_snapshot, active_buffs, \
    friends, foes, friended_character_ids, map_regions, \
    equipment, digivice, daily_reward, attendance, partner_slots, \
    available_channels, encyclopedia, union_hack_slots, deck_buff_id, \
    server_experience, premium, silk, membership_seconds, \
    quest_progress, npc_repurchase_log";

pub(crate) fn row_to_character(row: CharacterDb) -> CharacterSummary {
    let inventory: InventorySnapshot = serde_json::from_value(row.inventory).unwrap_or_default();
    let warehouse: InventorySnapshot = serde_json::from_value(row.warehouse).unwrap_or_default();
    let extra_inventory: InventorySnapshot =
        serde_json::from_value(row.extra_inventory).unwrap_or_default();
    let account_warehouse: Option<InventorySnapshot> = row
        .account_warehouse
        .and_then(|v| serde_json::from_value(v).ok());
    let seal_list: SealListSnapshot = serde_json::from_value(row.seal_list).unwrap_or_default();
    let guild: Option<GuildSnapshot> = row
        .guild_snapshot
        .and_then(|v| serde_json::from_value(v).ok());
    let xai: Option<XaiSnapshot> = row
        .xai_snapshot
        .and_then(|v| serde_json::from_value(v).ok());
    let active_buffs: Vec<ActiveBuffSnapshot> =
        serde_json::from_value(row.active_buffs).unwrap_or_default();
    let friends: Vec<RelationEntry> = serde_json::from_value(row.friends).unwrap_or_default();
    let foes: Vec<RelationEntry> = serde_json::from_value(row.foes).unwrap_or_default();
    let friended_character_ids: Vec<u64> =
        serde_json::from_value(row.friended_character_ids).unwrap_or_default();
    let map_region: Vec<u8> =
        serde_json::from_value(row.map_regions).unwrap_or_else(|_| vec![0u8; 255]);
    let equipment: Vec<u8> =
        serde_json::from_value(row.equipment).unwrap_or_else(|_| vec![0u8; 16 * 69]);
    let digivice = decode_digivice_snapshot(row.digivice);
    let daily_reward: DailyRewardStatus =
        serde_json::from_value(row.daily_reward).unwrap_or_default();
    let attendance: AttendanceStatus = serde_json::from_value(row.attendance).unwrap_or_default();
    let encyclopedia: EncyclopediaSnapshot =
        serde_json::from_value(row.encyclopedia).unwrap_or_default();
    let union_hack_slots: Vec<UnionHackSlotRow> =
        serde_json::from_value(row.union_hack_slots).unwrap_or_default();
    let quest_progress: QuestProgressSnapshot =
        serde_json::from_value(row.quest_progress).unwrap_or_default();
    let npc_repurchase_log: Vec<ItemRecord> =
        serde_json::from_value(row.npc_repurchase_log).unwrap_or_default();
    let mut partner_slots: Vec<PartnerSlotSnapshot> =
        serde_json::from_value(row.partner_slots).unwrap_or_default();
    let available_channels: Vec<ChannelAvailability> =
        serde_json::from_value(row.available_channels).unwrap_or_else(|_| {
            vec![ChannelAvailability {
                channel: 0,
                load: 1,
            }]
        });

    let mut summary = CharacterSummary {
        id: row.id as u64,
        account_id: row.account_id as u64,
        slot: row.slot as u8,
        map_id: row.current_map_id,
        x: row.current_x,
        y: row.current_y,
        z: 0.0,
        channel: row.channel as u8,
        general_handler: row.general_handler as u32,
        model: row.model,
        level: row.level as u8,
        name: row.name,
        current_experience: 0,
        hp: 1000,
        ds: 1000,
        current_hp: 1000,
        current_ds: 1000,
        fatigue: 0,
        at: 100,
        de: 100,
        ms: 300,
        proper_ms: 300,
        current_condition: row.current_condition as i32,
        partner_condition: 0,
        inventory_bits: row.bits,
        inventory_size: inventory.size,
        warehouse_size: warehouse.size,
        account_warehouse_size: account_warehouse.as_ref().map_or(14, |w| w.size),
        extra_inventory_size: extra_inventory.size,
        inventory,
        warehouse,
        account_warehouse,
        extra_inventory,
        digimon_slots: 7,
        current_title: 0,
        map_region,
        archive_slots: 7,
        deck_buff_id: row.deck_buff_id,
        equipment,
        digivice,
        shop_name: String::new(),
        size: 12_000,
        active_buffs,
        seal_list,
        daily_reward,
        attendance,
        server_experience: row.server_experience,
        premium: row.premium,
        silk: row.silk,
        membership_seconds: row.membership_seconds.max(0) as u32,
        available_channels,
        friends,
        foes,
        friended_character_ids,
        guild,
        xai,
        current_xgauge: row.xgauge as i32,
        current_xcrystals: row.xcrystals,
        partner_x: row.partner_current_x,
        partner_y: row.partner_current_y,
        partner_z: 0.0,
        partner_current_slot: row.partner_current_slot as u8,
        partner_current_type: row.partner_model,
        partner_slots: Vec::new(),
        partner_active_buffs: Vec::new(),
        partner_active_debuffs: Vec::new(),
        partner_model: row.partner_model,
        partner_level: 1,
        partner_name: row.partner_name,
        partner_handler: row.partner_handler as u32,
        partner_size: 12_000,
        partner_hatch_grade: 3,
        partner_current_experience: 0,
        partner_transcendence_experience: 0,
        partner_hp: 1000,
        partner_ds: 1000,
        partner_de: 100,
        partner_at: 100,
        partner_current_hp: 1000,
        partner_current_ds: 1000,
        partner_fs: 100,
        partner_ev: 0,
        partner_cc: 0,
        partner_ms: 250,
        partner_as: 1000,
        partner_ht: 0,
        partner_ar: 0,
        partner_bl: 0,
        partner_clone_level: 0,
        partner_clone_at_value: 0,
        partner_clone_bl_value: 0,
        partner_clone_ct_value: 0,
        partner_clone_ev_value: 0,
        partner_clone_hp_value: 0,
        partner_clone_at_level: 0,
        partner_clone_bl_level: 0,
        partner_clone_ct_level: 0,
        partner_clone_ev_level: 0,
        partner_clone_hp_level: 0,
        encyclopedia,
        active_deck_buff: row.deck_buff_id,
        union_hack_slots,
        quest_progress,
        npc_repurchase_log,
        ..CharacterSummary::default()
    };

    if partner_slots.is_empty() {
        partner_slots.push(active_partner_snapshot(&summary));
    }
    summary.partner_slots = partner_slots;
    if let Some(active_partner) = summary
        .partner_slots
        .iter()
        .find(|partner| partner.slot == summary.partner_current_slot)
        .cloned()
    {
        summary.partner_current_type = active_partner.digimon_type;
        summary.partner_model = active_partner.model;
        summary.partner_level = active_partner.level;
        summary.partner_name = active_partner.name;
        summary.partner_size = active_partner.size;
        summary.partner_hatch_grade = active_partner.hatch_grade;
        summary.partner_hp = active_partner.hp;
        summary.partner_ds = active_partner.ds;
        summary.partner_current_hp = active_partner.current_hp;
        summary.partner_current_ds = active_partner.current_ds;
        summary.partner_de = active_partner.de;
        summary.partner_at = active_partner.at;
        summary.partner_fs = active_partner.fs;
        summary.partner_ev = active_partner.ev;
        summary.partner_cc = active_partner.cc;
        summary.partner_ms = active_partner.ms;
        summary.partner_as = active_partner.as_value;
        summary.partner_ht = active_partner.ht;
        summary.partner_ar = active_partner.ar;
        summary.partner_bl = active_partner.bl;
        summary.partner_clone_level = active_partner.clone_level;
        summary.partner_clone_at_value = active_partner.clone_at_value;
        summary.partner_clone_bl_value = active_partner.clone_bl_value;
        summary.partner_clone_ct_value = active_partner.clone_ct_value;
        summary.partner_clone_ev_value = active_partner.clone_ev_value;
        summary.partner_clone_hp_value = active_partner.clone_hp_value;
        summary.partner_clone_at_level = active_partner.clone_at_level;
        summary.partner_clone_bl_level = active_partner.clone_bl_level;
        summary.partner_clone_ct_level = active_partner.clone_ct_level;
        summary.partner_clone_ev_level = active_partner.clone_ev_level;
        summary.partner_clone_hp_level = active_partner.clone_hp_level;
        summary.partner_active_buffs = active_partner.active_buffs;
    }

    if summary.map_id == DEFAULT_START_MAP_ID
        && summary.x == LEGACY_START_X
        && summary.y == LEGACY_START_Y
    {
        summary.x = DEFAULT_START_X;
        summary.y = DEFAULT_START_Y;
        summary.z = 0.0;
        summary.partner_x = DEFAULT_START_X;
        summary.partner_y = DEFAULT_START_Y;
        summary.partner_z = 0.0;
    }

    summary
}

fn decode_digivice_snapshot(value: serde_json::Value) -> DigiviceSnapshot {
    if let Ok(snapshot) = serde_json::from_value::<DigiviceSnapshot>(value.clone()) {
        let mut snapshot = snapshot;
        snapshot.normalize();
        return snapshot;
    }

    if let Ok(record) = serde_json::from_value::<Vec<u8>>(value) {
        let mut snapshot = DigiviceSnapshot {
            equipped_item: DigiviceItemSnapshot::from_record(record),
            ..DigiviceSnapshot::default()
        };
        snapshot.normalize();
        return snapshot;
    }

    let mut snapshot = DigiviceSnapshot::default();
    snapshot.normalize();
    snapshot
}

impl CharacterRepository for PgRepository {
    fn list_characters_by_account(
        &self,
        account_id: AccountId,
    ) -> anyhow::Result<Vec<CharacterSummary>> {
        let pool = self.pool().clone();
        let query =
            format!("SELECT {SELECT_COLS} FROM characters WHERE account_id = $1 ORDER BY slot");
        self.block_on(async move {
            let rows: Vec<CharacterDb> = sqlx::query_as(&query)
                .bind(account_id as i64)
                .fetch_all(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("list_characters_by_account: {e}"))?;

            Ok(rows.into_iter().map(row_to_character).collect())
        })
    }

    fn character_by_slot(
        &self,
        account_id: AccountId,
        slot: u8,
    ) -> anyhow::Result<Option<CharacterSummary>> {
        let pool = self.pool().clone();
        let query =
            format!("SELECT {SELECT_COLS} FROM characters WHERE account_id = $1 AND slot = $2");
        self.block_on(async move {
            let row: Option<CharacterDb> = sqlx::query_as(&query)
                .bind(account_id as i64)
                .bind(slot as i16)
                .fetch_optional(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("character_by_slot: {e}"))?;

            Ok(row.map(row_to_character))
        })
    }

    fn character_by_id(&self, character_id: u64) -> anyhow::Result<Option<CharacterSummary>> {
        let pool = self.pool().clone();
        let query = format!("SELECT {SELECT_COLS} FROM characters WHERE id = $1");
        self.block_on(async move {
            let row: Option<CharacterDb> = sqlx::query_as(&query)
                .bind(character_id as i64)
                .fetch_optional(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("character_by_id: {e}"))?;

            Ok(row.map(row_to_character))
        })
    }

    fn character_by_name(&self, name: &str) -> anyhow::Result<Option<CharacterSummary>> {
        let pool = self.pool().clone();
        let query = format!("SELECT {SELECT_COLS} FROM characters WHERE LOWER(name) = LOWER($1)");
        let name = name.to_string();
        self.block_on(async move {
            let row: Option<CharacterDb> = sqlx::query_as(&query)
                .bind(&name)
                .fetch_optional(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("character_by_name: {e}"))?;

            Ok(row.map(row_to_character))
        })
    }

    fn is_name_available(&self, name: &str) -> anyhow::Result<bool> {
        let pool = self.pool().clone();
        let name = name.to_string();
        self.block_on(async move {
            let count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM characters WHERE LOWER(name) = LOWER($1)")
                    .bind(&name)
                    .fetch_one(&pool)
                    .await?;

            Ok(count.0 == 0)
        })
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
        let pool = self.pool().clone();
        self.block_on(async move {
            let next_handler: (i32,) = sqlx::query_as(
                "SELECT COALESCE(MAX(general_handler), 10000) + 1 FROM characters",
            )
            .fetch_one(&pool)
            .await?;
            let general_handler = next_handler.0;
            let partner_handler = general_handler + 10_000;

            let default_inv = serde_json::json!({"bits": 0, "size": 30, "items": []});
            let default_warehouse = serde_json::json!({"bits": 0, "size": 21, "items": []});
            let default_account_warehouse = serde_json::json!({"bits": 0, "size": 14, "items": []});
            let default_seals = serde_json::json!({"seal_leader_id": 0, "seals": []});
            let default_channels = serde_json::json!([{"channel": 0, "load": 1}]);
            let default_encyclopedia = serde_json::json!({"entries": []});
            let default_union_hacks = serde_json::json!([]);
            let default_partner_slots = serde_json::json!([{
                "slot": 1, "digimon_type": partner_model, "model": partner_model,
                "level": 1, "name": partner_name, "size": 12000, "hatch_grade": 3,
                "hp": 1000, "ds": 1000, "current_hp": 1000, "current_ds": 1000,
                "de": 100, "at": 100, "fs": 100, "ev": 0, "cc": 0, "ms": 250, "as_value": 1000,
                "ht": 0, "ar": 0, "bl": 0, "clone_level": 0,
                "clone_at_value": 0, "clone_bl_value": 0, "clone_ct_value": 0, "clone_ev_value": 0, "clone_hp_value": 0,
                "clone_at_level": 0, "clone_bl_level": 0, "clone_ct_level": 0, "clone_ev_level": 0, "clone_hp_level": 0,
                "active_buffs": []
            }]);

            sqlx::query(
                "INSERT INTO characters \
                (account_id, slot, name, model, level, \
                 current_x, current_y, current_map_id, \
                 partner_current_x, partner_current_y, \
                 partner_current_slot, channel, current_condition, \
                 general_handler, partner_handler, \
                 partner_name, partner_model, bits, xgauge, xcrystals, \
                 inventory, warehouse, extra_inventory, account_warehouse, \
                 seal_list, available_channels, partner_slots, encyclopedia, \
                 union_hack_slots, deck_buff_id) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29,$30)",
            )
            .bind(account_id as i64)
            .bind(slot as i16)
            .bind(&tamer_name)
            .bind(tamer_model)
            .bind(1i16)
            .bind(DEFAULT_START_X)
            .bind(DEFAULT_START_Y)
            .bind(DEFAULT_START_MAP_ID)
            .bind(DEFAULT_START_X)
            .bind(DEFAULT_START_Y)
            .bind(1i16)
            .bind(0i16)
            .bind(0i16)
            .bind(general_handler)
            .bind(partner_handler)
            .bind(&partner_name)
            .bind(partner_model)
            .bind(0i64)
            .bind(0i16)
            .bind(0i16)
            .bind(&default_inv)
            .bind(&default_warehouse)
            .bind(&default_inv)
            .bind(&default_account_warehouse)
            .bind(&default_seals)
            .bind(&default_channels)
            .bind(&default_partner_slots)
            .bind(&default_encyclopedia)
            .bind(&default_union_hacks)
            .bind(0i32)
            .execute(&pool)
            .await
            .map_err(|e| anyhow::anyhow!("create_character: {e}"))?;

            // Fetch back the created row
            let query = format!("SELECT {SELECT_COLS} FROM characters WHERE account_id = $1 AND slot = $2");
            let row: CharacterDb = sqlx::query_as(&query)
                .bind(account_id as i64)
                .bind(slot as i16)
                .fetch_one(&pool)
                .await
                .map_err(|e| anyhow::anyhow!("create_character fetch: {e}"))?;

            Ok(row_to_character(row))
        })
    }

    fn delete_character(&self, account_id: AccountId, slot: u8) -> anyhow::Result<bool> {
        let pool = self.pool().clone();
        self.block_on(async move {
            let result = sqlx::query("DELETE FROM characters WHERE account_id = $1 AND slot = $2")
                .bind(account_id as i64)
                .bind(slot as i16)
                .execute(&pool)
                .await?;

            Ok(result.rows_affected() > 0)
        })
    }

    fn update_character_position(
        &self,
        character_id: u64,
        x: i32,
        y: i32,
        z: f32,
    ) -> anyhow::Result<()> {
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query(
                "UPDATE characters SET current_x = $1, current_y = $2, current_z = $3 WHERE id = $4",
            )
            .bind(x)
            .bind(y)
            .bind(z)
            .bind(character_id as i64)
            .execute(&pool)
            .await?;
            Ok(())
        })
    }

    fn update_partner_position(
        &self,
        character_id: u64,
        x: i32,
        y: i32,
        z: f32,
    ) -> anyhow::Result<()> {
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query(
                "UPDATE characters SET partner_current_x = $1, partner_current_y = $2, partner_current_z = $3 WHERE id = $4",
            )
            .bind(x)
            .bind(y)
            .bind(z)
            .bind(character_id as i64)
            .execute(&pool)
            .await?;
            Ok(())
        })
    }

    fn switch_partner(
        &self,
        character_id: u64,
        slot: u8,
    ) -> anyhow::Result<Option<CharacterSummary>> {
        let Some(mut character) = self.character_by_id(character_id)? else {
            return Ok(None);
        };

        if character.partner_current_slot == slot {
            return Ok(Some(character));
        }

        if let Some(current_index) = character
            .partner_slots
            .iter()
            .position(|partner| partner.slot == character.partner_current_slot)
        {
            character.partner_slots[current_index] = active_partner_snapshot(&character);
        }

        let Some(target_partner) = character
            .partner_slots
            .iter()
            .find(|partner| partner.slot == slot)
            .cloned()
        else {
            return Ok(None);
        };

        apply_partner_snapshot(&mut character, &target_partner);
        character.partner_current_slot = slot;

        let partner_slots_json = serde_json::to_value(&character.partner_slots)?;
        let pool = self.pool().clone();
        let updated = character.clone();
        self.block_on(async move {
            sqlx::query(
                "UPDATE characters SET \
                 partner_current_slot = $1, partner_name = $2, partner_model = $3, partner_current_type = $4, \
                 partner_level = $5, partner_hp = $6, partner_ds = $7, partner_current_hp = $8, partner_current_ds = $9, \
                 partner_slots = $10 WHERE id = $11",
            )
            .bind(updated.partner_current_slot as i16)
            .bind(&updated.partner_name)
            .bind(updated.partner_model)
            .bind(updated.partner_current_type)
            .bind(updated.partner_level as i16)
            .bind(updated.partner_hp)
            .bind(updated.partner_ds)
            .bind(updated.partner_current_hp)
            .bind(updated.partner_current_ds)
            .bind(&partner_slots_json)
            .bind(character_id as i64)
            .execute(&pool)
            .await?;
            Ok::<(), anyhow::Error>(())
        })?;

        Ok(Some(character))
    }

    fn update_character_map(
        &self,
        character_id: u64,
        map_id: i16,
        x: i32,
        y: i32,
    ) -> anyhow::Result<()> {
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query(
                "UPDATE characters SET current_map_id = $1, current_x = $2, current_y = $3, \
                 partner_current_x = $2, partner_current_y = $3 WHERE id = $4",
            )
            .bind(map_id)
            .bind(x)
            .bind(y)
            .bind(character_id as i64)
            .execute(&pool)
            .await?;
            Ok(())
        })
    }

    fn update_inventory(
        &self,
        character_id: u64,
        inventory: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        let inventory_json = serde_json::to_value(&inventory)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET inventory = $1 WHERE id = $2")
                .bind(&inventory_json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }

    fn update_equipment(&self, character_id: u64, equipment: Vec<u8>) -> anyhow::Result<()> {
        let equipment_json = serde_json::to_value(&equipment)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET equipment = $1 WHERE id = $2")
                .bind(&equipment_json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }

    fn update_digivice(&self, character_id: u64, digivice: DigiviceSnapshot) -> anyhow::Result<()> {
        let mut digivice = digivice;
        digivice.normalize();
        let digivice_json = serde_json::to_value(&digivice)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET digivice = $1 WHERE id = $2")
                .bind(&digivice_json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }

    fn update_extra_inventory(
        &self,
        character_id: u64,
        extra_inventory: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        let extra_inventory_json = serde_json::to_value(&extra_inventory)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET extra_inventory = $1 WHERE id = $2")
                .bind(&extra_inventory_json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }

    fn update_warehouse(
        &self,
        character_id: u64,
        warehouse: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        let warehouse_json = serde_json::to_value(&warehouse)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET warehouse = $1 WHERE id = $2")
                .bind(&warehouse_json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }

    fn update_account_warehouse(
        &self,
        character_id: u64,
        account_warehouse: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        let aw_json = serde_json::to_value(&account_warehouse)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET account_warehouse = $1 WHERE id = $2")
                .bind(&aw_json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }

    fn update_character_map_region(
        &self,
        _character_id: u64,
        _map_id: i16,
        _unlocked: bool,
    ) -> anyhow::Result<()> {
        // Map region unlock is not yet persisted in the character table
        Ok(())
    }
    fn update_character_state(&self, _character_id: u64, _state: u8) -> anyhow::Result<()> {
        // Character state is not yet persisted in the character table
        Ok(())
    }
    fn update_welcome_flag(&self, _account_id: AccountId, _welcome: bool) -> anyhow::Result<()> {
        // Welcome flag is not yet persisted in the accounts table
        Ok(())
    }
    fn update_tamer_resources(
        &self,
        character_id: u64,
        current_hp: i32,
        current_ds: i32,
        current_xgauge: i32,
    ) -> anyhow::Result<()> {
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query(
                "UPDATE characters SET current_hp = $1, current_ds = $2, xgauge = $3 WHERE id = $4",
            )
            .bind(current_hp)
            .bind(current_ds)
            .bind(current_xgauge.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16)
            .bind(character_id as i64)
            .execute(&pool)
            .await?;
            Ok(())
        })
    }
    fn update_partner_type(&self, _character_id: u64, _new_type: i32) -> anyhow::Result<()> {
        // Partner type evolution not yet persisted in the characters table
        Ok(())
    }
    fn update_partner_roster(
        &self,
        character_id: u64,
        partner_current_slot: u8,
        partner_slots: Vec<PartnerSlotSnapshot>,
    ) -> anyhow::Result<()> {
        let Some(active_partner) = partner_slots
            .iter()
            .find(|partner| partner.slot == partner_current_slot)
            .cloned()
        else {
            return Ok(());
        };

        let partner_slots_json = serde_json::to_value(&partner_slots)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query(
                "UPDATE characters SET \
                 partner_current_slot = $1, partner_name = $2, partner_model = $3, partner_current_type = $4, \
                 partner_level = $5, partner_hp = $6, partner_ds = $7, partner_current_hp = $8, partner_current_ds = $9, \
                 partner_slots = $10 WHERE id = $11",
            )
            .bind(partner_current_slot as i16)
            .bind(&active_partner.name)
            .bind(active_partner.model)
            .bind(active_partner.digimon_type)
            .bind(active_partner.level as i16)
            .bind(active_partner.hp)
            .bind(active_partner.ds)
            .bind(active_partner.current_hp)
            .bind(active_partner.current_ds)
            .bind(&partner_slots_json)
            .bind(character_id as i64)
            .execute(&pool)
            .await?;
            Ok::<(), anyhow::Error>(())
        })
    }

    fn update_encyclopedia(
        &self,
        character_id: u64,
        encyclopedia: odmo_types::EncyclopediaSnapshot,
    ) -> anyhow::Result<()> {
        let encyclopedia_json = serde_json::to_value(&encyclopedia)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET encyclopedia = $1 WHERE id = $2")
                .bind(&encyclopedia_json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }

    fn union_hack_slots(
        &self,
        character_id: u64,
    ) -> anyhow::Result<Vec<odmo_types::UnionHackSlotRow>> {
        let pool = self.pool().clone();
        self.block_on(async move {
            let row: (serde_json::Value,) =
                sqlx::query_as("SELECT union_hack_slots FROM characters WHERE id = $1")
                    .bind(character_id as i64)
                    .fetch_optional(&pool)
                    .await?
                    .unwrap_or((serde_json::json!([]),));
            Ok(serde_json::from_value(row.0).unwrap_or_default())
        })
    }

    fn update_union_hack_slot(
        &self,
        character_id: u64,
        slot: u8,
        part_id: i32,
        grade: i16,
    ) -> anyhow::Result<bool> {
        const MAX_SLOTS: usize = 6;
        let slot_index = slot as usize;
        if slot_index >= MAX_SLOTS {
            return Ok(false);
        }
        let pool = self.pool().clone();
        self.block_on(async move {
            let existing: (serde_json::Value,) =
                sqlx::query_as("SELECT union_hack_slots FROM characters WHERE id = $1")
                    .bind(character_id as i64)
                    .fetch_optional(&pool)
                    .await?
                    .unwrap_or((serde_json::json!([]),));
            let mut rows: Vec<odmo_types::UnionHackSlotRow> =
                serde_json::from_value(existing.0).unwrap_or_default();
            if rows.len() < slot_index + 1 {
                rows.resize(slot_index + 1, odmo_types::UnionHackSlotRow::default());
            }
            rows[slot_index] = odmo_types::UnionHackSlotRow {
                part_id,
                grade,
                locked: false,
            };
            let json = serde_json::to_value(&rows)?;
            let result = sqlx::query("UPDATE characters SET union_hack_slots = $1 WHERE id = $2")
                .bind(&json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(result.rows_affected() > 0)
        })
    }

    fn update_deck_buff(&self, character_id: u64, deck_buff_id: i32) -> anyhow::Result<()> {
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET deck_buff_id = $1 WHERE id = $2")
                .bind(deck_buff_id)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }

    fn update_inventory_bits(&self, character_id: u64, bits: i64) -> anyhow::Result<()> {
        let clamped = bits.max(0);
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query(
                "UPDATE characters SET bits = $1, inventory = jsonb_set(inventory, '{bits}', to_jsonb($1::bigint)) WHERE id = $2",
            )
            .bind(clamped)
            .bind(character_id as i64)
            .execute(&pool)
            .await?;
            Ok(())
        })
    }

    fn update_quest_progress(
        &self,
        character_id: u64,
        quest_progress: QuestProgressSnapshot,
    ) -> anyhow::Result<()> {
        let progress_json = serde_json::to_value(&quest_progress)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET quest_progress = $1 WHERE id = $2")
                .bind(&progress_json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }

    fn update_npc_repurchase_log(
        &self,
        character_id: u64,
        log: Vec<ItemRecord>,
    ) -> anyhow::Result<()> {
        let log_json = serde_json::to_value(&log)?;
        let pool = self.pool().clone();
        self.block_on(async move {
            sqlx::query("UPDATE characters SET npc_repurchase_log = $1 WHERE id = $2")
                .bind(&log_json)
                .bind(character_id as i64)
                .execute(&pool)
                .await?;
            Ok(())
        })
    }
}

impl CharacterAccountRepository for PgRepository {
    fn account_by_id(&self, account_id: AccountId) -> anyhow::Result<Option<Account>> {
        <Self as AccountRepository>::account_by_id(self, account_id)
    }
}

/// Returns the query to fetch a character by ID, for use by drop_repo.
pub(crate) fn character_by_id_query() -> String {
    format!("SELECT {SELECT_COLS} FROM characters WHERE id = $1")
}
