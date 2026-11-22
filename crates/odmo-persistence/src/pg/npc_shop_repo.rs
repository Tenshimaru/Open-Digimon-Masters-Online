use odmo_application::game::{NpcShopDefinition, NpcShopItem, NpcShopRepository};

use super::PgRepository;

#[derive(sqlx::FromRow)]
struct NpcShopDbRow {
    npc_id: i32,
    map_id: i16,
    shop_type: i16,
    items: serde_json::Value,
}

impl NpcShopRepository for PgRepository {
    fn shop_by_npc(&self, npc_id: i32, map_id: i16) -> anyhow::Result<Option<NpcShopDefinition>> {
        let pool = self.pool().clone();
        self.block_on(async move {
            let row: Option<NpcShopDbRow> = sqlx::query_as(
                "SELECT npc_id, map_id, shop_type, items FROM npc_shops WHERE npc_id = $1 AND map_id = $2",
            )
            .bind(npc_id)
            .bind(map_id)
            .fetch_optional(&pool)
            .await?;

            let Some(row) = row else {
                return Ok(None);
            };
            let items: Vec<NpcShopItem> = serde_json::from_value(row.items)?;
            Ok(Some(NpcShopDefinition {
                npc_id: row.npc_id,
                map_id: row.map_id,
                shop_type: row.shop_type.clamp(0, u8::MAX as i16) as u8,
                items,
            }))
        })
    }
}
