use anyhow::Context;
use odmo_application::game::MapMobRepository;
use odmo_types::{ActiveBuffSnapshot, MobSummary};

use crate::pg::PgRepository;

#[derive(Debug, sqlx::FromRow)]
struct MobRow {
    id: i64,
    map_id: i16,
    channel: i16,
    handler: i32,
    type_id: i32,
    model: i32,
    name: String,
    level: i16,
    x: i32,
    y: i32,
    previous_x: i32,
    previous_y: i32,
    current_hp: i32,
    max_hp: i32,
    #[allow(dead_code)]
    current_ds: i32,
    #[allow(dead_code)]
    max_ds: i32,
    #[allow(dead_code)]
    alive: bool,
    respawn: bool,
    active_debuffs: serde_json::Value,
}

impl From<MobRow> for MobSummary {
    fn from(r: MobRow) -> Self {
        let debuffs: Vec<ActiveBuffSnapshot> =
            serde_json::from_value(r.active_debuffs).unwrap_or_default();

        MobSummary {
            id: r.id as u64,
            map_id: r.map_id,
            channel: r.channel as u8,
            handler: r.handler as u32,
            type_id: r.type_id,
            model: r.model,
            name: r.name,
            level: r.level as u8,
            x: r.x,
            y: r.y,
            previous_x: r.previous_x,
            previous_y: r.previous_y,
            current_hp: r.current_hp,
            max_hp: r.max_hp,
            grow_stack: 0,
            disposed_objects: 0,
            respawn: r.respawn,
            active_debuffs: debuffs,
        }
    }
}

impl MapMobRepository for PgRepository {
    fn mobs_by_map(&self, map_id: i16, channel: u8) -> anyhow::Result<Vec<MobSummary>> {
        let pool = self.pool().clone();
        let ch = channel as i16;
        self.block_on(async move {
            let rows = sqlx::query_as::<_, MobRow>(
                "SELECT id, map_id, channel, handler, type_id, model, name, level, \
                 x, y, previous_x, previous_y, current_hp, max_hp, current_ds, max_ds, \
                 alive, respawn, active_debuffs \
                 FROM map_mobs WHERE map_id = $1 AND channel = $2",
            )
            .bind(map_id)
            .bind(ch)
            .fetch_all(&pool)
            .await
            .context("failed to query mobs by map")?;

            Ok(rows.into_iter().map(MobSummary::from).collect())
        })
    }

    fn update_mob_hp(
        &self,
        map_id: i16,
        channel: u8,
        handler: u32,
        current_hp: i32,
    ) -> anyhow::Result<()> {
        let pool = self.pool().clone();
        let ch = channel as i16;
        let hp = current_hp.max(0);
        self.block_on(async move {
            sqlx::query(
                "UPDATE map_mobs SET current_hp = $1, alive = ($1 > 0) \
                 WHERE map_id = $2 AND channel = $3 AND handler = $4",
            )
            .bind(hp)
            .bind(map_id)
            .bind(ch)
            .bind(handler as i32)
            .execute(&pool)
            .await
            .context("failed to update mob hp")?;

            Ok(())
        })
    }
}
