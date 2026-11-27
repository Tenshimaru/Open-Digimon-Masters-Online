//! Property-based test harness for the application layer.
//!
//! Invariant and conservation properties for the summon, combine, and evolution
//! use cases are added here. Every property runs at least `CASES` generated
//! inputs.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use odmo_application::{
    OnlineMapState,
    character::{CharacterAccountRepository, CharacterRepository},
    game::{
        DigiCombineRepository, DigiSummonRepository, DigimonAssetRepository, DropCollectionResult,
        EvolutionAssetRepository, ExtraEvolutionRepository, GameApplication, GameServiceConfig,
        GameSession, ItemAssetRepository, MapDropRepository, MapMobRepository, NpcShopDefinition,
        NpcShopRepository, PortalDefinition, PortalRepository, RandomBoxRepository,
        UnionCombineRepository,
    },
};
use odmo_protocol::{GameRequest, PacketReader};
use odmo_types::{
    AccessLevel, Account, AccountId, CharacterSummary, CombineCeilingEntry, CombineItemRef,
    DigiCombineCatalog, DigiCombineCeil, DigiCombineRank, DigiCombineReward, DigiSummonProduct,
    DigiSummonReward, DigiSummonTicket, DropSummary, ExtraEvolutionMaterial, ExtraEvolutionNpc,
    ExtraEvolutionRecipe, InventorySnapshot, ItemRecord, MobSummary, PartnerSlotSnapshot,
    RandomBoxReward,
};
use proptest::prelude::*;

/// Minimum number of generated cases per property.
const CASES: u32 = 100;

/// Sync result byte for a non-empty catalog.
const SYNC_SUCCESS: u8 = 0;
/// Sync result byte for an empty catalog.
const SYNC_NO_PRODUCTS: u8 = 1;
/// Purchase result byte for a completed purchase.
const PURCHASE_SUCCESS: u8 = 0;
/// Purchase result byte for an empty catalog.
const PURCHASE_NO_PRODUCTS: u8 = 1;
/// Purchase result byte for a product id absent from the catalog.
const PURCHASE_INVALID_PRODUCT: u8 = 2;
/// Purchase result byte for a missing or insufficient ticket.
const PURCHASE_NOT_ENOUGH_TICKET: u8 = 3;

/// Combine result byte for an accepted (valid-grid) roll. Mirrors the
/// `COMBINE_RESULT_SUCCESS` constant in the handler, which is not exported.
const COMBINE_SUCCESS: u8 = 0;
/// Combine result byte for a rejected grid. Mirrors `COMBINE_RESULT_INVALID_GRID`.
const COMBINE_INVALID_GRID: u8 = 1;
/// Combine result byte for a missing/insufficient material. Mirrors
/// `COMBINE_RESULT_MISSING_MATERIAL`.
const COMBINE_MISSING_MATERIAL: u8 = 2;

/// Reward item id granted by the combine catalog in the Property 11 cases. Kept
/// disjoint from the material id range (`4000..5100`) and the filler id range
/// (`2000..3000`) so a successful combine produces no aliasing deltas.
const COMBINE_REWARD_ITEM_ID: i32 = 9000;

fn config() -> ProptestConfig {
    ProptestConfig {
        cases: CASES,
        ..ProptestConfig::default()
    }
}

/// Minimal game repository double whose only meaningful behavior is returning a
/// controllable DATA Summon catalog plus a single seeded character. Every other
/// port returns an empty/default value so the real `GameApplication` can run the
/// sync and purchase handlers unmodified.
#[derive(Debug, Default)]
struct CatalogRepository {
    products: RwLock<Vec<DigiSummonProduct>>,
    /// Character returned by `character_by_id`, when seeded.
    character: RwLock<Option<CharacterSummary>>,
    /// Inventory captured from the most recent `update_inventory` call.
    persisted_inventory: RwLock<Option<InventorySnapshot>>,
    /// Combine catalog returned to the Digi/Union combine handlers.
    combine_catalog: RwLock<DigiCombineCatalog>,
    /// Extra Evolution NPCs returned to the evolution handlers.
    evolution_npcs: RwLock<Vec<ExtraEvolutionNpc>>,
    /// Bits balance captured from the most recent `update_inventory_bits` call.
    persisted_bits: RwLock<Option<i64>>,
    /// Partner roster captured from the most recent `update_partner_roster` call.
    persisted_roster: RwLock<Option<Vec<PartnerSlotSnapshot>>>,
    /// Account returned by `account_by_id`, when seeded. The Spirit-craft
    /// password gate reads this to validate the supplied secret.
    account: RwLock<Option<Account>>,

    /// Weighted reward pool returned to the random box handlers. Empty by
    /// default; seed it to exercise the list/purchase properties.
    random_box_rewards: RwLock<Vec<RandomBoxReward>>,
}

impl CatalogRepository {
    fn with_products(products: Vec<DigiSummonProduct>) -> Self {
        Self {
            products: RwLock::new(products),
            ..Self::default()
        }
    }

    /// Seed both the catalog and the character the purchase handler will read.
    fn with_catalog_and_character(
        products: Vec<DigiSummonProduct>,
        character: CharacterSummary,
    ) -> Self {
        Self {
            products: RwLock::new(products),
            character: RwLock::new(Some(character)),
            ..Self::default()
        }
    }

    /// Seed a character plus the Digi/Union combine catalog the combine handlers
    /// read. Used by the combine property tests.
    fn with_character_and_combine_catalog(
        character: CharacterSummary,
        combine_catalog: DigiCombineCatalog,
    ) -> Self {
        Self {
            character: RwLock::new(Some(character)),
            combine_catalog: RwLock::new(combine_catalog),
            ..Self::default()
        }
    }

    /// Seed a character plus a single Extra Evolution NPC the evolution handlers
    /// read. Used by the item-to-digimon conservation property test.
    fn with_character_and_evolution_npc(
        character: CharacterSummary,
        npc: ExtraEvolutionNpc,
    ) -> Self {
        Self {
            character: RwLock::new(Some(character)),
            evolution_npcs: RwLock::new(vec![npc]),
            ..Self::default()
        }
    }

    /// Seed a character, a single Extra Evolution NPC, and the account the
    /// Spirit-craft password gate validates against. Used by the evolution
    /// rejection property test, whose digimon-to-item regimes exercise that gate.
    fn with_character_evolution_npc_and_account(
        character: CharacterSummary,
        npc: ExtraEvolutionNpc,
        account: Account,
    ) -> Self {
        Self {
            character: RwLock::new(Some(character)),
            evolution_npcs: RwLock::new(vec![npc]),
            account: RwLock::new(Some(account)),
            ..Self::default()
        }
    }

    /// Seed a character plus a weighted random box reward pool the random box
    /// handlers read. Used by the random box property tests.
    fn with_character_and_random_box_rewards(
        character: CharacterSummary,
        rewards: Vec<RandomBoxReward>,
    ) -> Self {
        Self {
            character: RwLock::new(Some(character)),
            random_box_rewards: RwLock::new(rewards),
            ..Self::default()
        }
    }

    /// The inventory persisted by the handler, if any.
    fn persisted_inventory(&self) -> Option<InventorySnapshot> {
        self.persisted_inventory
            .read()
            .expect("persisted inventory poisoned")
            .clone()
    }

    /// The bits balance persisted by the handler, if any.
    fn persisted_bits(&self) -> Option<i64> {
        *self.persisted_bits.read().expect("persisted bits poisoned")
    }

    /// The partner roster persisted by the handler, if any.
    fn persisted_roster(&self) -> Option<Vec<PartnerSlotSnapshot>> {
        self.persisted_roster
            .read()
            .expect("persisted roster poisoned")
            .clone()
    }
}

impl DigiSummonRepository for CatalogRepository {
    fn digi_summon_products(&self) -> anyhow::Result<Vec<DigiSummonProduct>> {
        Ok(self.products.read().expect("catalog poisoned").clone())
    }
}

impl ExtraEvolutionRepository for CatalogRepository {
    fn extra_evolution_npcs(&self) -> anyhow::Result<Vec<odmo_types::ExtraEvolutionNpc>> {
        Ok(self
            .evolution_npcs
            .read()
            .expect("evolution npcs poisoned")
            .clone())
    }
}

impl EvolutionAssetRepository for CatalogRepository {
    fn evolution_assets(&self) -> anyhow::Result<Vec<odmo_types::EvolutionAsset>> {
        Ok(Vec::new())
    }
}

impl ItemAssetRepository for CatalogRepository {
    fn item_assets(&self) -> anyhow::Result<Vec<odmo_types::ItemAsset>> {
        Ok(Vec::new())
    }
}

impl DigimonAssetRepository for CatalogRepository {
    fn digimon_assets(&self) -> anyhow::Result<Vec<odmo_types::DigimonAsset>> {
        Ok(Vec::new())
    }
}

// The combine handlers read these catalogs; the repository double serves a
// controllable catalog so the combine property tests can seed ranks and
// ceilings. Digi and Union share the same catalog type, so both ports return
// the same seeded value.
impl DigiCombineRepository for CatalogRepository {
    fn digi_combine_catalog(&self) -> anyhow::Result<odmo_types::DigiCombineCatalog> {
        Ok(self
            .combine_catalog
            .read()
            .expect("combine catalog poisoned")
            .clone())
    }
}

impl UnionCombineRepository for CatalogRepository {
    fn union_combine_catalog(&self) -> anyhow::Result<odmo_types::UnionCombineCatalog> {
        Ok(self
            .combine_catalog
            .read()
            .expect("combine catalog poisoned")
            .clone())
    }
}

impl RandomBoxRepository for CatalogRepository {
    fn random_box_rewards(&self) -> anyhow::Result<Vec<RandomBoxReward>> {
        Ok(self
            .random_box_rewards
            .read()
            .expect("random box rewards poisoned")
            .clone())
    }
}

impl MapMobRepository for CatalogRepository {
    fn mobs_by_map(&self, _map_id: i16, _channel: u8) -> anyhow::Result<Vec<MobSummary>> {
        Ok(Vec::new())
    }
}

impl MapDropRepository for CatalogRepository {
    fn drops_by_map(&self, _map_id: i16, _channel: u8) -> anyhow::Result<Vec<DropSummary>> {
        Ok(Vec::new())
    }

    fn collect_drop(
        &self,
        _character_id: u64,
        _map_id: i16,
        _channel: u8,
        _drop_handler: u32,
    ) -> anyhow::Result<DropCollectionResult> {
        Ok(DropCollectionResult::Missing)
    }
}

impl PortalRepository for CatalogRepository {
    fn portal_by_id(&self, _portal_id: i32) -> anyhow::Result<Option<PortalDefinition>> {
        Ok(None)
    }
}

impl NpcShopRepository for CatalogRepository {
    fn shop_by_npc(&self, _npc_id: i32, _map_id: i16) -> anyhow::Result<Option<NpcShopDefinition>> {
        Ok(None)
    }
}

impl CharacterAccountRepository for CatalogRepository {
    fn account_by_id(&self, _account_id: AccountId) -> anyhow::Result<Option<Account>> {
        Ok(self.account.read().expect("account poisoned").clone())
    }
}

impl CharacterRepository for CatalogRepository {
    fn list_characters_by_account(
        &self,
        _account_id: AccountId,
    ) -> anyhow::Result<Vec<CharacterSummary>> {
        Ok(Vec::new())
    }

    fn character_by_slot(
        &self,
        _account_id: AccountId,
        _slot: u8,
    ) -> anyhow::Result<Option<CharacterSummary>> {
        Ok(None)
    }

    fn character_by_id(&self, _character_id: u64) -> anyhow::Result<Option<CharacterSummary>> {
        Ok(self.character.read().expect("character poisoned").clone())
    }

    fn character_by_name(&self, _name: &str) -> anyhow::Result<Option<CharacterSummary>> {
        Ok(None)
    }

    fn is_name_available(&self, _name: &str) -> anyhow::Result<bool> {
        Ok(true)
    }

    fn create_character(
        &self,
        _account_id: AccountId,
        _slot: u8,
        _tamer_name: String,
        _tamer_model: i32,
        _partner_name: String,
        _partner_model: i32,
    ) -> anyhow::Result<CharacterSummary> {
        Ok(CharacterSummary::default())
    }

    fn delete_character(&self, _account_id: AccountId, _slot: u8) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn update_character_position(
        &self,
        _character_id: u64,
        _x: i32,
        _y: i32,
        _z: f32,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_partner_position(
        &self,
        _character_id: u64,
        _x: i32,
        _y: i32,
        _z: f32,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_equipment(&self, _character_id: u64, _equipment: Vec<u8>) -> anyhow::Result<()> {
        Ok(())
    }

    fn switch_partner(
        &self,
        _character_id: u64,
        _slot: u8,
    ) -> anyhow::Result<Option<CharacterSummary>> {
        Ok(None)
    }

    fn update_character_map(
        &self,
        _character_id: u64,
        _map_id: i16,
        _x: i32,
        _y: i32,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_inventory(
        &self,
        _character_id: u64,
        inventory: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        *self
            .persisted_inventory
            .write()
            .expect("persisted inventory poisoned") = Some(inventory);
        Ok(())
    }

    fn update_inventory_bits(&self, _character_id: u64, bits: i64) -> anyhow::Result<()> {
        *self
            .persisted_bits
            .write()
            .expect("persisted bits poisoned") = Some(bits);
        Ok(())
    }

    fn update_partner_roster(
        &self,
        _character_id: u64,
        _partner_current_slot: u8,
        partner_slots: Vec<odmo_types::PartnerSlotSnapshot>,
    ) -> anyhow::Result<()> {
        *self
            .persisted_roster
            .write()
            .expect("persisted roster poisoned") = Some(partner_slots);
        Ok(())
    }

    fn update_extra_inventory(
        &self,
        _character_id: u64,
        _extra_inventory: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_warehouse(
        &self,
        _character_id: u64,
        _warehouse: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn update_account_warehouse(
        &self,
        _character_id: u64,
        _account_warehouse: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()> {
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
}

fn unique_test_dir(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("odmo-pbt-{name}-{}", uuid::Uuid::new_v4()))
}

/// Build a `GameApplication` backed by a catalog double seeded with `products`.
fn app_with_catalog(name: &str, products: Vec<DigiSummonProduct>) -> GameApplication {
    let repo = Arc::new(CatalogRepository::with_products(products));
    GameApplication::new(
        GameServiceConfig {
            portal_state_dir: unique_test_dir(name),
        },
        repo,
    )
}

/// Generate a single DATA Summon product. Only the wire-relevant numeric fields
/// vary; the rest stay at their defaults since the sync response ignores them.
fn product_strategy() -> impl Strategy<Value = DigiSummonProduct> {
    (
        any::<i32>(),
        any::<i32>(),
        any::<i32>(),
        any::<i32>(),
        any::<i32>(),
    )
        .prop_map(
            |(product_id, string_id, draw_count, rank, remaining_daily_limit)| DigiSummonProduct {
                product_id,
                string_id,
                draw_count,
                rank,
                remaining_daily_limit,
                ..DigiSummonProduct::default()
            },
        )
}

/// Drive the sync handler and return the decoded result byte from the frame.
fn sync_result_byte(app: &GameApplication) -> u8 {
    let mut session = GameSession::new(1);
    let responses = app
        .handle_request(&mut session, GameRequest::DigiSummonSyncRequest)
        .expect("sync request should complete");
    assert_eq!(responses.len(), 1, "sync emits exactly one frame");
    let raw = PacketReader::from_frame(&responses[0]).expect("frame should decode");
    let mut reader = PacketReader::new(raw.payload);
    reader.read_u8().expect("result byte")
}

/// A purchase scenario kept in the success regime: a single-draw product with a
/// usable ticket, a seeded inventory holding that ticket plus disjoint fillers,
/// and room to grant the rolled reward.
#[derive(Debug, Clone)]
struct PurchaseScenario {
    product: DigiSummonProduct,
    inventory: InventorySnapshot,
    product_id: i32,
    ticket_slot: i32,
    ticket_item_id: i32,
}

/// Generate a success-regime DATA Summon purchase scenario.
///
/// Item id ranges are kept mutually disjoint so the ticket, reward, and filler
/// items never alias: ticket in `1..1000`, reward in `1000..2000`, fillers in
/// `2000..3000`. The ticket cost is fixed at one so a successful purchase drops
/// the ticket count by exactly one. Inventory size carries spare slots so the
/// reward always fits (the success path is never an overflow).
fn purchase_scenario_strategy() -> impl Strategy<Value = PurchaseScenario> {
    let fillers = prop::collection::vec((2000_i32..3000, 1_i32..=50), 0..=6);
    (
        1_i32..=1_000_000, // product_id
        1_i32..1000,       // ticket_item_id
        1_i32..=20,        // ticket_amount (>= cost of 1)
        1000_i32..2000,    // reward_item_id
        1_i32..=10,        // reward_amount
        1_i32..=100,       // reward_weight (positive)
        fillers,
        0_usize..=6, // ticket insertion position
    )
        .prop_map(
            |(
                product_id,
                ticket_item_id,
                ticket_amount,
                reward_item_id,
                reward_amount,
                reward_weight,
                fillers,
                ticket_pos,
            )| {
                let mut items: Vec<ItemRecord> = fillers
                    .into_iter()
                    .map(|(id, amount)| ItemRecord::new(id, amount))
                    .collect();
                let ticket_slot = ticket_pos.min(items.len());
                items.insert(ticket_slot, ItemRecord::new(ticket_item_id, ticket_amount));

                let size = (items.len() + 8) as u16;
                let inventory = InventorySnapshot {
                    bits: 0,
                    size,
                    items,
                };

                let product = DigiSummonProduct {
                    product_id,
                    draw_count: 1,
                    tickets: vec![DigiSummonTicket {
                        item_id: ticket_item_id,
                        cost: 1,
                    }],
                    rewards: vec![DigiSummonReward {
                        item_id: reward_item_id,
                        amount: reward_amount,
                        weight: reward_weight,
                        ..DigiSummonReward::default()
                    }],
                    ..DigiSummonProduct::default()
                };

                PurchaseScenario {
                    product,
                    inventory,
                    product_id,
                    ticket_slot: ticket_slot as i32,
                    ticket_item_id,
                }
            },
        )
}

/// Build a `GameApplication` and keep a handle to its repository double so the
/// test can read back the persisted inventory.
fn app_and_repo_with_character(
    name: &str,
    products: Vec<DigiSummonProduct>,
    character: CharacterSummary,
) -> (GameApplication, Arc<CatalogRepository>) {
    let repo = Arc::new(CatalogRepository::with_catalog_and_character(
        products, character,
    ));
    let app = GameApplication::new(
        GameServiceConfig {
            portal_state_dir: unique_test_dir(name),
        },
        repo.clone(),
    );
    (app, repo)
}

/// Build a `GameApplication` seeded with a character and a combine catalog,
/// returning a handle to the repository double so the combine property tests
/// can read back whether the inventory was persisted.
fn app_and_repo_with_combine_catalog(
    name: &str,
    character: CharacterSummary,
    combine_catalog: DigiCombineCatalog,
) -> (GameApplication, Arc<CatalogRepository>) {
    let repo = Arc::new(CatalogRepository::with_character_and_combine_catalog(
        character,
        combine_catalog,
    ));
    let app = GameApplication::new(
        GameServiceConfig {
            portal_state_dir: unique_test_dir(name),
        },
        repo.clone(),
    );
    (app, repo)
}

/// Build a `GameApplication` seeded with a character and a single Extra Evolution
/// NPC, returning a handle to the repository double so the evolution property
/// tests can read back the persisted bits, roster, and inventory.
fn app_and_repo_with_evolution(
    name: &str,
    character: CharacterSummary,
    npc: ExtraEvolutionNpc,
) -> (GameApplication, Arc<CatalogRepository>) {
    let repo = Arc::new(CatalogRepository::with_character_and_evolution_npc(
        character, npc,
    ));
    let app = GameApplication::new(
        GameServiceConfig {
            portal_state_dir: unique_test_dir(name),
        },
        repo.clone(),
    );
    (app, repo)
}

/// Build a `GameApplication` seeded with a character, a single Extra Evolution
/// NPC, and an account the Spirit-craft password gate validates against,
/// returning a handle to the repository double so the evolution rejection
/// property test can confirm no bits, roster, or inventory write occurred.
fn app_and_repo_with_evolution_and_account(
    name: &str,
    character: CharacterSummary,
    npc: ExtraEvolutionNpc,
    account: Account,
) -> (GameApplication, Arc<CatalogRepository>) {
    let repo = Arc::new(CatalogRepository::with_character_evolution_npc_and_account(
        character, npc, account,
    ));
    let app = GameApplication::new(
        GameServiceConfig {
            portal_state_dir: unique_test_dir(name),
        },
        repo.clone(),
    );
    (app, repo)
}

/// Build an account whose `email` and `secondary_password` are known, so a
/// Spirit-craft validation string can be chosen to match or miss the gate.
fn seeded_account(account_id: AccountId, email: &str, secondary_password: &str) -> Account {
    Account {
        id: account_id,
        username: "tamer".to_string(),
        password_hash: "hash".to_string(),
        email: email.to_string(),
        access_level: AccessLevel::Player,
        secondary_password: Some(secondary_password.to_string()),
        suspension: None,
    }
}

/// Seed a minimal combine catalog with one weighted rank and one ceiling entry
/// for `ceiling_type`. The single reward grants `reward_item_id`, kept small so
/// a valid-grid combine never overflows the inventory.
fn combine_catalog_seed(ceiling_type: u8, reward_item_id: i32) -> DigiCombineCatalog {
    DigiCombineCatalog {
        rank_rows: vec![DigiCombineRank {
            ceiling_type,
            weight: 1,
            rewards: vec![DigiCombineReward {
                item_id: reward_item_id,
                amount: 1,
                grade: 0,
            }],
        }],
        item_list: Vec::new(),
        item_groups: Vec::new(),
        ceil_groups: vec![DigiCombineCeil {
            ceiling_type,
            entries: vec![CombineCeilingEntry {
                tier: ceiling_type,
                value_a: 0,
                value_b: 0,
            }],
        }],
    }
}

/// Aggregate an inventory into `item_id -> total amount`, ignoring empty slots.
/// Slot reuse and stacking are invisible at this level, which is exactly the
/// granularity the conservation property is stated at.
fn aggregate_items(inventory: &InventorySnapshot) -> BTreeMap<i32, i64> {
    let mut totals = BTreeMap::new();
    for item in &inventory.items {
        if item.item_id > 0 && item.amount > 0 {
            *totals.entry(item.item_id).or_insert(0) += i64::from(item.amount);
        }
    }
    totals
}

/// Build a varied inventory of filler items drawn from the disjoint id range
/// `2000..3000`, with spare trailing slots. Filler ids never alias a summon
/// ticket id (kept in `1..1000`), so an inventory of fillers alone holds no
/// usable ticket.
fn filler_inventory_strategy(
    min_len: usize,
    max_len: usize,
) -> impl Strategy<Value = InventorySnapshot> {
    prop::collection::vec((2000_i32..3000, 1_i32..=50), min_len..=max_len).prop_map(|fillers| {
        let items: Vec<ItemRecord> = fillers
            .into_iter()
            .map(|(id, amount)| ItemRecord::new(id, amount))
            .collect();
        let size = (items.len() + 8) as u16;
        InventorySnapshot {
            bits: 0,
            size,
            items,
        }
    })
}

/// One of the three DATA Summon purchase rejection regimes, each carrying the
/// catalog/inventory shape that drives the handler down its intended path.
#[derive(Debug, Clone)]
enum RejectionScenario {
    /// Empty catalog: handler rejects with `NoProducts` before any lookup.
    NoProducts {
        inventory: InventorySnapshot,
        product_id: i32,
        ticket_slot: i32,
    },
    /// Non-empty catalog, but the requested product id is absent.
    InvalidProduct {
        products: Vec<DigiSummonProduct>,
        inventory: InventorySnapshot,
        product_id: i32,
        ticket_slot: i32,
    },
    /// Valid product, but no inventory slot holds a usable ticket for it.
    NotEnoughTicket {
        product: DigiSummonProduct,
        inventory: InventorySnapshot,
        product_id: i32,
        ticket_slot: i32,
    },
}

/// Generate a rejection scenario across all three regimes.
///
/// Id ranges stay mutually disjoint so each regime lands in its intended path:
/// catalog product ids in `1..1000`, the `InvalidProduct` request id in
/// `1_000_000..2_000_000` (guaranteed absent), ticket ids in `1..1000`, and
/// inventory fillers in `2000..3000`. The `NotEnoughTicket` regime either omits
/// the ticket entirely or inserts it underfunded (amount strictly below cost),
/// so no slot ever satisfies the usability check.
fn rejection_scenario_strategy() -> impl Strategy<Value = RejectionScenario> {
    let no_products = (filler_inventory_strategy(1, 8), any::<i32>(), -1_i32..=20).prop_map(
        |(inventory, product_id, ticket_slot)| RejectionScenario::NoProducts {
            inventory,
            product_id,
            ticket_slot,
        },
    );

    let catalog_products = prop::collection::vec(
        (1_i32..1000, 1_i32..=100).prop_map(|(product_id, draw_count)| DigiSummonProduct {
            product_id,
            draw_count,
            ..DigiSummonProduct::default()
        }),
        1..=6,
    );
    let invalid_product = (
        catalog_products,
        filler_inventory_strategy(0, 8),
        1_000_000_i32..2_000_000,
        -1_i32..=20,
    )
        .prop_map(|(products, inventory, product_id, ticket_slot)| {
            RejectionScenario::InvalidProduct {
                products,
                inventory,
                product_id,
                ticket_slot,
            }
        });

    let not_enough_ticket = (
        1_i32..1000, // product_id (present in catalog)
        1_i32..1000, // ticket_item_id (disjoint from fillers)
        2_i32..=20,  // ticket cost (>= 2 leaves room for an underfunded amount)
        filler_inventory_strategy(0, 6),
        any::<bool>(), // whether to insert an underfunded ticket
        0_usize..=6,   // underfunded ticket insertion position
        -1_i32..=20,   // requested ticket slot
    )
        .prop_map(
            |(
                product_id,
                ticket_item_id,
                cost,
                mut inventory,
                insert_underfunded,
                pos,
                ticket_slot,
            )| {
                if insert_underfunded {
                    // Strictly below cost, so the slot never becomes usable.
                    let amount = cost - 1;
                    let pos = pos.min(inventory.items.len());
                    inventory
                        .items
                        .insert(pos, ItemRecord::new(ticket_item_id, amount));
                    inventory.size = (inventory.items.len() + 8) as u16;
                }
                let product = DigiSummonProduct {
                    product_id,
                    draw_count: 1,
                    tickets: vec![DigiSummonTicket {
                        item_id: ticket_item_id,
                        cost,
                    }],
                    rewards: vec![DigiSummonReward {
                        item_id: 1500,
                        amount: 1,
                        weight: 1,
                        ..DigiSummonReward::default()
                    }],
                    ..DigiSummonProduct::default()
                };
                RejectionScenario::NotEnoughTicket {
                    product,
                    inventory,
                    product_id,
                    ticket_slot,
                }
            },
        );

    prop_oneof![no_products, invalid_product, not_enough_ticket]
}

/// Build a combine material node for `item_id` carrying `count`. Item ids stay
/// within u16 range so the `item_type` cast the handler matches on is lossless.
fn combine_material_node(item_id: i32, count: u16) -> CombineItemRef {
    CombineItemRef {
        item_uid: 0,
        item_type: item_id as u16,
        count,
    }
}

/// Round a raw node count down to a whole number of row-groups (a multiple of
/// four), keeping at least one group so the grid is always valid and non-empty.
fn truncate_to_row_groups(len: usize) -> usize {
    (len / 4).max(1) * 4
}

/// A Property 11 combine scenario, one per conservation regime.
#[derive(Debug, Clone)]
enum CombineConservationScenario {
    /// A valid grid whose every material is stocked in sufficient quantity, so
    /// the combine succeeds and must consume exactly the submitted nodes.
    Success {
        inventory: InventorySnapshot,
        materials: Vec<CombineItemRef>,
        ceiling_type: u8,
        is_union: bool,
    },
    /// A valid grid carrying at least one material absent from the inventory, so
    /// the combine rejects with the missing-material byte and persists nothing.
    MissingMaterial {
        inventory: InventorySnapshot,
        materials: Vec<CombineItemRef>,
        ceiling_type: u8,
        is_union: bool,
    },
}

/// Generate a fully-stocked success combine scenario.
///
/// Two to four distinct material ids are drawn from `4000..5100`; the grid
/// distributes a multiple-of-four node count across them. Inventory holds one
/// over-funded stack per material id so repeated nodes targeting the same id
/// drain a single stack without ever running it dry. Fillers live in the
/// disjoint range `2000..3000` and the reward id (`COMBINE_REWARD_ITEM_ID`) sits
/// outside both, so the conservation check sees only the expected deltas.
fn combine_success_scenario_strategy() -> impl Strategy<Value = CombineConservationScenario> {
    (
        prop::collection::hash_set(4000_i32..5100, 2..=4),
        prop::collection::vec((any::<u32>(), 1_u16..=3), 4..=44),
        prop::collection::vec((2000_i32..3000, 1_i32..=50), 0..=6),
        0_u8..=3,
        any::<bool>(),
    )
        .prop_map(|(id_set, raw_nodes, fillers, ceiling_type, is_union)| {
            let mut ids: Vec<i32> = id_set.into_iter().collect();
            ids.sort_unstable();

            let n = truncate_to_row_groups(raw_nodes.len());
            let materials: Vec<CombineItemRef> = raw_nodes[..n]
                .iter()
                .map(|(selector, count)| {
                    let id = ids[(*selector as usize) % ids.len()];
                    combine_material_node(id, *count)
                })
                .collect();

            // Required total per material id, summed across the nodes sharing it.
            let mut required: BTreeMap<i32, i32> = BTreeMap::new();
            for material in &materials {
                *required.entry(i32::from(material.item_type)).or_insert(0) +=
                    i32::from(material.count);
            }

            // Disjoint fillers first, then one over-funded stack per material id.
            let mut items: Vec<ItemRecord> = fillers
                .into_iter()
                .map(|(id, amount)| ItemRecord::new(id, amount))
                .collect();
            for (id, total) in &required {
                items.push(ItemRecord::new(*id, total + 1));
            }

            // Spare slots so granting the disjoint reward never overflows.
            let size = (items.len() + 16) as u16;
            let inventory = InventorySnapshot {
                bits: 0,
                size,
                items,
            };

            CombineConservationScenario::Success {
                inventory,
                materials,
                ceiling_type,
                is_union,
            }
        })
}

/// Generate a valid grid that references at least one absent material.
///
/// Present ids are drawn from `4000..5100` and stocked over-funded; one node is
/// overwritten with an id from the disjoint range `5500..5600` that is never
/// stocked, so the consume step fails and the handler rolls back before persist.
fn combine_missing_material_scenario_strategy() -> impl Strategy<Value = CombineConservationScenario>
{
    (
        prop::collection::hash_set(4000_i32..5100, 1..=3),
        5500_i32..5600,
        prop::collection::vec((any::<u32>(), 1_u16..=3), 4..=44),
        any::<usize>(),
        prop::collection::vec((2000_i32..3000, 1_i32..=50), 0..=6),
        0_u8..=3,
        any::<bool>(),
    )
        .prop_map(
            |(id_set, bad_id, raw_nodes, bad_selector, fillers, ceiling_type, is_union)| {
                let mut ids: Vec<i32> = id_set.into_iter().collect();
                ids.sort_unstable();

                let n = truncate_to_row_groups(raw_nodes.len());
                let mut materials: Vec<CombineItemRef> = raw_nodes[..n]
                    .iter()
                    .map(|(selector, count)| {
                        let id = ids[(*selector as usize) % ids.len()];
                        combine_material_node(id, *count)
                    })
                    .collect();

                // Force one node to reference the absent id so consumption fails.
                let bad_index = bad_selector % materials.len();
                materials[bad_index] = combine_material_node(bad_id, 1);

                // Stock the present ids generously; the absent id is never stocked.
                let mut required: BTreeMap<i32, i32> = BTreeMap::new();
                for material in &materials {
                    let id = i32::from(material.item_type);
                    if id != bad_id {
                        *required.entry(id).or_insert(0) += i32::from(material.count);
                    }
                }
                let mut items: Vec<ItemRecord> = fillers
                    .into_iter()
                    .map(|(id, amount)| ItemRecord::new(id, amount))
                    .collect();
                for (id, total) in &required {
                    items.push(ItemRecord::new(*id, total + 1));
                }
                let size = (items.len() + 16) as u16;
                let inventory = InventorySnapshot {
                    bits: 0,
                    size,
                    items,
                };

                CombineConservationScenario::MissingMaterial {
                    inventory,
                    materials,
                    ceiling_type,
                    is_union,
                }
            },
        )
}

/// Generate either combine conservation regime: a fully-stocked success or a
/// grid with an absent material.
fn combine_conservation_scenario_strategy() -> impl Strategy<Value = CombineConservationScenario> {
    prop_oneof![
        combine_success_scenario_strategy(),
        combine_missing_material_scenario_strategy(),
    ]
}

/// A Property 12 reward-claim scenario: a combine catalog carrying several
/// distinct ceiling tiers and the single tier the claim targets, with the
/// keyed-to-ceiling expectations precomputed from the catalog.
#[derive(Debug, Clone)]
struct RewardClaimScenario {
    catalog: DigiCombineCatalog,
    inventory: InventorySnapshot,
    claimed_ceiling: u8,
    is_union: bool,
    /// Ceiling-map entries the response must echo for the claimed tier.
    expected_ceiling: Vec<CombineCeilingEntry>,
    /// Reward set the response must grant for the claimed tier.
    expected_rewards: Vec<DigiCombineReward>,
}

/// Generate a multi-ceiling reward-claim scenario.
///
/// Two to four distinct ceiling tiers are drawn from `0..=20`; each gets one
/// rank row whose single reward carries an id unique to that tier (`9000 + i`)
/// plus one ceiling-map entry with a tier-specific `value_a`/`value_b`. Reward
/// ids stay disjoint across tiers and outside the filler range (`2000..3000`),
/// so the claimed tier's grant is the only possible inventory delta and no
/// other tier's reward can alias it. Most cases claim a seeded tier (the
/// distinct-tier discrimination); a minority claim an unseeded tier (`255`,
/// never in range) to assert the empty-ceiling corollary grants nothing.
fn reward_claim_scenario_strategy() -> impl Strategy<Value = RewardClaimScenario> {
    (
        prop::collection::hash_set(0_u8..=20, 2..=4),
        prop::collection::vec((1_u8..=200, 1_u16..=10_000), 4),
        prop::collection::vec((1_u16..=5, 0_u8..=5), 4),
        prop::collection::vec((2000_i32..3000, 1_i32..=50), 0..=6),
        any::<usize>(),
        any::<bool>(),
        prop_oneof![3 => Just(false), 1 => Just(true)],
    )
        .prop_map(
            |(tier_set, ceil_values, rewards_meta, fillers, claim_sel, is_union, claim_empty)| {
                let mut tiers: Vec<u8> = tier_set.into_iter().collect();
                tiers.sort_unstable();

                // One rank row plus one ceiling group per tier, ids kept disjoint.
                let mut rank_rows = Vec::with_capacity(tiers.len());
                let mut ceil_groups = Vec::with_capacity(tiers.len());
                for (i, &tier) in tiers.iter().enumerate() {
                    let (amount, grade) = rewards_meta[i];
                    let reward = DigiCombineReward {
                        item_id: 9000 + i as i32,
                        amount,
                        grade,
                    };
                    rank_rows.push(DigiCombineRank {
                        ceiling_type: tier,
                        weight: 1,
                        rewards: vec![reward],
                    });
                    let (value_a, value_b) = ceil_values[i];
                    ceil_groups.push(DigiCombineCeil {
                        ceiling_type: tier,
                        entries: vec![CombineCeilingEntry {
                            tier,
                            value_a,
                            value_b,
                        }],
                    });
                }

                let catalog = DigiCombineCatalog {
                    rank_rows,
                    item_list: Vec::new(),
                    item_groups: Vec::new(),
                    ceil_groups,
                };

                // Fillers only, in a range disjoint from every reward id, with
                // generous spare slots so granting the reward never overflows.
                let items: Vec<ItemRecord> = fillers
                    .into_iter()
                    .map(|(id, amount)| ItemRecord::new(id, amount))
                    .collect();
                let size = (items.len() + 16) as u16;
                let inventory = InventorySnapshot {
                    bits: 0,
                    size,
                    items,
                };

                let (claimed_ceiling, expected_ceiling, expected_rewards) = if claim_empty {
                    // 255 is never produced by the `0..=20` seed, so the claim
                    // matches no rank row and no ceiling group.
                    (255_u8, Vec::new(), Vec::new())
                } else {
                    let tier = tiers[claim_sel % tiers.len()];
                    let expected_ceiling: Vec<CombineCeilingEntry> = catalog
                        .ceil_groups
                        .iter()
                        .filter(|group| group.ceiling_type == tier)
                        .flat_map(|group| group.entries.iter().cloned())
                        .collect();
                    let expected_rewards: Vec<DigiCombineReward> = catalog
                        .rank_rows
                        .iter()
                        .filter(|rank| rank.ceiling_type == tier)
                        .flat_map(|rank| rank.rewards.iter().cloned())
                        .collect();
                    (tier, expected_ceiling, expected_rewards)
                };

                RewardClaimScenario {
                    catalog,
                    inventory,
                    claimed_ceiling,
                    is_union,
                    expected_ceiling,
                    expected_rewards,
                }
            },
        )
}

/// A Property 14 item-to-digimon exchange scenario, kept strictly in the success
/// regime: a free partner slot, an affordable price, and every required material
/// stocked in sufficient quantity.
#[derive(Debug, Clone)]
struct EvolutionScenario {
    /// Seeded character with a free slot, the materials, and equal bits fields.
    character: CharacterSummary,
    /// Digimon type the exchange creates (not an inventory item).
    model_id: i32,
    /// NPC carrying the single item-to-digimon recipe.
    npc: ExtraEvolutionNpc,
    npc_id: i32,
    /// Name applied to the created partner.
    name: String,
    /// Bits cost the exchange deducts from both bits fields.
    price: i64,
    /// The previously-free slot the new partner must occupy.
    free_slot: u8,
    /// Partner count before the exchange.
    pre_roster_len: usize,
    /// Consumed total per material id (`item_id -> amount`) for NEED_ALL.
    consumed: BTreeMap<i32, i64>,
}

/// Generate a success-regime item-to-digimon exchange scenario.
///
/// Id ranges stay mutually disjoint so the conservation check sees only the
/// expected deltas: main material ids in `4000..4500`, sub material ids in
/// `4500..5000`, fillers in `2000..3000`, and the created digimon type in
/// `40_000..41_000` (a partner type, never an inventory item). The recipe uses
/// `way_type` NEED_ALL, so every main and sub material is consumed in full; each
/// material id is stocked over its required amount. `digimon_slots` is 2..=4 and
/// the pre-existing partners fill a strict prefix of those slots, so a free slot
/// always remains. Bits and `inventory_bits` are seeded equal and at least the
/// price, so the dual decrement is consistent and the purchase is affordable.
fn evolution_success_scenario_strategy() -> impl Strategy<Value = EvolutionScenario> {
    (
        2_u8..=4,                                                             // digimon_slots
        0_usize..=3,                                                          // num_existing (raw)
        40_000_i32..41_000,                                                   // model_id
        prop::collection::vec((4000_i32..4500, 1_i32..=3, 0_i32..=5), 1..=3), // main raw
        prop::collection::vec((4500_i32..5000, 1_i32..=3, 0_i32..=5), 0..=2), // sub raw
        1_i64..=10_000,                                                       // price
        0_i64..=100_000,                                                      // bits headroom
        prop::collection::vec((2000_i32..3000, 1_i32..=50), 0..=6),           // fillers
    )
        .prop_map(
            |(
                digimon_slots,
                num_existing_raw,
                model_id,
                main_raw,
                sub_raw,
                price,
                bits_headroom,
                fillers,
            )| {
                // At least one free slot remains: existing partners fill a strict
                // prefix `1..=num_existing` of the `1..=digimon_slots` range.
                let num_existing = (num_existing_raw as u8) % digimon_slots;
                let free_slot = num_existing + 1;

                let partner_slots: Vec<PartnerSlotSnapshot> = (1..=num_existing)
                    .map(|slot| PartnerSlotSnapshot {
                        slot,
                        ..PartnerSlotSnapshot::default()
                    })
                    .collect();
                let pre_roster_len = partner_slots.len();

                // Deduplicate material ids per group, keeping the first amount and
                // headroom seen for each id. Main and sub ranges are disjoint, so
                // no id is shared across the two groups.
                let dedup = |raw: Vec<(i32, i32, i32)>| -> Vec<(i32, i32, i32)> {
                    let mut seen = std::collections::BTreeSet::new();
                    raw.into_iter()
                        .filter(|(id, _, _)| seen.insert(*id))
                        .collect()
                };
                let main = dedup(main_raw);
                let sub = dedup(sub_raw);

                let main_materials: Vec<ExtraEvolutionMaterial> = main
                    .iter()
                    .map(|(id, amount, _)| ExtraEvolutionMaterial {
                        material_id: *id,
                        amount: *amount,
                    })
                    .collect();
                let sub_materials: Vec<ExtraEvolutionMaterial> = sub
                    .iter()
                    .map(|(id, amount, _)| ExtraEvolutionMaterial {
                        material_id: *id,
                        amount: *amount,
                    })
                    .collect();

                // Consumed total per id and the over-funded stock to seed.
                let mut consumed: BTreeMap<i32, i64> = BTreeMap::new();
                let mut items: Vec<ItemRecord> = fillers
                    .into_iter()
                    .map(|(id, amount)| ItemRecord::new(id, amount))
                    .collect();
                for (id, amount, headroom) in main.iter().chain(sub.iter()) {
                    *consumed.entry(*id).or_insert(0) += i64::from(*amount);
                    items.push(ItemRecord::new(*id, amount + headroom));
                }

                let bits = price + bits_headroom;
                let size = (items.len() + 8) as u16;
                let inventory = InventorySnapshot { bits, size, items };

                let npc_id = 91_001;
                let npc = ExtraEvolutionNpc {
                    npc_id,
                    recipes: vec![ExtraEvolutionRecipe {
                        exchange_type: 1, // item-to-digimon
                        object_id: model_id,
                        material_type: 0,
                        need_material_value: 0,
                        price,
                        way_type: 1, // NEED_ALL
                        main_materials,
                        sub_materials,
                    }],
                };

                let mut character = CharacterSummary {
                    digimon_slots,
                    inventory,
                    inventory_bits: bits,
                    partner_slots,
                    ..CharacterSummary::default()
                };
                character.id = 100;

                EvolutionScenario {
                    character,
                    model_id,
                    npc,
                    npc_id,
                    name: "Spirit".to_string(),
                    price,
                    free_slot,
                    pre_roster_len,
                    consumed,
                }
            },
        )
}

/// NPC id carrying the single evolution recipe in the Property 15 scenarios.
const EVOLUTION_REJECT_NPC_ID: i32 = 91_001;
/// Account id the Spirit-craft password gate reads in the digimon-to-item
/// rejection regimes.
const EVOLUTION_REJECT_ACCOUNT_ID: AccountId = 100;
/// Known account email; one of the two secrets the password gate accepts.
const EVOLUTION_REJECT_EMAIL: &str = "tamer@odmo.local";
/// Known secondary password; the other secret the password gate accepts.
const EVOLUTION_REJECT_SECONDARY: &str = "secret-pw";

/// Deduplicate `(id, amount)` pairs by id, keeping the first amount per id, and
/// project them to evolution materials. Disjoint id ranges across groups keep
/// the resulting recipe free of accidental aliasing.
fn dedup_materials(raw: Vec<(i32, i32)>) -> Vec<ExtraEvolutionMaterial> {
    let mut seen = std::collections::BTreeSet::new();
    raw.into_iter()
        .filter(|(id, _)| seen.insert(*id))
        .map(|(id, amount)| ExtraEvolutionMaterial {
            material_id: id,
            amount,
        })
        .collect()
}

/// Build an item-to-digimon NPC recipe (`exchange_type` 1) that creates
/// `model_id`, costs `price` bits, and consumes every `main_materials` entry
/// (NEED_ALL). Sub-materials stay empty so the consumed set is exactly the mains.
fn item_to_digimon_npc(
    model_id: i32,
    price: i64,
    main_materials: Vec<ExtraEvolutionMaterial>,
) -> ExtraEvolutionNpc {
    ExtraEvolutionNpc {
        npc_id: EVOLUTION_REJECT_NPC_ID,
        recipes: vec![ExtraEvolutionRecipe {
            exchange_type: 1,
            object_id: model_id,
            material_type: 0,
            need_material_value: 0,
            price,
            way_type: 1, // NEED_ALL
            main_materials,
            sub_materials: Vec::new(),
        }],
    }
}

/// Build a digimon-to-item NPC recipe (`exchange_type` 2) that consumes a
/// partner of type `digimon_type`. The recipe matches only when the partner's
/// level is at least `need_material_value`, so callers tune that threshold to
/// land in the under-level regime or to leave level satisfied.
fn digimon_to_item_npc(digimon_type: i32, need_material_value: i32) -> ExtraEvolutionNpc {
    ExtraEvolutionNpc {
        npc_id: EVOLUTION_REJECT_NPC_ID,
        recipes: vec![ExtraEvolutionRecipe {
            exchange_type: 2,
            object_id: 81_000,
            material_type: 1,
            need_material_value,
            price: 1,
            way_type: 1,
            main_materials: vec![ExtraEvolutionMaterial {
                material_id: digimon_type,
                amount: 1,
            }],
            sub_materials: Vec::new(),
        }],
    }
}

/// A character carrying a single partner plus a roomy, affordable inventory.
/// Used by the digimon-to-item rejection regimes, where the only intended
/// blocker is the password gate (BadPassword) or the partner level (UnderLevel).
fn digimon_to_item_character(partner: PartnerSlotSnapshot) -> CharacterSummary {
    let inventory = InventorySnapshot {
        bits: 1_000_000,
        size: 16,
        items: vec![ItemRecord::new(2000, 5), ItemRecord::new(2001, 5)],
    };
    let mut character = CharacterSummary {
        digimon_slots: 4,
        inventory,
        inventory_bits: 1_000_000,
        partner_slots: vec![partner],
        ..CharacterSummary::default()
    };
    character.id = 100;
    character
}

/// One of the five evolution rejection regimes. The item-to-digimon regimes
/// (`NoFreeSlot`, `InsufficientBits`, `InsufficientMaterials`) drive
/// `SpiritToDigimon`; the digimon-to-spirit regimes (`BadPassword`,
/// `UnderLevel`) drive `DigimonToSpirit`. Every regime must reject before any
/// persist, leaving roster, bits, and inventory untouched.
#[derive(Debug, Clone)]
enum EvolutionRejectScenario {
    /// Every partner slot is occupied, so the item-to-digimon exchange has
    /// nowhere to place the new partner. Bits and materials are sufficient, so
    /// the free-slot check is the sole blocker.
    NoFreeSlot {
        character: CharacterSummary,
        npc: ExtraEvolutionNpc,
        model_id: i32,
    },
    /// A free slot and the materials are present, but the inventory bits balance
    /// is strictly below the recipe price, so the bits check rejects first.
    InsufficientBits {
        character: CharacterSummary,
        npc: ExtraEvolutionNpc,
        model_id: i32,
    },
    /// A free slot and affordable bits, but one required material is absent, so
    /// material consumption fails and the handler restores and rejects.
    InsufficientMaterials {
        character: CharacterSummary,
        npc: ExtraEvolutionNpc,
        model_id: i32,
    },
    /// The supplied validation string matches neither the account email nor the
    /// secondary password, so the digimon-to-item gate rejects before mutation.
    BadPassword {
        character: CharacterSummary,
        npc: ExtraEvolutionNpc,
        slot: u8,
        validation: String,
        account: Account,
    },
    /// The password is correct, but the partner level is below the recipe
    /// threshold, so no recipe matches and the handler rejects before mutation.
    UnderLevel {
        character: CharacterSummary,
        npc: ExtraEvolutionNpc,
        slot: u8,
        validation: String,
        account: Account,
    },
}

/// Generate the no-free-slot item-to-digimon rejection regime.
///
/// Every slot in `1..=digimon_slots` is occupied, the recipe is affordable
/// (`bits >= price`), and its materials are stocked, so the only blocker is the
/// absent free slot. Material ids (`4000..4500`), fillers via the materials, and
/// the created digimon type (`40_000..41_000`) stay disjoint.
fn no_free_slot_scenario_strategy() -> impl Strategy<Value = EvolutionRejectScenario> {
    (
        1_u8..=4,
        40_000_i32..41_000,
        prop::collection::vec((4000_i32..4500, 1_i32..=3), 1..=3),
        1_i64..=10_000,
        0_i64..=50_000,
    )
        .prop_map(
            |(digimon_slots, model_id, raw_materials, price, headroom)| {
                let partner_slots: Vec<PartnerSlotSnapshot> = (1..=digimon_slots)
                    .map(|slot| PartnerSlotSnapshot {
                        slot,
                        ..PartnerSlotSnapshot::default()
                    })
                    .collect();

                let materials = dedup_materials(raw_materials);
                let items: Vec<ItemRecord> = materials
                    .iter()
                    .map(|m| ItemRecord::new(m.material_id, m.amount + 1))
                    .collect();
                let bits = price + headroom;
                let inventory = InventorySnapshot {
                    bits,
                    size: (items.len() + 8) as u16,
                    items,
                };

                let mut character = CharacterSummary {
                    digimon_slots,
                    inventory,
                    inventory_bits: bits,
                    partner_slots,
                    ..CharacterSummary::default()
                };
                character.id = 100;

                EvolutionRejectScenario::NoFreeSlot {
                    character,
                    npc: item_to_digimon_npc(model_id, price, materials),
                    model_id,
                }
            },
        )
}

/// Generate the insufficient-bits item-to-digimon rejection regime.
///
/// A free slot remains (existing partners fill a strict prefix) and the
/// materials are stocked, but `bits` is strictly below `price` (`price = bits +
/// price_over`), so the bits check rejects before the slot or material checks.
fn insufficient_bits_scenario_strategy() -> impl Strategy<Value = EvolutionRejectScenario> {
    (
        2_u8..=4,
        0_usize..=2,
        40_000_i32..41_000,
        prop::collection::vec((4000_i32..4500, 1_i32..=3), 1..=3),
        0_i64..=10_000, // bits
        1_i64..=10_000, // price_over (>= 1 forces bits < price)
    )
        .prop_map(
            |(digimon_slots, num_existing_raw, model_id, raw_materials, bits, price_over)| {
                let num_existing = (num_existing_raw as u8) % digimon_slots;
                let partner_slots: Vec<PartnerSlotSnapshot> = (1..=num_existing)
                    .map(|slot| PartnerSlotSnapshot {
                        slot,
                        ..PartnerSlotSnapshot::default()
                    })
                    .collect();

                let materials = dedup_materials(raw_materials);
                let items: Vec<ItemRecord> = materials
                    .iter()
                    .map(|m| ItemRecord::new(m.material_id, m.amount + 1))
                    .collect();
                let price = bits + price_over;
                let inventory = InventorySnapshot {
                    bits,
                    size: (items.len() + 8) as u16,
                    items,
                };

                let mut character = CharacterSummary {
                    digimon_slots,
                    inventory,
                    inventory_bits: bits,
                    partner_slots,
                    ..CharacterSummary::default()
                };
                character.id = 100;

                EvolutionRejectScenario::InsufficientBits {
                    character,
                    npc: item_to_digimon_npc(model_id, price, materials),
                    model_id,
                }
            },
        )
}

/// Generate the insufficient-materials item-to-digimon rejection regime.
///
/// A free slot remains and bits are affordable, but the recipe carries one
/// material drawn from the never-stocked range `5500..5600`, disjoint from the
/// stocked mains (`4000..4500`). NEED_ALL consumption fails on that node, so the
/// handler restores and rejects.
fn insufficient_materials_scenario_strategy() -> impl Strategy<Value = EvolutionRejectScenario> {
    (
        2_u8..=4,
        0_usize..=2,
        40_000_i32..41_000,
        prop::collection::vec((4000_i32..4500, 1_i32..=3), 0..=2),
        5500_i32..5600,
        1_i64..=10_000,
        0_i64..=50_000,
    )
        .prop_map(
            |(
                digimon_slots,
                num_existing_raw,
                model_id,
                present_materials,
                bad_id,
                price,
                headroom,
            )| {
                let num_existing = (num_existing_raw as u8) % digimon_slots;
                let partner_slots: Vec<PartnerSlotSnapshot> = (1..=num_existing)
                    .map(|slot| PartnerSlotSnapshot {
                        slot,
                        ..PartnerSlotSnapshot::default()
                    })
                    .collect();

                let mut materials = dedup_materials(present_materials);
                // Stock only the present mains; the bad id stays unstocked.
                let items: Vec<ItemRecord> = materials
                    .iter()
                    .map(|m| ItemRecord::new(m.material_id, m.amount + 1))
                    .collect();
                materials.push(ExtraEvolutionMaterial {
                    material_id: bad_id,
                    amount: 1,
                });

                let bits = price + headroom;
                let inventory = InventorySnapshot {
                    bits,
                    size: (items.len() + 8) as u16,
                    items,
                };

                let mut character = CharacterSummary {
                    digimon_slots,
                    inventory,
                    inventory_bits: bits,
                    partner_slots,
                    ..CharacterSummary::default()
                };
                character.id = 100;

                EvolutionRejectScenario::InsufficientMaterials {
                    character,
                    npc: item_to_digimon_npc(model_id, price, materials),
                    model_id,
                }
            },
        )
}

/// Generate the failed-password digimon-to-item rejection regime.
///
/// Everything but the password is valid: a partner sits at the target slot and
/// the recipe's level threshold is satisfied. The validation string is built as
/// `mismatch-{n}`, which can never equal the seeded email or secondary password,
/// so the password gate is the sole rejection cause.
fn bad_password_scenario_strategy() -> impl Strategy<Value = EvolutionRejectScenario> {
    (1_u8..=4, 41_000_i32..42_000, 1_u8..=50, 0_u32..1000).prop_map(
        |(slot, digimon_type, level, suffix)| {
            let partner = PartnerSlotSnapshot {
                slot,
                digimon_type,
                level,
                ..PartnerSlotSnapshot::default()
            };
            EvolutionRejectScenario::BadPassword {
                character: digimon_to_item_character(partner),
                // Level satisfied (threshold 0) so only the password blocks.
                npc: digimon_to_item_npc(digimon_type, 0),
                slot,
                validation: format!("mismatch-{suffix}"),
                account: seeded_account(
                    EVOLUTION_REJECT_ACCOUNT_ID,
                    EVOLUTION_REJECT_EMAIL,
                    EVOLUTION_REJECT_SECONDARY,
                ),
            }
        },
    )
}

/// Generate the under-level digimon-to-item rejection regime.
///
/// The password is correct (it matches the email or the secondary password) and
/// a partner sits at the target slot, but the recipe threshold is set strictly
/// above the partner level (`need = level + gap`), so no recipe matches and the
/// handler rejects before mutation.
fn under_level_scenario_strategy() -> impl Strategy<Value = EvolutionRejectScenario> {
    (
        1_u8..=4,
        41_000_i32..42_000,
        1_u8..=50,
        1_i32..=50,
        any::<bool>(),
    )
        .prop_map(|(slot, digimon_type, level, gap, use_email)| {
            let partner = PartnerSlotSnapshot {
                slot,
                digimon_type,
                level,
                ..PartnerSlotSnapshot::default()
            };
            let validation = if use_email {
                EVOLUTION_REJECT_EMAIL.to_string()
            } else {
                EVOLUTION_REJECT_SECONDARY.to_string()
            };
            EvolutionRejectScenario::UnderLevel {
                character: digimon_to_item_character(partner),
                // Threshold strictly above the partner level: level is the sole blocker.
                npc: digimon_to_item_npc(digimon_type, i32::from(level) + gap),
                slot,
                validation,
                account: seeded_account(
                    EVOLUTION_REJECT_ACCOUNT_ID,
                    EVOLUTION_REJECT_EMAIL,
                    EVOLUTION_REJECT_SECONDARY,
                ),
            }
        })
}

/// Generate any of the five evolution rejection regimes.
fn evolution_reject_scenario_strategy() -> impl Strategy<Value = EvolutionRejectScenario> {
    prop_oneof![
        no_free_slot_scenario_strategy(),
        insufficient_bits_scenario_strategy(),
        insufficient_materials_scenario_strategy(),
        bad_password_scenario_strategy(),
        under_level_scenario_strategy(),
    ]
}

/// Build a `GameApplication` seeded with a character and a weighted random box
/// reward pool, returning a handle to the repository double.
fn app_and_repo_with_random_box(
    name: &str,
    character: CharacterSummary,
    rewards: Vec<RandomBoxReward>,
) -> (GameApplication, Arc<CatalogRepository>) {
    let repo = Arc::new(CatalogRepository::with_character_and_random_box_rewards(
        character, rewards,
    ));
    let app = GameApplication::new(
        GameServiceConfig {
            portal_state_dir: unique_test_dir(name),
        },
        repo.clone(),
    );
    (app, repo)
}

/// A Property 5 summon-roll scenario: a product whose reward table mixes
/// positive-weight entries with a single zero-weight entry, plus a usable ticket
/// and ample room to grant the rolled rewards across repeated purchases.
///
/// Item id ranges are mutually disjoint so the zero-weight entry is uniquely
/// identifiable: ticket in `1..1000`, positive rewards in `1000..2000`, the
/// zero-weight reward in `2500..3000`, fillers in `3000..4000`.
#[derive(Debug, Clone)]
struct SummonRollScenario {
    product: DigiSummonProduct,
    inventory: InventorySnapshot,
    product_id: i32,
    ticket_slot: i32,
    /// Every reward item id in the product table (the catalog membership set).
    member_ids: BTreeSet<i32>,
    /// The single zero-weight reward id, which must never be granted.
    zero_weight_id: i32,
    /// How many purchases to drive; each starts from the seeded inventory.
    purchase_count: usize,
}

fn summon_roll_scenario_strategy() -> impl Strategy<Value = SummonRollScenario> {
    let positives = prop::collection::vec((1000_i32..2000, 1_i32..=100), 1..=4);
    let fillers = prop::collection::vec((3000_i32..4000, 1_i32..=50), 0..=4);
    (
        1_i32..=1_000_000, // product_id
        1_i32..1000,       // ticket_item_id
        2500_i32..3000,    // zero_weight_id
        1_i32..=5,         // draw_count (rolls per purchase)
        2_usize..=6,       // purchase_count
        positives,
        fillers,
    )
        .prop_map(
            |(
                product_id,
                ticket_item_id,
                zero_weight_id,
                draw_count,
                purchase_count,
                positives,
                fillers,
            )| {
                let mut rewards: Vec<DigiSummonReward> = positives
                    .into_iter()
                    .map(|(item_id, weight)| DigiSummonReward {
                        item_id,
                        amount: 1,
                        weight,
                        ..DigiSummonReward::default()
                    })
                    .collect();
                // The zero-weight entry sits in its own id range; with positive
                // entries present it must never be rolled.
                rewards.push(DigiSummonReward {
                    item_id: zero_weight_id,
                    amount: 1,
                    weight: 0,
                    ..DigiSummonReward::default()
                });
                let member_ids: BTreeSet<i32> =
                    rewards.iter().map(|reward| reward.item_id).collect();

                let mut items: Vec<ItemRecord> = fillers
                    .into_iter()
                    .map(|(id, amount)| ItemRecord::new(id, amount))
                    .collect();
                items.push(ItemRecord::new(ticket_item_id, 1000));
                let ticket_slot = (items.len() - 1) as i32;
                let size = (items.len() + 16) as u16;
                let inventory = InventorySnapshot {
                    bits: 0,
                    size,
                    items,
                };

                let product = DigiSummonProduct {
                    product_id,
                    draw_count,
                    tickets: vec![DigiSummonTicket {
                        item_id: ticket_item_id,
                        cost: 1,
                    }],
                    rewards,
                    ..DigiSummonProduct::default()
                };

                SummonRollScenario {
                    product,
                    inventory,
                    product_id,
                    ticket_slot,
                    member_ids,
                    zero_weight_id,
                    purchase_count,
                }
            },
        )
}

/// A Property 5 random-box-roll scenario: a weighted reward pool mixing
/// positive-weight entries with a single zero-weight entry, plus a roomy
/// inventory so every roll takes the success (grant) path.
///
/// Item id ranges match the summon scenario's disjoint layout: positive rewards
/// in `1000..2000`, the zero-weight reward in `2500..3000`, fillers in
/// `3000..4000`.
#[derive(Debug, Clone)]
struct RandomBoxRollScenario {
    rewards: Vec<RandomBoxReward>,
    inventory: InventorySnapshot,
    /// Every reward item id in the box pool (the catalog membership set).
    member_ids: BTreeSet<i32>,
    /// The single zero-weight reward id, which must never be granted.
    zero_weight_id: i32,
    /// How many purchases to drive; each starts from the seeded inventory.
    purchase_count: usize,
}

fn random_box_roll_scenario_strategy() -> impl Strategy<Value = RandomBoxRollScenario> {
    let positives = prop::collection::vec((1000_i32..2000, 1_u32..=100, 1_u16..=10), 1..=4);
    let fillers = prop::collection::vec((3000_i32..4000, 1_i32..=50), 0..=4);
    (
        2500_i32..3000, // zero_weight_id
        2_usize..=8,    // purchase_count
        positives,
        fillers,
    )
        .prop_map(|(zero_weight_id, purchase_count, positives, fillers)| {
            let mut rewards: Vec<RandomBoxReward> = positives
                .into_iter()
                .map(|(item_id, weight, amount)| RandomBoxReward {
                    item_id,
                    amount,
                    weight,
                })
                .collect();
            rewards.push(RandomBoxReward {
                item_id: zero_weight_id,
                amount: 1,
                weight: 0,
            });
            let member_ids: BTreeSet<i32> = rewards.iter().map(|reward| reward.item_id).collect();

            let items: Vec<ItemRecord> = fillers
                .into_iter()
                .map(|(id, amount)| ItemRecord::new(id, amount))
                .collect();
            let size = (items.len() + 16) as u16;
            let inventory = InventorySnapshot {
                bits: 0,
                size,
                items,
            };

            RandomBoxRollScenario {
                rewards,
                inventory,
                member_ids,
                zero_weight_id,
                purchase_count,
            }
        })
}

proptest! {
    #![proptest_config(config())]

    /// A registered character is visible on its map until it is unregistered.
    #[test]
    fn map_presence_register_then_unregister(
        map_id in any::<i16>(),
        channel in any::<u8>(),
        character_id in any::<u64>(),
    ) {
        let state = OnlineMapState::new();

        state.register_map_presence(map_id, channel, character_id);
        prop_assert!(state.characters_on_map(map_id, channel).contains(&character_id));

        state.unregister_map_presence(map_id, channel, character_id);
        prop_assert!(!state.characters_on_map(map_id, channel).contains(&character_id));
    }

    /// Feature: babel-npc-summon-fusion, Property 2: Non-empty catalog yields Success sync result
    ///
    /// A non-empty catalog yields sync result byte 0; an empty catalog yields 1.
    /// Validates: Requirements 3.3, 3.4
    #[test]
    fn non_empty_catalog_yields_success_sync_result(
        products in prop::collection::vec(product_strategy(), 0..=8),
    ) {
        let expected = if products.is_empty() {
            SYNC_NO_PRODUCTS
        } else {
            SYNC_SUCCESS
        };
        let app = app_with_catalog("summon-sync-prop2", products);
        prop_assert_eq!(sync_result_byte(&app), expected);
    }

    /// Feature: babel-npc-summon-fusion, Property 4: Successful purchase conserves inventory deltas
    ///
    /// After a successful purchase the selected ticket count drops by exactly
    /// one and the rolled reward items are present with the granted amount, with
    /// no other inventory change.
    /// Validates: Requirements 4.3, 4.6
    #[test]
    fn successful_purchase_conserves_inventory_deltas(scenario in purchase_scenario_strategy()) {
        let character = CharacterSummary {
            id: 100,
            inventory: scenario.inventory.clone(),
            ..Default::default()
        };

        let (app, repo) = app_and_repo_with_character(
            "summon-purchase-prop4",
            vec![scenario.product.clone()],
            character,
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::DigiSummonPurchase {
                    product_id: scenario.product_id,
                    ticket_slot: scenario.ticket_slot,
                },
            )
            .expect("purchase request should complete");

        // Success emits the inventory reload followed by the purchase response.
        prop_assert_eq!(responses.len(), 2, "success path emits two frames");

        let raw = PacketReader::from_frame(&responses[1]).expect("purchase frame should decode");
        let mut reader = PacketReader::new(raw.payload);
        let result = reader.read_u8().expect("result byte");
        prop_assert_eq!(result, PURCHASE_SUCCESS, "scenario should take the success path");
        let _product_id = reader.read_i32().expect("product id");

        // Decode the granted rewards straight from the response frame so the test
        // never reimplements the weighted roll.
        let reward_count = reader.read_u16().expect("reward count") as usize;
        let mut granted: BTreeMap<i32, i64> = BTreeMap::new();
        for _ in 0..reward_count {
            let item_id = reader.read_i32().expect("reward item id");
            let amount = reader.read_u16().expect("reward amount") as i64;
            let _grade = reader.read_u16().expect("reward grade");
            *granted.entry(item_id).or_insert(0) += amount;
        }
        prop_assert!(reward_count >= 1, "a successful single-draw purchase grants a reward");

        let persisted = repo
            .persisted_inventory()
            .expect("a successful purchase persists the inventory");

        let pre = aggregate_items(&scenario.inventory);
        let post = aggregate_items(&persisted);

        // The ticket count drops by exactly one.
        let pre_ticket = pre.get(&scenario.ticket_item_id).copied().unwrap_or(0);
        let post_ticket = post.get(&scenario.ticket_item_id).copied().unwrap_or(0);
        prop_assert_eq!(
            pre_ticket - post_ticket,
            1,
            "the consumed ticket count decreases by exactly one"
        );

        // Build the expected post-state: pre minus one ticket plus the granted
        // rewards. Anything else must be byte-for-byte identical.
        let mut expected = pre.clone();
        match expected.get_mut(&scenario.ticket_item_id) {
            Some(count) => {
                *count -= 1;
                if *count <= 0 {
                    expected.remove(&scenario.ticket_item_id);
                }
            }
            None => prop_assert!(false, "ticket must exist before purchase"),
        }
        for (item_id, amount) in &granted {
            *expected.entry(*item_id).or_insert(0) += *amount;
        }

        prop_assert_eq!(
            &expected,
            &post,
            "only the ticket delta and the granted rewards change the inventory"
        );

        // Every granted reward is present in the persisted inventory.
        for (item_id, amount) in &granted {
            prop_assert!(
                post.get(item_id).copied().unwrap_or(0) >= *amount,
                "granted reward {} x{} must be present after purchase",
                item_id,
                amount
            );
        }
    }

    /// Feature: babel-npc-summon-fusion, Property 6: Summon rejection causes no net inventory mutation
    ///
    /// The three purchase rejection regimes carry result bytes 1/2/3 and never
    /// touch the persisted inventory: NoProducts on an empty catalog, then
    /// InvalidProduct for an absent product id, then NotEnoughTicket when no slot
    /// holds a usable ticket. Each path returns before `update_inventory`, so the
    /// repository double captures no persist and the inventory stays unchanged.
    /// Validates: Requirements 5.1, 5.2, 5.3
    #[test]
    fn summon_rejection_causes_no_net_inventory_mutation(
        scenario in rejection_scenario_strategy(),
    ) {
        let (products, inventory, product_id, ticket_slot, expected_result) = match scenario {
            RejectionScenario::NoProducts {
                inventory,
                product_id,
                ticket_slot,
            } => (Vec::new(), inventory, product_id, ticket_slot, PURCHASE_NO_PRODUCTS),
            RejectionScenario::InvalidProduct {
                products,
                inventory,
                product_id,
                ticket_slot,
            } => (products, inventory, product_id, ticket_slot, PURCHASE_INVALID_PRODUCT),
            RejectionScenario::NotEnoughTicket {
                product,
                inventory,
                product_id,
                ticket_slot,
            } => (
                vec![product],
                inventory,
                product_id,
                ticket_slot,
                PURCHASE_NOT_ENOUGH_TICKET,
            ),
        };

        let character = CharacterSummary {
            id: 100,
            inventory: inventory.clone(),
            ..Default::default()
        };

        let (app, repo) = app_and_repo_with_character(
            "summon-purchase-prop6",
            products,
            character,
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::DigiSummonPurchase {
                    product_id,
                    ticket_slot,
                },
            )
            .expect("purchase request should complete");

        // A rejection emits exactly the purchase response, with no inventory reload.
        prop_assert_eq!(responses.len(), 1, "rejection path emits a single frame");

        let raw = PacketReader::from_frame(&responses[0]).expect("purchase frame should decode");
        let mut reader = PacketReader::new(raw.payload);
        let result = reader.read_u8().expect("result byte");
        prop_assert_eq!(
            result,
            expected_result,
            "scenario should take its intended rejection path"
        );

        // The core invariant: the handler rejected before any persist, so the
        // inventory is never mutated.
        prop_assert!(
            repo.persisted_inventory().is_none(),
            "a rejected purchase must not persist the inventory"
        );
    }

    /// Feature: babel-npc-summon-fusion, Property 9: Combine accepted iff every row-group is 0 or 4
    ///
    /// A combine submission is accepted (result byte 0) exactly when its filled
    /// grid is valid: at most the full 11x4 grid (<= 44 nodes) and a multiple of
    /// four (each of the 11 row-groups is empty or full). Any other node count is
    /// rejected with the invalid-grid byte (1) and never persists the inventory.
    ///
    /// The character holds a large stack of the single material id and the
    /// catalog grants one small reward, so on a valid grid material presence and
    /// reward fit are guaranteed and the only possible outcome is success. The
    /// count `n` spans both regimes: valid multiples of four up to 44, invalid
    /// non-multiples, and the over-limit case above 44. Digi and Union are both
    /// exercised through a generated flag.
    /// Validates: Requirements 8.2, 8.3, 8.4, 10.3, 10.4
    #[test]
    fn combine_accepted_iff_every_row_group_is_zero_or_four(
        n in 0usize..=60,
        ceiling_type in 0u8..=3,
        is_union in any::<bool>(),
    ) {
        const MATERIAL_ITEM_ID: i32 = 5000;
        const REWARD_ITEM_ID: i32 = 6000;

        // One generous material stack plus spare slots so consuming up to 44
        // nodes never empties it and the granted reward always fits.
        let character = CharacterSummary {
            id: 100,
            inventory: InventorySnapshot {
                bits: 0,
                size: 64,
                items: vec![ItemRecord::new(MATERIAL_ITEM_ID, 200)],
            },
            ..Default::default()
        };

        let (app, repo) = app_and_repo_with_combine_catalog(
            "combine-grid-prop9",
            character,
            combine_catalog_seed(ceiling_type, REWARD_ITEM_ID),
        );

        // Every node references the same in-inventory material, so material
        // presence is never the rejecting factor: only grid validity decides.
        let materials: Vec<CombineItemRef> = (0..n)
            .map(|_| CombineItemRef {
                item_uid: 0,
                item_type: MATERIAL_ITEM_ID as u16,
                count: 1,
            })
            .collect();

        let request = if is_union {
            GameRequest::UnionCombine {
                ceiling_type,
                materials,
            }
        } else {
            GameRequest::DigiCombine {
                ceiling_type,
                materials,
            }
        };

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(&mut session, request)
            .expect("combine request should complete");

        // The combine result is always the final frame; on success it trails the
        // inventory reload, on rejection it stands alone.
        let frame = responses.last().expect("combine emits at least one frame");
        let raw = PacketReader::from_frame(frame).expect("combine frame should decode");
        let mut reader = PacketReader::new(raw.payload);
        let result = reader.read_u8().expect("result byte");

        let grid_is_valid = n <= 44 && n % 4 == 0;
        if grid_is_valid {
            prop_assert_eq!(
                result,
                COMBINE_SUCCESS,
                "a valid grid of {} nodes must be accepted",
                n
            );
            prop_assert_eq!(
                responses.len(),
                2,
                "an accepted combine emits the inventory reload then the result"
            );
        } else {
            prop_assert_eq!(
                result,
                COMBINE_INVALID_GRID,
                "an invalid grid of {} nodes must be rejected",
                n
            );
            prop_assert_eq!(
                responses.len(),
                1,
                "a rejected combine emits a single result frame"
            );
            // Property 9 reject clause: a rejected grid mutates nothing.
            prop_assert!(
                repo.persisted_inventory().is_none(),
                "a rejected combine must not persist the inventory"
            );
        }
    }

    /// Feature: babel-npc-summon-fusion, Property 11: Successful combine consumes exactly the submitted materials
    ///
    /// A valid combine removes exactly the submitted material nodes and nothing
    /// else; a grid carrying an absent material rejects with no mutation. Both
    /// clauses run over a generated mix of Digi and Union combines.
    ///
    /// Success regime: 2-4 distinct material ids stocked over-funded, the reward
    /// id and fillers kept in ranges disjoint from the materials, so the only
    /// inventory deltas are the consumed materials and the granted reward. The
    /// expected post-state is `pre - submitted_materials + granted_rewards`,
    /// decoded straight from the response frame, and must equal the persisted
    /// inventory exactly.
    ///
    /// Missing-material regime: a valid grid where one node references an id that
    /// is never stocked, so consumption fails, the handler restores and rejects
    /// with the missing-material byte, and the repository captures no persist.
    /// Validates: Requirements 8.5, 8.6, 10.6
    #[test]
    fn successful_combine_consumes_exactly_the_submitted_materials(
        scenario in combine_conservation_scenario_strategy(),
    ) {
        match scenario {
            CombineConservationScenario::Success {
                inventory,
                materials,
                ceiling_type,
                is_union,
            } => {
                let character = CharacterSummary {
                    id: 100,
                    inventory: inventory.clone(),
                    ..Default::default()
                };

                let (app, repo) = app_and_repo_with_combine_catalog(
                    "combine-conserve-prop11-success",
                    character,
                    combine_catalog_seed(ceiling_type, COMBINE_REWARD_ITEM_ID),
                );

                let request = if is_union {
                    GameRequest::UnionCombine {
                        ceiling_type,
                        materials: materials.clone(),
                    }
                } else {
                    GameRequest::DigiCombine {
                        ceiling_type,
                        materials: materials.clone(),
                    }
                };

                let mut session = GameSession::new(1);
                session.character_id = Some(100);
                let responses = app
                    .handle_request(&mut session, request)
                    .expect("combine request should complete");

                // Success path: inventory reload followed by the combine result.
                prop_assert_eq!(
                    responses.len(),
                    2,
                    "an accepted combine emits the inventory reload then the result"
                );

                let raw = PacketReader::from_frame(&responses[1])
                    .expect("combine frame should decode");
                let mut reader = PacketReader::new(raw.payload);
                let result = reader.read_u8().expect("result byte");
                prop_assert_eq!(
                    result,
                    COMBINE_SUCCESS,
                    "a fully-stocked valid grid must be accepted"
                );

                // Skip the ceiling echo: u16 count then 4-byte entries.
                let ceiling_count = reader.read_u16().expect("ceiling count") as usize;
                let _ = reader.read_bytes(ceiling_count * 4).expect("ceiling bytes");
                // Skip the material echo: u16 count then 8-byte nodes.
                let material_count = reader.read_u16().expect("material count") as usize;
                let _ = reader.read_bytes(material_count * 8).expect("material bytes");
                // Decode the granted rewards from their fixed-size nodes.
                let reward_count = reader.read_u16().expect("reward count") as usize;
                let mut granted: BTreeMap<i32, i64> = BTreeMap::new();
                for _ in 0..reward_count {
                    let item_id = reader.read_i32().expect("reward item id");
                    let amount = reader.read_u16().expect("reward amount") as i64;
                    let _grade = reader.read_u8().expect("reward grade");
                    let _reserved = reader.read_bytes(64).expect("reward reserved bytes");
                    *granted.entry(item_id).or_insert(0) += amount;
                }

                let persisted = repo
                    .persisted_inventory()
                    .expect("a successful combine persists the inventory");

                let pre = aggregate_items(&inventory);
                let post = aggregate_items(&persisted);

                // Submitted totals per material id, summed across shared nodes.
                let mut submitted: BTreeMap<i32, i64> = BTreeMap::new();
                for material in &materials {
                    *submitted.entry(i32::from(material.item_type)).or_insert(0) +=
                        i64::from(material.count);
                }

                // Expected post-state: pre minus the submitted materials plus the
                // granted rewards. Nothing else may move.
                let mut expected = pre.clone();
                for (id, count) in &submitted {
                    let slot = expected.entry(*id).or_insert(0);
                    *slot -= *count;
                    if *slot <= 0 {
                        expected.remove(id);
                    }
                }
                for (id, amount) in &granted {
                    *expected.entry(*id).or_insert(0) += *amount;
                }

                prop_assert_eq!(
                    &expected,
                    &post,
                    "only the submitted materials and granted rewards may change the inventory"
                );

                // Exact consumption per material id, stated directly.
                for (id, count) in &submitted {
                    let pre_total = pre.get(id).copied().unwrap_or(0);
                    let post_total = post.get(id).copied().unwrap_or(0);
                    let reward_delta = granted.get(id).copied().unwrap_or(0);
                    prop_assert_eq!(
                        pre_total - post_total + reward_delta,
                        *count,
                        "material id {} must lose exactly its submitted count",
                        id
                    );
                }
            }
            CombineConservationScenario::MissingMaterial {
                inventory,
                materials,
                ceiling_type,
                is_union,
            } => {
                let character = CharacterSummary {
                    id: 100,
                    inventory: inventory.clone(),
                    ..Default::default()
                };

                let (app, repo) = app_and_repo_with_combine_catalog(
                    "combine-conserve-prop11-missing",
                    character,
                    combine_catalog_seed(ceiling_type, COMBINE_REWARD_ITEM_ID),
                );

                let request = if is_union {
                    GameRequest::UnionCombine {
                        ceiling_type,
                        materials,
                    }
                } else {
                    GameRequest::DigiCombine {
                        ceiling_type,
                        materials,
                    }
                };

                let mut session = GameSession::new(1);
                session.character_id = Some(100);
                let responses = app
                    .handle_request(&mut session, request)
                    .expect("combine request should complete");

                // Rejection path: a single result frame, no inventory reload.
                prop_assert_eq!(
                    responses.len(),
                    1,
                    "a rejected combine emits a single result frame"
                );

                let raw = PacketReader::from_frame(&responses[0])
                    .expect("combine frame should decode");
                let mut reader = PacketReader::new(raw.payload);
                let result = reader.read_u8().expect("result byte");
                prop_assert_eq!(
                    result,
                    COMBINE_MISSING_MATERIAL,
                    "a grid with an absent material must reject with the missing-material byte"
                );

                // The handler restored the inventory before persisting, so the
                // repository double captured no write.
                prop_assert!(
                    repo.persisted_inventory().is_none(),
                    "a missing-material combine must not persist the inventory"
                );
            }
        }
    }

    /// Feature: babel-npc-summon-fusion, Property 12: Combine reward claim is keyed on its ceiling
    ///
    /// A reward claim is keyed entirely on its `ceiling_type`. The reward
    /// response echoes the ceiling-map entries configured for the claimed tier,
    /// and the granted reward set is exactly that tier's rewards, never another
    /// tier's. A successful claim grants precisely those reward items into the
    /// inventory and changes nothing else.
    ///
    /// The catalog seeds several distinct ceiling tiers, each with a reward id
    /// unique to that tier and a tier-specific ceiling-map entry; reward ids and
    /// fillers occupy disjoint ranges so the keyed grant is unambiguous. A
    /// minority of cases claim an unseeded tier, which must echo an empty ceiling
    /// block and grant nothing. Digi and Union are both exercised via a flag.
    /// Validates: Requirements 9.2, 9.3, 10.6
    #[test]
    fn combine_reward_claim_is_keyed_on_its_ceiling(
        scenario in reward_claim_scenario_strategy(),
    ) {
        let character = CharacterSummary {
            id: 100,
            inventory: scenario.inventory.clone(),
            ..Default::default()
        };

        let (app, repo) = app_and_repo_with_combine_catalog(
            "combine-reward-claim-prop12",
            character,
            scenario.catalog.clone(),
        );

        let request = if scenario.is_union {
            GameRequest::UnionCombineRewardClaim {
                ceiling_type: scenario.claimed_ceiling,
            }
        } else {
            GameRequest::DigiCombineRewardClaim {
                ceiling_type: scenario.claimed_ceiling,
            }
        };

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(&mut session, request)
            .expect("reward claim request should complete");

        // The success path emits the inventory reload then the reward response;
        // the reward response is always the final frame.
        prop_assert_eq!(
            responses.len(),
            2,
            "a successful reward claim emits the inventory reload then the reward response"
        );

        let raw = PacketReader::from_frame(&responses[1])
            .expect("reward frame should decode");
        let mut reader = PacketReader::new(raw.payload);
        let result = reader.read_u8().expect("result byte");
        prop_assert_eq!(
            result,
            COMBINE_SUCCESS,
            "a roomy inventory means the claim never overflows"
        );

        // Keyed-to-ceiling: the echoed ceiling block equals the catalog entries
        // for the claimed tier, decoded as `{u8 tier, u8 value_a, u16 value_b}`.
        let ceiling_count = reader.read_u16().expect("ceiling count") as usize;
        let mut decoded_ceiling = Vec::with_capacity(ceiling_count);
        for _ in 0..ceiling_count {
            let tier = reader.read_u8().expect("ceiling tier");
            let value_a = reader.read_u8().expect("ceiling value_a");
            let value_b = reader.read_u16().expect("ceiling value_b");
            decoded_ceiling.push(CombineCeilingEntry {
                tier,
                value_a,
                value_b,
            });
        }
        prop_assert_eq!(
            &decoded_ceiling,
            &scenario.expected_ceiling,
            "the response echoes the ceiling block keyed to the claimed tier"
        );

        // The material echo is always empty on a reward claim.
        let material_count = reader.read_u16().expect("material count") as usize;
        prop_assert_eq!(material_count, 0, "a reward claim echoes no materials");

        // Keyed-to-ceiling: the granted reward list equals the claimed tier's
        // reward set exactly, decoded from the fixed-size reward nodes.
        let reward_count = reader.read_u16().expect("reward count") as usize;
        let mut decoded_rewards = Vec::with_capacity(reward_count);
        for _ in 0..reward_count {
            let item_id = reader.read_i32().expect("reward item id");
            let amount = reader.read_u16().expect("reward amount");
            let grade = reader.read_u8().expect("reward grade");
            let _reserved = reader.read_bytes(64).expect("reward reserved bytes");
            decoded_rewards.push(DigiCombineReward {
                item_id,
                amount,
                grade,
            });
        }
        prop_assert_eq!(
            &decoded_rewards,
            &scenario.expected_rewards,
            "the granted rewards are exactly the claimed tier's reward set"
        );

        // Grants exactly that reward: the only inventory delta is the claimed
        // tier's reward items, each increased by its amount.
        let persisted = repo
            .persisted_inventory()
            .expect("a successful claim persists the inventory");
        let pre = aggregate_items(&scenario.inventory);
        let post = aggregate_items(&persisted);

        let mut expected = pre.clone();
        for reward in &scenario.expected_rewards {
            *expected.entry(reward.item_id).or_insert(0) +=
                i64::from(reward.amount.max(1));
        }
        prop_assert_eq!(
            &expected,
            &post,
            "only the claimed tier's rewards may change the inventory"
        );

        // No other tier's reward item leaks into the inventory.
        let claimed_ids: std::collections::BTreeSet<i32> =
            scenario.expected_rewards.iter().map(|r| r.item_id).collect();
        for rank in &scenario.catalog.rank_rows {
            for reward in &rank.rewards {
                if claimed_ids.contains(&reward.item_id) {
                    continue;
                }
                prop_assert_eq!(
                    pre.get(&reward.item_id).copied().unwrap_or(0),
                    post.get(&reward.item_id).copied().unwrap_or(0),
                    "an unclaimed tier's reward id {} must not change",
                    reward.item_id
                );
            }
        }
    }

    /// Feature: babel-npc-summon-fusion, Property 14: Successful item-to-digimon exchange conserves resources
    ///
    /// With a free partner slot and sufficient bits and materials, the
    /// item-to-digimon exchange adds exactly one partner and deducts exactly the
    /// required bits and materials, changing nothing else.
    ///
    /// The scenario stays strictly in the success regime: `digimon_slots` is
    /// 2..=4 with the pre-existing partners filling a strict prefix so a free
    /// slot always remains, the price is affordable from equal `bits` and
    /// `inventory_bits` fields, and every NEED_ALL material is stocked above its
    /// required amount. Material ids (`4000..5000`), fillers (`2000..3000`), and
    /// the created digimon type (`40_000..41_000`, a partner type rather than an
    /// inventory item) occupy disjoint ranges, so the conservation check sees
    /// only the expected deltas. Conservation is checked three ways: one partner
    /// added in the free slot with `digimon_type == model_id`, exactly the price
    /// deducted from both bits fields, and exactly the required materials removed
    /// with every filler untouched.
    /// Validates: Requirements 11.3
    #[test]
    fn successful_item_to_digimon_exchange_conserves_resources(
        scenario in evolution_success_scenario_strategy(),
    ) {
        let pre_inventory = scenario.character.inventory.clone();
        let pre_bits = scenario.character.inventory_bits;

        let (app, repo) = app_and_repo_with_evolution(
            "evolution-conserve-prop14",
            scenario.character.clone(),
            scenario.npc.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::SpiritToDigimon {
                    model_id: scenario.model_id,
                    name: scenario.name.clone(),
                    npc_id: scenario.npc_id,
                },
            )
            .expect("evolution request should complete");

        // The success path emits the evolution result, then the inventory reload.
        prop_assert_eq!(
            responses.len(),
            2,
            "a successful exchange emits the result and inventory frames"
        );

        // Decode the result frame: `[u32 digimon_id][i64 remaining_bits]`.
        let raw = PacketReader::from_frame(&responses[0]).expect("result frame should decode");
        let mut reader = PacketReader::new(raw.payload);
        let digimon_id = reader.read_u32().expect("digimon id");
        prop_assert_eq!(
            digimon_id,
            scenario.model_id as u32,
            "the result echoes the created digimon type"
        );
        let remaining_bits = reader.read_u64().expect("remaining bits") as i64;
        prop_assert_eq!(
            remaining_bits,
            pre_bits - scenario.price,
            "the result reports the post-deduction bits balance"
        );

        // One partner added: the persisted roster grows by exactly one, and the
        // new partner occupies the previously-free slot with the created type.
        let roster = repo
            .persisted_roster()
            .expect("a successful exchange persists the partner roster");
        prop_assert_eq!(
            roster.len(),
            scenario.pre_roster_len + 1,
            "the roster gains exactly one partner"
        );
        let added = roster
            .iter()
            .find(|partner| partner.slot == scenario.free_slot)
            .expect("the new partner occupies the previously-free slot");
        prop_assert_eq!(
            added.digimon_type,
            scenario.model_id,
            "the new partner carries the created digimon type"
        );

        // Exactly the required bits deducted from both bits fields.
        let persisted_bits = repo
            .persisted_bits()
            .expect("a successful exchange persists the bits balance");
        prop_assert_eq!(
            persisted_bits,
            pre_bits - scenario.price,
            "exactly the price is deducted from the bits balance"
        );
        let persisted = repo
            .persisted_inventory()
            .expect("a successful exchange persists the inventory");
        prop_assert_eq!(
            persisted.bits,
            pre_inventory.bits - scenario.price,
            "exactly the price is deducted from the inventory bits field"
        );

        // Exactly the required materials deducted, nothing else. Build the
        // expected aggregate as pre minus the consumed materials; the created
        // partner is not an inventory item, so it never appears here.
        let pre = aggregate_items(&pre_inventory);
        let post = aggregate_items(&persisted);
        let mut expected = pre.clone();
        for (id, amount) in &scenario.consumed {
            let slot = expected.entry(*id).or_insert(0);
            *slot -= *amount;
            if *slot <= 0 {
                expected.remove(id);
            }
        }
        prop_assert_eq!(
            &expected,
            &post,
            "only the required materials change the item aggregate"
        );
    }

    /// Feature: babel-npc-summon-fusion, Property 15: Evolution rejection causes no roster or inventory mutation
    ///
    /// Every evolution rejection regime leaves the roster, bits, and inventory
    /// untouched. Three spirit-to-digimon regimes drive `SpiritToDigimon` and
    /// three reject before persist: `NoFreeSlot` (every slot occupied),
    /// `InsufficientBits` (`bits < price`), and `InsufficientMaterials` (a
    /// required material absent). Two digimon-to-spirit regimes drive `DigimonToSpirit`
    /// and reject before mutation: `BadPassword` (validation matches neither the
    /// email nor the secondary password) and `UnderLevel` (partner level below
    /// the recipe threshold, so no recipe matches).
    ///
    /// Each regime is seeded so its intended cause is the sole blocker: the
    /// item-to-digimon regimes keep every other gate satisfied, and the
    /// digimon-to-item regimes keep the partner present and the non-target gate
    /// passing. The cross-cutting invariant is that none of the three capture
    /// fields (`persisted_inventory`, `persisted_bits`, `persisted_roster`) is
    /// ever written. The item-to-digimon regimes and `UnderLevel` return an empty
    /// response vector; `BadPassword` returns exactly one
    /// `PARTNER_DELETE_RESPONSE` frame carrying `i32 -1`.
    /// Validates: Requirements 11.4, 11.5, 12.3, 12.4, 12.6
    #[test]
    fn evolution_rejection_causes_no_roster_or_inventory_mutation(
        scenario in evolution_reject_scenario_strategy(),
    ) {
        // Drive the matching handler and capture the response frames and the
        // repository double so the no-persist invariant can be checked uniformly.
        let (responses, repo, expect_bad_password) = match scenario {
            EvolutionRejectScenario::NoFreeSlot {
                character,
                npc,
                model_id,
            }
            | EvolutionRejectScenario::InsufficientBits {
                character,
                npc,
                model_id,
            }
            | EvolutionRejectScenario::InsufficientMaterials {
                character,
                npc,
                model_id,
            } => {
                let (app, repo) = app_and_repo_with_evolution(
                    "evolution-reject-prop15-item",
                    character,
                    npc,
                );
                let mut session = GameSession::new(1);
                session.character_id = Some(100);
                let responses = app
                    .handle_request(
                        &mut session,
                        GameRequest::SpiritToDigimon {
                            model_id,
                            name: "Spirit".to_string(),
                            npc_id: EVOLUTION_REJECT_NPC_ID,
                        },
                    )
                    .expect("evolution request should complete");
                (responses, repo, false)
            }
            EvolutionRejectScenario::BadPassword {
                character,
                npc,
                slot,
                validation,
                account,
            } => {
                let (app, repo) = app_and_repo_with_evolution_and_account(
                    "evolution-reject-prop15-badpw",
                    character,
                    npc,
                    account,
                );
                let mut session = GameSession::new(1);
                session.account_id = Some(EVOLUTION_REJECT_ACCOUNT_ID);
                session.character_id = Some(100);
                let responses = app
                    .handle_request(
                        &mut session,
                        GameRequest::DigimonToSpirit {
                            slot,
                            validation,
                            npc_id: EVOLUTION_REJECT_NPC_ID,
                        },
                    )
                    .expect("spirit craft request should complete");
                (responses, repo, true)
            }
            EvolutionRejectScenario::UnderLevel {
                character,
                npc,
                slot,
                validation,
                account,
            } => {
                let (app, repo) = app_and_repo_with_evolution_and_account(
                    "evolution-reject-prop15-underlevel",
                    character,
                    npc,
                    account,
                );
                let mut session = GameSession::new(1);
                session.account_id = Some(EVOLUTION_REJECT_ACCOUNT_ID);
                session.character_id = Some(100);
                let responses = app
                    .handle_request(
                        &mut session,
                        GameRequest::DigimonToSpirit {
                            slot,
                            validation,
                            npc_id: EVOLUTION_REJECT_NPC_ID,
                        },
                    )
                    .expect("spirit craft request should complete");
                (responses, repo, false)
            }
        };

        // Core invariant across every regime: no inventory, bits, or roster write.
        prop_assert!(
            repo.persisted_inventory().is_none(),
            "a rejected exchange must not persist the inventory"
        );
        prop_assert!(
            repo.persisted_bits().is_none(),
            "a rejected exchange must not persist the bits balance"
        );
        prop_assert!(
            repo.persisted_roster().is_none(),
            "a rejected exchange must not persist the partner roster"
        );

        // Regime-appropriate response: BadPassword returns a single
        // PARTNER_DELETE_RESPONSE (-1) frame; every other regime returns nothing.
        if expect_bad_password {
            prop_assert_eq!(
                responses.len(),
                1,
                "a failed password emits exactly one delete-response frame"
            );
            let raw = PacketReader::from_frame(&responses[0])
                .expect("delete-response frame should decode");
            prop_assert_eq!(
                raw.packet_type,
                odmo_protocol::opcode::game::PARTNER_DELETE_RESPONSE,
                "the failed-password reject uses the partner-delete opcode"
            );
            let mut reader = PacketReader::new(raw.payload);
            let code = reader.read_i32().expect("delete-response code");
            prop_assert_eq!(code, -1, "the failed-password reject carries i32 -1");
        } else {
            prop_assert!(
                responses.is_empty(),
                "an item-to-digimon or under-level reject emits no response frame"
            );
        }
    }

    /// Feature: babel-npc-summon-fusion, Property 5: Granted rewards are always catalog members
    ///
    /// Across DATA Summon purchase rolls and Random Box rolls, every granted item
    /// id belongs to the relevant reward table and the seeded zero-weight entry is
    /// never selected.
    /// Validates: Requirements 4.5, 13.2
    #[test]
    fn granted_rewards_are_always_catalog_members(
        summon in summon_roll_scenario_strategy(),
        random_box in random_box_roll_scenario_strategy(),
    ) {
        // --- DATA Summon purchase rolls ---
        let character = CharacterSummary {
            id: 100,
            inventory: summon.inventory.clone(),
            ..Default::default()
        };
        let (app, _repo) = app_and_repo_with_character(
            "summon-members-prop5",
            vec![summon.product.clone()],
            character,
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);

        for _ in 0..summon.purchase_count {
            let responses = app
                .handle_request(
                    &mut session,
                    GameRequest::DigiSummonPurchase {
                        product_id: summon.product_id,
                        ticket_slot: summon.ticket_slot,
                    },
                )
                .expect("summon purchase request should complete");
            prop_assert_eq!(responses.len(), 2, "summon success emits two frames");

            // Decode the granted rewards straight from the response frame so the
            // test never reimplements the weighted roll.
            let raw =
                PacketReader::from_frame(&responses[1]).expect("purchase frame should decode");
            let mut reader = PacketReader::new(raw.payload);
            let result = reader.read_u8().expect("result byte");
            prop_assert_eq!(result, PURCHASE_SUCCESS, "scenario stays in the success regime");
            let _product_id = reader.read_i32().expect("product id");
            let reward_count = reader.read_u16().expect("reward count") as usize;
            prop_assert!(reward_count >= 1, "a successful purchase grants at least one reward");
            for _ in 0..reward_count {
                let item_id = reader.read_i32().expect("reward item id");
                let _amount = reader.read_u16().expect("reward amount");
                let _grade = reader.read_u16().expect("reward grade");
                prop_assert!(
                    summon.member_ids.contains(&item_id),
                    "granted summon item {} is not a member of the product reward table",
                    item_id
                );
                prop_assert_ne!(
                    item_id,
                    summon.zero_weight_id,
                    "a zero-weight summon reward was granted"
                );
            }
        }

        // --- Random Box rolls ---
        let box_character = CharacterSummary {
            id: 100,
            inventory: random_box.inventory.clone(),
            ..Default::default()
        };
        let (box_app, _box_repo) = app_and_repo_with_random_box(
            "random-box-members-prop5",
            box_character,
            random_box.rewards.clone(),
        );

        let mut box_session = GameSession::new(1);
        box_session.character_id = Some(100);

        for _ in 0..random_box.purchase_count {
            let responses = box_app
                .handle_request(
                    &mut box_session,
                    GameRequest::RandomBoxPurchase {
                        flag: 0,
                        product_id: 0,
                        item_uid: 0,
                        count: 1,
                        state: 0,
                    },
                )
                .expect("random box purchase request should complete");
            // A roomy inventory always grants, so the success path emits the
            // inventory reload followed by the purchase response.
            prop_assert_eq!(responses.len(), 2, "random box grant emits two frames");

            let raw = PacketReader::from_frame(&responses[1])
                .expect("random box frame should decode");
            let mut reader = PacketReader::new(raw.payload);
            let _field0 = reader.read_i32().expect("field0");
            let _field1 = reader.read_i32().expect("field1");
            let _field2 = reader.read_u16().expect("field2");
            let granted_count = reader.read_u8().expect("granted list length") as usize;
            prop_assert_eq!(granted_count, 1, "a random box grant lists exactly one reward");
            for _ in 0..granted_count {
                let item_id = reader.read_i32().expect("granted item id");
                let _amount = reader.read_i32().expect("granted amount");
                prop_assert!(
                    random_box.member_ids.contains(&item_id),
                    "granted box item {} is not a member of the box reward table",
                    item_id
                );
                prop_assert_ne!(
                    item_id,
                    random_box.zero_weight_id,
                    "a zero-weight box reward was granted"
                );
            }
        }
    }
}

/// Build an inventory with no spare capacity: `size` equals the slot count and
/// every slot is occupied, so a later grant finds no existing stack to extend,
/// no empty slot to fill, and no room to push. This is the exact full-inventory
/// shape the grant helper rejects on.
fn full_inventory(items: Vec<ItemRecord>) -> InventorySnapshot {
    let size = items.len() as u16;
    InventorySnapshot {
        bits: 0,
        size,
        items,
    }
}

/// One of the three grant-overflow regimes for Property 7. Each carries a full
/// inventory whose every slot is occupied by an item disjoint from the reward
/// id, so the grant step overflows and the handler must roll back to the
/// pre-request state instead of persisting.
#[derive(Debug, Clone)]
enum GrantOverflowScenario {
    /// DATA Summon purchase: the consumed ticket is stocked above its cost so
    /// its slot never empties, leaving the inventory full when the rolled reward
    /// is granted.
    SummonPurchase {
        product: DigiSummonProduct,
        inventory: InventorySnapshot,
        product_id: i32,
        ticket_slot: i32,
    },
    /// Digi/Union reward claim: the claimed tier's reward is granted directly
    /// into a full inventory.
    RewardClaim {
        catalog: DigiCombineCatalog,
        inventory: InventorySnapshot,
        ceiling_type: u8,
        is_union: bool,
    },
    /// Random Box: the rolled reward is granted into a full inventory.
    RandomBox {
        rewards: Vec<RandomBoxReward>,
        inventory: InventorySnapshot,
    },
}

/// Generate a DATA Summon purchase overflow scenario.
///
/// Id ranges stay mutually disjoint so the reward never aliases an in-inventory
/// item: ticket in `1..1000`, reward in `1000..2000`, fillers in `2000..3000`.
/// The ticket cost is fixed at one and the seeded ticket count is at least two,
/// so consuming the ticket leaves its slot occupied (no empty slot opens up) and
/// the inventory stays full when the single rolled reward is granted.
fn summon_overflow_scenario_strategy() -> impl Strategy<Value = GrantOverflowScenario> {
    let fillers = prop::collection::vec((2000_i32..3000, 1_i32..=50), 0..=6);
    (
        1_i32..=1_000_000, // product_id
        1_i32..1000,       // ticket_item_id
        2_i32..=50,        // ticket_amount (strictly above the fixed cost of 1)
        1000_i32..2000,    // reward_item_id
        1_i32..=10,        // reward_amount
        1_i32..=100,       // reward_weight (positive)
        fillers,
        0_usize..=6, // ticket insertion position
    )
        .prop_map(
            |(
                product_id,
                ticket_item_id,
                ticket_amount,
                reward_item_id,
                reward_amount,
                reward_weight,
                fillers,
                ticket_pos,
            )| {
                let mut items: Vec<ItemRecord> = fillers
                    .into_iter()
                    .map(|(id, amount)| ItemRecord::new(id, amount))
                    .collect();
                let ticket_slot = ticket_pos.min(items.len());
                items.insert(ticket_slot, ItemRecord::new(ticket_item_id, ticket_amount));
                let inventory = full_inventory(items);

                let product = DigiSummonProduct {
                    product_id,
                    draw_count: 1,
                    tickets: vec![DigiSummonTicket {
                        item_id: ticket_item_id,
                        cost: 1,
                    }],
                    rewards: vec![DigiSummonReward {
                        item_id: reward_item_id,
                        amount: reward_amount,
                        weight: reward_weight,
                        ..DigiSummonReward::default()
                    }],
                    ..DigiSummonProduct::default()
                };

                GrantOverflowScenario::SummonPurchase {
                    product,
                    inventory,
                    product_id,
                    ticket_slot: ticket_slot as i32,
                }
            },
        )
}

/// Generate a Digi/Union combine reward-claim overflow scenario.
///
/// The single-rank catalog grants one reward whose id (`9000..9500`) is disjoint
/// from the inventory fillers (`2000..3000`), and the inventory is full, so the
/// keyed reward grant overflows. At least one filler keeps the inventory
/// non-empty. Digi and Union are exercised through a generated flag.
fn reward_claim_overflow_scenario_strategy() -> impl Strategy<Value = GrantOverflowScenario> {
    let fillers = prop::collection::vec((2000_i32..3000, 1_i32..=50), 1..=8);
    (
        9000_i32..9500, // reward_item_id (disjoint from fillers)
        0_u8..=3,       // ceiling_type
        any::<bool>(),  // is_union
        fillers,
    )
        .prop_map(|(reward_item_id, ceiling_type, is_union, fillers)| {
            let items: Vec<ItemRecord> = fillers
                .into_iter()
                .map(|(id, amount)| ItemRecord::new(id, amount))
                .collect();
            let inventory = full_inventory(items);
            let catalog = combine_catalog_seed(ceiling_type, reward_item_id);
            GrantOverflowScenario::RewardClaim {
                catalog,
                inventory,
                ceiling_type,
                is_union,
            }
        })
}

/// Generate a Random Box overflow scenario.
///
/// Every reward id (`1000..2000`) is disjoint from the inventory fillers
/// (`2000..3000`) and at least one reward carries a positive weight, so the
/// weighted pick always returns a reward whose grant overflows the full
/// inventory regardless of which entry is rolled.
fn random_box_overflow_scenario_strategy() -> impl Strategy<Value = GrantOverflowScenario> {
    let positives = prop::collection::vec((1000_i32..2000, 1_u32..=100, 1_u16..=10), 1..=4);
    let fillers = prop::collection::vec((2000_i32..3000, 1_i32..=50), 1..=8);
    (positives, fillers).prop_map(|(positives, fillers)| {
        let rewards: Vec<RandomBoxReward> = positives
            .into_iter()
            .map(|(item_id, weight, amount)| RandomBoxReward {
                item_id,
                amount,
                weight,
            })
            .collect();
        let items: Vec<ItemRecord> = fillers
            .into_iter()
            .map(|(id, amount)| ItemRecord::new(id, amount))
            .collect();
        let inventory = full_inventory(items);
        GrantOverflowScenario::RandomBox { rewards, inventory }
    })
}

/// Generate any of the three grant-overflow regimes.
fn grant_overflow_scenario_strategy() -> impl Strategy<Value = GrantOverflowScenario> {
    prop_oneof![
        summon_overflow_scenario_strategy(),
        reward_claim_overflow_scenario_strategy(),
        random_box_overflow_scenario_strategy(),
    ]
}

/// Summon purchase overflow result byte. Mirrors the handler's
/// `DIGI_SUMMON_INVENTORY_FULL`, which is not exported.
const PURCHASE_INVENTORY_FULL: u8 = 4;
/// Combine reward-claim overflow result byte. Mirrors the handler's
/// `COMBINE_RESULT_INVENTORY_FULL`, which is not exported.
const COMBINE_INVENTORY_FULL: u8 = 3;

proptest! {
    #![proptest_config(config())]

    /// Feature: babel-npc-summon-fusion, Property 7: Grant overflow rolls back to the pre-state
    ///
    /// Across DATA Summon purchase, Digi/Union reward claim, and Random Box, a
    /// grant that would overflow a full inventory is rejected and leaves the net
    /// inventory equal to the pre-request state: the provisional grant is removed
    /// and any consumed ticket is refunded. Each handler restores its snapshot
    /// and returns before `update_inventory`, so the repository double captures
    /// no persist at all — the strongest possible statement of no net mutation.
    ///
    /// Each regime seeds a full inventory (slot count equals capacity, every slot
    /// occupied) whose reward id is disjoint from every stocked item, so the
    /// grant finds no stack to extend, no empty slot, and no room to push. The
    /// summon ticket is stocked above its cost so consuming it never frees a
    /// slot. The summon and reward-claim rejects carry their inventory-full
    /// result byte; the random box reject carries an empty grant list.
    /// Validates: Requirements 5.4, 5.5, 9.4, 13.3
    #[test]
    fn grant_overflow_rolls_back_to_pre_state(scenario in grant_overflow_scenario_strategy()) {
        match scenario {
            GrantOverflowScenario::SummonPurchase {
                product,
                inventory,
                product_id,
                ticket_slot,
            } => {
                let character = CharacterSummary {
                    id: 100,
                    inventory: inventory.clone(),
                    ..Default::default()
                };

                let (app, repo) = app_and_repo_with_character(
                    "summon-overflow-prop7",
                    vec![product],
                    character,
                );

                let mut session = GameSession::new(1);
                session.character_id = Some(100);
                let responses = app
                    .handle_request(
                        &mut session,
                        GameRequest::DigiSummonPurchase {
                            product_id,
                            ticket_slot,
                        },
                    )
                    .expect("purchase request should complete");

                // An overflow rejection emits the purchase response alone, with
                // no inventory reload.
                prop_assert_eq!(responses.len(), 1, "an overflow rejection emits a single frame");

                let raw =
                    PacketReader::from_frame(&responses[0]).expect("purchase frame should decode");
                let mut reader = PacketReader::new(raw.payload);
                let result = reader.read_u8().expect("result byte");
                prop_assert_eq!(
                    result,
                    PURCHASE_INVENTORY_FULL,
                    "a grant overflow rejects with the inventory-full byte"
                );

                // No persist means the ticket is refunded and the grant removed:
                // the net inventory equals the pre-request state.
                prop_assert!(
                    repo.persisted_inventory().is_none(),
                    "an overflow rollback must not persist the inventory"
                );
            }
            GrantOverflowScenario::RewardClaim {
                catalog,
                inventory,
                ceiling_type,
                is_union,
            } => {
                let character = CharacterSummary {
                    id: 100,
                    inventory: inventory.clone(),
                    ..Default::default()
                };

                let (app, repo) = app_and_repo_with_combine_catalog(
                    "combine-claim-overflow-prop7",
                    character,
                    catalog,
                );

                let request = if is_union {
                    GameRequest::UnionCombineRewardClaim { ceiling_type }
                } else {
                    GameRequest::DigiCombineRewardClaim { ceiling_type }
                };

                let mut session = GameSession::new(1);
                session.character_id = Some(100);
                let responses = app
                    .handle_request(&mut session, request)
                    .expect("reward claim request should complete");

                // An overflow rejection emits the reward response alone.
                prop_assert_eq!(responses.len(), 1, "an overflow rejection emits a single frame");

                let raw =
                    PacketReader::from_frame(&responses[0]).expect("reward frame should decode");
                let mut reader = PacketReader::new(raw.payload);
                let result = reader.read_u8().expect("result byte");
                prop_assert_eq!(
                    result,
                    COMBINE_INVENTORY_FULL,
                    "a grant overflow rejects with the inventory-full byte"
                );

                prop_assert!(
                    repo.persisted_inventory().is_none(),
                    "an overflow rollback must not persist the inventory"
                );
            }
            GrantOverflowScenario::RandomBox { rewards, inventory } => {
                let character = CharacterSummary {
                    id: 100,
                    inventory: inventory.clone(),
                    ..Default::default()
                };

                let (app, repo) = app_and_repo_with_random_box(
                    "random-box-overflow-prop7",
                    character,
                    rewards,
                );

                let mut session = GameSession::new(1);
                session.character_id = Some(100);
                let responses = app
                    .handle_request(
                        &mut session,
                        GameRequest::RandomBoxPurchase {
                            flag: 0,
                            product_id: 0,
                            item_uid: 0,
                            count: 1,
                            state: 0,
                        },
                    )
                    .expect("random box purchase request should complete");

                // An overflow rejection emits the empty-result response alone.
                prop_assert_eq!(responses.len(), 1, "an overflow rejection emits a single frame");

                let raw = PacketReader::from_frame(&responses[0])
                    .expect("random box frame should decode");
                let mut reader = PacketReader::new(raw.payload);
                let _field0 = reader.read_i32().expect("field0");
                let _field1 = reader.read_i32().expect("field1");
                let _field2 = reader.read_u16().expect("field2");
                let granted_count = reader.read_u8().expect("granted list length");
                prop_assert_eq!(granted_count, 0, "an overflow grants no reward");

                prop_assert!(
                    repo.persisted_inventory().is_none(),
                    "an overflow rollback must not persist the inventory"
                );
            }
        }
    }
}

/// Account id used by the routing property; the seeded account shares it so the
/// Spirit-craft session guard finds a matching record.
const ROUTING_ACCOUNT_ID: AccountId = 100;

/// Generate an arbitrary list of combine material nodes. Field values are fully
/// arbitrary: grid validity and material availability are irrelevant to routing,
/// since both a valid and an invalid submission resolve to a handler response.
fn routing_combine_material_strategy() -> impl Strategy<Value = Vec<CombineItemRef>> {
    prop::collection::vec(
        (any::<u32>(), any::<u16>(), any::<u16>()).prop_map(|(item_uid, item_type, count)| {
            CombineItemRef {
                item_uid,
                item_type,
                count,
            }
        }),
        0..=8,
    )
}

/// Generate one of the twelve covered C2S request variants with arbitrary but
/// structurally valid field values. The set is split across two `prop_oneof!`
/// groups so each macro stays within the tuple-union arm limit; the two groups
/// compose into the full covered set with roughly uniform weighting.
fn covered_request_strategy() -> impl Strategy<Value = GameRequest> {
    // String fields use printable ASCII; their contents never affect routing.
    let summon_and_box = prop_oneof![
        (any::<i32>(), "[ -~]{0,16}", any::<i32>()).prop_map(|(model_id, name, npc_id)| {
            GameRequest::SpiritToDigimon {
                model_id,
                name,
                npc_id,
            }
        }),
        (any::<u8>(), "[ -~]{0,16}", any::<i32>()).prop_map(|(slot, validation, npc_id)| {
            GameRequest::DigimonToSpirit {
                slot,
                validation,
                npc_id,
            }
        }),
        Just(GameRequest::DigiSummonSyncRequest),
        (any::<i32>(), any::<i32>()).prop_map(|(product_id, ticket_slot)| {
            GameRequest::DigiSummonPurchase {
                product_id,
                ticket_slot,
            }
        }),
        (any::<u8>(), any::<i32>())
            .prop_map(|(flag, index)| GameRequest::RandomBoxList { flag, index }),
        (
            any::<u8>(),
            any::<i32>(),
            any::<i32>(),
            any::<u16>(),
            any::<i32>(),
        )
            .prop_map(|(flag, product_id, item_uid, count, state)| {
                GameRequest::RandomBoxPurchase {
                    flag,
                    product_id,
                    item_uid,
                    count,
                    state,
                }
            }),
    ];

    let combine = prop_oneof![
        Just(GameRequest::DigiCombineSyncRequest),
        (any::<u8>(), routing_combine_material_strategy()).prop_map(|(ceiling_type, materials)| {
            GameRequest::DigiCombine {
                ceiling_type,
                materials,
            }
        }),
        any::<u8>().prop_map(|ceiling_type| GameRequest::DigiCombineRewardClaim { ceiling_type }),
        Just(GameRequest::UnionCombineSyncRequest),
        (any::<u8>(), routing_combine_material_strategy()).prop_map(|(ceiling_type, materials)| {
            GameRequest::UnionCombine {
                ceiling_type,
                materials,
            }
        }),
        any::<u8>().prop_map(|ceiling_type| GameRequest::UnionCombineRewardClaim { ceiling_type }),
    ];

    prop_oneof![summon_and_box, combine]
}

proptest! {
    #![proptest_config(config())]

    /// Feature: babel-npc-summon-fusion, Property 17: Every covered opcode routes to a handler
    ///
    /// Every covered C2S request variant reaches a registered handler. The
    /// request match in the game application is exhaustive with no unsupported
    /// fall-through, so a routed handler always answers `Ok`; business-rule
    /// rejections are themselves `Ok` frames. A seeded character and account
    /// (plus an authenticated session) keep no covered variant from tripping the
    /// shared unauthenticated or character-not-found guards, so the call
    /// returning `Ok` proves it reached its handler rather than an unrouted sink.
    /// Validates: Requirements 14.4
    #[test]
    fn every_covered_opcode_routes_to_a_handler(request in covered_request_strategy()) {
        // Seed a character and account so the shared session guards never
        // short-circuit before routing.
        let character = CharacterSummary {
            id: 100,
            ..Default::default()
        };
        let account = seeded_account(ROUTING_ACCOUNT_ID, "tamer@odmo.local", "secret-pw");
        let repo = Arc::new(CatalogRepository {
            character: RwLock::new(Some(character)),
            account: RwLock::new(Some(account)),
            ..CatalogRepository::default()
        });
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("routing-prop17"),
            },
            repo,
        );

        let mut session = GameSession::new(1);
        session.account_id = Some(ROUTING_ACCOUNT_ID);
        session.character_id = Some(100);

        let result = app.handle_request(&mut session, request);
        prop_assert!(
            result.is_ok(),
            "a covered opcode must route to a handler, but the call returned: {:?}",
            result.err()
        );
    }
}
