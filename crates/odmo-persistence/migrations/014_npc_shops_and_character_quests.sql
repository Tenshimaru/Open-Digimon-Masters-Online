ALTER TABLE characters
    ADD COLUMN IF NOT EXISTS quest_progress JSONB NOT NULL DEFAULT '{}'::jsonb,
    ADD COLUMN IF NOT EXISTS npc_repurchase_log JSONB NOT NULL DEFAULT '[]'::jsonb;

CREATE TABLE IF NOT EXISTS npc_shops (
    npc_id INTEGER NOT NULL,
    map_id SMALLINT NOT NULL,
    shop_type SMALLINT NOT NULL DEFAULT 1,
    items JSONB NOT NULL DEFAULT '[]'::jsonb,
    PRIMARY KEY (npc_id, map_id)
);

INSERT INTO npc_shops (npc_id, map_id, shop_type, items) VALUES
(
    90921,
    105,
    1,
    '[
        {"item_id": 40001, "buy_price": 100, "sell_price": 10}
    ]'::jsonb
),
(
    90924,
    105,
    1,
    '[
        {"item_id": 12001, "buy_price": 1000, "sell_price": 100},
        {"item_id": 12008, "buy_price": 1000, "sell_price": 100},
        {"item_id": 12015, "buy_price": 1000, "sell_price": 100},
        {"item_id": 12022, "buy_price": 1000, "sell_price": 100},
        {"item_id": 12029, "buy_price": 1000, "sell_price": 100},
        {"item_id": 12036, "buy_price": 1000, "sell_price": 100},
        {"item_id": 14001, "buy_price": 1000, "sell_price": 100},
        {"item_id": 14007, "buy_price": 1000, "sell_price": 100},
        {"item_id": 14013, "buy_price": 1000, "sell_price": 100},
        {"item_id": 14019, "buy_price": 1000, "sell_price": 100},
        {"item_id": 14025, "buy_price": 1000, "sell_price": 100},
        {"item_id": 14031, "buy_price": 1000, "sell_price": 100},
        {"item_id": 102,   "buy_price": 200,  "sell_price": 20},
        {"item_id": 104,   "buy_price": 400,  "sell_price": 40},
        {"item_id": 106,   "buy_price": 800,  "sell_price": 80},
        {"item_id": 111,   "buy_price": 4000, "sell_price": 400},
        {"item_id": 203,   "buy_price": 400,  "sell_price": 40},
        {"item_id": 205,   "buy_price": 800,  "sell_price": 80},
        {"item_id": 207,   "buy_price": 4000, "sell_price": 400},
        {"item_id": 302,   "buy_price": 200,  "sell_price": 20},
        {"item_id": 304,   "buy_price": 400,  "sell_price": 40},
        {"item_id": 306,   "buy_price": 800,  "sell_price": 80},
        {"item_id": 311,   "buy_price": 4000, "sell_price": 400},
        {"item_id": 401,   "buy_price": 1000, "sell_price": 100},
        {"item_id": 501,   "buy_price": 400,  "sell_price": 40},
        {"item_id": 503,   "buy_price": 800,  "sell_price": 80},
        {"item_id": 508,   "buy_price": 4000, "sell_price": 400}
    ]'::jsonb
),
(
    90927,
    105,
    1,
    '[
        {"item_id": 9000,  "buy_price": 2000,  "sell_price": 100},
        {"item_id": 19999, "buy_price": 1000,  "sell_price": 1},
        {"item_id": 47759, "buy_price": 10000, "sell_price": 1},
        {"item_id": 47761, "buy_price": 10000, "sell_price": 1},
        {"item_id": 47757, "buy_price": 10000, "sell_price": 1}
    ]'::jsonb
),
(
    90932,
    105,
    1,
    '[
        {"item_id": 9000,  "buy_price": 2000,  "sell_price": 100},
        {"item_id": 19999, "buy_price": 1000,  "sell_price": 1},
        {"item_id": 47759, "buy_price": 10000, "sell_price": 1},
        {"item_id": 47761, "buy_price": 10000, "sell_price": 1},
        {"item_id": 47757, "buy_price": 10000, "sell_price": 1}
    ]'::jsonb
),
(
    91208,
    105,
    14,
    '[
        {"item_id": 80901, "buy_price": 0, "sell_price": 0, "item_index": 87, "barter_item_id": 80907, "barter_count": 1}
    ]'::jsonb
)
ON CONFLICT (npc_id, map_id) DO UPDATE SET
    shop_type = EXCLUDED.shop_type,
    items = EXCLUDED.items;
