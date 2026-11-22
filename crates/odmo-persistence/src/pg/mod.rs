mod account_repo;
mod asset_repo;
mod character_repo;
mod combine_repo;
mod digi_summon_repo;
mod drop_repo;
mod extra_evolution_repo;
mod mob_repo;
mod npc_shop_repo;
mod portal_bridge;
mod portal_repo;
mod random_box_repo;

use std::future::Future;

use odmo_types::{
    DEFAULT_GM_PARTNER_MODEL_ID, DEFAULT_GM_TAMER_MODEL_ID, DEFAULT_PARTNER_MODEL_ID,
    DEFAULT_START_MAP_ID, DEFAULT_START_X, DEFAULT_START_Y, DEFAULT_TAMER_MODEL_ID,
};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct PgRepository {
    pool: PgPool,
}

impl PgRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn open(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        sqlx::migrate!("./migrations").run(&self.pool).await?;
        Ok(())
    }

    pub async fn prepare_runtime(&self) -> anyhow::Result<()> {
        self.seed_server_asset_catalogs().await?;
        self.upsert_resource_hash(configured_resource_hash_hex())
            .await?;
        Ok(())
    }

    pub async fn seed_demo_world(&self) -> anyhow::Result<()> {
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM accounts WHERE username = 'admin'")
                .fetch_optional(&self.pool)
                .await?;
        if existing.is_some() {
            self.seed_digi_summon_demo().await?;
            self.seed_extra_evolution_demo().await?;
            self.seed_combine_demo().await?;
            self.upsert_resource_hash(
                configured_resource_hash_hex().or_else(|| Some("0123456789ABCDEF".to_string())),
            )
            .await?;
            return Ok(());
        }

        // Accounts
        sqlx::query(
            "INSERT INTO accounts (id, username, password_hash, email, access_level, secondary_password) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(1i64)
        .bind("admin")
        .bind("admin")
        .bind("admin@odmo.local")
        .bind(2i16) // Administrator
        .bind("4321")
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO accounts (id, username, password_hash, email, access_level, secondary_password) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(2i64)
        .bind("gm")
        .bind("gm")
        .bind("gm@odmo.local")
        .bind(1i16) // GameMaster
        .bind("4321")
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO accounts (id, username, password_hash, email, access_level, suspension_remaining_seconds, suspension_reason) VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(3i64)
        .bind("banned")
        .bind("banned")
        .bind("banned@odmo.local")
        .bind(0i16) // Player
        .bind(3600i32)
        .bind("Policy violation")
        .execute(&self.pool)
        .await?;

        // Default smoke account used by Tools/client-testing/Run-ClientSmokeTest.ps1
        // (its -Username default is "ODMO" / "123456"). Mirrors the JSON dev seed.
        sqlx::query(
            "INSERT INTO accounts (id, username, password_hash, email, access_level) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(4i64)
        .bind("ODMO")
        .bind("123456")
        .bind("smoke@odmo.local")
        .bind(0i16) // Player
        .execute(&self.pool)
        .await?;

        // Servers
        sqlx::query(
            "INSERT INTO servers (id, name, maintenance, overloaded, is_new, character_count) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(1i32)
        .bind("ODMO Alpha")
        .bind(false)
        .bind(false)
        .bind(true)
        .bind(0i16)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO servers (id, name, maintenance, overloaded, is_new, character_count) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(2i32)
        .bind("ODMO Beta")
        .bind(false)
        .bind(false)
        .bind(false)
        .bind(0i16)
        .execute(&self.pool)
        .await?;

        // Characters
        let default_inv = serde_json::json!({"bits": 0, "size": 30, "items": []});
        let default_warehouse = serde_json::json!({"bits": 0, "size": 21, "items": []});
        let default_account_warehouse = serde_json::json!({"bits": 0, "size": 14, "items": []});
        let default_seals = serde_json::json!({"seal_leader_id": 0, "seals": []});
        let default_channels = serde_json::json!([{"channel": 0, "load": 1}]);
        let default_encyclopedia = serde_json::json!({"entries": []});
        let admin_partner_slots = serde_json::json!([
            {
                "slot": 1, "digimon_type": DEFAULT_PARTNER_MODEL_ID, "model": DEFAULT_PARTNER_MODEL_ID,
                "level": 1, "name": "Agumon", "size": 12000, "hatch_grade": 3,
                "hp": 1000, "ds": 1000, "current_hp": 1000, "current_ds": 1000,
                "de": 100, "at": 100, "fs": 100, "ev": 0, "cc": 0, "ms": 250, "as_value": 1000,
                "ht": 0, "ar": 0, "bl": 0, "clone_level": 0,
                "clone_at_value": 0, "clone_bl_value": 0, "clone_ct_value": 0, "clone_ev_value": 0, "clone_hp_value": 0,
                "clone_at_level": 0, "clone_bl_level": 0, "clone_ct_level": 0, "clone_ev_level": 0, "clone_hp_level": 0,
                "active_buffs": []
            },
            {
                "slot": 2, "digimon_type": 31002, "model": 31002,
                "level": 11, "name": "Greymon", "size": 13000, "hatch_grade": 4,
                "hp": 1400, "ds": 1200, "current_hp": 1400, "current_ds": 1200,
                "de": 120, "at": 150, "fs": 120, "ev": 8, "cc": 5, "ms": 260, "as_value": 950,
                "ht": 3, "ar": 1, "bl": 2, "clone_level": 3,
                "clone_at_value": 1, "clone_bl_value": 1, "clone_ct_value": 0, "clone_ev_value": 0, "clone_hp_value": 1,
                "clone_at_level": 1, "clone_bl_level": 1, "clone_ct_level": 0, "clone_ev_level": 0, "clone_hp_level": 1,
                "active_buffs": []
            }
        ]);
        let gm_partner_slots = serde_json::json!([
            {
                "slot": 1, "digimon_type": DEFAULT_GM_PARTNER_MODEL_ID, "model": DEFAULT_GM_PARTNER_MODEL_ID,
                "level": 1, "name": "Gabumon", "size": 12000, "hatch_grade": 3,
                "hp": 1000, "ds": 1000, "current_hp": 1000, "current_ds": 1000,
                "de": 100, "at": 100, "fs": 100, "ev": 0, "cc": 0, "ms": 250, "as_value": 1000,
                "ht": 0, "ar": 0, "bl": 0, "clone_level": 0,
                "clone_at_value": 0, "clone_bl_value": 0, "clone_ct_value": 0, "clone_ev_value": 0, "clone_hp_value": 0,
                "clone_at_level": 0, "clone_bl_level": 0, "clone_ct_level": 0, "clone_ev_level": 0, "clone_hp_level": 0,
                "active_buffs": []
            }
        ]);

        sqlx::query(
            "INSERT INTO characters (id, account_id, slot, name, model, level, current_x, current_y, current_map_id, partner_current_x, partner_current_y, partner_current_slot, general_handler, partner_handler, partner_name, partner_model, bits, inventory, warehouse, extra_inventory, account_warehouse, seal_list, available_channels, partner_slots, encyclopedia, deck_buff_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26)",
        )
        .bind(100i64)
        .bind(1i64)
        .bind(0i16)
        .bind("AdminTamer")
        .bind(DEFAULT_TAMER_MODEL_ID)
        .bind(1i16)
        .bind(DEFAULT_START_X)
        .bind(DEFAULT_START_Y)
        .bind(DEFAULT_START_MAP_ID)
        .bind(DEFAULT_START_X)
        .bind(DEFAULT_START_Y)
        .bind(1i16)
        .bind(11000i32)
        .bind(21000i32)
        .bind("Agumon")
        .bind(DEFAULT_PARTNER_MODEL_ID)
        .bind(0i64)
        .bind(&default_inv)
        .bind(&default_warehouse)
        .bind(&default_inv)
        .bind(&default_account_warehouse)
        .bind(&default_seals)
        .bind(&default_channels)
        .bind(&admin_partner_slots)
        .bind(&default_encyclopedia)
        .bind(0i32)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO characters (id, account_id, slot, name, model, level, current_x, current_y, current_map_id, partner_current_x, partner_current_y, partner_current_slot, general_handler, partner_handler, partner_name, partner_model, bits, inventory, warehouse, extra_inventory, account_warehouse, seal_list, available_channels, partner_slots, encyclopedia, deck_buff_id) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26)",
        )
        .bind(200i64)
        .bind(2i64)
        .bind(0i16)
        .bind("GmTamer")
        .bind(DEFAULT_GM_TAMER_MODEL_ID)
        .bind(1i16)
        .bind(DEFAULT_START_X)
        .bind(DEFAULT_START_Y)
        .bind(DEFAULT_START_MAP_ID)
        .bind(DEFAULT_START_X)
        .bind(DEFAULT_START_Y)
        .bind(1i16)
        .bind(12000i32)
        .bind(22000i32)
        .bind("Gabumon")
        .bind(DEFAULT_GM_PARTNER_MODEL_ID)
        .bind(0i64)
        .bind(&default_inv)
        .bind(&default_warehouse)
        .bind(&default_inv)
        .bind(&default_account_warehouse)
        .bind(&default_seals)
        .bind(&default_channels)
        .bind(&gm_partner_slots)
        .bind(&default_encyclopedia)
        .bind(0i32)
        .execute(&self.pool)
        .await?;

        // Smoke account character (account 4 = "ODMO"), bound to Agumon so the
        // native auto-login + direct-character-select smoke gate passes.
        let smoke_partner_slots = serde_json::json!([
            {
                "slot": 1, "digimon_type": DEFAULT_PARTNER_MODEL_ID, "model": DEFAULT_PARTNER_MODEL_ID,
                "level": 1, "name": "Agumon", "size": 12000, "hatch_grade": 3,
                "hp": 1000, "ds": 1000, "current_hp": 1000, "current_ds": 1000,
                "de": 100, "at": 100, "fs": 100, "ev": 0, "cc": 0, "ms": 250, "as_value": 1000,
                "ht": 0, "ar": 0, "bl": 0, "clone_level": 0,
                "clone_at_value": 0, "clone_bl_value": 0, "clone_ct_value": 0, "clone_ev_value": 0, "clone_hp_value": 0,
                "clone_at_level": 0, "clone_bl_level": 0, "clone_ct_level": 0, "clone_ev_level": 0, "clone_hp_level": 0,
                "active_buffs": []
            }
        ]);

        let smoke_inv = serde_json::to_value(crate::smoke_tamer_inventory())?;
        let smoke_quests = serde_json::to_value(crate::smoke_tamer_quest_progress())?;

        sqlx::query(
            "INSERT INTO characters (id, account_id, slot, name, model, level, current_x, current_y, current_map_id, partner_current_x, partner_current_y, partner_current_slot, general_handler, partner_handler, partner_name, partner_model, bits, inventory, warehouse, extra_inventory, account_warehouse, seal_list, available_channels, partner_slots, encyclopedia, deck_buff_id, quest_progress) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27)",
        )
        .bind(300i64)
        .bind(4i64)
        .bind(0i16)
        .bind("SmokeTamer")
        .bind(DEFAULT_TAMER_MODEL_ID)
        .bind(1i16)
        .bind(DEFAULT_START_X)
        .bind(DEFAULT_START_Y)
        .bind(DEFAULT_START_MAP_ID)
        .bind(DEFAULT_START_X)
        .bind(DEFAULT_START_Y)
        .bind(1i16)
        .bind(13000i32)
        .bind(23000i32)
        .bind("Agumon")
        .bind(DEFAULT_PARTNER_MODEL_ID)
        .bind(500_000i64)
        .bind(&smoke_inv)
        .bind(&default_warehouse)
        .bind(&default_inv)
        .bind(&default_account_warehouse)
        .bind(&default_seals)
        .bind(&default_channels)
        .bind(&smoke_partner_slots)
        .bind(&default_encyclopedia)
        .bind(0i32)
        .bind(&smoke_quests)
        .execute(&self.pool)
        .await?;

        self.seed_digi_summon_demo().await?;
        self.seed_extra_evolution_demo().await?;
        self.seed_combine_demo().await?;

        // Mobs
        sqlx::query(
            "INSERT INTO map_mobs (map_id, channel, handler, type_id, model, name, level, x, y, previous_x, previous_y, current_hp, max_hp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        )
        .bind(DEFAULT_START_MAP_ID)
        .bind(0i16)
        .bind(34000i32)
        .bind(30331i32)
        .bind(31033i32)
        .bind("Goblimon")
        .bind(18i16)
        .bind(DEFAULT_START_X + 66)
        .bind(DEFAULT_START_Y + 45)
        .bind(DEFAULT_START_X + 46)
        .bind(DEFAULT_START_Y + 25)
        .bind(1000i32)
        .bind(1000i32)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO map_mobs (map_id, channel, handler, type_id, model, name, level, x, y, previous_x, previous_y, current_hp, max_hp) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)",
        )
        .bind(DEFAULT_START_MAP_ID)
        .bind(0i16)
        .bind(34001i32)
        .bind(40391i32)
        .bind(41039i32)
        .bind("Woodmon")
        .bind(12i16)
        .bind(DEFAULT_START_X + 416)
        .bind(DEFAULT_START_Y + 225)
        .bind(DEFAULT_START_X + 386)
        .bind(DEFAULT_START_Y + 195)
        .bind(1000i32)
        .bind(1000i32)
        .execute(&self.pool)
        .await?;

        // Drops
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        sqlx::query(
            "INSERT INTO map_drops (map_id, channel, handler, owner_id, owner_handler, item_id, amount, x, y, owner_expires_at_unix, expires_at_unix, bits_drop) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        )
        .bind(DEFAULT_START_MAP_ID)
        .bind(0i16)
        .bind(49200i32)
        .bind(100i64)
        .bind(11000i32)
        .bind(90600i32)
        .bind(123i32)
        .bind(DEFAULT_START_X + 76)
        .bind(DEFAULT_START_Y + 55)
        .bind(now + 60)
        .bind(now + 90)
        .bind(true)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO map_drops (map_id, channel, handler, item_id, amount, x, y, owner_expires_at_unix, expires_at_unix, no_owner) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)",
        )
        .bind(DEFAULT_START_MAP_ID)
        .bind(0i16)
        .bind(49201i32)
        .bind(5101i32)
        .bind(1i32)
        .bind(DEFAULT_START_X + 286)
        .bind(DEFAULT_START_Y + 145)
        .bind(now.saturating_sub(5))
        .bind(now + 30)
        .bind(true)
        .execute(&self.pool)
        .await?;

        self.upsert_resource_hash(
            configured_resource_hash_hex().or_else(|| Some("0123456789ABCDEF".to_string())),
        )
        .await?;

        Ok(())
    }

    async fn seed_server_asset_catalogs(&self) -> anyhow::Result<()> {
        let evolution_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM evolution_assets")
            .fetch_one(&self.pool)
            .await?;
        let item_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM item_assets")
            .fetch_one(&self.pool)
            .await?;
        let digimon_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM digimon_assets")
            .fetch_one(&self.pool)
            .await?;

        if evolution_count > 0 && item_count > 0 && digimon_count > 0 {
            return Ok(());
        }

        let evolution_assets = crate::load_evolution_asset_catalog()?;
        let item_assets = crate::load_item_asset_catalog()?;
        let digimon_assets = crate::load_digimon_asset_catalog()?;
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM evolution_assets")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM item_assets")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM digimon_assets")
            .execute(&mut *tx)
            .await?;

        for asset in evolution_assets {
            let payload = serde_json::to_value(&asset)?;
            sqlx::query(
                "INSERT INTO evolution_assets (base_type, payload) VALUES ($1, $2) \
                 ON CONFLICT (base_type) DO UPDATE SET payload = EXCLUDED.payload",
            )
            .bind(asset.base_type)
            .bind(payload)
            .execute(&mut *tx)
            .await?;
        }

        for asset in item_assets {
            let payload = serde_json::to_value(&asset)?;
            sqlx::query(
                "INSERT INTO item_assets (item_id, payload) VALUES ($1, $2) \
                 ON CONFLICT (item_id) DO UPDATE SET payload = EXCLUDED.payload",
            )
            .bind(asset.item_id)
            .bind(payload)
            .execute(&mut *tx)
            .await?;
        }

        for asset in digimon_assets {
            let payload = serde_json::to_value(&asset)?;
            sqlx::query(
                "INSERT INTO digimon_assets (digimon_id, payload) VALUES ($1, $2) \
                 ON CONFLICT (digimon_id) DO UPDATE SET payload = EXCLUDED.payload",
            )
            .bind(asset.digimon_id)
            .bind(payload)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        Ok(())
    }

    async fn seed_digi_summon_demo(&self) -> anyhow::Result<()> {
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM digi_summon_products WHERE product_id = $1")
                .bind(9001i32)
                .fetch_optional(&self.pool)
                .await?;
        if existing.is_some() {
            return Ok(());
        }

        sqlx::query(
            "INSERT INTO digi_summon_products (product_id, string_id, draw_count, rank, remaining_daily_limit, icon, name, description) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)",
        )
        .bind(9001i32)
        .bind(10001i32)
        .bind(1i32)
        .bind(1i32)
        .bind(0i32)
        .bind("digi_summon/sample_box.tga")
        .bind("Sample DigiSummon Box")
        .bind("Demo DigiSummon product used by the Rust smoke environment.")
        .execute(&self.pool)
        .await?;

        let product_row: (i64,) =
            sqlx::query_as("SELECT id FROM digi_summon_products WHERE product_id = $1")
                .bind(9001i32)
                .fetch_one(&self.pool)
                .await?;

        sqlx::query(
            "INSERT INTO digi_summon_tickets (product_row_id, item_id, cost) VALUES ($1,$2,$3), ($1,$4,$5)",
        )
        .bind(product_row.0)
        .bind(crate::DEMO_CATALOG_ITEM_A)
        .bind(1i32)
        .bind(crate::DEMO_CATALOG_ITEM_B)
        .bind(10i32)
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO digi_summon_rewards (product_row_id, item_list_id, item_id, grade, amount, weight, reward_group, group_code) VALUES ($1,$2,$3,$4,$5,$6,$7,$8), ($1,$9,$10,$11,$12,$13,$14,$15)",
        )
        .bind(product_row.0)
        .bind(1i32)
        .bind(5101i32)
        .bind(1i32)
        .bind(1i32)
        .bind(80i32)
        .bind(0i32)
        .bind(0i32)
        .bind(2i32)
        .bind(5102i32)
        .bind(2i32)
        .bind(1i32)
        .bind(20i32)
        .bind(0i32)
        .bind(0i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn seed_extra_evolution_demo(&self) -> anyhow::Result<()> {
        let existing: Option<(i64,)> =
            sqlx::query_as("SELECT id FROM extra_evolution_npcs WHERE npc_id = $1")
                .bind(91001i32)
                .fetch_optional(&self.pool)
                .await?;
        if existing.is_some() {
            return Ok(());
        }

        let npc_row: (i64,) =
            sqlx::query_as("INSERT INTO extra_evolution_npcs (npc_id) VALUES ($1) RETURNING id")
                .bind(91001i32)
                .fetch_one(&self.pool)
                .await?;

        let item_to_digimon_row: (i64,) = sqlx::query_as(
            "INSERT INTO extra_evolution_recipes \
             (npc_row_id, exchange_type, object_id, material_type, need_material_value, price, way_type) \
             VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id",
        )
        .bind(npc_row.0)
        .bind(1i16)
        .bind(31004i32)
        .bind(2i16)
        .bind(0i32)
        .bind(500i64)
        .bind(1i16)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO extra_evolution_materials (recipe_row_id, material_scope, material_id, amount) \
             VALUES ($1,$2,$3,$4), ($1,$5,$6,$7)",
        )
        .bind(item_to_digimon_row.0)
        .bind(1i16)
        .bind(crate::DEMO_CATALOG_ITEM_A)
        .bind(1i32)
        .bind(2i16)
        .bind(crate::DEMO_CATALOG_ITEM_B)
        .bind(1i32)
        .execute(&self.pool)
        .await?;

        let digimon_to_item_row: (i64,) = sqlx::query_as(
            "INSERT INTO extra_evolution_recipes \
             (npc_row_id, exchange_type, object_id, material_type, need_material_value, price, way_type) \
             VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id",
        )
        .bind(npc_row.0)
        .bind(2i16)
        .bind(crate::DEMO_CATALOG_ITEM_C)
        .bind(1i16)
        .bind(10i32)
        .bind(250i64)
        .bind(1i16)
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(
            "INSERT INTO extra_evolution_materials (recipe_row_id, material_scope, material_id, amount) \
             VALUES ($1,$2,$3,$4), ($1,$5,$6,$7)",
        )
        .bind(digimon_to_item_row.0)
        .bind(1i16)
        .bind(31002i32)
        .bind(1i32)
        .bind(2i16)
        .bind(crate::DEMO_CATALOG_ITEM_A)
        .bind(1i32)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn seed_combine_demo(&self) -> anyhow::Result<()> {
        // Seed both variants: 0 = Digi Combine, 1 = Union Combine.
        for variant in [0i16, 1i16] {
            let existing: Option<(i64,)> =
                sqlx::query_as("SELECT id FROM combine_ranks WHERE variant = $1 LIMIT 1")
                    .bind(variant)
                    .fetch_optional(&self.pool)
                    .await?;
            if existing.is_some() {
                continue;
            }

            // Rank tier with a small weighted reward pool.
            let rank_row: (i64,) = sqlx::query_as(
                "INSERT INTO combine_ranks (variant, ceiling_type, weight) VALUES ($1,$2,$3) RETURNING id",
            )
            .bind(variant)
            .bind(1i16)
            .bind(100i64)
            .fetch_one(&self.pool)
            .await?;

            sqlx::query(
                "INSERT INTO combine_rank_rewards (rank_row_id, item_id, amount, grade) \
                 VALUES ($1,$2,$3,$4), ($1,$5,$6,$7)",
            )
            .bind(rank_row.0)
            .bind(5101i32)
            .bind(1i32)
            .bind(1i16)
            .bind(5102i32)
            .bind(2i32)
            .bind(2i16)
            .execute(&self.pool)
            .await?;

            // Allowed materials referencing one group.
            sqlx::query(
                "INSERT INTO combine_items (variant, item_id, group_id) \
                 VALUES ($1,$2,$3), ($1,$4,$5)",
            )
            .bind(variant)
            .bind(crate::DEMO_CATALOG_ITEM_A)
            .bind(1i32)
            .bind(crate::DEMO_CATALOG_ITEM_B)
            .bind(1i32)
            .execute(&self.pool)
            .await?;

            let group_row: (i64,) = sqlx::query_as(
                "INSERT INTO combine_groups (variant, group_id) VALUES ($1,$2) RETURNING id",
            )
            .bind(variant)
            .bind(1i32)
            .fetch_one(&self.pool)
            .await?;

            sqlx::query(
                "INSERT INTO combine_group_members (group_row_id, member_id) VALUES ($1,$2), ($1,$3)",
            )
            .bind(group_row.0)
            .bind(crate::DEMO_CATALOG_ITEM_A)
            .bind(crate::DEMO_CATALOG_ITEM_B)
            .execute(&self.pool)
            .await?;

            // Ceiling tier with its progress entries.
            let ceil_row: (i64,) = sqlx::query_as(
                "INSERT INTO combine_ceils (variant, ceiling_type) VALUES ($1,$2) RETURNING id",
            )
            .bind(variant)
            .bind(1i16)
            .fetch_one(&self.pool)
            .await?;

            sqlx::query(
                "INSERT INTO combine_ceil_entries (ceil_row_id, tier, value_a, value_b) \
                 VALUES ($1,$2,$3,$4), ($1,$5,$6,$7)",
            )
            .bind(ceil_row.0)
            .bind(1i16)
            .bind(10i16)
            .bind(100i32)
            .bind(2i16)
            .bind(20i16)
            .bind(200i32)
            .execute(&self.pool)
            .await?;
        }

        Ok(())
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Bridge sync trait methods to async sqlx calls.
    /// Uses `tokio::task::block_in_place` to safely block inside a tokio runtime.
    pub(crate) fn block_on<F: Future>(&self, f: F) -> F::Output {
        tokio::task::block_in_place(move || {
            let handle = tokio::runtime::Handle::current();
            handle.block_on(f)
        })
    }

    async fn upsert_resource_hash(&self, hash_hex: Option<String>) -> anyhow::Result<()> {
        let Some(hash_hex) = hash_hex.filter(|value| !value.trim().is_empty()) else {
            return Ok(());
        };

        sqlx::query(
            "INSERT INTO server_config (key, value) VALUES ($1, $2) \
             ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
        )
        .bind("resource_hash_hex")
        .bind(hash_hex)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

fn configured_resource_hash_hex() -> Option<String> {
    std::env::var("ODMO_RESOURCE_HASH_HEX")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}
