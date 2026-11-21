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

            Ok(rows.into_iter().map(map_drop).collect())
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
        self.block_on(async move {
            // One transaction with the drop row locked: a concurrent pickup of the same
            // drop waits here and then finds it gone, so a drop is only ever paid out once.
            let mut tx = pool.begin().await?;
            let drop_row: Option<DropDb> = sqlx::query_as(
                "SELECT id, map_id, channel, handler, owner_id, owner_handler, item_id, amount, x, y, owner_expires_at_unix, expires_at_unix, bits_drop, no_owner, collected FROM map_drops WHERE map_id = $1 AND channel = $2 AND handler = $3 FOR UPDATE",
            )
            .bind(map_id)
            .bind(channel as i16)
            .bind(drop_handler as i32)
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

            // Load (and lock) the character: position for the distance check and, for
            // items, the inventory that is rewritten below.
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
                tx.commit().await?;

                let character = load_character(&pool, character_id).await?;
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
            tx.commit().await?;

            let character = load_character(&pool, character_id).await?;
            Ok(DropCollectionResult::ItemCollected { drop, item_id, amount, character })
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
    if let Some(existing) = items.iter_mut().find(|item| item.item_id == item_id) {
        existing.amount = existing.amount.saturating_add(amount.max(0));
        existing.sync_record();
        return true;
    }

    if items.len() >= size as usize {
        return false;
    }

    items.push(ItemRecord::new(item_id, amount.max(1)));
    true
}

async fn load_character(
    pool: &sqlx::PgPool,
    character_id: u64,
) -> anyhow::Result<odmo_types::CharacterSummary> {
    let query = character_by_id_query();
    let row: CharacterDb = sqlx::query_as(&query)
        .bind(character_id as i64)
        .fetch_one(pool)
        .await
        .map_err(|e| anyhow::anyhow!("load_character: {e}"))?;

    Ok(row_to_character(row))
}
