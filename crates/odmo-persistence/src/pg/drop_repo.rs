use odmo_application::game::{DropCollectionResult, MapDropRepository};
use odmo_types::{DropSummary, InventorySnapshot, ItemRecord};

use super::PgRepository;
use super::character_repo::{CharacterDb, character_by_id_query, row_to_character};

impl MapDropRepository for PgRepository {
    fn drops_by_map(&self, map_id: i16, channel: u8) -> anyhow::Result<Vec<DropSummary>> {
        let pool = self.pool().clone();
        self.block_on(async move {
            let rows: Vec<DropDb> = sqlx::query_as(
                "SELECT id, map_id, channel, handler, owner_id, owner_handler, item_id, amount, x, y, owner_expires_at_unix, expires_at_unix, bits_drop, no_owner, collected FROM map_drops WHERE map_id = $1 AND channel = $2 AND NOT collected",
            )
            .bind(map_id)
            .bind(channel as i16)
            .fetch_all(&pool)
            .await?;

            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            Ok(rows
                .into_iter()
                .map(map_drop)
                .filter_map(|mut drop| {
                    if drop.expires_at_unix > 0 && now >= drop.expires_at_unix {
                        drop.collected = true;
                    }
                    if !drop.collected
                        && drop.owner_expires_at_unix > 0
                        && now >= drop.owner_expires_at_unix
                    {
                        drop.no_owner = true;
                    }
                    (!drop.collected).then_some(drop)
                })
                .collect())
        })
    }

    fn collect_drop(
        &self,
        character_id: u64,
        map_id: i16,
        channel: u8,
        drop_handler: u32,
    ) -> anyhow::Result<DropCollectionResult> {
        let pool = self.pool().clone();
        let kind = drop_handler >> 16;
        let raw_handler = drop_handler as i32;
        let low_handler = if kind == 3 || kind == 0 {
            (drop_handler & 0xFFFF) as i32
        } else {
            raw_handler
        };
        self.block_on(async move {
            // Lock the drop and character rows inside one transaction so concurrent
            // pickups serialize here and a drop can only be paid out once.
            let mut tx = pool.begin().await?;
            let drop_row: Option<DropDb> = sqlx::query_as(
                "SELECT id, map_id, channel, handler, owner_id, owner_handler, item_id, amount, x, y, owner_expires_at_unix, expires_at_unix, bits_drop, no_owner, collected \
                 FROM map_drops \
                 WHERE map_id = $1 AND channel = $2 AND (handler = $3 OR handler = $4) \
                 ORDER BY (handler = $3) DESC \
                 LIMIT 1 \
                 FOR UPDATE",
            )
            .bind(map_id)
            .bind(channel as i16)
            .bind(raw_handler)
            .bind(low_handler)
            .fetch_optional(&mut *tx)
            .await?;

            let Some(drop_raw) = drop_row else {
                return Ok(DropCollectionResult::Missing);
            };

            let mut drop = map_drop(drop_raw);

            // Apply runtime state
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            if drop.expires_at_unix > 0 && now >= drop.expires_at_unix {
                drop.collected = true;
            }
            if !drop.collected && drop.owner_expires_at_unix > 0 && now >= drop.owner_expires_at_unix {
                drop.no_owner = true;
            }

            let drop_id = drop.id as i64;

            if drop.collected {
                sqlx::query("DELETE FROM map_drops WHERE id = $1")
                    .bind(drop_id)
                    .execute(&mut *tx)
                    .await?;
                tx.commit().await?;
                return Ok(DropCollectionResult::Missing);
            }

            // Lock the character row too so position checks and inventory rewrites stay atomic.
            let char_row: Option<(i32, i32, serde_json::Value)> = sqlx::query_as(
                "SELECT current_x, current_y, inventory FROM characters WHERE id = $1 FOR UPDATE",
            )
            .bind(character_id as i64)
            .fetch_optional(&mut *tx)
            .await?;

            let Some((char_x, char_y, inv_json)) = char_row else {
                return Ok(DropCollectionResult::Missing);
            };

            // Distance check
            let dx = (drop.x as i64 - char_x as i64).pow(2);
            let dy = (drop.y as i64 - char_y as i64).pow(2);
            let distance = ((dx + dy) as f64).sqrt() as i64;
            if distance >= 18_001 {
                return Ok(DropCollectionResult::TooFarAway);
            }

            // Ownership check
            if drop.owner_id != 0 && drop.owner_id != character_id && !drop.no_owner {
                return Ok(DropCollectionResult::NotTheOwner);
            }

            // Bits collection
            if drop.bits_drop {
                let amount = drop.amount;
                sqlx::query("UPDATE characters SET bits = bits + $1 WHERE id = $2")
                    .bind(i64::from(amount.max(0)))
                    .bind(character_id as i64)
                    .execute(&mut *tx)
                    .await?;

                sqlx::query("DELETE FROM map_drops WHERE id = $1")
                    .bind(drop_id)
                    .execute(&mut *tx)
                    .await?;

                let character = load_character(&mut *tx, character_id).await?;
                tx.commit().await?;
                return Ok(DropCollectionResult::BitsCollected { drop, amount, character });
            }

            // Item collection
            let mut inventory: InventorySnapshot =
                serde_json::from_value(inv_json).unwrap_or_default();

            if !add_inventory_item(&mut inventory.items, inventory.size, drop.item_id, drop.amount) {
                return Ok(DropCollectionResult::InventoryFull);
            }

            let item_id = drop.item_id;
            let amount = drop.amount.clamp(i16::MIN as i32, i16::MAX as i32) as i16;

            let inv_ser = serde_json::to_value(&inventory)?;
            sqlx::query("UPDATE characters SET inventory = $1 WHERE id = $2")
                .bind(&inv_ser)
                .bind(character_id as i64)
                .execute(&mut *tx)
                .await?;

            sqlx::query("DELETE FROM map_drops WHERE id = $1")
                .bind(drop_id)
                .execute(&mut *tx)
                .await?;

            let character = load_character(&mut *tx, character_id).await?;
            tx.commit().await?;
            Ok(DropCollectionResult::ItemCollected { drop, item_id, amount, character })
        })
    }

    fn spawn_drop(&self, mut drop: DropSummary) -> anyhow::Result<DropSummary> {
        let pool = self.pool().clone();
        self.block_on(async move {
            let max_handler: (Option<i32>,) = sqlx::query_as(
                "SELECT MAX(handler) FROM map_drops WHERE map_id = $1 AND channel = $2",
            )
            .bind(drop.map_id)
            .bind(drop.channel as i16)
            .fetch_one(&pool)
            .await?;

            let mut next_handler = max_handler
                .0
                .map_or(49_200, |h| h.saturating_add(1).max(49_200));
            if next_handler > 65_000 {
                next_handler = 49_200;
            }
            drop.handler = next_handler as u32;

            let row: (i64,) = sqlx::query_as(
                "INSERT INTO map_drops (map_id, channel, handler, owner_id, owner_handler, item_id, amount, x, y, owner_expires_at_unix, expires_at_unix, bits_drop, no_owner, collected) \
                 VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14) RETURNING id",
            )
            .bind(drop.map_id)
            .bind(drop.channel as i16)
            .bind(drop.handler as i32)
            .bind(drop.owner_id as i64)
            .bind(drop.owner_handler as i32)
            .bind(drop.item_id)
            .bind(drop.amount)
            .bind(drop.x)
            .bind(drop.y)
            .bind(drop.owner_expires_at_unix as i64)
            .bind(drop.expires_at_unix as i64)
            .bind(drop.bits_drop)
            .bind(drop.no_owner)
            .bind(drop.collected)
            .fetch_one(&pool)
            .await?;

            drop.id = row.0 as u64;
            Ok(drop)
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct DropDb {
    id: i64,
    map_id: i16,
    channel: i16,
    handler: i32,
    owner_id: i64,
    owner_handler: i32,
    item_id: i32,
    amount: i32,
    x: i32,
    y: i32,
    owner_expires_at_unix: i64,
    expires_at_unix: i64,
    bits_drop: bool,
    no_owner: bool,
    collected: bool,
}

fn map_drop(row: DropDb) -> DropSummary {
    DropSummary {
        id: row.id as u64,
        map_id: row.map_id,
        channel: row.channel as u8,
        handler: row.handler as u32,
        owner_id: row.owner_id as u64,
        owner_handler: row.owner_handler as u32,
        item_id: row.item_id,
        amount: row.amount,
        x: row.x,
        y: row.y,
        owner_expires_at_unix: row.owner_expires_at_unix as u64,
        expires_at_unix: row.expires_at_unix as u64,
        bits_drop: row.bits_drop,
        no_owner: row.no_owner,
        collected: row.collected,
    }
}

fn add_inventory_item(items: &mut Vec<ItemRecord>, size: u16, item_id: i32, amount: i32) -> bool {
    let limit = usize::from(size);
    let safe_amount = amount.max(1);
    if let Some(existing) = items
        .iter_mut()
        .take(limit)
        .find(|item| item.item_id == item_id && item.amount > 0)
    {
        existing.amount = existing.amount.saturating_add(safe_amount);
        existing.sync_record();
        return true;
    }

    if let Some(empty) = items
        .iter_mut()
        .take(limit)
        .find(|item| item.item_id <= 0 || item.amount <= 0)
    {
        *empty = ItemRecord::new(item_id, safe_amount);
        return true;
    }

    if items.len() >= limit {
        return false;
    }

    items.push(ItemRecord::new(item_id, safe_amount));
    true
}

async fn load_character(
    conn: &mut sqlx::PgConnection,
    character_id: u64,
) -> anyhow::Result<odmo_types::CharacterSummary> {
    let query = character_by_id_query();
    let row: CharacterDb = sqlx::query_as(&query)
        .bind(character_id as i64)
        .fetch_one(conn)
        .await
        .map_err(|e| anyhow::anyhow!("load_character: {e}"))?;

    Ok(row_to_character(row))
}
