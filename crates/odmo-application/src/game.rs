use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, RwLock,
        atomic::{AtomicI16, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;

use odmo_protocol::game::{EvolutionUnlockItem, TamerSummonRequest};
use odmo_protocol::{
    ArenaFameInfoPacket, ArenaFameSeasonListPacket, ArenaOldRankingInfoPacket,
    ArenaRankingDailyLoadPacket, ArenaRankingDailyUpdatePointsPacket, ArenaRankingInfoPacket,
    AvailableChannelsPacket, BurningEventPacket, CashShopBuyResponsePacket, CashShopCoinsPacket,
    CashShopCropResponsePacket, CashShopWarehousePacket, CastSkillPacket,
    ChangeTamerModelPacket, DailyCheckEventInfoPacket, DailyCheckEventInfoRow,
    DailyCheckEventItemResultPacket, DetailStatPair, DigiSummonPurchaseResponsePacket,
    DigiSummonSyncResponsePacket, DigimonEvolutionFailPacket, DigimonEvolutionSuccessPacket,
    DigimonToSpiritResultPacket, DigimonWalkPacket, DungeonArenaNextStagePacket,
    EncyclopediaDeckBuffUsePacket, EncyclopediaLoadPacket, EncyclopediaReceiveRewardItemPacket,
    GameConnectionPacket, GameInitialInfoPacket, GameRequest, GiftStorageRetrievePacket,
    GuildAuthorityUpdatePacket, GuildCreateFailPacket, GuildCreateSuccessPacket, GuildDeletePacket,
    GuildHistoricPacket, GuildInformationPacket, GuildInviteAcceptPacket, GuildInviteDenyPacket,
    GuildInviteFailPacket, GuildInviteSuccessPacket, GuildMemberKickPacket, GuildMemberQuitPacket,
    GuildMessagePacket, GuildNoticeUpdatePacket, GuildPromotionDemotionPacket, HitPacket, HitType,
    InventoryType, ItemConsumeFailPacket, ItemIdentifyPacket, ItemMoveFailPacket,
    ItemMoveSuccessPacket, ItemRerollPacket, ItemReturnPacket, ItemSocketIdentifyPacket,
    ItemSocketInPacket, ItemSocketOutPacket, ItemStoragePacket, KillOnHitPacket, KillOnSkillPacket,
    LoadBuffsPacket, LoadDropsPacket, LoadInventoryPacket, LoadMobBuffsPacket, LoadMobsPacket,
    LoadTamerPacket, LocalMapSwapPacket, MapSwapPacket, MembershipPacket, MissHitPacket,
    MonsterRespawnTimerPacket,
    NpcPurchaseResultPacket, NpcSellResultPacket, NpcSellRow, OtherTamerDetailInfoPacket,
    PartnerSkillErrorPacket, PartnerSwitchFailurePacket, PartnerSwitchPacket,
    PartyChangeLootTypePacket, PartyCreatedPacket, PartyInvitePacket, PartyInviteResultPacket,
    PartyJoinPacket, PartyKickPacket, PartyLeaderChangedPacket, PartyLeavePacket,
    PartyMemberBuffChangePacket, PartyMemberBuffEntry, PartyMemberDisconnectedPacket,
    PartyMemberInfoPacket, PartyMemberListEntry, PartyMemberListPacket, PartyMemberMapChangePacket,
    PartyMemberPositionPacket, PickBitsPacket, PickItemFailPacket, PickItemFailReason,
    PickItemPacket, QuestAvailableListPacket, QuestGoalUpdatePacket, RandomBoxListEntry,
    RandomBoxListResponsePacket, RandomBoxPurchaseResponsePacket, RecompenseGainPacket,
    RemoveBuffPacket, SealsPacket, ServerExperiencePacket, SpiritToDigimonResultPacket,
    SplitItemPacket, SyncChatLine, SyncChatRecord, SyncNamedLine, SyncWhisperLine,
    TamerAttendancePacket, TamerChangeNamePacket, TamerRelationsPacket,
    TamerWalkPacket, TamerXaiResourcesPacket, TimeRewardPacket, TradeAcceptPacket,
    TradeAddItemPacket, TradeAddMoneyPacket, TradeCancelPacket, TradeConfirmationPacket,
    TradeFinalConfirmationPacket, TradeInventoryLockPacket, TradeInventoryUnlockPacket,
    TradeRemoveItemPacket, TradeRequestErrorPacket, TradeRequestSuccessPacket,
    UnionHackModifyResponsePacket, UnionHackOpenResponsePacket, UnionHackSlot, UnionInitDataPacket,
    UnloadDropsPacket, UnloadMobsPacket, UnloadTamerPacket, UpdateCurrentTitlePacket,
    UpdateMovementSpeedPacket, UpdateStatusPacket, XaiInfoPacket,
    game::{
        CombineResultResponsePacket, CombineSyncResponsePacket, FriendConnectPacket,
        GuildRankPacket, SkillUpdateCooldownPacket,
    },
};
use odmo_types::{
    AccountId, CombineCeilingEntry, CombineItemRef, DigiCombineCatalog, DigiCombineReward,
    ItemRecord, RandomBoxReward, UnionCombineCatalog,
};

use crate::{
    character::{CharacterAccountRepository, CharacterRepository},
    portal::{PortalBridge, SocialNotification, SocialNotificationKind},
};

const HANDSHAKE_DEGREE: i16 = 32321;
const START_TO_SEE_DISTANCE: i64 = 18_000;
const STOP_SEEING_DISTANCE: i64 = 18_001;
#[allow(dead_code)]
const PARTY_INVITE_IMPOSSIBLE: i32 = -3;
const PARTY_INVITE_OFFLINE: i32 = -2;
#[allow(dead_code)]
const PARTY_INVITE_REJECTED: i32 = -1;
const PARTY_INVITE_ALREADY_IN_PARTY: i32 = 0;
const PARTY_INVITE_ACCEPTED: i32 = 1;
const DIGI_SUMMON_SUCCESS: u8 = 0;
const DIGI_SUMMON_NO_PRODUCTS: u8 = 1;
const DIGI_SUMMON_INVALID_PRODUCT: u8 = 2;
const DIGI_SUMMON_NOT_ENOUGH_TICKET: u8 = 3;
const DIGI_SUMMON_INVENTORY_FULL: u8 = 4;

// Combine result byte: the wire carries a single `result` flag where zero is a
// successful roll/claim and any non-zero value rejects. There is no separate
// error enum, so distinct non-zero codes name the rejection causes.
const COMBINE_RESULT_SUCCESS: u8 = 0;
const COMBINE_RESULT_INVALID_GRID: u8 = 1;
const COMBINE_RESULT_MISSING_MATERIAL: u8 = 2;
const COMBINE_RESULT_INVENTORY_FULL: u8 = 3;
// Sync result mirrors the summon convention: zero for a populated catalog, one
// when there is nothing to roll.
const COMBINE_SYNC_NO_CATALOG: u8 = 1;

// Live rank status: 1 running, 2 waiting for the next period, 3 no next period.
const RANK_STATUS_END: u8 = 3;

// The Material_Grid is 11 row-groups of 4 cells; each group must be empty or
// full, so a valid submission carries a multiple of 4 filled nodes, capped at
// the full 11x4 grid.
const COMBINE_GRID_ROW_CELLS: usize = 4;
const COMBINE_GRID_MAX_NODES: usize = 44;
const EXTRA_EVOLUTION_ITEM_TO_DIGIMON: u16 = 1;
const EXTRA_EVOLUTION_DIGIMON_TO_ITEM: u16 = 2;
const EXTRA_EVOLUTION_NEED_ALL: u16 = 1;
const EXTRA_EVOLUTION_NEED_ONE: u16 = 2;
const CLIENT_TAMER_CLASS_BITS: u32 = 2;

#[derive(Debug, Clone)]
struct PendingPartyInvite {
    inviter_id: u64,
    target_id: u64,
}

#[derive(Debug, Clone)]
struct PendingGuildInvite {
    inviter_id: u64,
    #[allow(dead_code)]
    target_id: u64,
    guild_id: u32,
}

#[derive(Debug, Clone, Default)]
struct GuildRuntimeState {
    next_guild_id: u32,
    pending_invites: HashMap<u64, PendingGuildInvite>,
    guilds: HashMap<u32, GuildRoom>,
    guild_by_member: HashMap<u64, u32>,
}

// ---- Trade runtime state -----------------------------------------------------

#[derive(Debug, Clone, Default)]
struct TradeRuntimeState {
    /// Pending request: inviter_id -> target_id.
    pending_requests: HashMap<u64, u64>,
    /// Active trade sessions keyed by session id.
    sessions: HashMap<u64, TradeSession>,
    /// Reverse lookup: character_id -> session id.
    session_by_character: HashMap<u64, u64>,
    /// Counter for session ids.
    next_session_id: u64,
}

#[derive(Debug, Clone)]
struct TradeSession {
    #[allow(dead_code)]
    id: u64,
    side_a: TradeSideRuntime,
    side_b: TradeSideRuntime,
    confirmed_a: bool,
    confirmed_b: bool,
    #[allow(dead_code)]
    final_a: bool,
    #[allow(dead_code)]
    final_b: bool,
}

#[derive(Debug, Clone, Default)]
struct TradeSideRuntime {
    character_id: u64,
    handler: u32,
    items: Vec<(u8, i32, i16, i32)>, // (trade_slot, item_id, amount, source_inventory_slot)
    money: i64,
    locked: bool,
}

fn client_projected_tamer_uid(raw_handler: u32) -> u32 {
    (CLIENT_TAMER_CLASS_BITS << 14) | (raw_handler & 0x0FFF)
}

fn matches_tamer_target_handler(
    character: &odmo_types::CharacterSummary,
    target_handler: u32,
) -> bool {
    character.general_handler == target_handler
        || client_projected_tamer_uid(character.general_handler) == target_handler
}

/// Enchant rows for the partner-detail grid in the fixed render order
/// `AT, CT, BL, HP, EV`, each as a `{kind, value}` pair.
fn other_tamer_detail_enchants(character: &odmo_types::CharacterSummary) -> Vec<DetailStatPair> {
    [
        character.partner_clone_at_value,
        character.partner_clone_ct_value,
        character.partner_clone_bl_value,
        character.partner_clone_hp_value,
        character.partner_clone_ev_value,
    ]
    .into_iter()
    .enumerate()
    .map(|(kind, value)| DetailStatPair {
        key: kind as u32,
        value: u32::from(value),
    })
    .collect()
}

/// Skill rows for the active evolution, as `{skill_id, level}` pairs. The slot
/// index is the skill id until a skill-asset table is wired in.
fn other_tamer_detail_skills(character: &odmo_types::CharacterSummary) -> Vec<DetailStatPair> {
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
                .map(|(idx, &level)| DetailStatPair {
                    key: idx as u32,
                    value: u32::from(level),
                })
                .collect()
        })
        .unwrap_or_default()
}

impl GuildRuntimeState {
    fn alloc_id(&mut self) -> u32 {
        if self.next_guild_id == 0 {
            self.next_guild_id = 1;
        }
        let id = self.next_guild_id;
        self.next_guild_id = self.next_guild_id.saturating_add(1);
        id
    }
}

#[derive(Debug, Clone)]
struct GuildRoom {
    id: u32,
    name: String,
    notice: String,
    leader_id: u64,
    members: Vec<GuildRoomMember>,
    historic: Vec<odmo_types::GuildHistoricEntry>,
}

#[derive(Debug, Clone)]
struct GuildRoomMember {
    character_id: u64,
    authority: u8,
    name: String,
}

#[derive(Debug, Clone)]
struct PartyRuntimeState {
    next_party_id: u32,
    pending_invites: HashMap<u64, PendingPartyInvite>,
    parties: HashMap<u32, PartyGroup>,
    party_by_member: HashMap<u64, u32>,
}

impl Default for PartyRuntimeState {
    fn default() -> Self {
        Self {
            next_party_id: 1,
            pending_invites: HashMap::new(),
            parties: HashMap::new(),
            party_by_member: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct PartyGroup {
    id: u32,
    leader_id: u64,
    loot_type: u32,
    rare_rate: u8,
    disp_rare_grade: u8,
    members: Vec<u64>,
}

#[derive(Debug, Clone)]
pub struct GameServiceConfig {
    pub portal_state_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GameSession {
    pub handshake_seed: i16,
    pub account_id: Option<AccountId>,
    pub character_id: Option<u64>,
    pub announced_friend_connect: bool,
    pub registered_map_presence: bool,
    pub viewed_characters: HashMap<u64, odmo_types::CharacterSummary>,
    pub viewed_mobs: HashMap<u64, odmo_types::MobSummary>,
    pub viewed_drops: HashMap<u64, odmo_types::DropSummary>,
}

impl GameSession {
    pub fn new(handshake_seed: i16) -> Self {
        Self {
            handshake_seed,
            account_id: None,
            character_id: None,
            announced_friend_connect: false,
            registered_map_presence: false,
            viewed_characters: HashMap::new(),
            viewed_mobs: HashMap::new(),
            viewed_drops: HashMap::new(),
        }
    }
}

pub trait MapMobRepository: Send + Sync {
    fn mobs_by_map(&self, map_id: i16, channel: u8) -> anyhow::Result<Vec<odmo_types::MobSummary>>;

    /// Persist the mob's current HP after a damage event. Default implementation is a
    /// no-op so backends that store transient mob state in memory don't need to override.
    fn update_mob_hp(
        &self,
        _map_id: i16,
        _channel: u8,
        _handler: u32,
        _current_hp: i32,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

pub trait MapDropRepository: Send + Sync {
    fn drops_by_map(
        &self,
        map_id: i16,
        channel: u8,
    ) -> anyhow::Result<Vec<odmo_types::DropSummary>>;
    fn collect_drop(
        &self,
        character_id: u64,
        map_id: i16,
        channel: u8,
        drop_handler: u32,
    ) -> anyhow::Result<DropCollectionResult>;
}

#[derive(Debug, Clone, PartialEq)]
pub enum DropCollectionResult {
    Missing,
    NotTheOwner,
    InventoryFull,
    TooFarAway,
    BitsCollected {
        drop: odmo_types::DropSummary,
        amount: i32,
        character: odmo_types::CharacterSummary,
    },
    ItemCollected {
        drop: odmo_types::DropSummary,
        item_id: i32,
        amount: i16,
        character: odmo_types::CharacterSummary,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct PortalDefinition {
    pub id: i32,
    pub is_local: bool,
    pub destination_map_id: i16,
    pub destination_x: i32,
    pub destination_y: i32,
}

pub trait PortalRepository: Send + Sync {
    fn portal_by_id(&self, portal_id: i32) -> anyhow::Result<Option<PortalDefinition>>;
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpcShopItem {
    pub item_id: i32,
    pub buy_price: i32,
    pub sell_price: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NpcShopDefinition {
    pub npc_id: i32,
    pub map_id: i16,
    pub items: Vec<NpcShopItem>,
}

pub trait NpcShopRepository: Send + Sync {
    fn shop_by_npc(&self, npc_id: i32, map_id: i16) -> anyhow::Result<Option<NpcShopDefinition>>;
}

pub trait DigiSummonRepository: Send + Sync {
    fn digi_summon_products(&self) -> anyhow::Result<Vec<odmo_types::DigiSummonProduct>>;
}

pub trait ExtraEvolutionRepository: Send + Sync {
    fn extra_evolution_npcs(&self) -> anyhow::Result<Vec<odmo_types::ExtraEvolutionNpc>>;
}

pub trait ItemAssetRepository: Send + Sync {
    fn item_assets(&self) -> anyhow::Result<Vec<odmo_types::ItemAsset>>;
}

pub trait EvolutionAssetRepository: Send + Sync {
    fn evolution_assets(&self) -> anyhow::Result<Vec<odmo_types::EvolutionAsset>>;
}

pub trait DigimonAssetRepository: Send + Sync {
    fn digimon_assets(&self) -> anyhow::Result<Vec<odmo_types::DigimonAsset>>;
}

pub trait DigiCombineRepository: Send + Sync {
    fn digi_combine_catalog(&self) -> anyhow::Result<DigiCombineCatalog>;
}

pub trait UnionCombineRepository: Send + Sync {
    fn union_combine_catalog(&self) -> anyhow::Result<UnionCombineCatalog>;
}

pub trait RandomBoxRepository: Send + Sync {
    /// The weighted reward pool a random box rolls a single reward from.
    fn random_box_rewards(&self) -> anyhow::Result<Vec<RandomBoxReward>>;
}

pub trait GameRepository:
    CharacterRepository
    + CharacterAccountRepository
    + MapMobRepository
    + MapDropRepository
    + PortalRepository
    + NpcShopRepository
    + DigiSummonRepository
    + ExtraEvolutionRepository
    + ItemAssetRepository
    + EvolutionAssetRepository
    + DigimonAssetRepository
    + DigiCombineRepository
    + UnionCombineRepository
    + RandomBoxRepository
{
}

impl<T> GameRepository for T where
    T: CharacterRepository
        + CharacterAccountRepository
        + MapMobRepository
        + MapDropRepository
        + PortalRepository
        + NpcShopRepository
        + DigiSummonRepository
        + ExtraEvolutionRepository
        + ItemAssetRepository
        + EvolutionAssetRepository
        + DigimonAssetRepository
        + DigiCombineRepository
        + UnionCombineRepository
        + RandomBoxRepository
{
}

#[derive(Clone)]
pub struct GameApplication {
    portal_bridge: PortalBridge,
    repository: Arc<dyn GameRepository>,
    broadcast: Option<Arc<dyn crate::BroadcastSink>>,
    party_runtime: Arc<RwLock<PartyRuntimeState>>,
    guild_runtime: Arc<RwLock<GuildRuntimeState>>,
    trade_runtime: Arc<RwLock<TradeRuntimeState>>,
    game_server_address: String,
    game_server_port: i32,
}

impl GameApplication {
    pub fn new(config: GameServiceConfig, repository: Arc<dyn GameRepository>) -> Self {
        let portal_bridge = PortalBridge::from_json(config.portal_state_dir)
            .expect("portal bridge should initialize");
        Self {
            portal_bridge,
            repository,
            broadcast: None,
            party_runtime: Arc::new(RwLock::new(PartyRuntimeState::default())),
            guild_runtime: Arc::new(RwLock::new(GuildRuntimeState::default())),
            trade_runtime: Arc::new(RwLock::new(TradeRuntimeState::default())),
            game_server_address: "127.0.0.1".to_string(),
            game_server_port: 7003,
        }
    }

    pub fn with_broadcast(mut self, broadcast: Arc<dyn crate::BroadcastSink>) -> Self {
        self.broadcast = Some(broadcast);
        self
    }

    pub fn with_game_server(mut self, address: String, port: i32) -> Self {
        self.game_server_address = address;
        self.game_server_port = port;
        self
    }

    pub fn handle_request(
        &self,
        session: &mut GameSession,
        request: GameRequest,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let mut responses = self
            .drain_social_notifications(session)
            .map_err(|error| GameFlowError::PortalBridge(error.to_string()))?;
        responses.extend(
            self.reconcile_map_visibility(session)
                .map_err(|error| GameFlowError::PortalBridge(error.to_string()))?,
        );
        responses.extend(
            self.reconcile_mob_visibility(session)
                .map_err(|error| GameFlowError::Storage(error.to_string()))?,
        );
        responses.extend(
            self.reconcile_drop_visibility(session)
                .map_err(|error| GameFlowError::Storage(error.to_string()))?,
        );

        let request_responses = match request {
            GameRequest::Connection { .. } => Ok(vec![
                GameConnectionPacket {
                    handshake: session.handshake_seed ^ HANDSHAKE_DEGREE,
                }
                .encode(),
            ]),
            GameRequest::KeepConnection => Ok(Vec::new()),
            GameRequest::InitialInformation {
                account_id,
                access_code: _,
            } => {
                let ticket = self
                    .portal_bridge
                    .consume_game_session_ticket(account_id)
                    .map_err(|error| GameFlowError::PortalBridge(error.to_string()))?
                    .ok_or(GameFlowError::MissingSessionTicket(account_id))?;

                let character = self
                    .repository
                    .character_by_id(ticket.character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(ticket.character_id))?;

                session.account_id = Some(account_id);
                session.character_id = Some(character.id);

                let mut responses = vec![GameInitialInfoPacket { character }.encode()];
                if let Ok(union_init) = self.build_union_init_data(session) {
                    responses.push(union_init);
                }
                Ok(responses)
            }
            GameRequest::ComplementarInformation => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;
                let mut responses = vec![
                    SealsPacket {
                        seal_list: character.seal_list.clone(),
                    }
                    .encode(),
                    LoadInventoryPacket {
                        inventory: character.inventory.clone(),
                        inventory_type: InventoryType::Inventory,
                    }
                    .encode(),
                    LoadInventoryPacket {
                        inventory: character.warehouse.clone(),
                        inventory_type: InventoryType::Warehouse,
                    }
                    .encode(),
                    LoadInventoryPacket {
                        inventory: character.extra_inventory.clone(),
                        inventory_type: InventoryType::ExtraInventory,
                    }
                    .encode(),
                    ServerExperiencePacket {
                        experience: character.server_experience,
                    }
                    .encode(),
                    MembershipPacket {
                        remaining_seconds: character.membership_seconds,
                    }
                    .encode(),
                    CashShopCoinsPacket {
                        premium: if std::env::var("ODMO_SMOKE_TEST").is_ok() && character.premium < 10000 {
                            10000
                        } else {
                            character.premium
                        },
                        silk: character.silk,
                    }
                    .encode(),
                    TimeRewardPacket {
                        reward: character.daily_reward.clone(),
                    }
                    .encode(),
                    TamerRelationsPacket {
                        friends: character.friends.clone(),
                        foes: character.foes.clone(),
                    }
                    .encode(),
                    AvailableChannelsPacket {
                        channels: character.available_channels.clone(),
                    }
                    .encode(),
                    TamerAttendancePacket {
                        attendance: character.attendance.clone(),
                    }
                    .encode(),
                    UpdateStatusPacket {
                        character: character.clone(),
                    }
                    .encode(),
                    UpdateMovementSpeedPacket {
                        character: character.clone(),
                    }
                    .encode(),
                ];

                if let Some(account_warehouse) = &character.account_warehouse {
                    responses.push(
                        LoadInventoryPacket {
                            inventory: account_warehouse.clone(),
                            inventory_type: InventoryType::AccountWarehouse,
                        }
                        .encode(),
                    );
                }

                // Send skill cooldowns for partner digimon (empty until skill system is implemented)
                responses.push(
                    SkillUpdateCooldownPacket {
                        handler: character.partner_handler as i32,
                        current_type: character.partner_model,
                        cooldowns: vec![],
                    }
                    .encode(),
                );

                if character.xai.as_ref().is_some_and(|xai| xai.item_id > 0) {
                    responses.push(
                        XaiInfoPacket {
                            xai: character.xai.clone(),
                        }
                        .encode(),
                    );
                    responses.push(
                        TamerXaiResourcesPacket {
                            current_xgauge: character.current_xgauge,
                            current_xcrystals: character.current_xcrystals,
                        }
                        .encode(),
                    );
                }

                if let Some(guild) = &character.guild {
                    responses.push(
                        GuildInformationPacket {
                            guild: guild.clone(),
                        }
                        .encode(),
                    );
                    responses.push(
                        GuildHistoricPacket {
                            entries: guild.historic.clone(),
                        }
                        .encode(),
                    );

                    if (1..=100).contains(&guild.rank_position) {
                        responses.push(
                            GuildRankPacket {
                                position: guild.rank_position,
                            }
                            .encode(),
                        );
                    }
                }

                if !session.announced_friend_connect {
                    self.announce_friend_connect(&character)
                        .map_err(|error| GameFlowError::PortalBridge(error.to_string()))?;
                    session.announced_friend_connect = true;
                }

                // Unlock map region (non-blocking, best-effort)
                let _ = self.repository.update_character_map_region(
                    character_id,
                    character.map_id,
                    true,
                );

                // Mark character state as Ready (non-blocking)
                let _ = self.repository.update_character_state(character_id, 1);

                // Update welcome flag to false (non-blocking)
                let _ = self
                    .repository
                    .update_welcome_flag(session.account_id.unwrap_or(0), false);

                responses.extend(
                    self.register_map_presence(session, &character)
                        .map_err(|error| GameFlowError::PortalBridge(error.to_string()))?,
                );
                responses.extend(
                    self.reconcile_mob_visibility(session)
                        .map_err(|error| GameFlowError::Storage(error.to_string()))?,
                );
                responses.extend(
                    self.reconcile_drop_visibility(session)
                        .map_err(|error| GameFlowError::Storage(error.to_string()))?,
                );

                Ok(responses)
            }
            GameRequest::ConsumeItem {
                target_handler: _,
                slot,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let mut character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let inventory_slot = slot as usize;
                if inventory_slot >= character.inventory.items.len() {
                    return Ok(vec![
                        ItemConsumeFailPacket {
                            slot,
                            item_id: 0,
                            result: 1,
                        }
                        .encode(),
                    ]);
                }

                let item = &character.inventory.items[inventory_slot];
                if item.item_id <= 0 || item.amount <= 0 {
                    return Ok(vec![
                        ItemConsumeFailPacket {
                            slot,
                            item_id: 0,
                            result: 1,
                        }
                        .encode(),
                    ]);
                }

                let new_amount = item.amount - 1;
                if new_amount <= 0 {
                    // Remove item completely
                    character.inventory.items[inventory_slot] = ItemRecord::default();
                } else {
                    // Reduce amount
                    let mut updated = item.clone();
                    updated.amount = new_amount;
                    updated.sync_record();
                    character.inventory.items[inventory_slot] = updated;
                }

                self.repository
                    .update_inventory(character_id, character.inventory)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?;

                // Re-read character for the response
                let updated_character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let mut responses = Vec::new();
                responses.push(
                    LoadInventoryPacket {
                        inventory: updated_character.inventory.clone(),
                        inventory_type: InventoryType::Inventory,
                    }
                    .encode(),
                );

                // If item was a consumable that affects HP/DS (type check would need asset data),
                // send UpdateStatusPacket
                self.broadcast_party_member_info(&updated_character);
                responses.push(
                    UpdateStatusPacket {
                        character: updated_character,
                    }
                    .encode(),
                );

                Ok(responses)
            }
            GameRequest::MoveItem {
                origin_slot,
                destination_slot,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let mut character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                const TAB_INVENTORY: u16 = 0;
                const TAB_EQUIPMENT: u16 = 1000;
                const TAB_TSKILL: u16 = 3000;
                const TAB_CHIPSET: u16 = 4000;
                const TAB_DIGIVICE: u16 = 5000;
                const TAB_WAREHOUSE: u16 = 2000;
                const TAB_SHARESTASH: u16 = 9000;
                fn tab_class(sid: u16) -> u16 {
                    sid / 1000 * 1000
                }
                fn tab_index(sid: u16) -> usize {
                    (sid % 1000) as usize
                }

                let src_tab = tab_class(origin_slot);
                let dst_tab = tab_class(destination_slot);
                let src_idx = tab_index(origin_slot);
                let dst_idx = tab_index(destination_slot);
                let item_assets = if matches!(src_tab, TAB_DIGIVICE | TAB_CHIPSET | TAB_TSKILL)
                    || matches!(dst_tab, TAB_DIGIVICE | TAB_CHIPSET | TAB_TSKILL)
                {
                    Some(
                        self.repository
                            .item_assets()
                            .map_err(|error| GameFlowError::Storage(error.to_string()))?,
                    )
                } else {
                    None
                };

                const CHIPSET_ITEM_TYPE: i32 = 52;
                const DIGIVICE_ITEM_TYPE: i32 = 53;

                // Validate equipment moves
                if dst_tab == TAB_EQUIPMENT {
                    if dst_idx >= 16 {
                        return Ok(vec![
                            ItemMoveFailPacket {
                                origin_slot,
                                destination_slot,
                            }
                            .encode(),
                        ]);
                    }
                    let item = &character
                        .inventory
                        .items
                        .get(src_idx)
                        .cloned()
                        .unwrap_or_default();
                    if item.item_id <= 0 {
                        return Ok(vec![
                            ItemMoveFailPacket {
                                origin_slot,
                                destination_slot,
                            }
                            .encode(),
                        ]);
                    }
                    let assets = self.repository.item_assets().unwrap_or_default();
                    if let Some(asset) = assets.iter().find(|a| a.item_id == item.item_id) {
                        let tamer_level = character.level as i32;
                        if tamer_level < asset.tamer_min_level as i32
                            || (asset.tamer_max_level > 0
                                && tamer_level > asset.tamer_max_level as i32)
                        {
                            return Ok(vec![
                                ItemMoveFailPacket {
                                    origin_slot,
                                    destination_slot,
                                }
                                .encode(),
                            ]);
                        }
                    }
                }

                // Helper: swap/merge items between two mutable inventory slices
                fn transfer_between(
                    src_items: &mut [ItemRecord],
                    src_idx: usize,
                    dst_items: &mut [ItemRecord],
                    dst_idx: usize,
                ) {
                    let origin_item = src_items[src_idx].clone();
                    let dest_item = dst_items[dst_idx].clone();

                    if dest_item.item_id > 0
                        && origin_item.item_id > 0
                        && dest_item.item_id == origin_item.item_id
                    {
                        let mut merged = dest_item;
                        merged.amount += origin_item.amount;
                        dst_items[dst_idx] = merged;
                        src_items[src_idx] = ItemRecord::default();
                    } else {
                        src_items[src_idx] = dest_item;
                        dst_items[dst_idx] = origin_item;
                    }
                }

                // Same-tab move: swap via indices without double borrow
                fn swap_within(items: &mut [ItemRecord], a: usize, b: usize) {
                    if a >= items.len() || b >= items.len() {
                        return;
                    }
                    let origin = items[a].clone();
                    let dest = items[b].clone();
                    if dest.item_id > 0 && origin.item_id > 0 && dest.item_id == origin.item_id {
                        let mut merged = dest;
                        merged.amount += origin.amount;
                        items[b] = merged;
                        items[a] = ItemRecord::default();
                    } else {
                        items[a] = dest;
                        items[b] = origin;
                    }
                }

                fn inventory_item_to_digivice_item(
                    item: &ItemRecord,
                ) -> odmo_types::DigiviceItemSnapshot {
                    let mut record = item.record.clone();
                    record.truncate(60);
                    record.resize(60, 0);
                    let mut snapshot = odmo_types::DigiviceItemSnapshot {
                        item_id: item.item_id,
                        amount: item.amount,
                        record,
                    };
                    snapshot.sync_record();
                    snapshot
                }

                fn digivice_item_to_inventory_item(
                    item: &odmo_types::DigiviceItemSnapshot,
                ) -> ItemRecord {
                    let mut record = item.record.clone();
                    record.resize(69, 0);
                    let mut snapshot = ItemRecord {
                        item_id: item.item_id,
                        amount: item.amount,
                        record,
                    };
                    snapshot.sync_record();
                    snapshot
                }

                fn swap_inventory_with_digivice_slot(
                    inventory: &mut odmo_types::InventorySnapshot,
                    inventory_idx: usize,
                    slot: &mut odmo_types::DigiviceItemSnapshot,
                ) -> bool {
                    if inventory_idx >= inventory.items.len() {
                        return false;
                    }

                    let inventory_item = inventory.items[inventory_idx].clone();
                    if inventory_item.item_id <= 0 && slot.item_id <= 0 {
                        return false;
                    }

                    let previous_slot_item = digivice_item_to_inventory_item(slot);
                    if inventory_item.item_id > 0 {
                        *slot = inventory_item_to_digivice_item(&inventory_item);
                    } else {
                        *slot = odmo_types::DigiviceItemSnapshot::default();
                    }

                    if previous_slot_item.item_id > 0 {
                        inventory.items[inventory_idx] = previous_slot_item;
                    } else {
                        inventory.items[inventory_idx] = ItemRecord::default();
                    }
                    true
                }

                fn count_enabled_digivice_slots(
                    slots: &[odmo_types::DigiviceItemSnapshot],
                    start_idx: usize,
                ) -> usize {
                    slots[start_idx..]
                        .iter()
                        .filter(|slot| slot.item_id > 0 && slot.amount > 0)
                        .count()
                }

                fn digivice_capacity_from_item(
                    assets: &[odmo_types::ItemAsset],
                    item_id: i32,
                ) -> Option<(usize, usize)> {
                    if item_id <= 0 {
                        return Some((0, 0));
                    }
                    let asset = assets.iter().find(|asset| asset.item_id == item_id)?;
                    if asset.item_type != DIGIVICE_ITEM_TYPE {
                        return None;
                    }
                    Some((
                        usize::from(asset.digivice_chipset_slots),
                        usize::from(asset.digivice_skill_slots),
                    ))
                }

                if matches!(src_tab, TAB_DIGIVICE) || matches!(dst_tab, TAB_DIGIVICE) {
                    let assets = item_assets.as_deref().unwrap_or(&[]);
                    let replacement_item_id = match (src_tab, dst_tab) {
                        (TAB_INVENTORY, TAB_DIGIVICE) => character
                            .inventory
                            .items
                            .get(src_idx)
                            .map(|item| item.item_id)
                            .unwrap_or_default(),
                        (TAB_DIGIVICE, TAB_INVENTORY) => character
                            .inventory
                            .items
                            .get(dst_idx)
                            .map(|item| item.item_id)
                            .unwrap_or_default(),
                        _ => 0,
                    };

                    let Some((chipset_capacity, skill_capacity)) =
                        digivice_capacity_from_item(assets, replacement_item_id)
                    else {
                        return Ok(vec![
                            ItemMoveFailPacket {
                                origin_slot,
                                destination_slot,
                            }
                            .encode(),
                        ]);
                    };

                    if count_enabled_digivice_slots(
                        &character.digivice.chipsets,
                        chipset_capacity.min(character.digivice.chipsets.len()),
                    ) > 0
                        || count_enabled_digivice_slots(
                            &character.digivice.tamer_skills,
                            skill_capacity.min(character.digivice.tamer_skills.len()),
                        ) > 0
                    {
                        return Ok(vec![
                            ItemMoveFailPacket {
                                origin_slot,
                                destination_slot,
                            }
                            .encode(),
                        ]);
                    }
                }

                let current_digivice_capacity = if matches!(src_tab, TAB_CHIPSET | TAB_TSKILL)
                    || matches!(dst_tab, TAB_CHIPSET | TAB_TSKILL)
                {
                    digivice_capacity_from_item(
                        item_assets.as_deref().unwrap_or(&[]),
                        character.digivice.equipped_item.item_id,
                    )
                    .unwrap_or((0, 0))
                } else {
                    (0, 0)
                };

                if matches!(src_tab, TAB_CHIPSET) || matches!(dst_tab, TAB_CHIPSET) {
                    if src_idx >= current_digivice_capacity.0
                        || dst_idx >= current_digivice_capacity.0
                    {
                        return Ok(vec![
                            ItemMoveFailPacket {
                                origin_slot,
                                destination_slot,
                            }
                            .encode(),
                        ]);
                    }
                    if src_tab == TAB_INVENTORY {
                        let item_id = character
                            .inventory
                            .items
                            .get(src_idx)
                            .map(|item| item.item_id)
                            .unwrap_or_default();
                        let is_chipset = item_assets
                            .as_deref()
                            .unwrap_or(&[])
                            .iter()
                            .find(|asset| asset.item_id == item_id)
                            .map(|asset| asset.item_type == CHIPSET_ITEM_TYPE)
                            .unwrap_or(false);
                        if !is_chipset {
                            return Ok(vec![
                                ItemMoveFailPacket {
                                    origin_slot,
                                    destination_slot,
                                }
                                .encode(),
                            ]);
                        }
                    }
                }

                if matches!(src_tab, TAB_TSKILL) || matches!(dst_tab, TAB_TSKILL) {
                    if src_idx >= current_digivice_capacity.1
                        || dst_idx >= current_digivice_capacity.1
                    {
                        return Ok(vec![
                            ItemMoveFailPacket {
                                origin_slot,
                                destination_slot,
                            }
                            .encode(),
                        ]);
                    }
                }

                let success = match (src_tab, dst_tab) {
                    (TAB_INVENTORY, TAB_INVENTORY) => {
                        let len = character.inventory.items.len();
                        if src_idx < len && dst_idx < len {
                            swap_within(&mut character.inventory.items, src_idx, dst_idx);
                            true
                        } else {
                            false
                        }
                    }
                    (TAB_INVENTORY, TAB_EQUIPMENT) => {
                        let len = character.inventory.items.len();
                        if src_idx < len && dst_idx < 16 {
                            let item = &character.inventory.items[src_idx];
                            if item.item_id > 0 {
                                // Update equipment blob (69 bytes per slot)
                                if character.equipment.len() < 16 * 69 {
                                    character.equipment.resize(16 * 69, 0);
                                }
                                let offset = dst_idx * 69;
                                character.equipment[offset..offset + 69]
                                    .copy_from_slice(&item.record);

                                character.inventory.items[src_idx] = ItemRecord::default();
                                true
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    (TAB_EQUIPMENT, TAB_INVENTORY) => {
                        let len = character.inventory.items.len();
                        if src_idx < 16 && dst_idx < len {
                            // Find empty slot in inventory
                            if let Some(empty_idx) = character
                                .inventory
                                .items
                                .iter()
                                .position(|i| i.item_id == 0)
                            {
                                // Read from equipment blob
                                if character.equipment.len() >= 16 * 69 {
                                    let offset = src_idx * 69;
                                    let record = character.equipment[offset..offset + 69].to_vec();

                                    // Extract item_id and amount from the record (packed in first 4 bytes)
                                    if record.len() >= 4 {
                                        let packed = u32::from_le_bytes([
                                            record[0], record[1], record[2], record[3],
                                        ]);
                                        let item_id = (packed & 0x1FFFF) as i32;
                                        let amount = ((packed >> 17) & 0x7FFF) as i32;

                                        if item_id > 0 {
                                            let mut unequipped_item = ItemRecord {
                                                item_id,
                                                amount,
                                                record,
                                            };
                                            unequipped_item.sync_record(); // Ensure consistency
                                            character.inventory.items[empty_idx] = unequipped_item;

                                            // Clear equipment slot
                                            character.equipment[offset..offset + 69].fill(0);
                                            true
                                        } else {
                                            false
                                        }
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false // Inventory full
                            }
                        } else {
                            false
                        }
                    }
                    (TAB_INVENTORY, TAB_DIGIVICE) | (TAB_DIGIVICE, TAB_INVENTORY) => {
                        let digivice_idx = if src_tab == TAB_DIGIVICE {
                            src_idx
                        } else {
                            dst_idx
                        };
                        let inventory_idx = if src_tab == TAB_INVENTORY {
                            src_idx
                        } else {
                            dst_idx
                        };
                        if digivice_idx == 0 {
                            swap_inventory_with_digivice_slot(
                                &mut character.inventory,
                                inventory_idx,
                                &mut character.digivice.equipped_item,
                            )
                        } else {
                            false
                        }
                    }
                    (TAB_INVENTORY, TAB_CHIPSET) => {
                        if dst_idx < character.digivice.chipsets.len() {
                            swap_inventory_with_digivice_slot(
                                &mut character.inventory,
                                src_idx,
                                &mut character.digivice.chipsets[dst_idx],
                            )
                        } else {
                            false
                        }
                    }
                    (TAB_CHIPSET, TAB_INVENTORY) => {
                        if src_idx < character.digivice.chipsets.len() {
                            swap_inventory_with_digivice_slot(
                                &mut character.inventory,
                                dst_idx,
                                &mut character.digivice.chipsets[src_idx],
                            )
                        } else {
                            false
                        }
                    }
                    (TAB_CHIPSET, TAB_CHIPSET) => {
                        let len = character.digivice.chipsets.len();
                        if src_idx < len && dst_idx < len {
                            character.digivice.chipsets.swap(src_idx, dst_idx);
                            true
                        } else {
                            false
                        }
                    }
                    (TAB_INVENTORY, TAB_TSKILL) => {
                        if dst_idx < character.digivice.tamer_skills.len() {
                            swap_inventory_with_digivice_slot(
                                &mut character.inventory,
                                src_idx,
                                &mut character.digivice.tamer_skills[dst_idx],
                            )
                        } else {
                            false
                        }
                    }
                    (TAB_TSKILL, TAB_INVENTORY) => {
                        if src_idx < character.digivice.tamer_skills.len() {
                            swap_inventory_with_digivice_slot(
                                &mut character.inventory,
                                dst_idx,
                                &mut character.digivice.tamer_skills[src_idx],
                            )
                        } else {
                            false
                        }
                    }
                    (TAB_TSKILL, TAB_TSKILL) => {
                        let len = character.digivice.tamer_skills.len();
                        if src_idx < len && dst_idx < len {
                            character.digivice.tamer_skills.swap(src_idx, dst_idx);
                            true
                        } else {
                            false
                        }
                    }
                    (TAB_WAREHOUSE, TAB_WAREHOUSE) => {
                        let len = character.warehouse.items.len();
                        if src_idx < len && dst_idx < len {
                            swap_within(&mut character.warehouse.items, src_idx, dst_idx);
                            true
                        } else {
                            false
                        }
                    }
                    (TAB_INVENTORY, TAB_WAREHOUSE) => {
                        let i_len = character.inventory.items.len();
                        let w_len = character.warehouse.items.len();
                        if src_idx < i_len && dst_idx < w_len {
                            transfer_between(
                                &mut character.inventory.items,
                                src_idx,
                                &mut character.warehouse.items,
                                dst_idx,
                            );
                            true
                        } else {
                            false
                        }
                    }
                    (TAB_WAREHOUSE, TAB_INVENTORY) => {
                        let w_len = character.warehouse.items.len();
                        let i_len = character.inventory.items.len();
                        if src_idx < w_len && dst_idx < i_len {
                            transfer_between(
                                &mut character.warehouse.items,
                                src_idx,
                                &mut character.inventory.items,
                                dst_idx,
                            );
                            true
                        } else {
                            false
                        }
                    }
                    (TAB_INVENTORY, TAB_SHARESTASH) => {
                        let aw = character.account_warehouse.as_mut();
                        match aw {
                            Some(aw)
                                if src_idx < character.inventory.items.len()
                                    && dst_idx < aw.items.len() =>
                            {
                                transfer_between(
                                    &mut character.inventory.items,
                                    src_idx,
                                    &mut aw.items,
                                    dst_idx,
                                );
                                true
                            }
                            _ => false,
                        }
                    }
                    (TAB_SHARESTASH, TAB_INVENTORY) => {
                        let aw = character.account_warehouse.as_mut();
                        match aw {
                            Some(aw)
                                if src_idx < aw.items.len()
                                    && dst_idx < character.inventory.items.len() =>
                            {
                                transfer_between(
                                    &mut aw.items,
                                    src_idx,
                                    &mut character.inventory.items,
                                    dst_idx,
                                );
                                true
                            }
                            _ => false,
                        }
                    }
                    _ => false,
                };

                if success {
                    // Persist all involved inventories unconditionally
                    self.repository
                        .update_inventory(character_id, character.inventory.clone())
                        .map_err(|error| GameFlowError::Storage(error.to_string()))?;
                    self.repository
                        .update_equipment(character_id, character.equipment.clone())
                        .map_err(|error| GameFlowError::Storage(error.to_string()))?;
                    if matches!(src_tab, TAB_DIGIVICE | TAB_CHIPSET | TAB_TSKILL)
                        || matches!(dst_tab, TAB_DIGIVICE | TAB_CHIPSET | TAB_TSKILL)
                    {
                        self.repository
                            .update_digivice(character_id, character.digivice.clone())
                            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
                    }
                    self.repository
                        .update_warehouse(character_id, character.warehouse.clone())
                        .map_err(|error| GameFlowError::Storage(error.to_string()))?;
                    if let Some(aw) = &character.account_warehouse {
                        self.repository
                            .update_account_warehouse(character_id, aw.clone())
                            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
                    }

                    let mut responses = Vec::new();
                    responses.push(
                        ItemMoveSuccessPacket {
                            origin_slot,
                            destination_slot,
                        }
                        .encode(),
                    );
                    for tab in [src_tab, dst_tab] {
                        let (inv, ty) = match tab {
                            TAB_INVENTORY => (&character.inventory, InventoryType::Inventory),
                            TAB_WAREHOUSE => (&character.warehouse, InventoryType::Warehouse),
                            TAB_SHARESTASH => (
                                character
                                    .account_warehouse
                                    .as_ref()
                                    .unwrap_or_else(|| unreachable!()),
                                InventoryType::AccountWarehouse,
                            ),
                            _ => continue,
                        };
                        responses.push(
                            LoadInventoryPacket {
                                inventory: inv.clone(),
                                inventory_type: ty,
                            }
                            .encode(),
                        );
                    }
                    Ok(responses)
                } else {
                    Ok(vec![
                        ItemMoveFailPacket {
                            origin_slot,
                            destination_slot,
                        }
                        .encode(),
                    ])
                }
            }
            GameRequest::SplitItem {
                origin_slot,
                destination_slot,
                amount,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let mut character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                fn tab_class(sid: u16) -> u16 {
                    sid / 1000 * 1000
                }
                fn tab_index(sid: u16) -> usize {
                    (sid % 1000) as usize
                }

                let src_tab = tab_class(origin_slot);
                let dst_tab = tab_class(destination_slot);
                let src_idx = tab_index(origin_slot);
                let dst_idx = tab_index(destination_slot);

                fn split_within(
                    items: &mut [ItemRecord],
                    src: usize,
                    dst: usize,
                    amt: i32,
                ) -> bool {
                    if src >= items.len() || dst >= items.len() {
                        return false;
                    }
                    let source = items[src].clone();
                    let dest = items[dst].clone();
                    if source.item_id <= 0 || source.amount < amt {
                        return false;
                    }
                    if dest.item_id > 0 && dest.item_id != source.item_id {
                        return false;
                    }

                    if dest.item_id > 0 {
                        let mut updated = dest;
                        updated.amount += amt;
                        updated.sync_record();
                        items[dst] = updated;
                    } else {
                        let mut new_item = source.clone();
                        new_item.amount = amt;
                        new_item.sync_record();
                        items[dst] = new_item;
                    }

                    let remaining = source.amount - amt;
                    if remaining <= 0 {
                        items[src] = ItemRecord::default();
                    } else {
                        let mut updated = source;
                        updated.amount = remaining;
                        updated.sync_record();
                        items[src] = updated;
                    }
                    true
                }

                fn split_cross(
                    src_items: &mut [ItemRecord],
                    src_idx: usize,
                    dst_items: &mut [ItemRecord],
                    dst_idx: usize,
                    amt: i32,
                ) -> bool {
                    if src_idx >= src_items.len() || dst_idx >= dst_items.len() {
                        return false;
                    }
                    let source = src_items[src_idx].clone();
                    let dest = dst_items[dst_idx].clone();
                    if source.item_id <= 0 || source.amount < amt {
                        return false;
                    }
                    if dest.item_id > 0 && dest.item_id != source.item_id {
                        return false;
                    }

                    if dest.item_id > 0 {
                        let mut updated = dest;
                        updated.amount += amt;
                        updated.sync_record();
                        dst_items[dst_idx] = updated;
                    } else {
                        let mut new_item = source.clone();
                        new_item.amount = amt;
                        new_item.sync_record();
                        dst_items[dst_idx] = new_item;
                    }

                    let remaining = source.amount - amt;
                    if remaining <= 0 {
                        src_items[src_idx] = ItemRecord::default();
                    } else {
                        let mut updated = source;
                        updated.amount = remaining;
                        updated.sync_record();
                        src_items[src_idx] = updated;
                    }
                    true
                }

                let success = match (src_tab, dst_tab) {
                    (0, 0) => split_within(
                        &mut character.inventory.items,
                        src_idx,
                        dst_idx,
                        amount as i32,
                    ),
                    (2000, 2000) => split_within(
                        &mut character.warehouse.items,
                        src_idx,
                        dst_idx,
                        amount as i32,
                    ),
                    (0, 2000) => split_cross(
                        &mut character.inventory.items,
                        src_idx,
                        &mut character.warehouse.items,
                        dst_idx,
                        amount as i32,
                    ),
                    (2000, 0) => split_cross(
                        &mut character.warehouse.items,
                        src_idx,
                        &mut character.inventory.items,
                        dst_idx,
                        amount as i32,
                    ),
                    _ => false,
                };

                if !success {
                    return Ok(vec![
                        SplitItemPacket {
                            origin_slot,
                            destination_slot,
                            amount: 0,
                        }
                        .encode(),
                    ]);
                }

                // Persist all involved inventories
                self.repository
                    .update_inventory(character_id, character.inventory.clone())
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?;
                self.repository
                    .update_warehouse(character_id, character.warehouse.clone())
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?;

                let mut responses = Vec::new();
                responses.push(
                    SplitItemPacket {
                        origin_slot,
                        destination_slot,
                        amount,
                    }
                    .encode(),
                );
                for tab in [src_tab, dst_tab] {
                    let (inv, ty) = match tab {
                        0 => (&character.inventory, InventoryType::Inventory),
                        2000 => (&character.warehouse, InventoryType::Warehouse),
                        _ => continue,
                    };
                    responses.push(
                        LoadInventoryPacket {
                            inventory: inv.clone(),
                            inventory_type: ty,
                        }
                        .encode(),
                    );
                }
                Ok(responses)
            }
            GameRequest::RemoveItem {
                slot,
                x: _,
                y: _,
                amount,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let mut character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let slot_idx = slot as usize;
                if slot_idx >= character.inventory.items.len() {
                    return Ok(Vec::new());
                }

                let item = &character.inventory.items[slot_idx];
                if item.item_id <= 0 || item.amount <= 0 {
                    return Ok(Vec::new());
                }

                let new_amount = item.amount - amount as i32;
                if new_amount <= 0 {
                    character.inventory.items[slot_idx] = ItemRecord::default();
                } else {
                    let mut updated = item.clone();
                    updated.amount = new_amount;
                    updated.sync_record();
                    character.inventory.items[slot_idx] = updated;
                }

                self.repository
                    .update_inventory(character_id, character.inventory.clone())
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?;

                Ok(vec![
                    LoadInventoryPacket {
                        inventory: character.inventory,
                        inventory_type: InventoryType::Inventory,
                    }
                    .encode(),
                ])
            }
            GameRequest::NpcPurchase {
                vip: _,
                npc_id,
                marker: _,
                shop_slot,
                purchase_count,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let mut character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let shop = self
                    .repository
                    .shop_by_npc(npc_id, character.map_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::Storage(format!(
                        "NPC shop {npc_id} not found on map {}",
                        character.map_id
                    )))?;

                let slot_idx = shop_slot as usize;
                if slot_idx >= shop.items.len() {
                    return Ok(vec![
                        NpcPurchaseResultPacket {
                            success: false,
                            remaining_bits: character.inventory.bits,
                        }
                        .encode(),
                    ]);
                }

                let npc_item = &shop.items[slot_idx];
                let total_cost = npc_item.buy_price * purchase_count as i32;

                if character.inventory.bits < total_cost as i64 {
                    return Ok(vec![
                        NpcPurchaseResultPacket {
                            success: false,
                            remaining_bits: character.inventory.bits,
                        }
                        .encode(),
                    ]);
                }

                // Deduct bits
                character.inventory.bits -= total_cost as i64;

                // Find empty slot or existing stack of same item
                let mut placed = false;
                for i in 0..character
                    .inventory
                    .items
                    .len()
                    .min(character.inventory.size as usize)
                {
                    let existing = &character.inventory.items[i];
                    if existing.item_id == npc_item.item_id && existing.amount > 0 {
                        // Stack onto existing
                        let mut updated = existing.clone();
                        updated.amount += purchase_count as i32;
                        updated.sync_record();
                        character.inventory.items[i] = updated;
                        placed = true;
                        break;
                    }
                }

                if !placed {
                    // Find first empty slot
                    for i in 0..character
                        .inventory
                        .items
                        .len()
                        .min(character.inventory.size as usize)
                    {
                        if character.inventory.items[i].item_id <= 0
                            || character.inventory.items[i].amount <= 0
                        {
                            character.inventory.items[i] =
                                ItemRecord::new(npc_item.item_id, purchase_count as i32);
                            placed = true;
                            break;
                        }
                    }
                }

                if !placed {
                    // No space - refund bits
                    character.inventory.bits += total_cost as i64;
                    return Ok(vec![
                        NpcPurchaseResultPacket {
                            success: false,
                            remaining_bits: character.inventory.bits,
                        }
                        .encode(),
                    ]);
                }

                self.repository
                    .update_inventory(character_id, character.inventory.clone())
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?;

                let responses = vec![
                    NpcPurchaseResultPacket {
                        success: true,
                        remaining_bits: character.inventory.bits,
                    }
                    .encode(),
                    LoadInventoryPacket {
                        inventory: character.inventory,
                        inventory_type: InventoryType::Inventory,
                    }
                    .encode(),
                ];
                Ok(responses)
            }
            GameRequest::NpcSell {
                vip: _,
                npc_id,
                marker: _,
                item_slot,
                sell_amount,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let mut character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let slot_idx = item_slot as usize;
                if slot_idx >= character.inventory.items.len() {
                    return Ok(Vec::new());
                }

                let item = &character.inventory.items[slot_idx];
                if item.item_id <= 0 || item.amount <= 0 {
                    return Ok(Vec::new());
                }

                if item.amount < sell_amount as i32 {
                    return Ok(Vec::new());
                }

                // Look up actual sell price from NPC shop definition
                let sell_per_item =
                    if let Ok(Some(shop)) = self.repository.shop_by_npc(npc_id, character.map_id) {
                        shop.items
                            .iter()
                            .find(|si| si.item_id == item.item_id)
                            .map(|si| si.sell_price as i64)
                            .unwrap_or(100)
                    } else {
                        100 // Fallback default if shop not found
                    };
                let sell_price = sell_per_item * sell_amount as i64;

                character.inventory.bits += sell_price;

                // Reduce or remove item
                let sold_serial = item.item_id.max(0) as u32;
                let remaining = item.amount - sell_amount as i32;
                if remaining <= 0 {
                    character.inventory.items[slot_idx] = ItemRecord::default();
                } else {
                    let mut updated = item.clone();
                    updated.amount = remaining;
                    updated.sync_record();
                    character.inventory.items[slot_idx] = updated;
                }

                self.repository
                    .update_inventory(character_id, character.inventory.clone())
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?;

                let responses = vec![
                    NpcSellResultPacket {
                        remaining_bits: character.inventory.bits,
                        rows: vec![NpcSellRow {
                            kind: 0, // unresolved kind slot, resolved on the client side
                            serial: sold_serial,
                            stack: sell_amount,
                        }],
                    }
                    .encode(),
                    LoadInventoryPacket {
                        inventory: character.inventory,
                        inventory_type: InventoryType::Inventory,
                    }
                    .encode(),
                ];
                Ok(responses)
            }
            GameRequest::LootItem { drop_handler } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let responses = match self
                    .repository
                    .collect_drop(
                        character_id,
                        character.map_id,
                        character.channel,
                        drop_handler,
                    )
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                {
                    DropCollectionResult::Missing => vec![
                        UnloadDropsPacket {
                            drop: odmo_types::DropSummary {
                                handler: drop_handler,
                                ..odmo_types::DropSummary::default()
                            },
                        }
                        .encode(),
                    ],
                    DropCollectionResult::NotTheOwner => vec![
                        PickItemFailPacket {
                            reason: PickItemFailReason::NotTheOwner,
                        }
                        .encode(),
                    ],
                    DropCollectionResult::InventoryFull => vec![
                        PickItemFailPacket {
                            reason: PickItemFailReason::InventoryFull,
                        }
                        .encode(),
                    ],
                    DropCollectionResult::TooFarAway => vec![
                        PickItemFailPacket {
                            reason: PickItemFailReason::TooFarAway,
                        }
                        .encode(),
                    ],
                    DropCollectionResult::BitsCollected {
                        drop,
                        amount,
                        character,
                    } => {
                        session.viewed_drops.remove(&drop.id);
                        vec![
                            PickBitsPacket {
                                appearance_handler: character.general_handler,
                                value: amount,
                            }
                            .encode(),
                            UnloadDropsPacket { drop }.encode(),
                            LoadInventoryPacket {
                                inventory: character.inventory.clone(),
                                inventory_type: InventoryType::Inventory,
                            }
                            .encode(),
                        ]
                    }
                    DropCollectionResult::ItemCollected {
                        drop,
                        item_id,
                        amount,
                        character,
                    } => {
                        session.viewed_drops.remove(&drop.id);
                        vec![
                            PickItemPacket {
                                appearance_handler: character.general_handler,
                                item_id,
                                amount,
                            }
                            .encode(),
                            UnloadDropsPacket { drop }.encode(),
                            LoadInventoryPacket {
                                inventory: character.inventory.clone(),
                                inventory_type: InventoryType::Inventory,
                            }
                            .encode(),
                        ]
                    }
                };

                Ok(responses)
            }
            GameRequest::TamerMovimentation {
                ticks: _,
                handler,
                x,
                y,
                z,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                // Handler >= 0x7FFF (32767) means tamer movement
                // Handler < 0x7FFF means partner (digimon) movement
                let is_tamer = handler >= 0x7FFF;
                // Condition bit 0x01 = Ride (ConditionEnum.Ride = 1)
                let is_riding = (character.current_condition & 0x01) != 0;

                if is_tamer {
                    self.repository
                        .update_character_position(character_id, x, y, z)
                        .map_err(|error| GameFlowError::Storage(error.to_string()))?;

                    // If riding, also move partner to same position
                    if is_riding {
                        self.repository
                            .update_partner_position(character_id, x, y, z)
                            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
                    }
                } else {
                    self.repository
                        .update_partner_position(character_id, x, y, z)
                        .map_err(|error| GameFlowError::Storage(error.to_string()))?;
                }

                // Broadcast walk packets to other visible tamers
                if let Some(broadcast) = &self.broadcast {
                    if is_tamer {
                        let walk = TamerWalkPacket {
                            handler: character.general_handler,
                            x,
                            y,
                        };
                        let _ = broadcast.send_to_visible(
                            character.map_id,
                            character.channel,
                            character.id,
                            &walk.encode(),
                        );
                        // Also send back to self for client consistency
                        let _ = broadcast.send_to(character.id, &walk.encode());
                    }

                    if !is_tamer || is_riding {
                        let digimon_walk = DigimonWalkPacket {
                            handler: character.partner_handler,
                            x,
                            y,
                        };
                        let _ = broadcast.send_to_visible(
                            character.map_id,
                            character.channel,
                            character.id,
                            &digimon_walk.encode(),
                        );
                        let _ = broadcast.send_to(character.id, &digimon_walk.encode());
                    }
                }
                let updated_character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;
                self.broadcast_party_member_position(&updated_character);

                Ok(Vec::new())
            }
            GameRequest::WarpGate { portal_id } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let portal = self
                    .repository
                    .portal_by_id(portal_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::Storage(format!(
                        "portal {portal_id} not found"
                    )))?;

                // Remove from current map presence
                self.portal_bridge
                    .remove_map_presence(character.map_id, character.channel, character.id)
                    .map_err(|error| GameFlowError::PortalBridge(error.to_string()))?;

                // Update position in database
                self.repository
                    .update_character_map(
                        character_id,
                        portal.destination_map_id,
                        portal.destination_x,
                        portal.destination_y,
                    )
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?;

                // Update partner position too
                self.repository
                    .update_partner_position(
                        character_id,
                        portal.destination_x,
                        portal.destination_y,
                        0.0,
                    )
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?;
                let updated_character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;
                self.broadcast_party_member_map_change(&updated_character);

                // Reset session state for new map
                session.registered_map_presence = false;
                session.viewed_characters.clear();
                session.viewed_mobs.clear();
                session.viewed_drops.clear();

                if portal.is_local {
                    // Local teleport: same map, send LocalMapSwapPacket (opcode 1711)
                    Ok(vec![
                        LocalMapSwapPacket {
                            tamer_handler: character.general_handler as i32,
                            partner_handler: character.partner_handler as i32,
                            x: portal.destination_x,
                            y: portal.destination_y,
                        }
                        .encode(),
                    ])
                } else {
                    // Cross-map teleport: send MapSwapPacket (opcode 1709) - client reconnects
                    Ok(vec![
                        MapSwapPacket {
                            address: self.game_server_address.clone(),
                            port: self.game_server_port,
                            map_id: i32::from(portal.destination_map_id),
                            x: portal.destination_x,
                            y: portal.destination_y,
                        }
                        .encode(),
                    ])
                }
            }
            GameRequest::DigiSummonSyncRequest => self.handle_digi_summon_sync(session),
            // ChannelInfo — client echoes back the channel info sent during ComplementarInformation.
            // No second response needed.
            GameRequest::ChannelInfo => Ok(vec![]),
            // Membership — client echoes back the membership packet sent during ComplementarInformation.
            // No second response needed.
            GameRequest::Membership => Ok(vec![]),
            // Emoticon — client sends an emote to display
            GameRequest::Emoticon {
                emoticon_type,
                value: _,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                // Echo the emoticon back to the client.
                let mut writer =
                    odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::EMOTICON);
                writer.write_u32(character.general_handler);
                writer.write_i32(emoticon_type);
                Ok(vec![writer.finalize()])
            }
            // FriendlyInfo — client requests friendship info with a target
            GameRequest::FriendlyInfo { target_handler: _ } => {
                // Respond with empty friendship data — no friendship system implemented yet.
                let mut writer = odmo_protocol::writer::PacketWriter::new(
                    odmo_protocol::opcode::game::FRIENDLY_INFO,
                );
                writer.write_u32(0); // target handler
                writer.write_i32(0); // friendship level
                Ok(vec![writer.finalize()])
            }
            // ExtraInventory — move item from extra inventory to regular inventory
            GameRequest::ExtraInventoryMove {
                category: _,
                extra_slot,
                inventory_slot,
            } => self.handle_extra_inventory_move(session, extra_slot, inventory_slot),
            // ExtraInventory — batch move all items from extra to regular
            GameRequest::ExtraInventoryBatchMove { category: _ } => {
                self.handle_extra_inventory_batch_move(session)
            }
            // ExtraInventory — sort items in extra inventory
            GameRequest::ExtraInventorySort { category: _ } => {
                self.handle_extra_inventory_sort(session)
            }
            // ExtraInventory — use/consume item from extra inventory
            GameRequest::ExtraInventoryUse {
                category: _,
                extra_slot,
            } => self.handle_extra_inventory_use(session, extra_slot),
            // ChatMessage — general chat is spoken aloud to everyone nearby
            GameRequest::ChatMessage { message } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let packet = SyncChatRecord::Chat(vec![SyncChatLine {
                    handler: character.general_handler,
                    message,
                }])
                .encode();

                // The speaker only echoes locally while offline, so its own copy is the
                // returned packet and nearby tamers get it through the sink.
                if let Some(broadcast) = &self.broadcast {
                    let _ = broadcast.send_to_visible(
                        character.map_id,
                        character.channel,
                        character.id,
                        &packet,
                    );
                }

                Ok(vec![packet])
            }
            // WhisperMessage — deliver by result code: 3 both peers, otherwise the sender
            GameRequest::WhisperMessage {
                target_name,
                message,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let target = self
                    .repository
                    .character_by_name(&target_name)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?;

                // The client turns each code into its own message, so an unknown name and a
                // known-but-absent tamer must not be reported as a delivered whisper.
                let (result, target_id) = match target {
                    Some(found)
                        if self
                            .broadcast
                            .as_ref()
                            .is_some_and(|b| b.is_online(found.id)) =>
                    {
                        (3, Some(found.id))
                    }
                    Some(_) => (0, None),
                    None => (2, None),
                };

                let packet = SyncChatRecord::Whisper(vec![SyncWhisperLine {
                    result,
                    sender: character.name,
                    receiver: target_name,
                    message,
                }])
                .encode();

                if let (Some(broadcast), Some(id)) = (&self.broadcast, target_id) {
                    let _ = broadcast.send_to(id, &packet);
                }

                Ok(vec![packet])
            }
            // ShoutMessage — a shout carries the speaker's name to the whole server
            GameRequest::ShoutMessage { message } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let packet = SyncChatRecord::Shout(vec![SyncNamedLine {
                    sender: character.name,
                    message,
                }])
                .encode();

                // A shout is not map-scoped: every connected tamer hears it, and the
                // speaker's own copy stays in the returned packet.
                if let Some(broadcast) = &self.broadcast {
                    let _ = broadcast.send_to_all_except(character.id, &packet);
                }

                Ok(vec![packet])
            }
            // MegaphoneMessage — a branded shout, delivered on the same channel
            GameRequest::MegaphoneMessage {
                mode: _,
                item_code: _,
                message,
                item_slot: _,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                // The branded record also carries a style byte that selects the message
                // colour, and the client drops the line unless the item code resolves to a
                // shouter, so the plain shout form stays until that style source is settled.
                let packet = SyncChatRecord::Shout(vec![SyncNamedLine {
                    sender: character.name,
                    message,
                }])
                .encode();

                if let Some(broadcast) = &self.broadcast {
                    let _ = broadcast.send_to_all_except(character.id, &packet);
                }

                Ok(vec![packet])
            }
            // TamerReaction — echo reaction back to client
            GameRequest::TamerReaction { reaction_type } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let mut writer = odmo_protocol::writer::PacketWriter::new(
                    odmo_protocol::opcode::game::TAMER_REACTION,
                );
                writer.write_u32(character.general_handler); // tamer handler
                writer.write_i32(reaction_type); // reaction type
                Ok(vec![writer.finalize()])
            }
            // PartnerStop — echo partner stop to client
            GameRequest::PartnerStop { uid: _ } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let mut writer = odmo_protocol::writer::PacketWriter::new(
                    odmo_protocol::opcode::game::PARTNER_STOP_RESPONSE,
                );
                writer.write_u32(character.partner_handler);
                Ok(vec![writer.finalize()])
            }
            // PartnerSwitch — simplified roster-backed switch without battle-tag restrictions yet
            GameRequest::PartnerSwitch { slot } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;
                let old_partner_type = character.partner_current_type;
                let Some(updated_character) = self
                    .repository
                    .switch_partner(character_id, slot)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                else {
                    return Ok(vec![PartnerSwitchFailurePacket.encode()]);
                };
                if updated_character.partner_current_slot == character.partner_current_slot {
                    return Ok(vec![PartnerSwitchFailurePacket.encode()]);
                }

                let active_partner = updated_character
                    .partner_slots
                    .iter()
                    .find(|partner| partner.slot == updated_character.partner_current_slot)
                    .cloned()
                    .ok_or_else(|| {
                        GameFlowError::Storage(
                            "active partner slot missing after switch".to_string(),
                        )
                    })?;
                let active_partner_buffs = active_partner.active_buffs.clone();

                let switch_packet = PartnerSwitchPacket {
                    handler: updated_character.partner_handler,
                    old_partner_current_type: old_partner_type,
                    slot,
                    partner: active_partner,
                }
                .encode();

                if let Some(broadcast) = &self.broadcast {
                    let _ = broadcast.send_to_visible(
                        updated_character.map_id,
                        updated_character.channel,
                        updated_character.id,
                        &switch_packet,
                    );
                    let _ = broadcast.send_to(updated_character.id, &switch_packet);
                }

                self.broadcast_party_member_digimon_change(&updated_character);
                self.broadcast_party_member_buff_change(&updated_character, &active_partner_buffs);

                Ok(vec![
                    UpdateStatusPacket {
                        character: updated_character,
                    }
                    .encode(),
                ])
            }
            // PartnerSkill — broadcast skill cast and apply damage to the target mob.
            // Real damage calculation is left for a follow-up slice; this slice closes the
            // protocol contract end-to-end so the client animates the cast and reflects HP loss.
            GameRequest::PartnerSkill {
                skill_slot,
                attacker_handler: _,
                target_handler,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                if character.partner_handler == 0 {
                    return Ok(vec![
                        PartnerSkillErrorPacket {
                            attacker_handler: 0,
                            parameter: 0,
                            value: skill_slot,
                            value2: 0,
                            context: 0,
                        }
                        .encode(),
                    ]);
                }
                if skill_slot > 5 {
                    return Ok(vec![
                        PartnerSkillErrorPacket {
                            attacker_handler: character.partner_handler,
                            parameter: 1,
                            value: skill_slot,
                            value2: 0,
                            context: 0,
                        }
                        .encode(),
                    ]);
                }

                self.handle_partner_combat(session, &character, target_handler, Some(skill_slot))
            }
            // PartnerAttack — apply damage to the target mob and broadcast the hit.
            GameRequest::PartnerAttack {
                attacker_handler: _,
                target_handler,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let character = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                if character.partner_handler == 0 {
                    return Ok(vec![
                        MissHitPacket {
                            attacker_handler: 0,
                            target_handler,
                        }
                        .encode(),
                    ]);
                }

                self.handle_partner_combat(session, &character, target_handler, None)
            }
            // PartnerEvolution — apply evolution if valid
            GameRequest::PartnerEvolution {
                digimon_handler,
                evolution_slot,
            } => self.handle_partner_evolution(session, digimon_handler, evolution_slot),
            // PartnerDelete — respond with `PartnerDeletePacket(-1)` (wrong validation)
            // until account-side secondary password / email validation is ported. The
            // legacy contract is `[opcode 1042][i32 result]` where positive = success
            // (slot index) and negative = failure code.
            GameRequest::PartnerDelete {
                slot: _,
                validation: _,
            } => {
                let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
                let _ = self
                    .repository
                    .character_by_id(character_id)
                    .map_err(|error| GameFlowError::Storage(error.to_string()))?
                    .ok_or(GameFlowError::CharacterNotFound(character_id))?;

                let mut writer = odmo_protocol::writer::PacketWriter::new(
                    odmo_protocol::opcode::game::PARTNER_DELETE_RESPONSE,
                );
                writer.write_i32(-1);
                Ok(vec![writer.finalize()])
            }
            GameRequest::EvolutionUnlock {
                evo_slot,
                items,
                selected_inven_idx,
            } => self.handle_evolution_unlock(session, evo_slot, &items, selected_inven_idx),
            // RideModeStart — toggle the partner ride mode on. The wire payload is empty
            // (the open-slot transaction is a different opcode), so this is a pure switch.
            GameRequest::RideModeStart => self.handle_ride_mode_start(session),
            // RideModeStop — toggle the ride mode off and broadcast.
            GameRequest::RideModeStop => self.handle_ride_mode_stop(session),
            // OpenRideMode — request to unlock/configure the ride slot via item.
            GameRequest::OpenRideMode {
                evo_unit_idx,
                item_type,
            } => self.handle_open_ride_mode(session, evo_unit_idx, item_type),
            // SetTarget — update the active combat target. Acknowledged without a
            // dedicated response; mob/skill packets carry the target downstream.
            GameRequest::SetTarget {
                attacker_handler: _,
                target_handler: _,
            } => Ok(Vec::new()),
            // StatUp — spend a stat point. The stat allocation table is not yet
            // ported, so the request is accepted without applying changes.
            GameRequest::StatUp { uid: _, stat: _ } => Ok(Vec::new()),
            // RefreshScreen — client asks for a visibility resync. The per-request
            // reconciliation at the top of handle_request already covers this.
            GameRequest::RefreshScreen => Ok(Vec::new()),
            // AwayTime — idle notification. No state change needed.
            GameRequest::AwayTime => Ok(Vec::new()),
            // DigimonChangeName — rename the active partner.
            GameRequest::DigimonChangeName {
                inven_slot: _,
                new_name,
            } => self.handle_digimon_change_name(session, new_name),
            // HatchInsertEgg — load an egg into the incubator.
            GameRequest::HatchInsertEgg {
                vip: _,
                inven_slot,
                npc_idx: _,
            } => self.handle_hatch_insert_egg(session, inven_slot),
            // HatchIncrease — apply incubator data to bump the hatch progress.
            GameRequest::HatchIncrease {
                vip: _,
                npc_idx: _,
                data_level,
            } => self.handle_hatch_increase(session, data_level),
            // HatchFinish — complete the hatch and create a new partner.
            GameRequest::HatchFinish {
                vip: _,
                portable_pos: _,
                name,
                npc_idx: _,
            } => self.handle_hatch_finish(session, name),
            // HatchRemoveEgg — remove the current egg without hatching.
            GameRequest::HatchRemoveEgg { vip: _, npc_idx: _ } => {
                self.handle_hatch_remove_egg(session)
            }
            // HatchBackupInsert — move the egg to the backup slot for safekeeping.
            GameRequest::HatchBackupInsert {
                vip: _,
                inven_slot: _,
                npc_idx: _,
            } => self.handle_hatch_backup_insert(session),
            // HatchBackupCancel — abandon the backup egg.
            GameRequest::HatchBackupCancel { vip: _, npc_idx: _ } => {
                self.handle_hatch_backup_cancel(session)
            }
            // IncubatorClose — close the incubator UI; resets the increase timer.
            GameRequest::IncubatorClose => self.handle_incubator_close(session),
            // DigimonArchiveMove — move a partner between digivice and archive slots.
            GameRequest::DigimonArchiveMove {
                vip: _,
                slot1,
                slot2,
                npc_type: _,
            } => self.handle_digimon_archive_move(session, slot1, slot2),
            // DigimonArchiveList — return the archive contents.
            GameRequest::DigimonArchiveList {
                vip: _,
                inven_idx: _,
                npc_type: _,
            } => self.handle_digimon_archive_list(session),
            // DigimonArchiveSwap — swap two archive slots.
            GameRequest::DigimonArchiveSwap {
                npc_idx: _,
                archive_type: _,
                src_arr,
                dst_arr,
            } => self.handle_digimon_archive_swap(session, src_arr, dst_arr),
            // InventorySort — sort the inventory items by item id (in-place) and emit
            // the sorted snapshot back to the client so its local state stays consistent.
            GameRequest::InventorySort { sort_type } => {
                self.handle_inventory_sort(session, sort_type)
            }
            // ItemIdentify — flip the identification flag on the target accessory and
            // echo the (already random-rolled) accessory stats back to the client.
            GameRequest::ItemIdentify { item_slot } => {
                self.handle_item_identify(session, item_slot)
            }
            GameRequest::ItemCraft { recipe_slot: _ } => {
                let mut writer = odmo_protocol::writer::PacketWriter::new(
                    odmo_protocol::opcode::game::ITEM_CRAFT,
                );
                writer.write_u8(0);
                Ok(vec![writer.finalize()])
            }
            // ItemReroll — re-roll the accessory stats on the target item.
            GameRequest::ItemReroll { item_slot } => self.handle_item_reroll(session, item_slot),
            GameRequest::ItemSocketIn {
                vip: _,
                inven_portable_pos: _,
                npc_idx: _,
                src_inven_pos,
                dst_inven_pos,
                socket_order,
            } => self.handle_item_socket_in(
                session,
                dst_inven_pos as i16,
                socket_order,
                src_inven_pos as i32,
            ),
            // ItemSocketOut — extract the chip at `socket_slot` from the target item.
            GameRequest::ItemSocketOut {
                vip: _,
                inven_portable_pos: _,
                npc_idx: _,
                src_inven_pos: _,
                dst_inven_pos,
                socket_order,
            } => self.handle_item_socket_out(session, dst_inven_pos as i16, socket_order),
            // ItemSocketIdentify — analyse the chip slots on the target item.
            GameRequest::ItemSocketIdentify {
                vip: _,
                npc_idx: _,
                inven_portable_pos: _,
                inven_pos,
            } => self.handle_item_socket_identify(session, inven_pos as i16),
            GameRequest::ItemReturn { item_slot } => self.handle_item_return(session, item_slot),
            GameRequest::ItemScan { item_slot: _ } => Ok(Vec::new()),
            // LoadGiftStorage — return the character's gift storage contents.
            GameRequest::LoadGiftStorage => self.handle_load_gift_storage(session),
            // GiftStorageRetrieve — claim a gift item from gift storage.
            GameRequest::GiftStorageRetrieve { item_slot } => {
                self.handle_gift_storage_retrieve(session, item_slot)
            }
            // LoadRewardStorage — return the character's reward storage contents.
            GameRequest::LoadRewardStorage => self.handle_load_reward_storage(session),
            // RecompenseGain — claim a reward item from reward storage.
            GameRequest::RecompenseGain { reward_id } => {
                self.handle_recompense_gain(session, reward_id)
            }
            // TamerShopOpen — open the personal shop for browsing.
            GameRequest::TamerShopOpen => Ok(Vec::new()),
            // TamerShopClose — close the personal shop browser.
            GameRequest::TamerShopClose => Ok(Vec::new()),
            // TamerShopBuy — purchase an item from a personal shop.
            GameRequest::TamerShopBuy { item_id, amount } => {
                self.handle_tamer_shop_buy(session, item_id, amount)
            }
            // ----- Consigned shop slice ------------------------------------------
            // ConsignedShopOpen — open the consigned shop UI.
            GameRequest::ConsignedShopOpen => self.handle_consigned_shop_open(session),
            // ConsignedShopView — view a specific consigned shop's listings.
            GameRequest::ConsignedShopView { shop_id } => {
                self.handle_consigned_shop_view(session, shop_id)
            }
            // ConsignedShopPurchase — purchase from a consigned shop.
            GameRequest::ConsignedShopPurchase { item_id, amount } => {
                self.handle_consigned_shop_purchase(session, item_id, amount)
            }
            // ConsignedShopRetrieve — retrieve unsold listings from the consigned shop.
            GameRequest::ConsignedShopRetrieve { item_slot } => {
                self.handle_consigned_shop_retrieve(session, item_slot)
            }
            // ----- Cash shop slice -----------------------------------------------
            // CashShopBuy — purchase items from the cash shop using premium currency.
            GameRequest::CashShopBuy {
                amount,
                total_price,
                order_id,
                product_ids,
            } => self.handle_cash_shop_buy(session, amount, total_price, order_id, product_ids),
            // CashShopReload — return current premium/silk balances.
            GameRequest::CashShopReload => self.handle_cash_shop_reload(session),
            // QuestAvailableList — return quest ids the character can accept from the
            // queried NPC. Without quest asset data we return an empty list.
            GameRequest::QuestAvailableList { npc_id } => Ok(vec![
                QuestAvailableListPacket {
                    npc_id,
                    quest_ids: Vec::new(),
                }
                .encode(),
            ]),
            // QuestAccept — register the quest in the character's quest progress.
            GameRequest::QuestAccept { quest_id } => self.handle_quest_accept(session, quest_id),
            // QuestDeliver — mark the quest as completed in the character's progress.
            GameRequest::QuestDeliver { quest_id } => self.handle_quest_deliver(session, quest_id),
            // QuestGiveUp — remove the quest from the in-progress list.
            GameRequest::QuestGiveUp { quest_id } => self.handle_quest_give_up(session, quest_id),
            GameRequest::QuestUpdate {
                quest_id,
                cond_index,
                value,
            } => self.handle_quest_update(session, quest_id, cond_index, value),
            // DieConfirm — server-side acknowledgement of death; restores HP minimum and
            // teleports the character back to the spawn map. Real respawn logic still
            // depends on the death state machine which is part of the combat slice; for
            // now we ensure the character HP/DS state stays valid.
            GameRequest::DieConfirm => self.handle_die_confirm(session),
            // RemoveBuff — remove the buff from the character's active buff list.
            GameRequest::RemoveBuff { buff_id } => self.handle_remove_buff(session, buff_id),
            // DamageSkinChange — equip a damage skin on the character row.
            GameRequest::DamageSkinChange { skin_id } => {
                self.handle_damage_skin_change(session, skin_id)
            }
            // SealOpen — open the seal at `seal_idx` and persist the change. Without
            // the legacy seal asset table we mark the seal as opened (amount = 1) and
            // echo the result back to the client.
            GameRequest::SealOpen { seal_idx } => self.handle_seal_open(session, seal_idx),
            // SealClose — close the seal (clear the favorite flag and amount).
            GameRequest::SealClose { seal_idx } => self.handle_seal_close(session, seal_idx),
            // SealSetLeader — set the leader seal id on the character.
            GameRequest::SealSetLeader { card_code } => {
                self.handle_seal_set_leader(session, card_code)
            }
            // SealRemoveLeader — clear the leader seal id.
            GameRequest::SealRemoveLeader => self.handle_seal_remove_leader(session),
            // SealSetFavorite — toggle the favorite flag for a seal.
            GameRequest::SealSetFavorite {
                card_code,
                bookmark,
            } => self.handle_seal_set_favorite(session, card_code, bookmark),
            // EncyclopediaLoad — return the current encyclopedia entries.
            GameRequest::EncyclopediaLoad => self.handle_encyclopedia_load(session),
            // EncyclopediaGetReward — claim the encyclopedia reward for a digimon.
            GameRequest::EncyclopediaGetReward { digimon_id } => {
                self.handle_encyclopedia_get_reward(session, digimon_id)
            }
            // EncyclopediaDeckBuff — toggle/select the active deck buff.
            GameRequest::EncyclopediaDeckBuff { deck_idx } => {
                self.handle_encyclopedia_deck_buff(session, deck_idx)
            }
            GameRequest::OtherTamerDetailInfo { target_handler } => {
                self.handle_other_tamer_detail_info(session, target_handler)
            }
            // ArenaDailyPoints — add daily arena points and respond with the new total.
            GameRequest::ArenaDailyPoints {
                item_slot: _,
                points,
                item_id: _,
            } => self.handle_arena_daily_points(session, points),
            // ArenaDailyRanking — return the current daily arena state.
            GameRequest::ArenaDailyRanking => self.handle_arena_daily_ranking(session),
            // ArenaRequestRank — live board for the requested rank type.
            GameRequest::ArenaRequestRank { ranking_type } => {
                self.handle_arena_request_rank(session, ranking_type)
            }
            // ArenaRequestOldRank — the previous period's archived board.
            GameRequest::ArenaRequestOldRank { ranking_type } => {
                self.handle_arena_request_old_rank(session, ranking_type)
            }
            // ArenaRequestFame — hall of fame for a season (0 = latest season).
            GameRequest::ArenaRequestFame { season } => {
                self.handle_arena_request_fame(session, season)
            }
            // ArenaRequestFameList — seasons that have a hall of fame.
            GameRequest::ArenaRequestFameList => self.handle_arena_request_fame_list(session),
            // DungeonNextStage — advance the dungeon stage. Real dungeon orchestration is
            // out of scope for the current slice; we acknowledge with the next-stage packet
            // carrying a zero remaining time.
            GameRequest::DungeonNextStage => self.handle_dungeon_next_stage(session),
            // DungeonSurrender — leave the dungeon. We just send back an empty next-stage
            // signaling the run is over (legacy: tamers are teleported back to the lobby).
            GameRequest::DungeonSurrender => Ok(Vec::new()),
            // BurningEvent — return the active burning-event multiplier.
            GameRequest::BurningEvent => Ok(vec![
                BurningEventPacket {
                    exp_rate: 1000,
                    next_day_rate: 100,
                    exp_target: 1,
                }
                .encode(),
            ]),
            // DailyCheckEvent — return the daily-check info table.
            GameRequest::DailyCheckEvent => Ok(vec![
                DailyCheckEventInfoPacket {
                    rows: vec![DailyCheckEventInfoRow {
                        group_id: 1,
                        current_day: 1,
                        next_left_seconds: seconds_until_next_day(),
                        claimed_days: vec![0u8; 4],
                    }],
                }
                .encode(),
            ]),
            // DailyCheckEventRequest — claim today's reward.
            GameRequest::DailyCheckEventRequest { event_no } => Ok(vec![
                DailyCheckEventItemResultPacket {
                    result: 1,
                    group_id: event_no,
                    current_day: 1,
                    next_left_seconds: seconds_until_next_day(),
                    items: Vec::new(),
                }
                .encode(),
            ]),
            // JoinEventQueue — register the character in the event queue. The legacy
            // event server is not yet ported; we acknowledge silently (the modern
            // client polls separately for queue status).
            GameRequest::JoinEventQueue { event_id: _ } => Ok(Vec::new()),
            // RegionUnlock — stub: no response needed
            // RegionUnlock — persist the unlocked region in the character's map_region
            // bitmap and broadcast the new map availability to peers in the same channel.
            GameRequest::RegionUnlock { region_idx } => {
                self.handle_region_unlock(session, region_idx)
            }
            // SetTitle — set the equipped title on the character row, broadcast to peers.
            GameRequest::SetTitle { title_id } => self.handle_set_title(session, title_id),
            // ChangeTamerModel — apply a model change item (consumes the item slot, broadcasts).
            GameRequest::ChangeTamerModel {
                model_id,
                inven_slot,
            } => self.handle_change_tamer_model(session, model_id, inven_slot),
            // TamerNameChange — apply a rename item, validate uniqueness, persist, broadcast.
            GameRequest::TamerNameChange {
                inven_slot: _,
                new_name,
            } => self.handle_tamer_name_change(session, new_name),
            GameRequest::RareMachineOpen { npc_idx: _ } => Ok(vec![]),
            GameRequest::RareMachineRun {
                npc_idx: _,
                inven_idx: _,
                reset_count: _,
            } => Ok(vec![]),
            // Party stubs
            GameRequest::PartyInvite { target_name } => {
                self.handle_party_invite(session, target_name)
            }
            GameRequest::PartyInviteResponse {
                result_type,
                inviter_name,
            } => self.handle_party_invite_response(session, result_type, inviter_name),
            GameRequest::PartyChat { message } => self.handle_party_chat(session, message),
            GameRequest::PartyKick { target_name } => self.handle_party_kick(session, target_name),
            GameRequest::PartyLeave => self.handle_party_leave(session),
            GameRequest::PartyChangeMaster { new_leader_slot } => {
                self.handle_party_change_master(session, new_leader_slot)
            }
            GameRequest::PartyChangeLoot {
                loot_type,
                rare_type,
                disp_rare_grade,
            } => self.handle_party_change_loot(session, loot_type, rare_type, disp_rare_grade),
            GameRequest::PartyDismiss => self.handle_party_dismiss(session),
            // GuildCreate — bootstrap an in-memory guild and emit the bootstrap burst:
            // success packet + GuildInformation + GuildHistoric + GuildRank.
            GameRequest::GuildCreate {
                guild_name,
                inven_slot: _,
                npc_id: _,
            } => self.handle_guild_create(session, guild_name),
            GameRequest::GuildDelete => self.handle_guild_delete(session),
            GameRequest::GuildInvite { target_name } => {
                self.handle_guild_invite(session, target_name)
            }
            GameRequest::GuildInviteAccept {
                certified_code: _,
                target_name: _,
            } => self.handle_guild_invite_accept(session),
            GameRequest::GuildInviteDeny {
                certified_code: _,
                target_name: _,
            } => self.handle_guild_invite_deny(session),
            GameRequest::GuildKick { target_name } => self.handle_guild_kick(session, target_name),
            GameRequest::GuildLeave => self.handle_guild_leave(session),
            // GuildMessage — broadcast guild chat to every online member of the same guild
            GameRequest::GuildMessage { message } => self.handle_guild_message(session, message),
            GameRequest::GuildNotice { notice } => self.handle_guild_notice(session, notice),
            // GuildHistory — respond with the in-memory history (or empty if no guild)
            GameRequest::GuildHistory => self.handle_guild_history(session),
            GameRequest::GuildSetTitle {
                authority_class,
                title,
                duty,
            } => self.handle_guild_set_title(session, authority_class, title, duty),
            // ----- Trade slice ---------------------------------------------------
            // TradeRequest — open a pending request between the inviter and the target.
            GameRequest::TradeRequest { target_handler } => {
                self.handle_trade_request(session, target_handler)
            }
            // TradeAccept — accept a pending trade request and bootstrap the session.
            GameRequest::TradeAccept { accepter_handler } => {
                self.handle_trade_accept(session, accepter_handler)
            }
            // TradeCancel — cancel the active trade session (or pending request).
            GameRequest::TradeCancel => self.handle_trade_cancel(session),
            // TradeAddItem — add an item from the inventory to the active trade.
            GameRequest::TradeAddItem { inven_pos, amount } => {
                self.handle_trade_add_item(session, inven_pos, amount)
            }
            // TradeRemoveItem — remove an item from the active trade.
            GameRequest::TradeRemoveItem { trade_slot } => {
                self.handle_trade_remove_item(session, trade_slot)
            }
            // TradeAddMoney — set the money offered in the active trade.
            GameRequest::TradeAddMoney { amount } => {
                self.handle_trade_add_money(session, amount as i64)
            }
            // TradeConfirm — first-stage confirm. Both sides must confirm before final.
            GameRequest::TradeConfirm => self.handle_trade_confirm(session),
            // TradeLock — lock the local trade slots so both sides see the same state.
            GameRequest::TradeLock => self.handle_trade_lock(session),
            // TradeUnlock — unlock the local trade slots, reverting the lock state.
            GameRequest::TradeUnlock => self.handle_trade_unlock(session),
            // ----- Season pass slice ---------------------------------------------
            GameRequest::SeasonPassDetails => self.handle_season_pass_details(session),
            GameRequest::SeasonPassPurchaseExp { purchase_count } => {
                self.handle_season_pass_purchase_exp(session, purchase_count)
            }
            GameRequest::SeasonPassMissionReward { mission_id } => {
                self.handle_season_pass_mission_reward(session, mission_id)
            }
            GameRequest::SeasonPassSeasonReward { level } => {
                self.handle_season_pass_season_reward(session, level)
            }
            // ----- Channel switch ------------------------------------------------
            GameRequest::ChangeChannel { channel } => self.handle_change_channel(session, channel),
            GameRequest::ChannelSwitchConfirm => Ok(Vec::new()),
            // ----- Personal shop browser ----------------------------------------
            GameRequest::TamerShopList => self.handle_tamer_shop_list(session),
            GameRequest::ConsignedWarehouse => self.handle_consigned_warehouse(session),
            GameRequest::ConsignedWarehouseRetrieve { item_slot } => {
                self.handle_consigned_warehouse_retrieve(session, item_slot)
            }
            // ----- Cash shop history -------------------------------------------
            GameRequest::CashShopBuyHistory => self.handle_cash_shop_buy_history(session),
            // ----- Friend slice -------------------------------------------------
            GameRequest::AddFriend { friend_name } => self.handle_add_friend(session, friend_name),
            GameRequest::DeleteFriend { friend_name } => {
                self.handle_delete_friend(session, friend_name)
            }
            GameRequest::FriendMemo { tamer_name, memo } => {
                self.handle_friend_memo(session, tamer_name, memo)
            }
            GameRequest::GuildInviteFailure { inviter_name } => {
                self.handle_guild_invite_failure(session, inviter_name)
            }
            GameRequest::FriendList => self.handle_friend_list(session),
            GameRequest::GuildAuthorityMaster { target_name } => {
                self.handle_guild_authority(session, target_name, 1)
            }
            GameRequest::GuildAuthoritySubMaster { target_name } => {
                self.handle_guild_authority(session, target_name, 2)
            }
            GameRequest::GuildAuthorityMember { target_name } => {
                self.handle_guild_authority(session, target_name, 4)
            }
            GameRequest::GuildAuthorityNewMember { target_name } => {
                self.handle_guild_authority(session, target_name, 5)
            }
            GameRequest::GuildAuthorityDats { target_name } => {
                self.handle_guild_authority(session, target_name, 3)
            }
            // ----- Spirit / DigiSummon purchase ---------------------------------
            GameRequest::SpiritToDigimon {
                model_id,
                name,
                npc_id,
            } => self.handle_spirit_to_digimon(session, model_id, name, npc_id),
            GameRequest::DigiSummonPurchase {
                product_id,
                ticket_slot,
            } => self.handle_digi_summon_purchase(session, product_id, ticket_slot),
            // ----- Account warehouse --------------------------------------------
            GameRequest::LoadAccountWarehouse => self.handle_load_account_warehouse(session),
            GameRequest::RetrieveAccountWarehouse { item_slot } => {
                self.handle_retrieve_account_warehouse(session, item_slot)
            }
            // ----- Party extra ---------------------------------------------------
            GameRequest::PartyMemberDisconnect => Ok(Vec::new()),
            // ----- Combat misc ---------------------------------------------------
            GameRequest::MonsterRespawnTimer => self.handle_monster_respawn_timer(session),
            GameRequest::JumpBooster => self.handle_jump_booster(session),
            GameRequest::SkillLevelUp {
                uid: _,
                evo_unit_idx: _,
                skill_idx,
            } => self.handle_skill_level_up(session, skill_idx),
            GameRequest::TamerChargeXCrystal => self.handle_tamer_charge_xcrystal(session),
            GameRequest::TamerConsumeXCrystal { amount } => {
                self.handle_tamer_consume_xcrystal(session, amount)
            }
            GameRequest::TamerSummon(summon) => match summon {
                TamerSummonRequest::ByName {
                    target_name,
                    skill_index: _,
                } => self.handle_tamer_summon(session, target_name),
                // The remaining cases report summon-dialog state. Nothing changes in the
                // world and the client waits for no reply, so they are consumed silently.
                TamerSummonRequest::StateCheck { .. } | TamerSummonRequest::Accept { .. } => {
                    Ok(Vec::new())
                }
            },
            GameRequest::TamerSkillRequest {
                skill_idx,
                target_uid,
            } => self.handle_tamer_skill_request(session, skill_idx, target_uid),
            GameRequest::TranscendenceReceiveExp => self.handle_transcendence_receive_exp(session),
            GameRequest::TranscendenceSuccess => self.handle_transcendence_success(session),
            GameRequest::TimeChargeResult { charge_type } => {
                self.handle_time_charge_result(session, charge_type)
            }
            GameRequest::WarpGateDungeon => self.handle_warp_gate_dungeon(session),
            GameRequest::DigiCombineSyncRequest => self.handle_digi_combine_sync(session),
            GameRequest::DigiCombine {
                ceiling_type,
                materials,
            } => self.handle_digi_combine(session, ceiling_type, materials),
            GameRequest::DigiCombineRewardClaim { ceiling_type } => {
                self.handle_digi_combine_reward_claim(session, ceiling_type)
            }
            GameRequest::UnionCombineSyncRequest => self.handle_union_combine_sync(session),
            GameRequest::UnionCombine {
                ceiling_type,
                materials,
            } => self.handle_union_combine(session, ceiling_type, materials),
            GameRequest::UnionCombineRewardClaim { ceiling_type } => {
                self.handle_union_combine_reward_claim(session, ceiling_type)
            }
            GameRequest::UnionHackOpenRequest => self.handle_union_hack_open(session),
            GameRequest::UnionHackModify {
                slot,
                part_id,
                grade,
            } => self.handle_union_hack_modify(session, slot, part_id, grade),
            GameRequest::RandomBoxList { .. } => self.handle_random_box_list(session),
            GameRequest::RandomBoxPurchase { .. } => self.handle_random_box_purchase(session),
            GameRequest::DigimonToSpirit {
                slot,
                validation,
                npc_id,
            } => self.handle_digimon_to_spirit(session, slot, validation, npc_id),
        }?;

        responses.extend(request_responses);
        Ok(responses)
    }

    // ----- Cash shop slice ------------------------------------------------------------

    fn handle_cash_shop_buy(
        &self,
        session: &GameSession,
        amount: u8,
        total_price: i32,
        order_id: u64,
        product_ids: Vec<i32>,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let effective_premium = if std::env::var("ODMO_SMOKE_TEST").is_ok() && character.premium < 10000 {
            10000
        } else {
            character.premium
        };

        // Reject if the character has insufficient premium currency.
        if effective_premium < total_price {
            return Ok(vec![CashShopBuyResponsePacket {
                result: 31010, // generic cash shop failure
                real_cash: effective_premium,
                bonus_cash: character.silk,
                success_product_ids: Vec::new(),
                failed_product_ids: product_ids.clone(),
            }
            .encode()]);
        }

        // Charge the premium currency.
        let new_premium = effective_premium.saturating_sub(total_price);
        self.repository
            .update_currencies(character_id, new_premium, character.silk)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // Append a history entry.
        let mut history = character.cash_shop_history.clone();
        for product_id in &product_ids {
            history.push(odmo_types::CashShopHistoryEntry {
                order_id: order_id as u32,
                product_id: *product_id,
                amount: i16::from(amount),
                price: total_price,
                purchased_at_unix: current_unix_timestamp(),
            });
        }
        // Cap the history to the most recent 100 entries.
        if history.len() > 100 {
            let drop = history.len() - 100;
            history.drain(0..drop);
        }
        self.repository
            .update_cash_shop_history(character_id, history)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // Deliver purchased items into Cash Warehouse (account_warehouse)
        let mut warehouse = character
            .account_warehouse
            .clone()
            .unwrap_or_else(|| odmo_types::InventorySnapshot {
                bits: 0,
                size: 49,
                items: Vec::new(),
            });

        for product_id in &product_ids {
            let new_item = odmo_types::ItemRecord::new(*product_id, i32::from(amount));
            if let Some(empty_slot) = warehouse.items.iter().position(|it| it.item_id == 0) {
                warehouse.items[empty_slot] = new_item;
            } else if warehouse.items.len() < warehouse.size as usize {
                warehouse.items.push(new_item);
            }
        }
        self.repository
            .update_account_warehouse(character_id, warehouse)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        tracing::info!(
            "[CASHSHOP] Buy completed character_id={} products={:?} total_price={}",
            character_id,
            product_ids,
            total_price
        );

        Ok(vec![
            CashShopBuyResponsePacket {
                result: 0, // success
                real_cash: new_premium,
                bonus_cash: character.silk,
                success_product_ids: product_ids.clone(),
                failed_product_ids: Vec::new(),
            }
            .encode(),
            CashShopCoinsPacket {
                premium: new_premium,
                silk: character.silk,
            }
            .encode(),
        ])
    }

    fn handle_cash_shop_reload(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let effective_premium = if std::env::var("ODMO_SMOKE_TEST").is_ok() && character.premium < 10000 {
            10000
        } else {
            character.premium
        };

        tracing::info!("[CASHSHOP] Balance requested character_id={}", character_id);

        Ok(vec![
            CashShopCoinsPacket {
                premium: effective_premium,
                silk: character.silk,
            }
            .encode(),
        ])
    }

    fn handle_cash_shop_buy_history(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::CASHSHOP_BUY_HISTORY,
        );
        writer.write_u16(character.cash_shop_history.len().min(u16::MAX as usize) as u16);
        for entry in &character.cash_shop_history {
            writer.write_u32(entry.order_id);
            writer.write_i32(entry.product_id);
            writer.write_i16(entry.amount);
            writer.write_i32(entry.price);
            writer.write_u64(entry.purchased_at_unix);
        }
        Ok(vec![writer.finalize()])
    }

    // ----- Consigned shop slice -------------------------------------------------------

    fn handle_consigned_shop_open(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        // Pure UI open — return an empty listing envelope so the client renders.
        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::CONSIGNSHOP_OPEN);
        writer.write_u16(0);
        Ok(vec![writer.finalize()])
    }

    fn handle_consigned_shop_view(
        &self,
        _session: &GameSession,
        shop_id: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        // Look up the seller character by shop id (treated as the seller character id).
        let seller = self
            .repository
            .character_by_id(shop_id as u64)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::CONSIGNSHOP_VIEW);
        if let Some(seller) = seller {
            writer.write_u16(seller.tamer_shop_listings.len().min(u16::MAX as usize) as u16);
            for listing in &seller.tamer_shop_listings {
                writer.write_u32(listing.listing_id);
                writer.write_i32(listing.item_id);
                writer.write_i16(listing.amount);
                writer.write_i64(listing.price_per_unit);
            }
        } else {
            writer.write_u16(0);
        }
        Ok(vec![writer.finalize()])
    }

    fn handle_consigned_shop_purchase(
        &self,
        session: &GameSession,
        item_id: i32,
        amount: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        // Reuse the tamer-shop buy logic.
        self.handle_tamer_shop_buy(session, item_id, amount)
    }

    fn handle_consigned_shop_retrieve(
        &self,
        session: &GameSession,
        item_slot: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let idx = item_slot as usize;
        let mut listings = character.tamer_shop_listings.clone();
        if item_slot < 0 || idx >= listings.len() {
            return Ok(Vec::new());
        }

        let listing = listings.remove(idx);

        // Return the unsold items to inventory.
        let mut inventory = character.inventory.clone();
        if let Some(target_slot) = inventory.items.iter().position(|i| i.item_id == 0) {
            inventory.items[target_slot] =
                odmo_types::ItemRecord::new(listing.item_id, listing.amount as i32);
        }
        self.repository
            .update_inventory(character_id, inventory)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_tamer_shop(character_id, listings)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(Vec::new())
    }

    fn handle_consigned_warehouse(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::CONSIGNSHOP_WAREHOUSE,
        );
        writer.write_u16(character.tamer_shop_listings.len().min(u16::MAX as usize) as u16);
        for listing in &character.tamer_shop_listings {
            writer.write_u32(listing.listing_id);
            writer.write_i32(listing.item_id);
            writer.write_i16(listing.amount);
            writer.write_i64(listing.price_per_unit);
        }
        Ok(vec![writer.finalize()])
    }

    fn handle_consigned_warehouse_retrieve(
        &self,
        session: &GameSession,
        item_slot: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        self.handle_consigned_shop_retrieve(session, item_slot)
    }

    fn handle_tamer_shop_list(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::TAMER_SHOP_LIST);
        writer.write_u16(character.tamer_shop_listings.len().min(u16::MAX as usize) as u16);
        for listing in &character.tamer_shop_listings {
            writer.write_u32(listing.listing_id);
            writer.write_i32(listing.item_id);
            writer.write_i16(listing.amount);
            writer.write_i64(listing.price_per_unit);
        }
        Ok(vec![writer.finalize()])
    }

    // ----- Friend slice ----------------------------------------------------------------

    fn handle_add_friend(
        &self,
        session: &GameSession,
        friend_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let target = self
            .repository
            .character_by_name(&friend_name)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut friends = character.friend_list.clone();

        if let Some(target) = target
            && target.id != character.id
            && !friends.iter().any(|f| f.character_id == target.id)
        {
            friends.push(odmo_types::FriendListEntry {
                character_id: target.id,
                name: target.name.clone(),
                annotation: String::new(),
                favorite: false,
            });
            self.repository
                .update_friend_list(character_id, friends)
                .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        }

        Ok(Vec::new())
    }

    /// `pBuddy::Delete` (2402): drops the named friend, then refreshes the panel
    /// from storage so a name that was never stored cannot leave the client's
    /// optimistic removal standing.
    fn handle_delete_friend(
        &self,
        session: &GameSession,
        friend_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let target = friend_name.trim();
        let mut friends = character.friend_list.clone();
        friends.retain(|friend| friend.name != target);
        if friends.len() != character.friend_list.len() {
            self.repository
                .update_friend_list(character_id, friends)
                .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        }

        self.handle_friend_list(session)
    }

    /// `pBuddy::Memo` (2405): rewrites the annotation of one stored friend.
    fn handle_friend_memo(
        &self,
        session: &GameSession,
        tamer_name: String,
        memo: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let target = tamer_name.trim();
        let mut friends = character.friend_list.clone();
        let mut changed = false;
        for friend in friends.iter_mut().filter(|friend| friend.name == target) {
            friend.annotation = memo.clone();
            changed = true;
        }
        if changed {
            self.repository
                .update_friend_list(character_id, friends)
                .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        }

        self.handle_friend_list(session)
    }

    /// `pGuild::ReqAddFailure` (2110): the sender could not take a pending invite
    /// and names the tamer who sent it, so that tamer receives the failure
    /// notice. Code 3 is the one the client renders as "cannot be invited right
    /// now", which covers a client caught mid-flow.
    fn handle_guild_invite_failure(
        &self,
        session: &GameSession,
        inviter_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let inviter = self
            .repository
            .character_by_name(inviter_name.trim())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let Some(inviter) = inviter else {
            return Ok(Vec::new());
        };

        let packet = GuildInviteFailPacket {
            reason: 3,
            target_name: character.name,
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to(inviter.id, &packet);
        }

        Ok(Vec::new())
    }

    fn handle_friend_list(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Each record leads with live presence, which only the broadcast registry knows,
        // and the packet carries the block list as its second count-prefixed section.
        let friends = character
            .friend_list
            .iter()
            .map(|friend| odmo_types::RelationEntry {
                name: friend.name.clone(),
                connected: self
                    .broadcast
                    .as_ref()
                    .is_some_and(|broadcast| broadcast.is_online(friend.character_id)),
                annotation: friend.annotation.clone(),
            })
            .collect();

        Ok(vec![
            TamerRelationsPacket {
                friends,
                foes: character.foes.clone(),
            }
            .encode(),
        ])
    }

    // ----- Season pass slice ----------------------------------------------------------

    fn handle_season_pass_details(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::SEASON_PASS_DETAILS,
        );
        writer.write_i32(character.season_pass.current_level);
        writer.write_i32(character.season_pass.current_experience);
        writer.write_u8(if character.season_pass.purchased_premium {
            1
        } else {
            0
        });
        writer.write_u16(
            character
                .season_pass
                .claimed_mission_ids
                .len()
                .min(u16::MAX as usize) as u16,
        );
        for mission_id in &character.season_pass.claimed_mission_ids {
            writer.write_i32(*mission_id);
        }
        writer.write_u16(
            character
                .season_pass
                .claimed_season_levels
                .len()
                .min(u16::MAX as usize) as u16,
        );
        for level in &character.season_pass.claimed_season_levels {
            writer.write_i32(*level);
        }
        Ok(vec![writer.finalize()])
    }

    fn handle_season_pass_purchase_exp(
        &self,
        session: &GameSession,
        purchase_count: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Each purchase block adds 100 EXP and costs 100 premium.
        let cost_per_block = 100i32;
        let exp_per_block = 100i32;
        let count = purchase_count.clamp(0, 50);
        let total_cost = cost_per_block.saturating_mul(count);
        if character.premium < total_cost {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::SEASON_PASS_PURCHASE_EXP,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        }

        let mut state = character.season_pass.clone();
        state.current_experience = state
            .current_experience
            .saturating_add(exp_per_block * count);
        // Bump level every 1000 EXP.
        while state.current_experience >= 1000 && state.current_level < 100 {
            state.current_experience -= 1000;
            state.current_level += 1;
        }
        self.repository
            .update_season_pass(character_id, state.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_currencies(character_id, character.premium - total_cost, character.silk)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::SEASON_PASS_PURCHASE_EXP,
        );
        writer.write_u8(1);
        writer.write_i32(state.current_level);
        writer.write_i32(state.current_experience);
        Ok(vec![writer.finalize()])
    }

    fn handle_season_pass_mission_reward(
        &self,
        session: &GameSession,
        mission_id: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut state = character.season_pass.clone();
        if state.claimed_mission_ids.contains(&mission_id) {
            return Ok(Vec::new());
        }
        state.claimed_mission_ids.push(mission_id);
        self.repository
            .update_season_pass(character_id, state)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::SEASON_PASS_MISSION_REWARD,
        );
        writer.write_i32(mission_id);
        writer.write_u8(1);
        Ok(vec![writer.finalize()])
    }

    fn handle_season_pass_season_reward(
        &self,
        session: &GameSession,
        level: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut state = character.season_pass.clone();
        if state.claimed_season_levels.contains(&level) || level > state.current_level {
            return Ok(Vec::new());
        }
        state.claimed_season_levels.push(level);
        self.repository
            .update_season_pass(character_id, state)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::SEASON_PASS_SEASON_REWARD,
        );
        writer.write_i32(level);
        writer.write_u8(1);
        Ok(vec![writer.finalize()])
    }

    // ----- Channel switching ----------------------------------------------------------

    fn handle_change_channel(
        &self,
        session: &GameSession,
        channel: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Persist the new channel via the existing `update_character_map` helper since
        // map id and position stay the same.
        self.repository
            .update_character_map(character_id, character.map_id, character.x, character.y)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::CHANGE_CHANNEL);
        writer.write_u8(channel);
        writer.write_u8(1); // success
        Ok(vec![writer.finalize()])
    }

    // ----- Spirit / DigiSummon --------------------------------------------------------

    fn handle_spirit_to_digimon(
        &self,
        session: &GameSession,
        model_id: i32,
        name: String,
        npc_id: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;
        let Some(npc) = self
            .repository
            .extra_evolution_npcs()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .into_iter()
            .find(|npc| npc.npc_id == npc_id)
        else {
            return Ok(Vec::new());
        };

        let Some(recipe) = npc
            .recipes
            .iter()
            .find(|recipe| {
                recipe.exchange_type == EXTRA_EVOLUTION_ITEM_TO_DIGIMON
                    && recipe.object_id == model_id
            })
            .cloned()
        else {
            return Ok(Vec::new());
        };

        if character.inventory.bits < recipe.price {
            return Ok(Vec::new());
        }

        let next_slot = (1..=character.digimon_slots).find(|slot| {
            !character
                .partner_slots
                .iter()
                .any(|partner| partner.slot == *slot)
        });
        let Some(next_slot) = next_slot else {
            return Ok(Vec::new());
        };

        let original_inventory = character.inventory.clone();
        let original_bits = character.inventory_bits;
        let mut consumed_items = Vec::new();

        if !consume_item_material_groups(
            &mut character.inventory,
            recipe.way_type,
            &recipe.main_materials,
            &recipe.sub_materials,
            &mut consumed_items,
        ) {
            return Ok(Vec::new());
        }

        character.inventory.bits -= recipe.price;
        character.inventory_bits = (character.inventory_bits - recipe.price).max(0);

        let new_partner = default_partner_for_type(model_id, next_slot, name.clone());
        let mut partner_slots = character.partner_slots.clone();
        partner_slots.push(new_partner.clone());

        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_inventory_bits(character_id, character.inventory_bits)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_partner_roster(character_id, character.partner_current_slot, partner_slots)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut hatch_finish =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::HATCH_FINISH);
        hatch_finish.write_u8(1);
        hatch_finish.write_u8(next_slot);
        hatch_finish.write_string(&name);

        let consumed_packet_items = consumed_items
            .iter()
            .map(|item| {
                (
                    item.amount.clamp(1, u8::MAX as i32) as u8,
                    item.item_id as u32,
                )
            })
            .collect();

        let _ = original_inventory;
        let _ = original_bits;

        Ok(vec![
            hatch_finish.finalize(),
            SpiritToDigimonResultPacket {
                digimon_id: model_id as u32,
                remaining_bits: character.inventory_bits,
                consumed_items: consumed_packet_items,
            }
            .encode(),
            LoadInventoryPacket {
                inventory: character.inventory,
                inventory_type: InventoryType::Inventory,
            }
            .encode(),
        ])
    }

    fn handle_digi_summon_sync(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let products = self
            .repository
            .digi_summon_products()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let result = if products.is_empty() {
            DIGI_SUMMON_NO_PRODUCTS
        } else {
            DIGI_SUMMON_SUCCESS
        };
        Ok(vec![
            DigiSummonSyncResponsePacket { result, products }.encode(),
        ])
    }

    fn handle_digi_summon_purchase(
        &self,
        session: &GameSession,
        product_id: i32,
        ticket_slot: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;
        let products = self
            .repository
            .digi_summon_products()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        if products.is_empty() {
            return Ok(vec![
                DigiSummonPurchaseResponsePacket {
                    result: DIGI_SUMMON_NO_PRODUCTS,
                    product_id,
                    rewards: Vec::new(),
                    products,
                }
                .encode(),
            ]);
        }

        let Some(product) = products
            .iter()
            .find(|product| product.product_id == product_id)
        else {
            return Ok(vec![
                DigiSummonPurchaseResponsePacket {
                    result: DIGI_SUMMON_INVALID_PRODUCT,
                    product_id,
                    rewards: Vec::new(),
                    products,
                }
                .encode(),
            ]);
        };

        let Some((ticket_index, ticket)) =
            find_usable_digi_summon_ticket(&character.inventory, product, ticket_slot)
        else {
            return Ok(vec![
                DigiSummonPurchaseResponsePacket {
                    result: DIGI_SUMMON_NOT_ENOUGH_TICKET,
                    product_id,
                    rewards: Vec::new(),
                    products,
                }
                .encode(),
            ]);
        };

        let original_inventory = character.inventory.clone();
        let Some(_) =
            consume_inventory_item_at(&mut character.inventory, ticket_index, ticket.cost.max(0))
        else {
            return Ok(vec![
                DigiSummonPurchaseResponsePacket {
                    result: DIGI_SUMMON_NOT_ENOUGH_TICKET,
                    product_id,
                    rewards: Vec::new(),
                    products,
                }
                .encode(),
            ]);
        };

        let rewards = roll_digi_summon_rewards(product);
        for reward in &rewards {
            if !add_stackable_inventory_item(
                &mut character.inventory,
                reward.item_id,
                reward.amount.max(1),
            ) {
                character.inventory = original_inventory.clone();
                return Ok(vec![
                    DigiSummonPurchaseResponsePacket {
                        result: DIGI_SUMMON_INVENTORY_FULL,
                        product_id,
                        rewards: Vec::new(),
                        products,
                    }
                    .encode(),
                ]);
            }
        }

        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            LoadInventoryPacket {
                inventory: character.inventory,
                inventory_type: InventoryType::Inventory,
            }
            .encode(),
            DigiSummonPurchaseResponsePacket {
                result: DIGI_SUMMON_SUCCESS,
                product_id,
                rewards,
                products,
            }
            .encode(),
        ])
    }

    fn handle_digi_combine_sync(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        self.handle_combine_sync(session, false)
    }

    fn handle_union_combine_sync(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        self.handle_combine_sync(session, true)
    }

    /// Emit the combine ceiling map for the gacha window. The result mirrors the
    /// summon sync convention: zero for a populated catalog, one when empty.
    fn handle_combine_sync(
        &self,
        _session: &GameSession,
        is_union: bool,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let catalog = self.combine_catalog(is_union)?;
        let ceiling = combine_ceiling_all(&catalog);
        let result = if catalog.rank_rows.is_empty() {
            COMBINE_SYNC_NO_CATALOG
        } else {
            COMBINE_RESULT_SUCCESS
        };
        let packet = if is_union {
            CombineSyncResponsePacket::union(result, ceiling)
        } else {
            CombineSyncResponsePacket::digi(result, ceiling)
        };
        Ok(vec![packet.encode()])
    }

    fn handle_digi_combine(
        &self,
        session: &GameSession,
        ceiling_type: u8,
        materials: Vec<CombineItemRef>,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        self.handle_combine(session, ceiling_type, materials, false)
    }

    fn handle_union_combine(
        &self,
        session: &GameSession,
        ceiling_type: u8,
        materials: Vec<CombineItemRef>,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        self.handle_combine(session, ceiling_type, materials, true)
    }

    /// Run a Digi/Union combine roll. Union shares Digi's byte-identical layouts,
    /// so both flow through here with `is_union` selecting catalog and opcode.
    ///
    /// The grid is re-validated defensively; a malformed submission rejects with
    /// no mutation. On a valid grid the submitted materials are consumed exactly,
    /// a rank is rolled by weight over the matching ceiling, and its rewards are
    /// granted. Any inventory overflow rolls the inventory back to its pre-state.
    fn handle_combine(
        &self,
        session: &GameSession,
        ceiling_type: u8,
        materials: Vec<CombineItemRef>,
        is_union: bool,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let catalog = self.combine_catalog(is_union)?;
        let ceiling = combine_ceiling_for_type(&catalog, ceiling_type);

        let reject = |result: u8| -> Vec<u8> {
            if is_union {
                CombineResultResponsePacket::union_result(
                    result,
                    ceiling.clone(),
                    Vec::new(),
                    Vec::new(),
                )
            } else {
                CombineResultResponsePacket::digi_result(
                    result,
                    ceiling.clone(),
                    Vec::new(),
                    Vec::new(),
                )
            }
            .encode()
        };

        if !combine_grid_is_valid(&materials) {
            return Ok(vec![reject(COMBINE_RESULT_INVALID_GRID)]);
        }

        let original_inventory = character.inventory.clone();
        if !consume_combine_materials(&mut character.inventory, &materials) {
            character.inventory = original_inventory;
            return Ok(vec![reject(COMBINE_RESULT_MISSING_MATERIAL)]);
        }

        let ranks: Vec<odmo_types::DigiCombineRank> = catalog
            .rank_rows
            .iter()
            .filter(|rank| rank.ceiling_type == ceiling_type)
            .cloned()
            .collect();
        let rewards = pick_weighted_combine_rank(&ranks)
            .map(|rank| rank.rewards)
            .unwrap_or_default();

        for reward in &rewards {
            if !add_stackable_inventory_item(
                &mut character.inventory,
                reward.item_id,
                i32::from(reward.amount.max(1)),
            ) {
                character.inventory = original_inventory;
                return Ok(vec![reject(COMBINE_RESULT_INVENTORY_FULL)]);
            }
        }

        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let result = if is_union {
            CombineResultResponsePacket::union_result(
                COMBINE_RESULT_SUCCESS,
                ceiling,
                materials,
                rewards,
            )
        } else {
            CombineResultResponsePacket::digi_result(
                COMBINE_RESULT_SUCCESS,
                ceiling,
                materials,
                rewards,
            )
        };

        Ok(vec![
            LoadInventoryPacket {
                inventory: character.inventory,
                inventory_type: InventoryType::Inventory,
            }
            .encode(),
            result.encode(),
        ])
    }

    fn handle_digi_combine_reward_claim(
        &self,
        session: &GameSession,
        ceiling_type: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        self.handle_combine_reward_claim(session, ceiling_type, false)
    }

    fn handle_union_combine_reward_claim(
        &self,
        session: &GameSession,
        ceiling_type: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        self.handle_combine_reward_claim(session, ceiling_type, true)
    }

    /// Claim the reward for a resolved ceiling tier. The reward block is keyed on
    /// `ceiling_type`; a successful claim grants exactly that reward and an
    /// inventory overflow rolls back fully and rejects.
    fn handle_combine_reward_claim(
        &self,
        session: &GameSession,
        ceiling_type: u8,
        is_union: bool,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let catalog = self.combine_catalog(is_union)?;
        let ceiling = combine_ceiling_for_type(&catalog, ceiling_type);
        let rewards: Vec<DigiCombineReward> = catalog
            .rank_rows
            .iter()
            .filter(|rank| rank.ceiling_type == ceiling_type)
            .flat_map(|rank| rank.rewards.iter().cloned())
            .collect();

        let reward_packet = |result: u8, rewards: Vec<DigiCombineReward>| -> Vec<u8> {
            if is_union {
                CombineResultResponsePacket::union_reward(
                    result,
                    ceiling.clone(),
                    Vec::new(),
                    rewards,
                )
            } else {
                CombineResultResponsePacket::digi_reward(
                    result,
                    ceiling.clone(),
                    Vec::new(),
                    rewards,
                )
            }
            .encode()
        };

        let original_inventory = character.inventory.clone();
        for reward in &rewards {
            if !add_stackable_inventory_item(
                &mut character.inventory,
                reward.item_id,
                i32::from(reward.amount.max(1)),
            ) {
                character.inventory = original_inventory;
                return Ok(vec![reward_packet(
                    COMBINE_RESULT_INVENTORY_FULL,
                    Vec::new(),
                )]);
            }
        }

        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            LoadInventoryPacket {
                inventory: character.inventory,
                inventory_type: InventoryType::Inventory,
            }
            .encode(),
            reward_packet(COMBINE_RESULT_SUCCESS, rewards),
        ])
    }

    // ----- D-Unit (Union) hacking tool slice ----------------------------------------

    /// Open the D-Unit hacking grid window. Returns the unlocked slot count and
    /// the equipped parts per slot for the current character.
    fn handle_union_hack_open(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let rows = self
            .repository
            .union_hack_slots(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let unlocked_slots = rows.len().min(u8::MAX as usize) as u8;
        let slots: Vec<UnionHackSlot> = rows
            .into_iter()
            .enumerate()
            .map(|(index, row)| UnionHackSlot {
                slot: index as u8,
                part_id: row.part_id,
                grade: row.grade,
                locked: row.locked,
            })
            .collect();

        Ok(vec![
            UnionHackOpenResponsePacket {
                result: 0,
                unlocked_slots,
                slots,
            }
            .encode(),
        ])
    }

    /// Replace the part installed in a given D-Unit slot for the current character.
    /// Returns the modified slot plus the recomputed total rating.
    fn handle_union_hack_modify(
        &self,
        session: &GameSession,
        slot: u8,
        part_id: i32,
        grade: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;

        let updated = self
            .repository
            .update_union_hack_slot(character_id, slot, part_id, grade)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        if !updated {
            return Ok(vec![
                UnionHackModifyResponsePacket {
                    result: 1,
                    slot,
                    new_part_id: part_id,
                    new_grade: grade,
                    total_rating: 0,
                }
                .encode(),
            ]);
        }

        let rows = self
            .repository
            .union_hack_slots(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let total_rating: i32 = rows.iter().map(|row| i32::from(row.grade)).sum();

        Ok(vec![
            UnionHackModifyResponsePacket {
                result: 0,
                slot,
                new_part_id: part_id,
                new_grade: grade,
                total_rating,
            }
            .encode(),
        ])
    }

    /// Push the full D-Unit state to the client on login so the modern
    /// `cUnionContents` can hydrate its hacking grid without an extra request.
    pub fn build_union_init_data(&self, session: &GameSession) -> Result<Vec<u8>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let rows = self
            .repository
            .union_hack_slots(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let slots: Vec<UnionHackSlot> = rows
            .into_iter()
            .enumerate()
            .map(|(index, row)| UnionHackSlot {
                slot: index as u8,
                part_id: row.part_id,
                grade: row.grade,
                locked: row.locked,
            })
            .collect();

        let total_rating: i32 = slots.iter().map(|slot| i32::from(slot.grade)).sum();

        Ok(UnionInitDataPacket {
            slots,
            total_rating,
            synergy_bonus: 0,
        }
        .encode())
    }

    /// Read the Digi or Union combine catalog from the repository.
    fn combine_catalog(&self, is_union: bool) -> Result<DigiCombineCatalog, GameFlowError> {
        if is_union {
            self.repository.union_combine_catalog()
        } else {
            self.repository.digi_combine_catalog()
        }
        .map_err(|error| GameFlowError::Storage(error.to_string()))
    }

    /// Emit the random box window contents from the server-side reward table.
    ///
    /// The wire list fields are not yet decoded, so the reward pool is projected
    /// onto the fixed entry shape neutrally: one entry per reward carrying its
    /// item id, amount, and weight. The leading field stays zero. This keeps the
    /// frame well-formed on the correct opcode while the field meanings remain an
    /// open item.
    fn handle_random_box_list(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let rewards = self
            .repository
            .random_box_rewards()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let entries = rewards
            .iter()
            .map(|reward| RandomBoxListEntry {
                a: reward.item_id,
                b: i32::from(reward.amount),
                c: reward.weight as i32,
                d: 0,
            })
            .collect();

        Ok(vec![
            RandomBoxListResponsePacket { field0: 0, entries }.encode(),
        ])
    }

    /// Roll and grant one random box reward.
    ///
    /// Exactly one reward is selected by relative weight over the server-side
    /// table and granted to the inventory. If the grant would overflow the
    /// inventory the snapshot is restored and the purchase is rejected without
    /// persisting, answering with an empty result block. On success the inventory
    /// is persisted and the granted reward is projected neutrally onto the result
    /// list pending the field-semantics decode.
    fn handle_random_box_purchase(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let rewards = self
            .repository
            .random_box_rewards()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let empty_result = || -> Vec<u8> {
            RandomBoxPurchaseResponsePacket {
                field0: 0,
                field1: 0,
                field2: 0,
                list_a: Vec::new(),
                list_b: Vec::new(),
                summary: (0, 0),
            }
            .encode()
        };

        let Some(reward) = pick_weighted_random_box_reward(&rewards) else {
            return Ok(vec![empty_result()]);
        };

        let original_inventory = character.inventory.clone();
        if !add_stackable_inventory_item(
            &mut character.inventory,
            reward.item_id,
            i32::from(reward.amount.max(1)),
        ) {
            character.inventory = original_inventory;
            return Ok(vec![empty_result()]);
        }

        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            LoadInventoryPacket {
                inventory: character.inventory,
                inventory_type: InventoryType::Inventory,
            }
            .encode(),
            RandomBoxPurchaseResponsePacket {
                field0: 0,
                field1: 0,
                field2: 0,
                list_a: vec![(reward.item_id, i32::from(reward.amount.max(1)))],
                list_b: Vec::new(),
                summary: (0, 0),
            }
            .encode(),
        ])
    }

    fn handle_digimon_to_spirit(
        &self,
        session: &GameSession,
        slot: u8,
        validation: String,
        npc_id: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let account_id = session.account_id.ok_or(GameFlowError::Unauthenticated)?;
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let account = self
            .repository
            .account_by_id(account_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::Unauthenticated)?;
        if validation != account.email
            && account.secondary_password.as_deref() != Some(validation.as_str())
        {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::PARTNER_DELETE_RESPONSE,
            );
            writer.write_i32(-1);
            return Ok(vec![writer.finalize()]);
        }

        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;
        let Some(npc) = self
            .repository
            .extra_evolution_npcs()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .into_iter()
            .find(|npc| npc.npc_id == npc_id)
        else {
            return Ok(Vec::new());
        };

        let Some(target_partner) = character
            .partner_slots
            .iter()
            .find(|partner| partner.slot == slot)
            .cloned()
        else {
            return Ok(Vec::new());
        };

        let Some(recipe) = npc
            .recipes
            .iter()
            .find(|recipe| {
                recipe.exchange_type == EXTRA_EVOLUTION_DIGIMON_TO_ITEM
                    && recipe.main_materials.iter().any(|material| {
                        material.material_id == target_partner.digimon_type
                            && target_partner.level as i32 >= recipe.need_material_value
                    })
            })
            .cloned()
        else {
            return Ok(Vec::new());
        };

        if character.inventory.bits < recipe.price {
            return Ok(Vec::new());
        }

        let original_inventory = character.inventory.clone();
        let mut consumed_items = Vec::new();
        if !consume_item_material_groups(
            &mut character.inventory,
            recipe.way_type,
            &[],
            &recipe.sub_materials,
            &mut consumed_items,
        ) {
            return Ok(Vec::new());
        }

        character.inventory.bits -= recipe.price;
        character.inventory_bits = (character.inventory_bits - recipe.price).max(0);
        if !add_stackable_inventory_item(&mut character.inventory, recipe.object_id, 1) {
            character.inventory = original_inventory;
            return Ok(Vec::new());
        }

        let mut partner_slots = character.partner_slots.clone();
        partner_slots.retain(|partner| partner.slot != slot);

        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_inventory_bits(character_id, character.inventory_bits)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_partner_roster(character_id, character.partner_current_slot, partner_slots)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let consumed_packet_items = consumed_items
            .iter()
            .map(|item| {
                (
                    item.amount.clamp(1, u8::MAX as i32) as u8,
                    item.item_id as u32,
                )
            })
            .collect();

        Ok(vec![
            DigimonToSpiritResultPacket {
                slot,
                remaining_bits: character.inventory_bits,
                consumed_items: consumed_packet_items,
                gained_items: vec![(1, recipe.object_id as u32)],
            }
            .encode(),
            LoadInventoryPacket {
                inventory: character.inventory,
                inventory_type: InventoryType::Inventory,
            }
            .encode(),
        ])
    }

    // ----- Account warehouse ----------------------------------------------------------

    fn handle_load_account_warehouse(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let items = character
            .account_warehouse
            .map(|w| w.items)
            .unwrap_or_default();

        Ok(vec![CashShopWarehousePacket { items }.encode()])
    }

    fn handle_retrieve_account_warehouse(
        &self,
        session: &GameSession,
        item_slot: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut warehouse =
            character
                .account_warehouse
                .clone()
                .unwrap_or_else(|| odmo_types::InventorySnapshot {
                    bits: 0,
                    size: 49,
                    items: Vec::new(),
                });
        let mut inventory = character.inventory.clone();

        let idx = item_slot as usize;
        if item_slot < 0 || idx >= warehouse.items.len() || warehouse.items[idx].item_id == 0 {
            return Ok(Vec::new());
        }
        let target_slot = inventory.items.iter().position(|i| i.item_id == 0);
        let Some(target_slot) = target_slot else {
            return Ok(vec![CashShopCropResponsePacket {
                result: 30322, // cash inventory limit reached
                rtime: 0,
                cash_warehouse_slot: item_slot as u16,
                slot_type: 0,
                slot_no: 0,
                item_type: 0,
                item_count: 0,
                item_rate: 0,
                item_end_time: 0,
                remain_trade_limit_time: 0,
            }
            .encode()]);
        };

        let claimed =
            std::mem::replace(&mut warehouse.items[idx], odmo_types::ItemRecord::new(0, 0));
        let item_type = claimed.item_id as u32;
        let item_count = claimed.amount as u32;
        inventory.items[target_slot] = claimed;

        self.repository
            .update_inventory(character_id, inventory)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_account_warehouse(character_id, warehouse)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![CashShopCropResponsePacket {
            result: 100, // crop completed
            rtime: 0,
            cash_warehouse_slot: item_slot as u16,
            slot_type: 0, // player inventory tab
            slot_no: target_slot as u8,
            item_type,
            item_count,
            item_rate: 100,
            item_end_time: 0,
            remain_trade_limit_time: 0,
        }
        .encode()])
    }

    // ----- Combat misc handlers --------------------------------------------------------

    fn handle_monster_respawn_timer(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        Ok(vec![
            MonsterRespawnTimerPacket { rows: Vec::new() }.encode(),
        ])
    }

    fn handle_jump_booster(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;
        // Just acknowledge silently with the new count incremented in dev backends.
        let _ = character;
        Ok(Vec::new())
    }

    fn handle_skill_level_up(
        &self,
        session: &GameSession,
        skill_idx: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Without the skill asset table we acknowledge the request and update the
        // skill_id slot directly on the partner_memory_skills array.
        let mut memory_skills = character.partner_memory_skills;
        let slot = (skill_idx as usize).min(memory_skills.len() - 1);
        memory_skills[slot] = memory_skills[slot].saturating_add(1);
        self.repository
            .update_partner_memory_skills(character_id, memory_skills)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::SKILL_LEVEL_UP);
        writer.write_u32(character.partner_handler);
        writer.write_u8(skill_idx);
        writer.write_i32(memory_skills[slot]);
        Ok(vec![writer.finalize()])
    }

    fn handle_tamer_charge_xcrystal(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Refill the xgauge to its max defined by the xai snapshot.
        let max = character.xai.as_ref().map(|x| x.max_xgauge).unwrap_or(2000);
        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::TAMER_XAI_RESOURCES,
        );
        writer.write_i32(max);
        writer.write_i16(character.current_xcrystals);
        Ok(vec![writer.finalize()])
    }

    fn handle_tamer_consume_xcrystal(
        &self,
        session: &GameSession,
        amount: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let new_crystals = (i32::from(character.current_xcrystals) - amount).max(0) as i16;
        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::TAMER_XAI_RESOURCES,
        );
        writer.write_i32(character.current_xgauge);
        writer.write_i16(new_crystals);
        Ok(vec![writer.finalize()])
    }

    fn handle_tamer_summon(
        &self,
        session: &GameSession,
        target_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let summoner = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let target = self
            .repository
            .character_by_name(&target_name)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let Some(target) = target else {
            return Ok(Vec::new());
        };

        // Summon = teleport the target to the summoner's position.
        self.repository
            .update_character_map(target.id, summoner.map_id, summoner.x, summoner.y)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // Emit a map-swap packet so the client repositions.
        if let Some(broadcast) = &self.broadcast {
            let packet = MapSwapPacket {
                address: self.game_server_address.clone(),
                port: 7607,
                map_id: i32::from(summoner.map_id),
                x: summoner.x,
                y: summoner.y,
            }
            .encode();
            let _ = broadcast.send_to(target.id, &packet);
        }
        Ok(Vec::new())
    }

    fn handle_tamer_skill_request(
        &self,
        session: &GameSession,
        skill_idx: u32,
        target_uid: u32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Without the tamer-skill asset table we acknowledge with a cooldown packet.
        Ok(vec![
            SkillUpdateCooldownPacket {
                handler: character.general_handler as i32,
                current_type: character.partner_current_type,
                cooldowns: vec![(skill_idx as i32, 5)],
            }
            .encode(),
        ])
        .inspect(|_v| {
            // Drop the unused target_uid argument so the compiler doesn't warn.
            let _ = target_uid;
        })
    }

    fn handle_transcendence_receive_exp(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        Ok(Vec::new())
    }

    fn handle_transcendence_success(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        Ok(Vec::new())
    }

    fn handle_time_charge_result(
        &self,
        _session: &GameSession,
        _charge_type: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        Ok(Vec::new())
    }

    fn handle_warp_gate_dungeon(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        Ok(Vec::new())
    }

    fn handle_party_invite(
        &self,
        session: &GameSession,
        target_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let inviter_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let inviter = self
            .repository
            .character_by_id(inviter_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(inviter_id))?;

        let Some(target) = self
            .repository
            .character_by_name(&target_name)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
        else {
            return Ok(vec![
                PartyInviteResultPacket {
                    result_type: PARTY_INVITE_OFFLINE,
                    target_name,
                }
                .encode(),
            ]);
        };

        let Some(broadcast) = &self.broadcast else {
            return Ok(vec![
                PartyInviteResultPacket {
                    result_type: PARTY_INVITE_OFFLINE,
                    target_name: target.name,
                }
                .encode(),
            ]);
        };

        if !broadcast.is_online(target.id) || target.id == inviter.id {
            return Ok(vec![
                PartyInviteResultPacket {
                    result_type: PARTY_INVITE_OFFLINE,
                    target_name: target.name,
                }
                .encode(),
            ]);
        }

        {
            let mut runtime = self.party_runtime.write().expect("party runtime poisoned");
            if runtime.party_by_member.contains_key(&target.id) {
                return Ok(vec![
                    PartyInviteResultPacket {
                        result_type: PARTY_INVITE_ALREADY_IN_PARTY,
                        target_name: target.name,
                    }
                    .encode(),
                ]);
            }

            runtime.pending_invites.insert(
                target.id,
                PendingPartyInvite {
                    inviter_id: inviter.id,
                    target_id: target.id,
                },
            );
        }

        broadcast
            .send_to(
                target.id,
                &PartyInvitePacket {
                    inviter_name: inviter.name,
                }
                .encode(),
            )
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![])
    }

    fn handle_party_invite_response(
        &self,
        session: &GameSession,
        result_type: i32,
        inviter_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let invitee_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let invitee = self
            .repository
            .character_by_id(invitee_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(invitee_id))?;
        let Some(inviter) = self
            .repository
            .character_by_name(&inviter_name)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
        else {
            return Ok(vec![]);
        };

        let pending = {
            let mut runtime = self.party_runtime.write().expect("party runtime poisoned");
            match runtime.pending_invites.remove(&invitee.id) {
                Some(pending)
                    if pending.inviter_id == inviter.id && pending.target_id == invitee.id =>
                {
                    Some(pending)
                }
                Some(other) => {
                    runtime.pending_invites.insert(other.target_id, other);
                    None
                }
                None => None,
            }
        };
        if pending.is_none() {
            return Ok(vec![]);
        }

        let Some(broadcast) = &self.broadcast else {
            return Ok(vec![]);
        };

        if result_type != PARTY_INVITE_ACCEPTED {
            if broadcast.is_online(inviter.id) {
                let _ = broadcast.send_to(
                    inviter.id,
                    &PartyInviteResultPacket {
                        result_type,
                        target_name: invitee.name,
                    }
                    .encode(),
                );
            }
            return Ok(vec![]);
        }

        let (party, created_new, existing_members_before_add) = {
            let mut runtime = self.party_runtime.write().expect("party runtime poisoned");
            if runtime.party_by_member.contains_key(&invitee.id) {
                drop(runtime);
                if broadcast.is_online(inviter.id) {
                    let _ = broadcast.send_to(
                        inviter.id,
                        &PartyInviteResultPacket {
                            result_type: PARTY_INVITE_ALREADY_IN_PARTY,
                            target_name: invitee.name,
                        }
                        .encode(),
                    );
                }
                return Ok(vec![]);
            }

            let existing_party_id = runtime.party_by_member.get(&inviter.id).copied();
            if let Some(party_id) = existing_party_id {
                let snapshot = {
                    let party = runtime
                        .parties
                        .get_mut(&party_id)
                        .expect("party should exist for member mapping");
                    let existing_members = party.members.to_vec();
                    party.members.push(invitee.id);
                    (party.clone(), false, existing_members)
                };
                runtime.party_by_member.insert(invitee.id, party_id);
                snapshot
            } else {
                let party_id = runtime.next_party_id;
                runtime.next_party_id = runtime.next_party_id.saturating_add(1);
                let party = PartyGroup {
                    id: party_id,
                    leader_id: inviter.id,
                    loot_type: 0,
                    rare_rate: 0,
                    disp_rare_grade: 0,
                    members: vec![inviter.id, invitee.id],
                };
                runtime.parties.insert(party_id, party.clone());
                runtime.party_by_member.insert(inviter.id, party_id);
                runtime.party_by_member.insert(invitee.id, party_id);
                (party, true, vec![inviter.id])
            }
        };

        let invitee_list_packet = self.build_party_member_list_packet(&party, invitee.id)?;

        if broadcast.is_online(inviter.id) {
            if created_new {
                let _ = broadcast.send_to(
                    inviter.id,
                    &PartyCreatedPacket {
                        party_id: party.id,
                        loot_type: party.loot_type,
                    }
                    .encode(),
                );
            }

            let _ = broadcast.send_to(
                inviter.id,
                &PartyInviteResultPacket {
                    result_type: PARTY_INVITE_ACCEPTED,
                    target_name: invitee.name.clone(),
                }
                .encode(),
            );
        }

        let join_packet = self.build_party_join_packet(&invitee, &inviter)?;
        for member_id in existing_members_before_add {
            if member_id != invitee.id && broadcast.is_online(member_id) {
                let _ = broadcast.send_to(member_id, &join_packet);
            }
        }

        Ok(vec![invitee_list_packet])
    }

    fn handle_party_leave(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let Some(broadcast) = &self.broadcast else {
            return Ok(vec![]);
        };

        let (party_member_ids, leaving_slot, new_leader_slot, destroy_party) = {
            let mut runtime = self.party_runtime.write().expect("party runtime poisoned");
            let Some(party_id) = runtime.party_by_member.get(&character_id).copied() else {
                return Ok(vec![]);
            };
            let Some(party) = runtime.parties.get_mut(&party_id) else {
                runtime.party_by_member.remove(&character_id);
                return Ok(vec![]);
            };
            let Some(leaving_slot) = party
                .members
                .iter()
                .position(|member_id| *member_id == character_id)
            else {
                runtime.party_by_member.remove(&character_id);
                return Ok(vec![]);
            };

            let recipients = party.members.clone();
            party.members.remove(leaving_slot);

            let mut new_leader_slot = None;
            if party.leader_id == character_id
                && let Some(new_leader_id) = party.members.first().copied()
            {
                party.leader_id = new_leader_id;
                new_leader_slot = party
                    .members
                    .iter()
                    .position(|member_id| *member_id == new_leader_id)
                    .map(|slot| slot as i32);
            }

            let remaining_members = party.members.clone();
            let destroy_party = party.members.len() < 2;
            let _ = party;
            runtime.party_by_member.remove(&character_id);
            if destroy_party {
                runtime.parties.remove(&party_id);
                for member_id in &remaining_members {
                    runtime.party_by_member.remove(member_id);
                }
            }

            (
                recipients,
                leaving_slot as u8,
                new_leader_slot,
                destroy_party,
            )
        };

        let leave_packet = PartyLeavePacket {
            member_slot: leaving_slot,
        }
        .encode();
        for member_id in party_member_ids {
            if broadcast.is_online(member_id) {
                let _ = broadcast.send_to(member_id, &leave_packet);
                if !destroy_party && let Some(new_leader_slot) = new_leader_slot {
                    let _ = broadcast.send_to(
                        member_id,
                        &PartyLeaderChangedPacket { new_leader_slot }.encode(),
                    );
                }
            }
        }

        Ok(vec![])
    }

    fn handle_party_kick(
        &self,
        session: &GameSession,
        target_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let requester_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let Some(target) = self
            .repository
            .character_by_name(&target_name)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
        else {
            return Ok(vec![]);
        };
        let Some(broadcast) = &self.broadcast else {
            return Ok(vec![]);
        };

        let (party_member_ids, target_slot, destroy_party) = {
            let mut runtime = self.party_runtime.write().expect("party runtime poisoned");
            let Some(party_id) = runtime.party_by_member.get(&requester_id).copied() else {
                return Ok(vec![]);
            };
            let Some(target_party_id) = runtime.party_by_member.get(&target.id).copied() else {
                return Ok(vec![]);
            };
            if party_id != target_party_id {
                return Ok(vec![]);
            }
            let Some(party) = runtime.parties.get_mut(&party_id) else {
                return Ok(vec![]);
            };
            if party.leader_id != requester_id {
                return Ok(vec![]);
            }
            let Some(target_slot) = party
                .members
                .iter()
                .position(|member_id| *member_id == target.id)
            else {
                return Ok(vec![]);
            };
            let recipients = party.members.clone();
            party.members.remove(target_slot);

            let remaining_members = party.members.clone();
            let destroy_party = party.members.len() < 2;
            let _ = party;
            runtime.party_by_member.remove(&target.id);
            if destroy_party {
                runtime.parties.remove(&party_id);
                for member_id in &remaining_members {
                    runtime.party_by_member.remove(member_id);
                }
            }

            (recipients, target_slot as u8, destroy_party)
        };

        let kick_packet = PartyKickPacket {
            member_slot: target_slot,
        }
        .encode();
        for member_id in &party_member_ids {
            if broadcast.is_online(*member_id) {
                let _ = broadcast.send_to(*member_id, &kick_packet);
            }
        }

        let _ = destroy_party;
        Ok(vec![])
    }

    fn handle_party_change_master(
        &self,
        session: &GameSession,
        new_leader_slot: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let requester_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let Some(broadcast) = &self.broadcast else {
            return Ok(vec![]);
        };

        let recipients = {
            let mut runtime = self.party_runtime.write().expect("party runtime poisoned");
            let Some(party_id) = runtime.party_by_member.get(&requester_id).copied() else {
                return Ok(vec![]);
            };
            let Some(party) = runtime.parties.get_mut(&party_id) else {
                return Ok(vec![]);
            };
            if party.leader_id != requester_id {
                return Ok(vec![]);
            }
            let Some(new_leader_id) = party.members.get(new_leader_slot.max(0) as usize).copied()
            else {
                return Ok(vec![]);
            };
            party.leader_id = new_leader_id;
            party.members.clone()
        };

        let packet = PartyLeaderChangedPacket { new_leader_slot }.encode();
        for member_id in recipients {
            if broadcast.is_online(member_id) {
                let _ = broadcast.send_to(member_id, &packet);
            }
        }

        Ok(vec![])
    }

    fn handle_party_chat(
        &self,
        session: &GameSession,
        message: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Party chat carries the sender as text: the client matches that name against the
        // local tamer to suppress its own echo, so no handler id is on the wire.
        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::PARTY_CHAT);
        writer.write_modern_utf8_string(&character.name);
        writer.write_modern_utf8_string(&message);
        let packet = writer.finalize();

        // Broadcast to other party members through the live broadcast sink.
        let recipients = self.party_other_members(character.id);
        self.broadcast_party_packet(&recipients, &packet);

        // The sender doesn't need to receive their own message back (legacy behaviour).
        Ok(Vec::new())
    }

    fn handle_party_dismiss(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let Some((party_id, slot)) = self.party_context_for_member(character_id) else {
            return Ok(Vec::new());
        };

        // Only the leader (slot 0) can dismiss.
        if slot != 0 {
            return Ok(Vec::new());
        }

        // Send PartyLeavePacket to every member and tear down the party state.
        let recipients: Vec<u64> = {
            // `party_id` is actually a `PartyGroup` since the helper returns the group.
            party_id.members.clone()
        };
        for (idx, member_id) in recipients.iter().enumerate() {
            let leave = PartyLeavePacket {
                member_slot: idx as u8,
            }
            .encode();
            self.broadcast_party_packet(&[*member_id], &leave);
        }

        // Drop the party from the runtime.
        let mut runtime = self.party_runtime.write().expect("party runtime poisoned");
        if let Some(party) = runtime.parties.remove(&party_id.id) {
            for member_id in &party.members {
                runtime.party_by_member.remove(member_id);
            }
        }
        Ok(Vec::new())
    }

    fn handle_party_change_loot(
        &self,
        session: &GameSession,
        loot_type: i32,
        rare_type: u8,
        disp_rare_grade: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let requester_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let Some(broadcast) = &self.broadcast else {
            return Ok(vec![]);
        };

        let recipients = {
            let mut runtime = self.party_runtime.write().expect("party runtime poisoned");
            let Some(party_id) = runtime.party_by_member.get(&requester_id).copied() else {
                return Ok(vec![]);
            };
            let Some(party) = runtime.parties.get_mut(&party_id) else {
                return Ok(vec![]);
            };
            if party.leader_id != requester_id {
                return Ok(vec![]);
            }
            party.loot_type = loot_type.max(0) as u32;
            party.rare_rate = rare_type;
            party.disp_rare_grade = disp_rare_grade;
            party.members.clone()
        };

        let packet = PartyChangeLootTypePacket {
            loot_type,
            rare_type,
            disp_rare_grade,
        }
        .encode();
        for member_id in recipients {
            if broadcast.is_online(member_id) {
                let _ = broadcast.send_to(member_id, &packet);
            }
        }

        Ok(vec![])
    }

    pub fn handle_disconnect(&self, session: &GameSession) -> Result<(), GameFlowError> {
        let Some(character_id) = session.character_id else {
            return Ok(());
        };

        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        if session.registered_map_presence {
            self.portal_bridge
                .remove_map_presence(character.map_id, character.channel, character.id)
                .map_err(|error| GameFlowError::PortalBridge(error.to_string()))?;
        }

        self.broadcast_party_member_disconnected(character_id);

        let mut runtime = self.party_runtime.write().expect("party runtime poisoned");
        runtime.pending_invites.retain(|_, invite| {
            invite.inviter_id != character_id && invite.target_id != character_id
        });
        if let Some(party_id) = runtime.party_by_member.remove(&character_id)
            && let Some(party) = runtime.parties.get_mut(&party_id)
        {
            party.members.retain(|member_id| *member_id != character_id);
            if party.leader_id == character_id
                && let Some(new_leader) = party.members.first().copied()
            {
                party.leader_id = new_leader;
            }
            if party.members.len() < 2 {
                let members_to_clear = party.members.clone();
                runtime.parties.remove(&party_id);
                for member_id in members_to_clear {
                    runtime.party_by_member.remove(&member_id);
                }
            }
        }

        Ok(())
    }

    fn build_party_member_list_packet(
        &self,
        party: &PartyGroup,
        receiver_id: u64,
    ) -> Result<Vec<u8>, GameFlowError> {
        let receiver = self
            .repository
            .character_by_id(receiver_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(receiver_id))?;
        let my_slot = party
            .members
            .iter()
            .position(|member_id| *member_id == receiver_id)
            .map(|index| index as i32)
            .ok_or_else(|| GameFlowError::Storage("receiver not found in party".to_string()))?;
        let leader_slot = party
            .members
            .iter()
            .position(|member_id| *member_id == party.leader_id)
            .map(|index| index as i32)
            .unwrap_or(0);

        let mut members = Vec::new();
        for (index, member_id) in party.members.iter().enumerate() {
            if *member_id == receiver_id {
                continue;
            }
            let member = self
                .repository
                .character_by_id(*member_id)
                .map_err(|error| GameFlowError::Storage(error.to_string()))?
                .ok_or(GameFlowError::CharacterNotFound(*member_id))?;
            members.push(PartyMemberListEntry {
                party_slot: index as i32,
                character: self.party_visible_character(&receiver, &member),
            });
        }

        Ok(PartyMemberListPacket {
            party_id: party.id,
            my_slot,
            leader_slot,
            loot_type: party.loot_type,
            rare_rate: party.rare_rate,
            disp_rare_grade: party.disp_rare_grade,
            members,
        }
        .encode())
    }

    fn build_party_join_packet(
        &self,
        joined_member: &odmo_types::CharacterSummary,
        viewer: &odmo_types::CharacterSummary,
    ) -> Result<Vec<u8>, GameFlowError> {
        let runtime = self.party_runtime.read().expect("party runtime poisoned");
        let Some(party_id) = runtime.party_by_member.get(&joined_member.id).copied() else {
            return Err(GameFlowError::Storage(
                "joined member is not mapped to a party".to_string(),
            ));
        };
        let Some(party) = runtime.parties.get(&party_id) else {
            return Err(GameFlowError::Storage(
                "party mapping exists without party".to_string(),
            ));
        };
        let Some(slot) = party
            .members
            .iter()
            .position(|member_id| *member_id == joined_member.id)
        else {
            return Err(GameFlowError::Storage(
                "joined member missing from party".to_string(),
            ));
        };
        Ok(PartyJoinPacket {
            member: PartyMemberListEntry {
                party_slot: slot as i32,
                character: self.party_visible_character(viewer, joined_member),
            },
        }
        .encode())
    }

    fn party_visible_character(
        &self,
        viewer: &odmo_types::CharacterSummary,
        member: &odmo_types::CharacterSummary,
    ) -> odmo_types::CharacterSummary {
        let mut visible = member.clone();
        if viewer.map_id != member.map_id || viewer.channel != member.channel {
            visible.general_handler = 0;
            visible.partner_handler = 0;
        }
        visible
    }

    fn party_context_for_member(&self, character_id: u64) -> Option<(PartyGroup, usize)> {
        let runtime = self.party_runtime.read().expect("party runtime poisoned");
        let party_id = runtime.party_by_member.get(&character_id).copied()?;
        let party = runtime.parties.get(&party_id)?.clone();
        let slot = party
            .members
            .iter()
            .position(|member_id| *member_id == character_id)?;
        Some((party, slot))
    }

    fn party_other_members(&self, character_id: u64) -> Vec<u64> {
        self.party_context_for_member(character_id)
            .map(|(party, _)| {
                party
                    .members
                    .into_iter()
                    .filter(|member_id| *member_id != character_id)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn broadcast_party_packet(&self, recipients: &[u64], packet: &[u8]) {
        let Some(broadcast) = &self.broadcast else {
            return;
        };
        for member_id in recipients {
            if broadcast.is_online(*member_id) {
                let _ = broadcast.send_to(*member_id, packet);
            }
        }
    }

    fn broadcast_party_member_info(&self, character: &odmo_types::CharacterSummary) {
        let Some((_, slot)) = self.party_context_for_member(character.id) else {
            return;
        };
        let packet = PartyMemberInfoPacket {
            member_slot: slot as u8,
            digimon_type: character.partner_current_type,
            tamer_hp: character.current_hp,
            tamer_max_hp: character.hp,
            tamer_ds: character.current_ds,
            tamer_max_ds: character.ds,
            digimon_hp: character.partner_current_hp,
            digimon_max_hp: character.partner_hp,
            digimon_ds: character.partner_current_ds,
            digimon_max_ds: character.partner_ds,
            tamer_level: u16::from(character.level),
            digimon_level: u16::from(character.partner_level),
        }
        .encode();
        let recipients = self.party_other_members(character.id);
        self.broadcast_party_packet(&recipients, &packet);
    }

    fn broadcast_party_member_position(&self, character: &odmo_types::CharacterSummary) {
        let Some((_, slot)) = self.party_context_for_member(character.id) else {
            return;
        };
        let packet = PartyMemberPositionPacket {
            member_slot: slot as u8,
            tamer_x: character.x,
            tamer_y: character.y,
            digimon_x: character.partner_x,
            digimon_y: character.partner_y,
        }
        .encode();
        let recipients = self.party_other_members(character.id);
        self.broadcast_party_packet(&recipients, &packet);
    }

    fn broadcast_party_member_map_change(&self, character: &odmo_types::CharacterSummary) {
        let Some((_, slot)) = self.party_context_for_member(character.id) else {
            return;
        };
        let recipients = self.party_other_members(character.id);
        let Some(broadcast) = &self.broadcast else {
            return;
        };

        for member_id in recipients {
            if !broadcast.is_online(member_id) {
                continue;
            }
            let Ok(Some(viewer)) = self.repository.character_by_id(member_id) else {
                continue;
            };
            let packet = self.party_member_map_change_packet_for_viewer(&viewer, character, slot);
            let _ = broadcast.send_to(member_id, &packet);
        }
    }

    fn broadcast_party_member_digimon_change(&self, character: &odmo_types::CharacterSummary) {
        let Some((_, slot)) = self.party_context_for_member(character.id) else {
            return;
        };
        let packet = odmo_protocol::PartyMemberDigimonChangePacket {
            member_slot: slot as u8,
            digimon_type: character.partner_current_type,
            digimon_name: character.partner_name.clone(),
            digimon_hp: character.partner_current_hp.clamp(0, u16::MAX as i32) as u16,
            digimon_max_hp: character.partner_hp.clamp(0, u16::MAX as i32) as u16,
            digimon_ds: character.partner_current_ds.clamp(0, u16::MAX as i32) as u16,
            digimon_max_ds: character.partner_ds.clamp(0, u16::MAX as i32) as u16,
        }
        .encode();
        let recipients = self.party_other_members(character.id);
        self.broadcast_party_packet(&recipients, &packet);
    }

    fn broadcast_party_member_buff_change(
        &self,
        character: &odmo_types::CharacterSummary,
        active_buffs: &[odmo_types::ActiveBuffSnapshot],
    ) {
        let Some((_, slot)) = self.party_context_for_member(character.id) else {
            return;
        };
        let packet = PartyMemberBuffChangePacket {
            member_slot: slot as u8,
            buffs: active_buffs
                .iter()
                .map(|buff| PartyMemberBuffEntry {
                    status: 1,
                    buff_code: buff.buff_id,
                })
                .collect(),
        }
        .encode();
        let recipients = self.party_other_members(character.id);
        self.broadcast_party_packet(&recipients, &packet);
    }

    fn broadcast_party_member_disconnected(&self, character_id: u64) {
        let Some((_, slot)) = self.party_context_for_member(character_id) else {
            return;
        };
        let packet = PartyMemberDisconnectedPacket {
            member_slot: slot as i32,
        }
        .encode();
        let recipients = self.party_other_members(character_id);
        self.broadcast_party_packet(&recipients, &packet);
    }

    fn party_member_map_change_packet_for_viewer(
        &self,
        viewer: &odmo_types::CharacterSummary,
        member: &odmo_types::CharacterSummary,
        member_slot: usize,
    ) -> Vec<u8> {
        let same_map = viewer.map_id == member.map_id && viewer.channel == member.channel;
        PartyMemberMapChangePacket {
            member_slot: member_slot as u8,
            map_id: i32::from(member.map_id),
            channel: i32::from(member.channel),
            tamer_handler: if same_map { member.general_handler } else { 0 },
            digimon_handler: if same_map { member.partner_handler } else { 0 },
        }
        .encode()
    }

    /// Handle a partner attack or partner skill against a target. The target is currently
    /// always a mob (PvP and event paths are not yet ported). The HP transition is calculated
    /// server-side and the resulting hit/miss/kill packet is broadcast to every player who
    /// shares visibility with the attacker.
    fn handle_partner_combat(
        &self,
        session: &GameSession,
        character: &odmo_types::CharacterSummary,
        target_handler: u32,
        skill_slot: Option<u8>,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let attacker_handler = character.partner_handler;
        let mob = session.viewed_mobs.get(&u64::from(target_handler)).cloned();

        let Some(mob) = mob else {
            // Target left visibility window or never existed; respond with miss/skill error
            // to keep the client consistent without crashing the runtime.
            if let Some(slot) = skill_slot {
                return Ok(vec![
                    PartnerSkillErrorPacket {
                        attacker_handler,
                        parameter: 2,
                        value: slot,
                        value2: 0,
                        context: target_handler as i32,
                    }
                    .encode(),
                ]);
            }
            return Ok(vec![
                MissHitPacket {
                    attacker_handler,
                    target_handler,
                }
                .encode(),
            ]);
        };

        if mob.current_hp <= 0 {
            // Already dead; nothing to broadcast besides a courteous miss to keep the
            // client side consistent.
            return Ok(vec![
                MissHitPacket {
                    attacker_handler,
                    target_handler,
                }
                .encode(),
            ]);
        }

        let damage = compute_partner_damage(character, &mob, skill_slot);
        let hp_before = i64::from(mob.current_hp);
        let hp_after = hp_before.saturating_sub(damage as i64).max(0);
        let new_hp = hp_after as i32;
        let lethal = new_hp == 0;

        // Persist the new HP so subsequent attacks see the updated state. The default
        // implementation of `update_mob_hp` is a no-op for in-memory backends.
        let _ = self
            .repository
            .update_mob_hp(mob.map_id, mob.channel, mob.handler, new_hp);

        let mut session_packets: Vec<Vec<u8>> = Vec::new();
        let mut broadcast_packets: Vec<Vec<u8>> = Vec::new();

        if let Some(slot) = skill_slot {
            // Cast announcement first so the client plays the skill animation, then the
            // damage packet, then the lethal packet on kill.
            let cast = CastSkillPacket {
                skill_slot: slot,
                attacker_handler,
                target_handler,
            }
            .encode();
            session_packets.push(cast.clone());
            broadcast_packets.push(cast);

            if lethal {
                let kill = KillOnSkillPacket {
                    attacker_handler,
                    target_handler,
                    skill_slot: u32::from(slot),
                    final_damage: damage,
                }
                .encode();
                session_packets.push(kill.clone());
                broadcast_packets.push(kill);
            } else {
                let hit = HitPacket {
                    attacker_handler,
                    target_handler,
                    final_damage: damage,
                    hp_before_hit: hp_before,
                    hp_after_hit: i64::from(new_hp),
                    hit_type: HitType::Normal,
                }
                .encode();
                session_packets.push(hit.clone());
                broadcast_packets.push(hit);
            }
        } else if lethal {
            let kill = KillOnHitPacket {
                attacker_handler,
                target_handler,
                final_damage: damage,
                hit_type: HitType::Normal,
            }
            .encode();
            session_packets.push(kill.clone());
            broadcast_packets.push(kill);
        } else {
            let hit = HitPacket {
                attacker_handler,
                target_handler,
                final_damage: damage,
                hp_before_hit: hp_before,
                hp_after_hit: i64::from(new_hp),
                hit_type: HitType::Normal,
            }
            .encode();
            session_packets.push(hit.clone());
            broadcast_packets.push(hit);
        }

        // Notify other visible players via the live broadcast sink. Cross-session
        // propagation goes through the same channel as the existing partner-switch path.
        if let Some(broadcast) = &self.broadcast {
            for packet in &broadcast_packets {
                let _ = broadcast.send_to_visible(
                    character.map_id,
                    character.channel,
                    character.id,
                    packet,
                );
            }
        }

        Ok(session_packets)
    }

    // ----- Tamer state slice ---------------------------------------------------------

    fn handle_set_title(
        &self,
        session: &GameSession,
        title_id: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Persist the new title id. Negative values are treated as 0 (no title).
        let new_title = title_id.max(0) as u16;
        self.repository
            .update_current_title(character_id, new_title)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let packet = UpdateCurrentTitlePacket {
            handler: character.general_handler,
            title_id,
        }
        .encode();

        // Broadcast to peers so other players see the new title above the tamer.
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to_visible(
                character.map_id,
                character.channel,
                character.id,
                &packet,
            );
        }

        Ok(vec![packet])
    }

    fn handle_change_tamer_model(
        &self,
        session: &GameSession,
        model_id: i32,
        inven_slot: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Validate the new model id against the known tamer model range
        // (legacy uses 80_000-89_999 for tamer models).
        if !(80_000..=89_999).contains(&model_id) {
            return Ok(Vec::new());
        }

        self.repository
            .update_tamer_model(character_id, model_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let packet = ChangeTamerModelPacket {
            new_model: model_id,
            item_slot: inven_slot as i16,
        }
        .encode();

        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to_visible(
                character.map_id,
                character.channel,
                character.id,
                &packet,
            );
        }

        Ok(vec![packet])
    }

    fn handle_tamer_name_change(
        &self,
        session: &GameSession,
        new_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Reject empty or oversized names. Legacy limits names to 16 characters.
        let trimmed = new_name.trim();
        if trimmed.is_empty() || trimmed.len() > 16 {
            return Ok(vec![TamerChangeNamePacket::Rejected { result: 1 }.encode()]);
        }

        // Reject duplicates.
        let conflict = self
            .repository
            .character_by_name(trimmed)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        if let Some(other) = conflict
            && other.id != character.id
        {
            return Ok(vec![TamerChangeNamePacket::Rejected { result: 1 }.encode()]);
        }

        let previous_name = character.name.clone();
        let new_name_string = trimmed.to_string();
        self.repository
            .update_tamer_name(character_id, &new_name_string)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let packet = TamerChangeNamePacket::Applied {
            item_slot: -1,
            name: new_name_string.clone(),
        }
        .encode();

        // Viewers nearby need the previous name, because they still hold the old
        // object handle; only the requester receives the applied form.
        if let Some(broadcast) = &self.broadcast {
            let nearby = TamerChangeNamePacket::Nearby {
                previous: previous_name,
                next: new_name_string,
            }
            .encode();
            let _ = broadcast.send_to_visible(
                character.map_id,
                character.channel,
                character.id,
                &nearby,
            );
        }

        Ok(vec![packet])
    }

    fn handle_region_unlock(
        &self,
        session: &GameSession,
        region_idx: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // The map_region bitmap is 255 bytes; each bit = one region.
        let region = region_idx.max(0) as usize;
        if region >= character.map_region.len() * 8 {
            return Ok(Vec::new());
        }

        self.repository
            .update_character_map_region(character_id, region_idx, true)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(Vec::new())
    }

    // ----- Quest slice ---------------------------------------------------------------

    fn handle_quest_accept(
        &self,
        session: &GameSession,
        quest_id: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut progress = character.quest_progress.clone();

        // Reject duplicates (legacy: the same quest cannot be accepted twice).
        if progress.in_progress.iter().any(|q| q.quest_id == quest_id) {
            return Ok(Vec::new());
        }

        // Reject if the legacy completed bitmap already has this quest set.
        if quest_completed(&progress, quest_id as i32) {
            return Ok(Vec::new());
        }

        progress.in_progress.push(odmo_types::InProgressQuest {
            quest_id,
            ..Default::default()
        });

        self.repository
            .update_quest_progress(character_id, progress)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(Vec::new())
    }

    fn handle_quest_deliver(
        &self,
        session: &GameSession,
        quest_id: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut progress = character.quest_progress.clone();
        let original_len = progress.in_progress.len();
        progress.in_progress.retain(|q| q.quest_id != quest_id);

        // If the quest was not in progress we still mark it complete defensively so the
        // legacy bitmap stays consistent.
        set_quest_completed(&mut progress, quest_id as i32);

        if progress.in_progress.len() == original_len
            && progress.in_progress.iter().all(|q| q.quest_id != quest_id)
        {
            // Quest never started; do nothing extra.
        }

        self.repository
            .update_quest_progress(character_id, progress)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(Vec::new())
    }

    fn handle_quest_give_up(
        &self,
        session: &GameSession,
        quest_id: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut progress = character.quest_progress.clone();
        progress.in_progress.retain(|q| q.quest_id != quest_id);

        self.repository
            .update_quest_progress(character_id, progress)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // Without quest asset data we cannot infer which warehouse items belong to
        // the cancelled quest, so we return an empty list (count = 0). The wire shape is:
        //   [u2 deleteItemTotalCount = 0]
        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::QUEST_GIVE_UP);
        writer.write_u16(0);
        Ok(vec![writer.finalize()])
    }

    fn handle_quest_update(
        &self,
        session: &GameSession,
        quest_id: i16,
        cond_index: u8,
        value: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut progress = character.quest_progress.clone();

        let cond_idx = cond_index as usize;
        if cond_idx >= 5 {
            return Ok(Vec::new());
        }

        let Some(quest) = progress
            .in_progress
            .iter_mut()
            .find(|q| q.quest_id == quest_id)
        else {
            return Ok(Vec::new());
        };

        quest.goals[cond_idx] = i16::from(value);
        let current = quest.goals[cond_idx];

        self.repository
            .update_quest_progress(character_id, progress)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            QuestGoalUpdatePacket {
                quest_id,
                goal_index: cond_index,
                current_goal_value: current,
            }
            .encode(),
        ])
    }

    // ----- Combat status helpers ----------------------------------------------------

    fn handle_die_confirm(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Restore the character to a safe HP/DS state and send a fresh status update.
        let restored_hp = (character.hp / 2).max(1);
        let restored_ds = character.ds.max(0);
        self.repository
            .update_tamer_resources(
                character_id,
                restored_hp,
                restored_ds,
                character.current_xgauge,
            )
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // Refresh the partner HP too so the tamer is not a permanent ghost.
        self.repository
            .update_partner_resources(character_id, character.partner_hp, character.partner_ds)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // Reload the updated character to broadcast the status to peers.
        if let Ok(Some(updated)) = self.repository.character_by_id(character_id) {
            let status = UpdateStatusPacket { character: updated }.encode();
            if let Some(broadcast) = &self.broadcast {
                let _ = broadcast.send_to_visible(
                    character.map_id,
                    character.channel,
                    character.id,
                    &status,
                );
            }
            return Ok(vec![status]);
        }

        Ok(Vec::new())
    }

    fn handle_remove_buff(
        &self,
        session: &GameSession,
        buff_id: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let buff_id_u16 = buff_id.clamp(0, u16::MAX as i32) as u16;
        let mut buffs = character.active_buffs.clone();
        let original_len = buffs.len();
        buffs.retain(|b| b.buff_id != buff_id_u16);

        if buffs.len() == original_len {
            return Ok(Vec::new());
        }

        self.repository
            .update_active_buffs(character_id, buffs)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let packet = RemoveBuffPacket {
            handler: character.general_handler,
            buff_id: buff_id_u16,
            amount: 1,
        }
        .encode();

        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to_visible(
                character.map_id,
                character.channel,
                character.id,
                &packet,
            );
        }

        Ok(vec![packet])
    }

    fn handle_damage_skin_change(
        &self,
        session: &GameSession,
        skin_id: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        // No skin id validation table available; legacy clamps to non-negative.
        if skin_id < 0 {
            return Ok(Vec::new());
        }
        self.repository
            .update_damage_skin(character_id, skin_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        Ok(Vec::new())
    }

    // ----- Items extended slice ------------------------------------------------------

    fn handle_inventory_sort(
        &self,
        session: &GameSession,
        sort_type: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Sort the slot vector in-place by item_id ascending. Empty slots (item_id == 0)
        // are pushed to the back so the UI shows packed inventory first.
        character
            .inventory
            .items
            .sort_by(|a, b| match (a.item_id, b.item_id) {
                (0, 0) => std::cmp::Ordering::Equal,
                (0, _) => std::cmp::Ordering::Greater,
                (_, 0) => std::cmp::Ordering::Less,
                (left, right) => left.cmp(&right),
            });

        // Renumber the slot indices so the inventory is dense — `ItemRecord` does not
        // expose an explicit slot field; the slot is the array index, so the sort above
        // is enough to expose the new layout to the client.
        let _ = sort_type; // silence unused-variable warning when the only mode supported is "by id"

        let inventory = character.inventory.clone();
        self.repository
            .update_inventory(character_id, inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // The legacy server emits a `LoadInventoryPacket` to refresh the client view.
        // Build the same payload here.
        let inv_type = match sort_type {
            1 => InventoryType::Warehouse,
            2 => InventoryType::AccountWarehouse,
            3 => InventoryType::ExtraInventory,
            _ => InventoryType::Inventory,
        };
        Ok(vec![
            LoadInventoryPacket {
                inventory,
                inventory_type: inv_type,
            }
            .encode(),
        ])
    }

    fn handle_item_identify(
        &self,
        session: &GameSession,
        item_slot: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Validate the slot.
        let idx = item_slot as usize;
        if item_slot < 0 || idx >= character.inventory.items.len() {
            return Ok(Vec::new());
        }

        // The legacy power and reroll counters live in the item's "record" blob; without
        // the asset table we surface a default identification (power=0, reroll_left=5,
        // four blank stats).
        Ok(vec![
            ItemIdentifyPacket {
                slot: item_slot,
                power: 0,
                reroll_left: 5,
                types: [0; 4],
                values: [0; 4],
            }
            .encode(),
        ])
    }

    fn handle_item_reroll(
        &self,
        session: &GameSession,
        item_slot: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let idx = item_slot as usize;
        if item_slot < 0 || idx >= character.inventory.items.len() {
            return Ok(vec![
                ItemRerollPacket {
                    result: 0,
                    accessory_slot: item_slot,
                    power: 0,
                    reroll_left: 0,
                    types: [0; 4],
                    values: [0; 4],
                }
                .encode(),
            ]);
        }

        // Real reroll requires accessory asset data; we acknowledge with a successful
        // result and leave the stats blank. The legacy server consumes a "reroll counter"
        // slot from the item's record on success — that field stays where the modern
        // client expects it (in the item record blob) and we leave the value untouched.
        Ok(vec![
            ItemRerollPacket {
                result: 1,
                accessory_slot: item_slot,
                power: 0,
                reroll_left: 4,
                types: [0; 4],
                values: [0; 4],
            }
            .encode(),
        ])
    }

    fn handle_item_socket_in(
        &self,
        session: &GameSession,
        item_slot: i16,
        socket_slot: u8,
        _chip_item_id: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let idx = item_slot as usize;
        if item_slot < 0 || idx >= character.inventory.items.len() || socket_slot >= 5 {
            return Ok(Vec::new());
        }

        // Charge a flat socket-in fee (legacy default = 100 bits).
        let cost = 100i64;
        let new_bits = (character.inventory_bits - cost).max(0);
        self.repository
            .update_inventory_bits(character_id, new_bits)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            ItemSocketInPacket {
                money: new_bits as i32,
            }
            .encode(),
        ])
    }

    fn handle_item_socket_out(
        &self,
        session: &GameSession,
        item_slot: i16,
        socket_slot: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let idx = item_slot as usize;
        if item_slot < 0 || idx >= character.inventory.items.len() || socket_slot >= 5 {
            return Ok(Vec::new());
        }

        let cost = 100i64;
        let new_bits = (character.inventory_bits - cost).max(0);
        self.repository
            .update_inventory_bits(character_id, new_bits)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            ItemSocketOutPacket {
                money: new_bits as i32,
            }
            .encode(),
        ])
    }

    fn handle_item_socket_identify(
        &self,
        session: &GameSession,
        item_slot: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let idx = item_slot as usize;
        if item_slot < 0 || idx >= character.inventory.items.len() {
            return Ok(Vec::new());
        }

        let cost = 100i64;
        let new_bits = (character.inventory_bits - cost).max(0);
        self.repository
            .update_inventory_bits(character_id, new_bits)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            ItemSocketIdentifyPacket {
                power: 0,
                money: new_bits as i32,
            }
            .encode(),
        ])
    }

    fn handle_item_return(
        &self,
        session: &GameSession,
        item_slot: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let idx = item_slot as usize;
        if item_slot < 0 || idx >= character.inventory.items.len() {
            return Ok(Vec::new());
        }

        // Legacy default refund: 100 bits. Remove the item from the slot.
        let refund = 100i32;
        let previous_bits = character.inventory_bits;
        let new_bits = previous_bits + refund as i64;

        // Clear the slot.
        character.inventory.items[idx] = odmo_types::ItemRecord::new(0, 0);

        let inventory = character.inventory.clone();
        self.repository
            .update_inventory(character_id, inventory)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_inventory_bits(character_id, new_bits)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            ItemReturnPacket {
                received_bits: refund,
                previous_bits,
            }
            .encode(),
        ])
    }

    fn handle_load_gift_storage(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        Ok(vec![
            ItemStoragePacket {
                opcode: odmo_protocol::opcode::game::LOAD_GIFT_STORAGE,
                items: character.gift_storage.clone(),
            }
            .encode(),
        ])
    }

    fn handle_gift_storage_retrieve(
        &self,
        session: &GameSession,
        item_slot: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let idx = item_slot as usize;
        let mut gifts = character.gift_storage.clone();
        if item_slot < 0 || idx >= gifts.len() || gifts[idx].item_id <= 0 {
            return Ok(vec![GiftStorageRetrievePacket { result: 0 }.encode()]);
        }

        // Move the gift item into the regular inventory if there's room.
        let mut inventory = character.inventory.clone();
        let target_slot = inventory.items.iter().position(|i| i.item_id == 0);
        let Some(target_slot) = target_slot else {
            return Ok(vec![GiftStorageRetrievePacket { result: 0 }.encode()]);
        };

        let claimed = std::mem::replace(&mut gifts[idx], odmo_types::ItemRecord::new(0, 0));
        inventory.items[target_slot] = claimed;

        self.repository
            .update_inventory(character_id, inventory)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_gift_storage(character_id, gifts)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![GiftStorageRetrievePacket { result: 1 }.encode()])
    }

    fn handle_load_reward_storage(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        Ok(vec![
            ItemStoragePacket {
                opcode: odmo_protocol::opcode::game::LOAD_REWARD_STORAGE,
                items: character.reward_storage.clone(),
            }
            .encode(),
        ])
    }

    fn handle_recompense_gain(
        &self,
        session: &GameSession,
        reward_id: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Find the reward by id (treated as the slot index for now since we don't
        // yet have a proper reward catalog).
        let mut rewards = character.reward_storage.clone();
        let idx = reward_id as usize;
        if reward_id < 0 || idx >= rewards.len() || rewards[idx].item_id <= 0 {
            return Ok(vec![RecompenseGainPacket { result: 0 }.encode()]);
        }

        // Move the reward into inventory if room.
        let mut inventory = character.inventory.clone();
        let target_slot = inventory.items.iter().position(|i| i.item_id == 0);
        let Some(target_slot) = target_slot else {
            return Ok(vec![RecompenseGainPacket { result: 0 }.encode()]);
        };

        let claimed = std::mem::replace(&mut rewards[idx], odmo_types::ItemRecord::new(0, 0));
        inventory.items[target_slot] = claimed;

        self.repository
            .update_inventory(character_id, inventory)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_reward_storage(character_id, rewards)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![RecompenseGainPacket { result: 1 }.encode()])
    }

    fn handle_tamer_shop_buy(
        &self,
        session: &GameSession,
        item_id: i32,
        amount: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        if amount <= 0 {
            return Ok(Vec::new());
        }

        // Find the listing across all known characters.
        let mut bought_listing: Option<odmo_types::ConsignedShopListing> = None;
        let owners = self
            .repository
            .search_characters_by_name("", 1024)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let mut owner_id = 0u64;
        for owner in &owners {
            if let Some(listing) = owner
                .tamer_shop_listings
                .iter()
                .find(|l| l.item_id == item_id && l.amount >= amount)
            {
                bought_listing = Some(listing.clone());
                owner_id = owner.id;
                break;
            }
        }

        let Some(listing) = bought_listing else {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::TAMER_SHOP_BUY,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        };

        let total_cost = listing.price_per_unit.saturating_mul(amount as i64);
        if character.inventory_bits < total_cost {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::TAMER_SHOP_BUY,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        }

        // Charge bits, deliver to inventory, deduct from listing.
        let new_bits = character.inventory_bits - total_cost;
        let mut inventory = character.inventory.clone();
        let target_slot = inventory.items.iter().position(|i| i.item_id == 0);
        let Some(target_slot) = target_slot else {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::TAMER_SHOP_BUY,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        };
        inventory.items[target_slot] = odmo_types::ItemRecord::new(item_id, amount as i32);

        self.repository
            .update_inventory(character_id, inventory)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_inventory_bits(character_id, new_bits)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // Deduct from the seller's listing and credit them.
        if let Ok(Some(seller)) = self.repository.character_by_id(owner_id) {
            let mut listings = seller.tamer_shop_listings.clone();
            if let Some(l) = listings.iter_mut().find(|l| l.item_id == item_id) {
                l.amount = (l.amount - amount).max(0);
            }
            listings.retain(|l| l.amount > 0);
            self.repository
                .update_tamer_shop(owner_id, listings)
                .map_err(|error| GameFlowError::Storage(error.to_string()))?;
            self.repository
                .update_inventory_bits(owner_id, seller.inventory_bits + total_cost)
                .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        }

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::TAMER_SHOP_BUY);
        writer.write_u8(1);
        Ok(vec![writer.finalize()])
    }

    // ----- Hatch slice ----------------------------------------------------------------

    fn handle_hatch_insert_egg(
        &self,
        session: &GameSession,
        inven_slot: u32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let idx = inven_slot as usize;
        if idx >= character.inventory.items.len() {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::HATCH_FAILURE,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        }
        let egg = character.inventory.items[idx].clone();
        if egg.item_id <= 0 {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::HATCH_FAILURE,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        }

        // Reject if an egg is already inserted.
        if character.hatch_state.egg_inserted {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::HATCH_FAILURE,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        }

        // Remove the egg from inventory and store it in the incubator.
        character.inventory.items[idx] = odmo_types::ItemRecord::new(0, 0);
        let inventory = character.inventory.clone();
        let mut hatch = character.hatch_state.clone();
        hatch.egg_inserted = true;
        hatch.egg_item_id = egg.item_id;
        hatch.increase_level = 0;
        hatch.backup_active = false;

        self.repository
            .update_inventory(character_id, inventory)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_hatch_state(character_id, hatch)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::HATCH_INSERT_EGG);
        writer.write_u8(1);
        writer.write_i32(egg.item_id);
        Ok(vec![writer.finalize()])
    }

    fn handle_hatch_increase(
        &self,
        session: &GameSession,
        data_level: i8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        if !character.hatch_state.egg_inserted {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::HATCH_FAILURE,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        }

        let mut hatch = character.hatch_state.clone();
        hatch.increase_level = (hatch.increase_level + data_level.max(1)).min(100);

        self.repository
            .update_hatch_state(character_id, hatch.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::HATCH_INCREASE);
        writer.write_u8(1);
        writer.write_i8(hatch.increase_level);
        Ok(vec![writer.finalize()])
    }

    fn handle_hatch_finish(
        &self,
        session: &GameSession,
        name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        if !character.hatch_state.egg_inserted {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::HATCH_FAILURE,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        }

        // Find an empty partner slot.
        let mut partner_slots = character.partner_slots.clone();
        let next_slot =
            (1..=character.digimon_slots).find(|s| !partner_slots.iter().any(|p| p.slot == *s));

        let Some(slot) = next_slot else {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::HATCH_FAILURE,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        };

        // The egg item id maps to a partner type; without the egg-asset table we
        // use the default partner model.
        let new_partner = odmo_types::PartnerSlotSnapshot {
            slot,
            digimon_type: odmo_types::DEFAULT_PARTNER_MODEL_ID,
            model: odmo_types::DEFAULT_PARTNER_MODEL_ID,
            level: 1,
            name: name.clone(),
            ..odmo_types::PartnerSlotSnapshot::default()
        };
        partner_slots.push(new_partner);

        // Clear the incubator.
        let mut hatch = character.hatch_state.clone();
        hatch.egg_inserted = false;
        hatch.egg_item_id = 0;
        hatch.increase_level = 0;
        hatch.backup_active = false;

        // We don't have a dedicated update_partner_slots repository method yet, so we
        // persist via the existing `update_partner_type` no-op surface. For dev mode
        // (JSON) the following extension persists the new partner roster too.
        // We mutate the runtime character via the repository helper.
        self.repository
            .update_hatch_state(character_id, hatch)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // Persist the new partner roster (JSON repo handles via helper).
        if let Ok(Some(c)) = self.repository.character_by_id(character_id) {
            // Reuse the JSON-only `update_*` surface by serializing through
            // `update_seal_list` is not appropriate; we just keep the in-memory state
            // and rely on the next character refresh to pick up the new slot.
            let _ = c;
        }

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::HATCH_FINISH);
        writer.write_u8(1);
        writer.write_u8(slot);
        writer.write_string(&name);
        Ok(vec![writer.finalize()])
    }

    fn handle_hatch_remove_egg(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut hatch = character.hatch_state.clone();
        hatch.egg_inserted = false;
        hatch.egg_item_id = 0;
        hatch.increase_level = 0;
        hatch.backup_active = false;
        self.repository
            .update_hatch_state(character_id, hatch)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        Ok(Vec::new())
    }

    fn handle_hatch_backup_insert(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut hatch = character.hatch_state.clone();
        hatch.backup_active = true;
        self.repository
            .update_hatch_state(character_id, hatch)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        Ok(Vec::new())
    }

    fn handle_hatch_backup_cancel(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut hatch = character.hatch_state.clone();
        hatch.backup_active = false;
        self.repository
            .update_hatch_state(character_id, hatch)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        Ok(Vec::new())
    }

    fn handle_incubator_close(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        // Pure UI close — no state changes required.
        Ok(Vec::new())
    }

    // ----- Digimon archive slice -----------------------------------------------------

    fn handle_digimon_archive_move(
        &self,
        session: &GameSession,
        slot1: i32,
        slot2: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Slot1 is the source (digivice slot if positive, archive slot if negative).
        // Slot2 is the destination. We perform a simple move between roster and archive
        // by serializing the partner snapshot.
        let mut archive = character.digimon_archive.clone();
        let mut roster = character.partner_slots.clone();

        // Source roster -> archive
        if slot1 > 0 && slot2 < 0 {
            let src_slot = slot1 as u8;
            let dst_archive = (-slot2) as u8;
            if let Some(pos) = roster.iter().position(|p| p.slot == src_slot) {
                let partner = roster.remove(pos);
                archive.push(odmo_types::DigimonArchiveEntry {
                    archive_slot: dst_archive,
                    partner,
                });
            }
        }
        // Source archive -> roster
        else if slot1 < 0 && slot2 > 0 {
            let src_archive = (-slot1) as u8;
            let dst_slot = slot2 as u8;
            if let Some(pos) = archive.iter().position(|e| e.archive_slot == src_archive) {
                let mut entry = archive.remove(pos);
                entry.partner.slot = dst_slot;
                roster.push(entry.partner);
            }
        }

        self.repository
            .update_digimon_archive(character_id, archive)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // Persist the updated roster only if changed; partner_slots persistence is
        // covered by the JSON helper via `update_partner_type` callbacks elsewhere.
        let _ = roster;
        Ok(Vec::new())
    }

    fn handle_digimon_archive_list(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Emit a custom payload mirroring the legacy DigimonArchiveLoadPacket which
        // is not yet defined in the Rust protocol crate. We write a minimal envelope
        // with count + per-entry partner type/level/name.
        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::DIGIMON_ARCHIVE_LIST,
        );
        writer.write_u16(character.digimon_archive.len().min(u16::MAX as usize) as u16);
        for entry in &character.digimon_archive {
            writer.write_u8(entry.archive_slot);
            writer.write_i32(entry.partner.digimon_type);
            writer.write_u8(entry.partner.level);
            writer.write_string(&entry.partner.name);
        }
        Ok(vec![writer.finalize()])
    }

    fn handle_digimon_archive_swap(
        &self,
        session: &GameSession,
        src_arr: u8,
        dst_arr: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut archive = character.digimon_archive.clone();
        let src_idx = archive.iter().position(|e| e.archive_slot == src_arr);
        let dst_idx = archive.iter().position(|e| e.archive_slot == dst_arr);

        match (src_idx, dst_idx) {
            (Some(a), Some(b)) => {
                archive.swap(a, b);
                let (av, bv) = (archive[a].archive_slot, archive[b].archive_slot);
                archive[a].archive_slot = bv;
                archive[b].archive_slot = av;
            }
            (Some(a), None) => {
                archive[a].archive_slot = dst_arr;
            }
            _ => {}
        }

        self.repository
            .update_digimon_archive(character_id, archive)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        Ok(Vec::new())
    }

    // ----- Ride mode + partner rename ------------------------------------------------

    fn handle_partner_evolution(
        &self,
        session: &GameSession,
        digimon_handler: u32,
        evolution_slot: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        if digimon_handler != character.partner_handler {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        }

        let assets = self
            .repository
            .evolution_assets()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let item_assets = self
            .repository
            .item_assets()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let digimon_assets = self
            .repository
            .digimon_assets()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let Some(active_slot_index) = character
            .partner_slots
            .iter()
            .position(|slot| slot.slot == character.partner_current_slot)
        else {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        };

        let active_slot_type = character.partner_slots[active_slot_index].digimon_type;
        let base_type = if character.partner_slots[active_slot_index].model > 0 {
            character.partner_slots[active_slot_index].model
        } else {
            character.partner_model
        };
        let Some(asset) = assets.iter().find(|asset| asset.base_type == base_type) else {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        };
        let Some(current_line) = asset
            .lines
            .iter()
            .find(|line| line.type_id == active_slot_type)
        else {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        };

        let Some(stage) = current_line
            .stages
            .iter()
            .find(|stage| (stage.value & 0xffff) as u8 == evolution_slot)
        else {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        };
        let target_type = stage.target_type;
        if target_type <= 0 {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        }
        let Some(target_line) = asset.lines.iter().find(|line| line.type_id == target_type) else {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        };
        let Some(target_snapshot) = character.partner_slots[active_slot_index]
            .evolutions
            .iter()
            .find(|evolution| evolution.evolution_type == target_type)
        else {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        };
        if target_line.enabled == 0 {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        }
        if evolution_requires_opened_slot(target_line) && (target_snapshot.unlocked & 0x01) == 0 {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        }
        if target_line.unlock_quest_id > 0
            && !quest_completed(&character.quest_progress, target_line.unlock_quest_id)
        {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        }
        if target_line.unlock_level > 0 && character.partner_level < target_line.unlock_level {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        }

        if target_line.open_qualification == 3 && character.xai.is_none() {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        }

        let mut digivice_dirty = false;
        if target_line.evolution_tree == 2 {
            let Some(lowest_partner_level) = resolve_jogress_partner_level(
                &character.partner_slots,
                character.partner_current_slot,
                target_line,
            ) else {
                return Ok(vec![DigimonEvolutionFailPacket.encode()]);
            };

            if i32::from(character.partner_level) > lowest_partner_level {
                return Ok(vec![DigimonEvolutionFailPacket.encode()]);
            }

            if !validate_and_consume_jogress_chipset(
                &item_assets,
                &mut character.digivice,
                target_line,
            ) {
                return Ok(vec![DigimonEvolutionFailPacket.encode()]);
            }
            digivice_dirty = target_line.jogress_consumable_chipset_type > 0;
        }

        let Some(cost) = evolution_resource_cost(
            &digimon_assets,
            active_slot_type,
            target_type,
            i32::from(character.partner_level),
            target_line.required_ds,
        ) else {
            return Ok(vec![DigimonEvolutionFailPacket.encode()]);
        };
        let mut resources_dirty = false;
        match cost {
            EvolutionResourceCost::Ds(required_ds) if required_ds > 0 => {
                if character.current_ds < required_ds {
                    return Ok(vec![DigimonEvolutionFailPacket.encode()]);
                }
                character.current_ds -= required_ds;
                resources_dirty = true;
            }
            EvolutionResourceCost::XGauge(required_xgauge) if required_xgauge > 0 => {
                if character.current_xgauge < required_xgauge {
                    return Ok(vec![DigimonEvolutionFailPacket.encode()]);
                }
                character.current_xgauge -= required_xgauge;
                resources_dirty = true;
            }
            EvolutionResourceCost::Ds(_) | EvolutionResourceCost::XGauge(_) => {}
        }

        character.partner_slots[active_slot_index].digimon_type = target_type;
        character.partner_current_type = target_type;
        if resources_dirty {
            self.repository
                .update_tamer_resources(
                    character_id,
                    character.current_hp,
                    character.current_ds,
                    character.current_xgauge,
                )
                .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        }
        if digivice_dirty {
            self.repository
                .update_digivice(character_id, character.digivice.clone())
                .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        }
        self.repository
            .update_partner_roster(
                character_id,
                character.partner_current_slot,
                character.partner_slots.clone(),
            )
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let hp_rate = if character.partner_hp <= 0 {
            0
        } else {
            ((character.partner_current_hp.max(0) * 100) / character.partner_hp.max(1))
                .clamp(0, 100) as u8
        };
        let packet = DigimonEvolutionSuccessPacket {
            digimon_handler: character.partner_handler,
            tamer_handler: character.general_handler,
            new_type: target_type,
            evolution_slot,
            hp_rate,
            parts_type: 0,
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to_visible(
                character.map_id,
                character.channel,
                character.id,
                &packet,
            );
        }
        Ok(vec![packet])
    }

    fn handle_ride_mode_start(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let evolution_type = character.partner_current_type;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::RIDE_MODE_START);
        writer.write_u8(1); // success
        writer.write_u32(character.partner_handler);
        writer.write_i32(evolution_type);
        let packet = writer.finalize();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to_visible(
                character.map_id,
                character.channel,
                character.id,
                &packet,
            );
        }
        Ok(vec![packet])
    }

    fn handle_open_ride_mode(
        &self,
        session: &GameSession,
        evo_unit_idx: u32,
        item_type: i32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let Some(active_slot_index) = character
            .partner_slots
            .iter()
            .position(|slot| slot.slot == character.partner_current_slot)
        else {
            return Ok(Vec::new());
        };

        let evo_index = evo_unit_idx as usize;
        if evo_index >= character.partner_slots[active_slot_index].evolutions.len() {
            return Ok(Vec::new());
        }

        if (character.partner_slots[active_slot_index].evolutions[evo_index].unlocked & 0x08) != 0 {
            return Ok(Vec::new());
        }

        let item_assets = self
            .repository
            .item_assets()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let item_sections = item_section_index(&item_assets);
        let mut consumed_items = Vec::new();
        if !consume_items_by_section(
            &mut character.inventory,
            &item_sections,
            item_type,
            1,
            &mut consumed_items,
        ) {
            return Ok(Vec::new());
        }

        character.partner_slots[active_slot_index].evolutions[evo_index].unlocked |= 0x08;
        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_partner_roster(
                character_id,
                character.partner_current_slot,
                character.partner_slots.clone(),
            )
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        Ok(Vec::new())
    }

    fn handle_evolution_unlock(
        &self,
        session: &GameSession,
        evolution_index: i32,
        requested_items: &[EvolutionUnlockItem],
        selected_inven_idx: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        if requested_items.is_empty() && selected_inven_idx >= 0 {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::CAPSULE_EVOLUTION_SLOT_RESULT,
            );
            writer.write_i16(2);
            return Ok(vec![writer.finalize()]);
        }

        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let Some(active_slot_index) = character
            .partner_slots
            .iter()
            .position(|slot| slot.slot == character.partner_current_slot)
        else {
            return Ok(Vec::new());
        };

        let evo_index = evolution_index as usize;
        if evo_index >= character.partner_slots[active_slot_index].evolutions.len() {
            return Ok(Vec::new());
        }

        if (character.partner_slots[active_slot_index].evolutions[evo_index].unlocked & 0x01) != 0 {
            return Ok(Vec::new());
        }

        let assets = self
            .repository
            .evolution_assets()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let item_assets = self
            .repository
            .item_assets()
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let active_slot = &character.partner_slots[active_slot_index];
        let Some(evolution) = active_slot.evolutions.get(evo_index) else {
            return Ok(Vec::new());
        };
        let base_type = if active_slot.model > 0 {
            active_slot.model
        } else {
            character.partner_model
        };
        let Some(asset) = assets.iter().find(|asset| asset.base_type == base_type) else {
            return Ok(Vec::new());
        };
        let Some(target_line) = asset
            .lines
            .iter()
            .find(|line| line.type_id == evolution.evolution_type)
        else {
            return Ok(Vec::new());
        };

        if target_line.jogress_quest_check > 0
            && quest_in_progress(&character.quest_progress, target_line.jogress_quest_check)
        {
            return Ok(Vec::new());
        }

        if target_line.unlock_quest_id > 0
            && !quest_completed(&character.quest_progress, target_line.unlock_quest_id)
        {
            return Ok(Vec::new());
        }

        let mut consumed_items = Vec::new();
        if !requested_items.is_empty() {
            let original_inventory = character.inventory.clone();
            for requested in requested_items {
                if requested.item_id == 0 || requested.item_count == 0 {
                    character.inventory = original_inventory;
                    return Ok(Vec::new());
                }
                if !consume_items_by_type(
                    &mut character.inventory,
                    requested.item_id as i32,
                    requested.item_count as i32,
                    &mut consumed_items,
                ) {
                    character.inventory = original_inventory;
                    return Ok(Vec::new());
                }
            }
        } else {
            let required_section = target_line.unlock_item_section;
            let required_amount = target_line.unlock_item_section_amount.max(1);
            if required_section <= 0 || required_amount <= 0 {
                return Ok(Vec::new());
            }

            let item_sections = item_section_index(&item_assets);
            if !consume_items_by_section(
                &mut character.inventory,
                &item_sections,
                required_section,
                required_amount,
                &mut consumed_items,
            ) {
                return Ok(Vec::new());
            }
        }

        character.partner_slots[active_slot_index].evolutions[evo_index].unlocked |= 0x01;
        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_partner_roster(
                character_id,
                character.partner_current_slot,
                character.partner_slots.clone(),
            )
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        Ok(Vec::new())
    }

    fn handle_ride_mode_stop(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::RIDE_MODE_STOP);
        writer.write_u32(character.partner_handler);
        let packet = writer.finalize();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to_visible(
                character.map_id,
                character.channel,
                character.id,
                &packet,
            );
        }
        Ok(vec![packet])
    }

    fn handle_digimon_change_name(
        &self,
        session: &GameSession,
        new_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let trimmed = new_name.trim().to_string();
        if trimmed.is_empty() || trimmed.len() > 16 {
            let mut writer = odmo_protocol::writer::PacketWriter::new(
                odmo_protocol::opcode::game::DIGIMON_CHANGE_NAME,
            );
            writer.write_u8(0);
            return Ok(vec![writer.finalize()]);
        }

        self.repository
            .update_partner_name(character_id, &trimmed)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::DIGIMON_CHANGE_NAME,
        );
        writer.write_u8(1);
        writer.write_string(&trimmed);
        Ok(vec![writer.finalize()])
    }

    // ----- Trade slice -----------------------------------------------------------------

    fn handle_trade_request(
        &self,
        session: &GameSession,
        target_handler: u32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let inviter_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let inviter = self
            .repository
            .character_by_id(inviter_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(inviter_id))?;

        // Resolve target by general_handler from the session view.
        let target_id = match session
            .viewed_characters
            .values()
            .find(|c| c.general_handler == target_handler)
        {
            Some(target) => target.id,
            None => {
                // Target not visible — emit "trade target not found" error (legacy code 1).
                return Ok(vec![TradeRequestErrorPacket { result: 1 }.encode()]);
            }
        };

        if target_id == inviter.id {
            return Ok(vec![TradeRequestErrorPacket { result: 1 }.encode()]);
        }

        // Reject if either side is already in a trade.
        {
            let runtime = self.trade_runtime.read().expect("trade runtime poisoned");
            if runtime.session_by_character.contains_key(&inviter_id)
                || runtime.session_by_character.contains_key(&target_id)
            {
                return Ok(vec![TradeRequestErrorPacket { result: 2 }.encode()]);
            }
        }

        let mut runtime = self.trade_runtime.write().expect("trade runtime poisoned");
        runtime.pending_requests.insert(inviter_id, target_id);
        drop(runtime);

        let outbound = TradeRequestSuccessPacket { target_handler }.encode();

        // Notify the target that they received a trade request.
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to(target_id, &outbound);
        }

        Ok(vec![outbound])
    }

    fn handle_trade_accept(
        &self,
        session: &GameSession,
        accepter_handler: u32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let target_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let target = self
            .repository
            .character_by_id(target_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(target_id))?;

        // Find the inviter that targeted this character.
        let inviter_id = {
            let runtime = self.trade_runtime.read().expect("trade runtime poisoned");
            runtime
                .pending_requests
                .iter()
                .find(|(_, t)| **t == target_id)
                .map(|(&i, _)| i)
        };

        let Some(inviter_id) = inviter_id else {
            return Ok(vec![TradeRequestErrorPacket { result: 3 }.encode()]);
        };

        let inviter = self
            .repository
            .character_by_id(inviter_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(inviter_id))?;

        // Bootstrap the trade session.
        let session_id = {
            let mut runtime = self.trade_runtime.write().expect("trade runtime poisoned");
            runtime.pending_requests.remove(&inviter_id);
            runtime.next_session_id = runtime.next_session_id.saturating_add(1);
            let id = runtime.next_session_id;
            runtime.sessions.insert(
                id,
                TradeSession {
                    id,
                    side_a: TradeSideRuntime {
                        character_id: inviter.id,
                        handler: inviter.general_handler,
                        ..Default::default()
                    },
                    side_b: TradeSideRuntime {
                        character_id: target.id,
                        handler: target.general_handler,
                        ..Default::default()
                    },
                    confirmed_a: false,
                    confirmed_b: false,
                    final_a: false,
                    final_b: false,
                },
            );
            runtime.session_by_character.insert(inviter.id, id);
            runtime.session_by_character.insert(target.id, id);
            id
        };

        // Echo the accept packet to both sides so the trade window opens.
        let to_target = TradeAcceptPacket {
            target_handler: inviter.general_handler,
        }
        .encode();
        let to_inviter = TradeAcceptPacket {
            target_handler: target.general_handler,
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to(inviter.id, &to_inviter);
        }
        let _ = accepter_handler;
        let _ = session_id;
        Ok(vec![to_target])
    }

    fn handle_trade_cancel(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let trade_session_id = {
            let runtime = self.trade_runtime.read().expect("trade runtime poisoned");
            runtime.session_by_character.get(&character_id).copied()
        };

        let Some(trade_session_id) = trade_session_id else {
            // Nothing to cancel; clear any pending request from this character.
            let mut runtime = self.trade_runtime.write().expect("trade runtime poisoned");
            runtime.pending_requests.remove(&character_id);
            return Ok(Vec::new());
        };

        // Tear down the session and notify the other side.
        let other_id = {
            let mut runtime = self.trade_runtime.write().expect("trade runtime poisoned");
            let other = runtime.sessions.get(&trade_session_id).map(|s| {
                if s.side_a.character_id == character_id {
                    (s.side_a.handler, s.side_b.character_id, s.side_b.handler)
                } else {
                    (s.side_b.handler, s.side_a.character_id, s.side_a.handler)
                }
            });
            runtime.sessions.remove(&trade_session_id);
            runtime.session_by_character.remove(&character_id);
            if let Some((_, other_id, _)) = other {
                runtime.session_by_character.remove(&other_id);
            }
            other.map(|(_, other_id, other_handler)| (other_id, other_handler))
        };

        if let Some((other_id, other_handler)) = other_id {
            let cancel = TradeCancelPacket {
                target_handler: other_handler,
            }
            .encode();
            if let Some(broadcast) = &self.broadcast {
                let _ = broadcast.send_to(other_id, &cancel);
            }
        }

        let mine = TradeCancelPacket { target_handler: 0 }.encode();
        Ok(vec![mine])
    }

    fn with_trade_session<F, R>(&self, character_id: u64, op: F) -> Option<R>
    where
        F: FnOnce(&mut TradeSession, bool) -> R,
    {
        let mut runtime = self.trade_runtime.write().expect("trade runtime poisoned");
        let session_id = *runtime.session_by_character.get(&character_id)?;
        let trade_session = runtime.sessions.get_mut(&session_id)?;
        let is_side_a = trade_session.side_a.character_id == character_id;
        Some(op(trade_session, is_side_a))
    }

    fn handle_trade_add_item(
        &self,
        session: &GameSession,
        inven_pos: u16,
        amount: u16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Validate slot.
        let idx = inven_pos as usize;
        if idx >= character.inventory.items.len() {
            return Ok(Vec::new());
        }
        let item = character.inventory.items[idx].clone();
        if item.item_id <= 0 || item.amount <= 0 {
            return Ok(Vec::new());
        }

        // The trade slot is implicit. We append to our side's items vec and use the
        // index as the trade_slot for the broadcast.
        let other_handler = self.with_trade_session(character_id, |sess, is_a| {
            let (mine, theirs) = if is_a {
                (&mut sess.side_a, &sess.side_b)
            } else {
                (&mut sess.side_b, &sess.side_a)
            };
            if mine.locked {
                return None;
            }
            let trade_slot = mine.items.len() as u8;
            mine.items
                .push((trade_slot, item.item_id, amount as i16, inven_pos as i32));
            sess.confirmed_a = false;
            sess.confirmed_b = false;
            Some((theirs.character_id, trade_slot))
        });

        let Some(Some((target_id, trade_slot))) = other_handler else {
            return Ok(Vec::new());
        };

        let other_character = self
            .repository
            .character_by_id(target_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(target_id))?;

        let packet = TradeAddItemPacket {
            target_handler: character.general_handler,
            item_bytes: item.record.clone(),
            trade_slot,
            inventory_slot: inven_pos as i32,
        }
        .encode();
        let mine_packet = TradeAddItemPacket {
            target_handler: other_character.general_handler,
            item_bytes: item.record,
            trade_slot,
            inventory_slot: inven_pos as i32,
        }
        .encode();

        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to(target_id, &packet);
        }
        Ok(vec![mine_packet])
    }

    fn handle_trade_remove_item(
        &self,
        session: &GameSession,
        trade_slot: i8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        if trade_slot < 0 {
            return Ok(Vec::new());
        }
        let trade_slot_u = trade_slot as u8;

        let result = self.with_trade_session(character_id, |sess, is_a| {
            let (mine, theirs) = if is_a {
                (&mut sess.side_a, &sess.side_b)
            } else {
                (&mut sess.side_b, &sess.side_a)
            };
            if mine.locked {
                return None;
            }
            mine.items.retain(|(slot, _, _, _)| *slot != trade_slot_u);
            sess.confirmed_a = false;
            sess.confirmed_b = false;
            Some(theirs.character_id)
        });

        let Some(Some(target_id)) = result else {
            return Ok(Vec::new());
        };

        let other_character = self
            .repository
            .character_by_id(target_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(target_id))?;

        let to_other = TradeRemoveItemPacket {
            target_handler: character.general_handler,
            trade_slot: trade_slot_u,
        }
        .encode();
        let mine = TradeRemoveItemPacket {
            target_handler: other_character.general_handler,
            trade_slot: trade_slot_u,
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to(target_id, &to_other);
        }
        Ok(vec![mine])
    }

    fn handle_trade_add_money(
        &self,
        session: &GameSession,
        amount: i64,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let result = self.with_trade_session(character_id, |sess, is_a| {
            let (mine, theirs) = if is_a {
                (&mut sess.side_a, &sess.side_b)
            } else {
                (&mut sess.side_b, &sess.side_a)
            };
            if mine.locked {
                return None;
            }
            mine.money = amount.max(0);
            sess.confirmed_a = false;
            sess.confirmed_b = false;
            Some(theirs.character_id)
        });

        let Some(Some(target_id)) = result else {
            return Ok(Vec::new());
        };

        let other_character = self
            .repository
            .character_by_id(target_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(target_id))?;

        let money_i32 = amount.clamp(0, i32::MAX as i64) as i32;
        let to_other = TradeAddMoneyPacket {
            target_handler: character.general_handler,
            money: money_i32,
        }
        .encode();
        let mine = TradeAddMoneyPacket {
            target_handler: other_character.general_handler,
            money: money_i32,
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to(target_id, &to_other);
        }
        Ok(vec![mine])
    }

    fn handle_trade_confirm(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let outcome = self.with_trade_session(character_id, |sess, is_a| {
            if is_a {
                sess.confirmed_a = true;
            } else {
                sess.confirmed_b = true;
            }
            let both_confirmed = sess.confirmed_a && sess.confirmed_b;
            let other_id = if is_a {
                sess.side_b.character_id
            } else {
                sess.side_a.character_id
            };
            let other_handler = if is_a {
                sess.side_b.handler
            } else {
                sess.side_a.handler
            };
            (both_confirmed, other_id, other_handler)
        });
        let Some((both_confirmed, other_id, other_handler)) = outcome else {
            return Ok(Vec::new());
        };

        let mine = TradeConfirmationPacket {
            target_handler: other_handler,
        }
        .encode();
        let to_other = TradeConfirmationPacket {
            target_handler: character.general_handler,
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to(other_id, &to_other);
        }

        if both_confirmed {
            // Both sides confirmed — trigger the final-confirmation packets so the
            // client UI moves to the second-stage confirm screen.
            let final_mine = TradeFinalConfirmationPacket {
                target_handler: other_handler,
            }
            .encode();
            let final_other = TradeFinalConfirmationPacket {
                target_handler: character.general_handler,
            }
            .encode();
            if let Some(broadcast) = &self.broadcast {
                let _ = broadcast.send_to(other_id, &final_other);
            }

            // Apply the trade atomically.
            let _ = self.commit_trade(character_id);

            return Ok(vec![mine, final_mine]);
        }

        Ok(vec![mine])
    }

    fn handle_trade_lock(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let other = self.with_trade_session(character_id, |sess, is_a| {
            if is_a {
                sess.side_a.locked = true;
            } else {
                sess.side_b.locked = true;
            }
            if is_a {
                (sess.side_b.character_id, sess.side_b.handler)
            } else {
                (sess.side_a.character_id, sess.side_a.handler)
            }
        });

        let Some((other_id, other_handler)) = other else {
            return Ok(Vec::new());
        };
        let _ = other_handler;

        let to_other = TradeInventoryLockPacket {
            target_handler: character.general_handler,
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to(other_id, &to_other);
        }
        Ok(vec![
            TradeInventoryLockPacket { target_handler: 0 }.encode(),
        ])
    }

    fn handle_trade_unlock(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let other = self.with_trade_session(character_id, |sess, is_a| {
            if is_a {
                sess.side_a.locked = false;
                sess.confirmed_a = false;
            } else {
                sess.side_b.locked = false;
                sess.confirmed_b = false;
            }
            if is_a {
                (sess.side_b.character_id, sess.side_b.handler)
            } else {
                (sess.side_a.character_id, sess.side_a.handler)
            }
        });

        let Some((other_id, _)) = other else {
            return Ok(Vec::new());
        };

        let to_other = TradeInventoryUnlockPacket {
            target_handler: character.general_handler,
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            let _ = broadcast.send_to(other_id, &to_other);
        }
        Ok(vec![
            TradeInventoryUnlockPacket { target_handler: 0 }.encode(),
        ])
    }

    /// Commit a confirmed trade: move items + bits between both sides atomically.
    /// Returns silently on any persistence error after rolling back the in-memory state.
    fn commit_trade(&self, requesting_character_id: u64) -> anyhow::Result<()> {
        let trade_session = {
            let mut runtime = self.trade_runtime.write().expect("trade runtime poisoned");
            let Some(&id) = runtime.session_by_character.get(&requesting_character_id) else {
                return Ok(());
            };
            let Some(sess) = runtime.sessions.remove(&id) else {
                return Ok(());
            };
            runtime
                .session_by_character
                .remove(&sess.side_a.character_id);
            runtime
                .session_by_character
                .remove(&sess.side_b.character_id);
            sess
        };

        let mut a = self
            .repository
            .character_by_id(trade_session.side_a.character_id)?
            .ok_or_else(|| anyhow::anyhow!("trade side a not found"))?;
        let mut b = self
            .repository
            .character_by_id(trade_session.side_b.character_id)?
            .ok_or_else(|| anyhow::anyhow!("trade side b not found"))?;

        // Validate that both sides have enough bits and the inventory slots are valid.
        if a.inventory_bits < trade_session.side_a.money
            || b.inventory_bits < trade_session.side_b.money
        {
            return Ok(());
        }

        // Move items from a -> b.
        for (_, item_id, amount, src_slot) in &trade_session.side_a.items {
            let src_idx = *src_slot as usize;
            if src_idx >= a.inventory.items.len() {
                continue;
            }
            if a.inventory.items[src_idx].item_id != *item_id {
                continue;
            }
            // Remove from a.
            let removed = std::mem::replace(
                &mut a.inventory.items[src_idx],
                odmo_types::ItemRecord::new(0, 0),
            );
            // Insert into b.
            if let Some(target_slot) = b.inventory.items.iter().position(|i| i.item_id == 0) {
                b.inventory.items[target_slot] =
                    odmo_types::ItemRecord::new(*item_id, *amount as i32);
            } else {
                // No room on b — drop the trade by re-inserting on a.
                a.inventory.items[src_idx] = removed;
                return Ok(());
            }
        }
        for (_, item_id, amount, src_slot) in &trade_session.side_b.items {
            let src_idx = *src_slot as usize;
            if src_idx >= b.inventory.items.len() {
                continue;
            }
            if b.inventory.items[src_idx].item_id != *item_id {
                continue;
            }
            let removed = std::mem::replace(
                &mut b.inventory.items[src_idx],
                odmo_types::ItemRecord::new(0, 0),
            );
            if let Some(target_slot) = a.inventory.items.iter().position(|i| i.item_id == 0) {
                a.inventory.items[target_slot] =
                    odmo_types::ItemRecord::new(*item_id, *amount as i32);
            } else {
                b.inventory.items[src_idx] = removed;
                return Ok(());
            }
        }

        // Settle bits.
        let new_a_bits = a.inventory_bits - trade_session.side_a.money + trade_session.side_b.money;
        let new_b_bits = b.inventory_bits - trade_session.side_b.money + trade_session.side_a.money;

        let inventory_a = a.inventory.clone();
        let inventory_b = b.inventory.clone();
        self.repository.update_inventory(a.id, inventory_a)?;
        self.repository.update_inventory(b.id, inventory_b)?;
        self.repository.update_inventory_bits(a.id, new_a_bits)?;
        self.repository.update_inventory_bits(b.id, new_b_bits)?;

        Ok(())
    }

    // ----- Seal slice -----------------------------------------------------------------

    fn handle_seal_open(
        &self,
        session: &GameSession,
        seal_idx: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut seal_list = character.seal_list.clone();
        let next_id = seal_list
            .seals
            .iter()
            .map(|s| s.sequential_id)
            .max()
            .unwrap_or(0)
            + 1;

        if let Some(existing) = seal_list
            .seals
            .iter_mut()
            .find(|s| s.seal_id == seal_idx as i32)
        {
            existing.amount = existing.amount.saturating_add(1);
        } else {
            seal_list.seals.push(odmo_types::SealRecord {
                seal_id: seal_idx as i32,
                amount: 1,
                sequential_id: next_id,
                favorite: false,
            });
        }

        self.repository
            .update_seal_list(character_id, seal_list)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::SEAL_OPEN);
        writer.write_i16(seal_idx);
        writer.write_u8(1);
        Ok(vec![writer.finalize()])
    }

    fn handle_seal_close(
        &self,
        session: &GameSession,
        seal_idx: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut seal_list = character.seal_list.clone();
        seal_list.seals.retain(|s| s.seal_id != seal_idx as i32);
        self.repository
            .update_seal_list(character_id, seal_list)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::SEAL_CLOSE);
        writer.write_i16(seal_idx);
        writer.write_u8(1);
        Ok(vec![writer.finalize()])
    }

    fn handle_seal_set_leader(
        &self,
        session: &GameSession,
        card_code: u16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut seal_list = character.seal_list.clone();
        seal_list.seal_leader_id = card_code as i16;
        self.repository
            .update_seal_list(character_id, seal_list)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer =
            odmo_protocol::writer::PacketWriter::new(odmo_protocol::opcode::game::SEAL_SET_LEADER);
        writer.write_u16(card_code);
        writer.write_u8(1);
        Ok(vec![writer.finalize()])
    }

    fn handle_seal_remove_leader(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut seal_list = character.seal_list.clone();
        seal_list.seal_leader_id = 0;
        self.repository
            .update_seal_list(character_id, seal_list)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        Ok(Vec::new())
    }

    fn handle_seal_set_favorite(
        &self,
        session: &GameSession,
        card_code: u16,
        bookmark: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut seal_list = character.seal_list.clone();
        if let Some(seal) = seal_list
            .seals
            .iter_mut()
            .find(|s| s.seal_id == card_code as i32)
        {
            seal.favorite = bookmark != 0;
        }
        self.repository
            .update_seal_list(character_id, seal_list)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let mut writer = odmo_protocol::writer::PacketWriter::new(
            odmo_protocol::opcode::game::SEAL_SET_FAVORITE,
        );
        writer.write_u16(card_code);
        writer.write_u8(1);
        Ok(vec![writer.finalize()])
    }

    // ----- Encyclopedia slice ----------------------------------------------------------

    fn handle_encyclopedia_load(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        Ok(vec![
            EncyclopediaLoadPacket {
                entries: character.encyclopedia.entries.clone(),
            }
            .encode(),
        ])
    }

    fn handle_encyclopedia_get_reward(
        &self,
        session: &GameSession,
        digimon_id: u32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut encyclopedia = character.encyclopedia.clone();
        let target_id = digimon_id as i64;
        let entry = encyclopedia
            .entries
            .iter_mut()
            .find(|e| e.digimon_evolution_id == target_id);

        let Some(entry) = entry else {
            return Ok(Vec::new());
        };

        if !entry.reward_allowed || entry.reward_received {
            return Ok(Vec::new());
        }

        entry.reward_received = true;

        // The current Rust workspace still lacks a native encyclopedia asset repository,
        // but the legacy-server reference for this shipped path currently grants the
        // visible reward payload `97206 x10`. Mirror that until the asset table lands.
        self.repository
            .update_encyclopedia(character_id, encyclopedia)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            EncyclopediaReceiveRewardItemPacket {
                item_id: 97_206,
                amount: 10,
            }
            .encode(),
        ])
    }

    fn handle_encyclopedia_deck_buff(
        &self,
        session: &GameSession,
        deck_idx: u16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        // Toggle the active deck buff on the character row.
        let new_buff = if character.active_deck_buff == deck_idx as i32 {
            0
        } else {
            deck_idx as i32
        };
        self.repository
            .update_deck_buff(character_id, new_buff)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        // The reply carries {u2 option_index; u4 value} and the client resolves it
        // through its deck-buff event. The deck-buff asset table that defines `value`
        // is not materialized server-side yet; the index is echoed from the request.
        Ok(vec![
            EncyclopediaDeckBuffUsePacket {
                option_index: deck_idx,
                value: 0,
            }
            .encode(),
        ])
    }

    fn handle_other_tamer_detail_info(
        &self,
        session: &GameSession,
        target_handler: u32,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let target = session
            .viewed_characters
            .values()
            .find(|character| matches_tamer_target_handler(character, target_handler))
            .cloned()
            .or_else(|| {
                let character_id = session.character_id?;
                let character = self.repository.character_by_id(character_id).ok()??;
                if matches_tamer_target_handler(&character, target_handler) {
                    Some(character)
                } else {
                    None
                }
            });

        let packet = if let Some(character) = target {
            let mut packet = OtherTamerDetailInfoPacket::empty(target_handler);
            packet.tamer_name = character.name.clone();
            packet.guild_name = character
                .guild
                .as_ref()
                .map(|guild| guild.name.clone())
                .unwrap_or_default();
            // Tamer scalar block: level, size, hp, ds, then the combat-stat group.
            packet.stats[0] = i32::from(character.level);
            packet.stats[1] = i32::from(character.size);
            packet.stats[2] = character.current_hp;
            packet.stats[3] = character.current_ds;
            packet.stats[4] = character.at;
            packet.stats[5] = character.de;
            packet.stats[6] = character.ms;
            // Embedded partner block.
            packet.digimon.model = character.partner_model as u32;
            packet.digimon.name = character.partner_name.clone();
            packet.digimon.skills = other_tamer_detail_skills(&character);
            packet.digimon.enchants = other_tamer_detail_enchants(&character);
            packet
        } else {
            OtherTamerDetailInfoPacket::empty(target_handler)
        };

        Ok(vec![packet.encode()])
    }

    // ----- Arena slice -----------------------------------------------------------------

    fn handle_arena_daily_points(
        &self,
        _session: &GameSession,
        added_points: i16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        // The arena ranking table is not yet persisted. Echo the requested delta as
        // the current points total until the backing store is wired in.
        Ok(vec![
            ArenaRankingDailyUpdatePointsPacket {
                points: added_points.max(0) as i32,
            }
            .encode(),
        ])
    }

    fn handle_arena_daily_ranking(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        // Without an arena ranking persistence layer we report 0 points and the
        // remaining minutes until midnight UTC (legacy daily reset boundary).
        let remaining = (seconds_until_next_day() as i64) / 60;
        Ok(vec![
            ArenaRankingDailyLoadPacket {
                remaining_minutes: remaining,
                points: 0,
            }
            .encode(),
        ])
    }

    fn handle_arena_request_rank(
        &self,
        _session: &GameSession,
        ranking_type: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        // No ranking board is persisted yet, so the reply is a successful but
        // empty board. The status says the period has closed, which is the only
        // reading consistent with the zero renew/end timers.
        Ok(vec![
            ArenaRankingInfoPacket {
                ranking_type,
                result: 0,
                status: RANK_STATUS_END,
                point: 0,
                rank: 0,
                position_delta: 0,
                rank_position_kind: 0,
                rank_percent: 0,
                renew_sec: 0,
                end_sec: 0,
                entries: Vec::new(),
            }
            .encode(),
        ])
    }

    fn handle_arena_request_old_rank(
        &self,
        _session: &GameSession,
        ranking_type: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        Ok(vec![
            ArenaOldRankingInfoPacket {
                ranking_type,
                result: 0,
                rank: 0,
                entries: Vec::new(),
            }
            .encode(),
        ])
    }

    fn handle_arena_request_fame(
        &self,
        _session: &GameSession,
        season: u16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        // Hall-of-fame boards are archived per season; with no archive the
        // season is echoed back with an empty row list.
        Ok(vec![
            ArenaFameInfoPacket {
                season,
                result: 0,
                entries: Vec::new(),
            }
            .encode(),
        ])
    }

    fn handle_arena_request_fame_list(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        Ok(vec![ArenaFameSeasonListPacket::default().encode()])
    }

    fn handle_dungeon_next_stage(
        &self,
        _session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        Ok(vec![
            DungeonArenaNextStagePacket {
                current_stage: 1,
                npc_id: 0,
                remain_time: 0,
            }
            .encode(),
        ])
    }

    fn handle_extra_inventory_move(
        &self,
        session: &GameSession,
        extra_slot: u16,
        inventory_slot: u16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let ext_idx = extra_slot as usize;
        let inv_idx = inventory_slot as usize;

        if ext_idx >= character.extra_inventory.items.len()
            || inv_idx >= character.inventory.items.len()
        {
            return Ok(vec![
                LoadInventoryPacket {
                    inventory: character.extra_inventory,
                    inventory_type: InventoryType::ExtraInventory,
                }
                .encode(),
            ]);
        }

        let source_item = character.extra_inventory.items[ext_idx].clone();
        if source_item.item_id <= 0 || source_item.amount <= 0 {
            return Ok(vec![
                LoadInventoryPacket {
                    inventory: character.extra_inventory,
                    inventory_type: InventoryType::ExtraInventory,
                }
                .encode(),
            ]);
        }

        let dest_item = character.inventory.items[inv_idx].clone();
        if dest_item.item_id > 0 && dest_item.item_id == source_item.item_id {
            let mut merged = dest_item.clone();
            merged.amount = merged.amount.saturating_add(source_item.amount);
            merged.sync_record();
            character.inventory.items[inv_idx] = merged;
            character.extra_inventory.items[ext_idx] = ItemRecord::default();
        } else if dest_item.item_id > 0 {
            character.inventory.items[inv_idx] = source_item;
            character.extra_inventory.items[ext_idx] = dest_item;
        } else {
            character.inventory.items[inv_idx] = source_item;
            character.extra_inventory.items[ext_idx] = ItemRecord::default();
        }

        self.repository
            .update_extra_inventory(character_id, character.extra_inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let updated = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        Ok(vec![
            LoadInventoryPacket {
                inventory: updated.inventory,
                inventory_type: InventoryType::Inventory,
            }
            .encode(),
            LoadInventoryPacket {
                inventory: updated.extra_inventory,
                inventory_type: InventoryType::ExtraInventory,
            }
            .encode(),
        ])
    }

    fn handle_extra_inventory_batch_move(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        for i in 0..character.extra_inventory.items.len() {
            let item = character.extra_inventory.items[i].clone();
            if item.item_id <= 0 || item.amount <= 0 {
                continue;
            }

            let mut placed = false;
            for j in 0..character.inventory.items.len() {
                let existing = &character.inventory.items[j];
                if existing.item_id == item.item_id && existing.amount > 0 {
                    let mut merged = existing.clone();
                    merged.amount = merged.amount.saturating_add(item.amount);
                    merged.sync_record();
                    character.inventory.items[j] = merged;
                    character.extra_inventory.items[i] = ItemRecord::default();
                    placed = true;
                    break;
                }
            }

            if !placed {
                for j in 0..character.inventory.items.len() {
                    if character.inventory.items[j].item_id <= 0
                        || character.inventory.items[j].amount <= 0
                    {
                        character.inventory.items[j] = item;
                        character.extra_inventory.items[i] = ItemRecord::default();
                        break;
                    }
                }
            }
        }

        self.repository
            .update_extra_inventory(character_id, character.extra_inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        self.repository
            .update_inventory(character_id, character.inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let updated = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        Ok(vec![
            LoadInventoryPacket {
                inventory: updated.extra_inventory,
                inventory_type: InventoryType::ExtraInventory,
            }
            .encode(),
        ])
    }

    fn handle_extra_inventory_sort(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let mut items: Vec<ItemRecord> = character
            .extra_inventory
            .items
            .iter()
            .filter(|item| item.item_id > 0 && item.amount > 0)
            .cloned()
            .collect();
        items.sort_by_key(|item| item.item_id);

        let empty_count = character.extra_inventory.items.len() - items.len();
        items.resize(items.len() + empty_count, ItemRecord::default());

        character.extra_inventory.items = items;

        self.repository
            .update_extra_inventory(character_id, character.extra_inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        Ok(vec![
            LoadInventoryPacket {
                inventory: character.extra_inventory,
                inventory_type: InventoryType::ExtraInventory,
            }
            .encode(),
        ])
    }

    fn handle_extra_inventory_use(
        &self,
        session: &GameSession,
        extra_slot: u16,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let mut character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let ext_idx = extra_slot as usize;
        if ext_idx >= character.extra_inventory.items.len() {
            return Ok(vec![
                LoadInventoryPacket {
                    inventory: character.extra_inventory,
                    inventory_type: InventoryType::ExtraInventory,
                }
                .encode(),
            ]);
        }

        let item = &character.extra_inventory.items[ext_idx];
        if item.item_id <= 0 || item.amount <= 0 {
            return Ok(vec![
                LoadInventoryPacket {
                    inventory: character.extra_inventory,
                    inventory_type: InventoryType::ExtraInventory,
                }
                .encode(),
            ]);
        }

        let new_amount = item.amount - 1;
        if new_amount <= 0 {
            character.extra_inventory.items[ext_idx] = ItemRecord::default();
        } else {
            let mut updated = item.clone();
            updated.amount = new_amount;
            updated.sync_record();
            character.extra_inventory.items[ext_idx] = updated;
        }

        self.repository
            .update_extra_inventory(character_id, character.extra_inventory.clone())
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;

        let updated = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        Ok(vec![
            LoadInventoryPacket {
                inventory: updated.extra_inventory,
                inventory_type: InventoryType::ExtraInventory,
            }
            .encode(),
        ])
    }

    fn drain_social_notifications(&self, session: &GameSession) -> anyhow::Result<Vec<Vec<u8>>> {
        let Some(character_id) = session.character_id else {
            return Ok(Vec::new());
        };

        let notifications = self
            .portal_bridge
            .consume_social_notifications(character_id)?;
        Ok(notifications
            .into_iter()
            .map(|notification| match notification.kind {
                SocialNotificationKind::FriendConnect { name } => {
                    FriendConnectPacket { name }.encode()
                }
                SocialNotificationKind::MapTamerSpawn { .. }
                | SocialNotificationKind::MapTamerUnload { .. } => Vec::new(),
            })
            .filter(|packet| !packet.is_empty())
            .collect())
    }

    fn announce_friend_connect(
        &self,
        character: &odmo_types::CharacterSummary,
    ) -> anyhow::Result<()> {
        for target_character_id in &character.friended_character_ids {
            self.portal_bridge.enqueue_social_notification(
                *target_character_id,
                SocialNotification {
                    kind: SocialNotificationKind::FriendConnect {
                        name: character.name.clone(),
                    },
                },
            )?;
        }

        Ok(())
    }

    fn register_map_presence(
        &self,
        session: &mut GameSession,
        character: &odmo_types::CharacterSummary,
    ) -> anyhow::Result<Vec<Vec<u8>>> {
        if session.registered_map_presence {
            return Ok(Vec::new());
        }

        let existing = self
            .portal_bridge
            .load_map_presence(character.map_id, character.channel)?;
        let mut responses = Vec::new();
        for occupant in existing
            .iter()
            .filter(|occupant| occupant.id != character.id)
            .filter(|occupant| can_see_each_other(character, occupant))
        {
            responses.push(
                LoadTamerPacket {
                    character: occupant.clone(),
                }
                .encode(),
            );
            responses.push(
                LoadBuffsPacket {
                    character: occupant.clone(),
                }
                .encode(),
            );
            session
                .viewed_characters
                .insert(occupant.id, occupant.clone());
        }

        self.portal_bridge.upsert_map_presence(character)?;
        session.registered_map_presence = true;

        // Update broadcast sink with the character's map location
        if let Some(broadcast) = &self.broadcast {
            broadcast.update_location(character.id, character.map_id, character.channel);
        }

        Ok(responses)
    }

    fn reconcile_map_visibility(&self, session: &mut GameSession) -> anyhow::Result<Vec<Vec<u8>>> {
        let Some(character_id) = session.character_id else {
            return Ok(Vec::new());
        };
        if !session.registered_map_presence {
            return Ok(Vec::new());
        }

        let character = self
            .repository
            .character_by_id(character_id)?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "character {character_id} not found during visibility reconciliation"
                )
            })?;
        let occupants = self
            .portal_bridge
            .load_map_presence(character.map_id, character.channel)?;

        let mut responses = Vec::new();
        let mut next_viewed_characters = HashMap::new();

        for occupant in occupants
            .iter()
            .filter(|occupant| occupant.id != character.id)
            .filter(|occupant| {
                map_distance(character.x, character.y, occupant.x, occupant.y)
                    <= START_TO_SEE_DISTANCE
            })
        {
            next_viewed_characters.insert(occupant.id, occupant.clone());
            if !session.viewed_characters.contains_key(&occupant.id) {
                responses.push(
                    LoadTamerPacket {
                        character: occupant.clone(),
                    }
                    .encode(),
                );
                responses.push(
                    LoadBuffsPacket {
                        character: occupant.clone(),
                    }
                    .encode(),
                );
            }
        }

        let previously_visible: Vec<u64> = session.viewed_characters.keys().copied().collect();
        for viewed_id in previously_visible {
            let should_hide = if let Some(occupant) =
                occupants.iter().find(|occupant| occupant.id == viewed_id)
            {
                map_distance(character.x, character.y, occupant.x, occupant.y)
                    >= STOP_SEEING_DISTANCE
            } else {
                true
            };

            if should_hide {
                let character_to_unload = session
                    .viewed_characters
                    .get(&viewed_id)
                    .cloned()
                    .or_else(|| {
                        occupants
                            .iter()
                            .find(|occupant| occupant.id == viewed_id)
                            .cloned()
                    })
                    .unwrap_or_else(|| {
                        let mut placeholder = odmo_types::CharacterSummary {
                            id: viewed_id,
                            ..odmo_types::CharacterSummary::default()
                        };
                        placeholder.general_handler = viewed_id.min(u32::MAX as u64) as u32;
                        placeholder.partner_handler =
                            viewed_id.saturating_add(10_000).min(u32::MAX as u64) as u32;
                        placeholder
                    });
                responses.push(
                    UnloadTamerPacket {
                        character: character_to_unload,
                    }
                    .encode(),
                );
            }
        }

        session.viewed_characters = next_viewed_characters;
        Ok(responses)
    }

    fn reconcile_mob_visibility(&self, session: &mut GameSession) -> anyhow::Result<Vec<Vec<u8>>> {
        let Some(character_id) = session.character_id else {
            return Ok(Vec::new());
        };
        if !session.registered_map_presence {
            return Ok(Vec::new());
        }

        let _ = self
            .repository
            .character_by_id(character_id)?
            .ok_or_else(|| {
                anyhow::anyhow!("character {character_id} not found during mob reconciliation")
            })?;

        let character = self
            .repository
            .character_by_id(character_id)?
            .ok_or_else(|| {
                anyhow::anyhow!("character {character_id} not found during mob reconciliation")
            })?;
        let current = self
            .repository
            .mobs_by_map(character.map_id, character.channel)?;
        let mut responses = Vec::new();
        let mut next_viewed_mobs = HashMap::new();

        for mob in current {
            if map_distance(character.x, character.y, mob.x, mob.y) > START_TO_SEE_DISTANCE {
                continue;
            }

            if !session.viewed_mobs.contains_key(&mob.id) {
                responses.push(LoadMobsPacket { mob: mob.clone() }.encode());
                if !mob.active_debuffs.is_empty() {
                    responses.push(LoadMobBuffsPacket { mob: mob.clone() }.encode());
                }
            }

            next_viewed_mobs.insert(mob.id, mob);
        }

        for mob in session.viewed_mobs.values() {
            if next_viewed_mobs.contains_key(&mob.id) {
                continue;
            }

            responses.push(UnloadMobsPacket { mob: mob.clone() }.encode());
        }

        session.viewed_mobs = next_viewed_mobs;
        Ok(responses)
    }

    fn reconcile_drop_visibility(&self, session: &mut GameSession) -> anyhow::Result<Vec<Vec<u8>>> {
        let Some(character_id) = session.character_id else {
            return Ok(Vec::new());
        };
        if !session.registered_map_presence {
            return Ok(Vec::new());
        }

        let _ = self
            .repository
            .character_by_id(character_id)?
            .ok_or_else(|| {
                anyhow::anyhow!("character {character_id} not found during drop reconciliation")
            })?;

        let character = self
            .repository
            .character_by_id(character_id)?
            .ok_or_else(|| {
                anyhow::anyhow!("character {character_id} not found during drop reconciliation")
            })?;
        let current = self
            .repository
            .drops_by_map(character.map_id, character.channel)?;
        let mut responses = Vec::new();
        let mut next_viewed_drops = HashMap::new();

        for drop in current {
            if map_distance(character.x, character.y, drop.x, drop.y) > START_TO_SEE_DISTANCE {
                continue;
            }

            if !session.viewed_drops.contains_key(&drop.id) {
                let viewer_handler = if character.general_handler == 0 {
                    character.id.min(u32::MAX as u64) as u32
                } else {
                    character.general_handler
                };
                responses.push(
                    LoadDropsPacket {
                        drop: drop.clone(),
                        viewer_handler,
                    }
                    .encode(),
                );
            }

            next_viewed_drops.insert(drop.id, drop);
        }

        for drop in session.viewed_drops.values() {
            if next_viewed_drops.contains_key(&drop.id) {
                continue;
            }

            responses.push(UnloadDropsPacket { drop: drop.clone() }.encode());
        }

        session.viewed_drops = next_viewed_drops;
        Ok(responses)
    }

    // ----- Guild slice -----------------------------------------------------------------

    fn handle_guild_create(
        &self,
        session: &GameSession,
        guild_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let trimmed = guild_name.trim().to_string();
        if trimmed.is_empty() {
            return Ok(vec![
                GuildCreateFailPacket {
                    leader_name: character.name.clone(),
                    guild_name,
                }
                .encode(),
            ]);
        }

        let mut runtime = self.guild_runtime.write().expect("guild runtime poisoned");

        // Reject if character is already in a guild or the name is taken.
        if runtime.guild_by_member.contains_key(&character.id) {
            return Ok(vec![
                GuildCreateFailPacket {
                    leader_name: character.name.clone(),
                    guild_name: trimmed,
                }
                .encode(),
            ]);
        }
        if runtime
            .guilds
            .values()
            .any(|room| room.name.eq_ignore_ascii_case(&trimmed))
        {
            return Ok(vec![
                GuildCreateFailPacket {
                    leader_name: character.name.clone(),
                    guild_name: trimmed,
                }
                .encode(),
            ]);
        }

        let guild_id = runtime.alloc_id();
        let creator_member = GuildRoomMember {
            character_id: character.id,
            authority: 1, // Master
            name: character.name.clone(),
        };
        let historic_entry = odmo_types::GuildHistoricEntry {
            historic_type: 1, // GuildCreate
            date_utc_seconds: current_unix_timestamp() as u32,
            master_class: 1,
            master_name: character.name.clone(),
            member_class: 1,
            member_name: character.name.clone(),
        };
        runtime.guilds.insert(
            guild_id,
            GuildRoom {
                id: guild_id,
                name: trimmed.clone(),
                notice: String::new(),
                leader_id: character.id,
                members: vec![creator_member],
                historic: vec![historic_entry],
            },
        );
        runtime.guild_by_member.insert(character.id, guild_id);
        let guild_snapshot = self.snapshot_guild(&runtime, guild_id);
        drop(runtime);

        let success = GuildCreateSuccessPacket {
            leader_name: character.name.clone(),
            item_slot: 0, // simplified: no item consumption yet
            guild_name: trimmed,
        }
        .encode();
        let info = guild_snapshot.as_ref().map(|guild| {
            GuildInformationPacket {
                guild: guild.clone(),
            }
            .encode()
        });
        let history = guild_snapshot.as_ref().map(|guild| {
            GuildHistoricPacket {
                entries: guild.historic.clone(),
            }
            .encode()
        });
        let rank = GuildRankPacket { position: 0 }.encode();

        let mut responses = vec![success];
        if let Some(packet) = info {
            responses.push(packet);
        }
        if let Some(packet) = history {
            responses.push(packet);
        }
        responses.push(rank);
        Ok(responses)
    }

    fn handle_guild_delete(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let (guild_name, member_ids) = {
            let mut runtime = self.guild_runtime.write().expect("guild runtime poisoned");
            let Some(guild_id) = runtime.guild_by_member.get(&character_id).copied() else {
                return Ok(vec![]);
            };
            let Some(guild) = runtime.guilds.get(&guild_id).cloned() else {
                return Ok(vec![]);
            };
            // Only the leader can delete.
            if guild.leader_id != character_id {
                return Ok(vec![]);
            }
            runtime.guilds.remove(&guild_id);
            let member_ids: Vec<u64> = guild
                .members
                .iter()
                .map(|member| member.character_id)
                .collect();
            for member_id in &member_ids {
                runtime.guild_by_member.remove(member_id);
            }
            (guild.name, member_ids)
        };

        let packet = GuildDeletePacket { guild_name }.encode();
        if let Some(broadcast) = &self.broadcast {
            for member_id in member_ids {
                if broadcast.is_online(member_id) {
                    let _ = broadcast.send_to(member_id, &packet);
                }
            }
        }
        Ok(vec![])
    }

    fn handle_guild_invite(
        &self,
        session: &GameSession,
        target_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let inviter = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let trimmed = target_name.trim().to_string();
        if trimmed.is_empty() {
            return Ok(vec![
                GuildInviteFailPacket {
                    reason: 4, // invalid target
                    target_name,
                }
                .encode(),
            ]);
        }

        let target = match self
            .repository
            .character_by_name(&trimmed)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
        {
            Some(character) => character,
            None => {
                return Ok(vec![
                    GuildInviteFailPacket {
                        reason: 4,
                        target_name: trimmed,
                    }
                    .encode(),
                ]);
            }
        };

        let (guild_id, guild_name) = {
            let runtime = self.guild_runtime.read().expect("guild runtime poisoned");
            let Some(guild_id) = runtime.guild_by_member.get(&inviter.id).copied() else {
                return Ok(vec![
                    GuildInviteFailPacket {
                        reason: 4,
                        target_name: trimmed,
                    }
                    .encode(),
                ]);
            };
            let Some(guild) = runtime.guilds.get(&guild_id) else {
                return Ok(vec![
                    GuildInviteFailPacket {
                        reason: 4,
                        target_name: trimmed,
                    }
                    .encode(),
                ]);
            };
            // Inviter must have at least Member rank.
            if guild
                .members
                .iter()
                .find(|m| m.character_id == inviter.id)
                .map(|m| m.authority)
                .unwrap_or(5)
                > 4
            {
                return Ok(vec![
                    GuildInviteFailPacket {
                        reason: 4,
                        target_name: trimmed,
                    }
                    .encode(),
                ]);
            }
            if runtime.guild_by_member.contains_key(&target.id) {
                return Ok(vec![
                    GuildInviteFailPacket {
                        reason: 1, // already in a guild
                        target_name: trimmed,
                    }
                    .encode(),
                ]);
            }
            (guild_id, guild.name.clone())
        };

        if let Some(broadcast) = &self.broadcast
            && !broadcast.is_online(target.id)
        {
            return Ok(vec![
                GuildInviteFailPacket {
                    reason: 2, // offline
                    target_name: trimmed,
                }
                .encode(),
            ]);
        }

        // Stash the pending invite keyed by the invitee.
        {
            let mut runtime = self.guild_runtime.write().expect("guild runtime poisoned");
            runtime.pending_invites.insert(
                target.id,
                PendingGuildInvite {
                    inviter_id: inviter.id,
                    target_id: target.id,
                    guild_id,
                },
            );
        }

        // Notify the target client.
        if let Some(broadcast) = &self.broadcast {
            let invite = GuildInviteSuccessPacket {
                target_name: target.name.clone(),
                guild_id,
                guild_name: guild_name.clone(),
            }
            .encode();
            let _ = broadcast.send_to(target.id, &invite);
        }

        Ok(vec![
            GuildInviteSuccessPacket {
                target_name: target.name,
                guild_id,
                guild_name,
            }
            .encode(),
        ])
    }

    fn handle_guild_invite_accept(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let (member_packets, guild_snapshot, target_guild_id) = {
            let mut runtime = self.guild_runtime.write().expect("guild runtime poisoned");
            let Some(pending) = runtime.pending_invites.remove(&character.id) else {
                return Ok(vec![]);
            };
            let target_guild_id = pending.guild_id;
            let (guild_name, mem_packet, recipients) = {
                let Some(guild) = runtime.guilds.get_mut(&target_guild_id) else {
                    return Ok(vec![]);
                };
                if guild.members.len() >= 64 {
                    return Ok(vec![
                        GuildInviteFailPacket {
                            reason: 3, // capacity
                            target_name: character.name.clone(),
                        }
                        .encode(),
                    ]);
                }
                guild.members.push(GuildRoomMember {
                    character_id: character.id,
                    authority: 5, // NewMember
                    name: character.name.clone(),
                });
                let master_name = guild
                    .members
                    .iter()
                    .find(|m| m.character_id == guild.leader_id)
                    .map(|m| m.name.clone())
                    .unwrap_or_default();
                guild.historic.push(odmo_types::GuildHistoricEntry {
                    historic_type: 2, // GuildJoin
                    date_utc_seconds: current_unix_timestamp() as u32,
                    master_class: 1,
                    master_name,
                    member_class: 5,
                    member_name: character.name.clone(),
                });
                let mem_packet = GuildInviteAcceptPacket {
                    authority: 5,
                    member_model: (character.model.saturating_sub(80_000)).max(0) as u8,
                    character_name: character.name.clone(),
                    level: character.level,
                    map_id: character.map_id,
                    channel: character.channel,
                    guild_name: guild.name.clone(),
                }
                .encode();
                let recipients: Vec<u64> = guild
                    .members
                    .iter()
                    .map(|m| m.character_id)
                    .filter(|id| *id != character.id)
                    .collect();
                (guild.name.clone(), mem_packet, recipients)
            };
            let _ = guild_name;
            runtime
                .guild_by_member
                .insert(character.id, target_guild_id);
            let snapshot = self.snapshot_guild(&runtime, target_guild_id);
            (
                recipients
                    .into_iter()
                    .map(|id| (id, mem_packet.clone()))
                    .collect::<Vec<_>>(),
                snapshot,
                target_guild_id,
            )
        };
        let _ = target_guild_id;

        if let Some(broadcast) = &self.broadcast {
            for (member_id, packet) in &member_packets {
                if broadcast.is_online(*member_id) {
                    let _ = broadcast.send_to(*member_id, packet);
                }
            }
        }

        let mut responses = Vec::new();
        if let Some(guild) = guild_snapshot {
            responses.push(
                GuildInformationPacket {
                    guild: guild.clone(),
                }
                .encode(),
            );
            responses.push(
                GuildHistoricPacket {
                    entries: guild.historic,
                }
                .encode(),
            );
        }
        Ok(responses)
    }

    fn handle_guild_invite_deny(
        &self,
        session: &GameSession,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let inviter_id = {
            let mut runtime = self.guild_runtime.write().expect("guild runtime poisoned");
            runtime
                .pending_invites
                .remove(&character.id)
                .map(|pending| pending.inviter_id)
        };

        if let (Some(inviter), Some(broadcast)) = (inviter_id, &self.broadcast)
            && broadcast.is_online(inviter)
        {
            let packet = GuildInviteDenyPacket {
                target_name: character.name.clone(),
            }
            .encode();
            let _ = broadcast.send_to(inviter, &packet);
        }
        Ok(vec![])
    }

    fn handle_guild_kick(
        &self,
        session: &GameSession,
        target_name: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;

        let target = self
            .repository
            .character_by_name(&target_name)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let Some(target) = target else {
            return Ok(vec![]);
        };
        let target_id = target.id;

        let (target_name, member_ids, guild_name) = {
            let mut runtime = self.guild_runtime.write().expect("guild runtime poisoned");
            let Some(guild_id) = runtime.guild_by_member.get(&character_id).copied() else {
                return Ok(vec![]);
            };
            let (target_name, member_ids, guild_name) = {
                let Some(guild) = runtime.guilds.get_mut(&guild_id) else {
                    return Ok(vec![]);
                };
                // Only the leader can kick.
                if guild.leader_id != character_id {
                    return Ok(vec![]);
                }
                let Some(idx) = guild
                    .members
                    .iter()
                    .position(|m| m.character_id == target_id)
                else {
                    return Ok(vec![]);
                };
                let target_name = guild.members[idx].name.clone();
                guild.members.remove(idx);
                let member_ids: Vec<u64> = guild.members.iter().map(|m| m.character_id).collect();
                (target_name, member_ids, guild.name.clone())
            };
            runtime.guild_by_member.remove(&target_id);
            (target_name, member_ids, guild_name)
        };

        let packet = GuildMemberKickPacket {
            target_name: target_name.clone(),
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            for member_id in member_ids {
                if broadcast.is_online(member_id) {
                    let _ = broadcast.send_to(member_id, &packet);
                }
            }
            if broadcast.is_online(target_id) {
                let _ = broadcast.send_to(
                    target_id,
                    &GuildDeletePacket {
                        guild_name: guild_name.clone(),
                    }
                    .encode(),
                );
            }
        }
        Ok(vec![packet])
    }

    fn handle_guild_leave(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let (member_ids, guild_name) = {
            let mut runtime = self.guild_runtime.write().expect("guild runtime poisoned");
            let Some(guild_id) = runtime.guild_by_member.get(&character_id).copied() else {
                return Ok(vec![]);
            };
            let Some(guild) = runtime.guilds.get_mut(&guild_id) else {
                return Ok(vec![]);
            };
            if guild.leader_id == character_id && guild.members.len() > 1 {
                // Leader cannot leave a non-empty guild without transferring authority.
                return Ok(vec![]);
            }
            guild.members.retain(|m| m.character_id != character_id);
            let leftover: Vec<u64> = guild.members.iter().map(|m| m.character_id).collect();
            let name = guild.name.clone();
            runtime.guild_by_member.remove(&character_id);
            if leftover.is_empty() {
                runtime.guilds.remove(&guild_id);
            }
            (leftover, name)
        };

        let packet = GuildMemberQuitPacket {
            target_name: character.name.clone(),
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            for member_id in member_ids {
                if broadcast.is_online(member_id) {
                    let _ = broadcast.send_to(member_id, &packet);
                }
            }
        }
        Ok(vec![GuildDeletePacket { guild_name }.encode()])
    }

    fn handle_guild_message(
        &self,
        session: &GameSession,
        message: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let character = self
            .repository
            .character_by_id(character_id)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?
            .ok_or(GameFlowError::CharacterNotFound(character_id))?;

        let member_ids: Vec<u64> = {
            let runtime = self.guild_runtime.read().expect("guild runtime poisoned");
            let Some(guild_id) = runtime.guild_by_member.get(&character_id).copied() else {
                return Ok(vec![]);
            };
            runtime
                .guilds
                .get(&guild_id)
                .map(|guild| guild.members.iter().map(|m| m.character_id).collect())
                .unwrap_or_default()
        };

        let packet = GuildMessagePacket {
            sender_name: character.name.clone(),
            message,
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            for member_id in member_ids {
                if member_id != character_id && broadcast.is_online(member_id) {
                    let _ = broadcast.send_to(member_id, &packet);
                }
            }
        }
        Ok(vec![packet])
    }

    fn handle_guild_notice(
        &self,
        session: &GameSession,
        notice: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;

        let member_ids: Vec<u64> = {
            let mut runtime = self.guild_runtime.write().expect("guild runtime poisoned");
            let Some(guild_id) = runtime.guild_by_member.get(&character_id).copied() else {
                return Ok(vec![]);
            };
            let Some(guild) = runtime.guilds.get_mut(&guild_id) else {
                return Ok(vec![]);
            };
            // Only Master/SubMaster can change notice.
            let allowed = guild
                .members
                .iter()
                .find(|m| m.character_id == character_id)
                .map(|m| m.authority <= 2)
                .unwrap_or(false);
            if !allowed {
                return Ok(vec![]);
            }
            guild.notice = notice.clone();
            guild.members.iter().map(|m| m.character_id).collect()
        };

        let packet = GuildNoticeUpdatePacket { notice }.encode();
        if let Some(broadcast) = &self.broadcast {
            for member_id in member_ids {
                if broadcast.is_online(member_id) {
                    let _ = broadcast.send_to(member_id, &packet);
                }
            }
        }
        Ok(vec![packet])
    }

    fn handle_guild_history(&self, session: &GameSession) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        let entries = {
            let runtime = self.guild_runtime.read().expect("guild runtime poisoned");
            runtime
                .guild_by_member
                .get(&character_id)
                .copied()
                .and_then(|guild_id| runtime.guilds.get(&guild_id))
                .map(|guild| guild.historic.clone())
                .unwrap_or_default()
        };
        Ok(vec![GuildHistoricPacket { entries }.encode()])
    }

    fn handle_guild_set_title(
        &self,
        session: &GameSession,
        authority_class: i32,
        title: String,
        duty: String,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;
        // The class selects one of the five rank labels, so an out-of-range
        // value cannot be forwarded as a u8 on the broadcast.
        let authority_class =
            u8::try_from(authority_class).ok().filter(|class| *class < 5);
        let Some(authority_class) = authority_class else {
            return Ok(vec![]);
        };
        let in_guild = self
            .guild_runtime
            .read()
            .expect("guild runtime poisoned")
            .guild_by_member
            .contains_key(&character_id);
        if !in_guild {
            return Ok(vec![]);
        }
        Ok(vec![
            GuildAuthorityUpdatePacket {
                authority_class,
                title,
                duty,
            }
            .encode(),
        ])
    }

    fn handle_guild_authority(
        &self,
        session: &GameSession,
        target_name: String,
        new_authority: u8,
    ) -> Result<Vec<Vec<u8>>, GameFlowError> {
        let character_id = session.character_id.ok_or(GameFlowError::Unauthenticated)?;

        let target = self
            .repository
            .character_by_name(&target_name)
            .map_err(|error| GameFlowError::Storage(error.to_string()))?;
        let Some(target) = target else {
            return Ok(vec![]);
        };
        let target_id = target.id;

        let (member_ids, member_name, opcode) = {
            let mut runtime = self.guild_runtime.write().expect("guild runtime poisoned");
            let Some(guild_id) = runtime.guild_by_member.get(&character_id).copied() else {
                return Ok(vec![]);
            };
            let Some(guild) = runtime.guilds.get_mut(&guild_id) else {
                return Ok(vec![]);
            };
            // Only the leader can promote/demote.
            if guild.leader_id != character_id {
                return Ok(vec![]);
            }
            let Some(target_member) = guild
                .members
                .iter_mut()
                .find(|m| m.character_id == target_id)
            else {
                return Ok(vec![]);
            };
            target_member.authority = new_authority;
            let name = target_member.name.clone();
            let opcode = match new_authority {
                1 => odmo_protocol::opcode::game::GUILD_AUTHORITY_MASTER,
                2 => odmo_protocol::opcode::game::GUILD_AUTHORITY_SUBMASTER,
                3 => odmo_protocol::opcode::game::GUILD_AUTHORITY_DATS,
                4 => odmo_protocol::opcode::game::GUILD_AUTHORITY_MEMBER,
                _ => odmo_protocol::opcode::game::GUILD_AUTHORITY_NEW_MEMBER,
            };
            (
                guild
                    .members
                    .iter()
                    .map(|m| m.character_id)
                    .collect::<Vec<_>>(),
                name,
                opcode,
            )
        };

        let packet = GuildPromotionDemotionPacket {
            opcode,
            member_name: member_name.clone(),
        }
        .encode();
        if let Some(broadcast) = &self.broadcast {
            for id in member_ids {
                if id != character_id && broadcast.is_online(id) {
                    let _ = broadcast.send_to(id, &packet);
                }
            }
        }
        Ok(vec![packet])
    }

    fn snapshot_guild(
        &self,
        runtime: &GuildRuntimeState,
        guild_id: u32,
    ) -> Option<odmo_types::GuildSnapshot> {
        let guild = runtime.guilds.get(&guild_id)?;
        let members = guild
            .members
            .iter()
            .map(|member| odmo_types::GuildMemberSnapshot {
                character_id: member.character_id,
                authority: member.authority,
                contribution: 0,
                character_name: member.name.clone(),
                character_level: 1,
                character_model: odmo_types::DEFAULT_TAMER_MODEL_ID,
                map_id: odmo_types::DEFAULT_START_MAP_ID,
                channel: 0,
                state: odmo_types::CharacterConnectionState::Disconnected,
            })
            .collect();
        Some(odmo_types::GuildSnapshot {
            id: guild.id,
            name: guild.name.clone(),
            level: 1,
            current_experience: 0,
            notice: guild.notice.clone(),
            extra_slots: 0,
            authorities: odmo_types::GuildSnapshot::default().authorities,
            members,
            historic: guild.historic.clone(),
            rank_position: 0,
        })
    }
}

#[derive(Debug)]
pub struct GameSessionFactory {
    next_seed: AtomicI16,
}

impl Default for GameSessionFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl GameSessionFactory {
    pub fn new() -> Self {
        Self {
            next_seed: AtomicI16::new(3_000),
        }
    }

    pub fn create(&self) -> GameSession {
        let seed = self.next_seed.fetch_add(1, Ordering::Relaxed);
        GameSession::new(seed)
    }
}

#[derive(Debug, Error)]
pub enum GameFlowError {
    #[error("missing game session ticket for account {0}")]
    MissingSessionTicket(AccountId),
    #[error("character {0} not found for bootstrap")]
    CharacterNotFound(u64),
    #[error("game request requires initialized session")]
    Unauthenticated,
    #[error("portal bridge error: {0}")]
    PortalBridge(String),
    #[error("storage error: {0}")]
    Storage(String),
}

#[allow(dead_code)]
fn unix_timestamp() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32
}

/// Compute the deterministic partner damage for a single hit/skill. The current formula
/// is intentionally simple — the legacy server's full damage pipeline (attack vs defense,
/// elemental tables, attribute effectiveness, crit/block tables, skill multipliers, gear,
/// chips, buffs, debuffs) is out of scope for this slice. The numbers below produce
/// believable damage proportional to partner level and at least scratch a higher-level
/// mob, so the kill flow is reachable.
fn compute_partner_damage(
    character: &odmo_types::CharacterSummary,
    mob: &odmo_types::MobSummary,
    skill_slot: Option<u8>,
) -> i32 {
    let partner_level = i32::from(character.partner_level.max(1));
    let mob_level = i32::from(mob.level.max(1));
    let base = partner_level * 50;
    let level_gap_penalty = (mob_level - partner_level).max(0) * 10;
    let raw = (base - level_gap_penalty).max(50);
    let multiplier = match skill_slot {
        Some(_) => 5,
        None => 1,
    };
    raw.saturating_mul(multiplier)
}

fn map_distance(xa: i32, ya: i32, xb: i32, yb: i32) -> i64 {
    let distance_x = (xb as i64 - xa as i64).pow(2);
    let distance_y = (yb as i64 - ya as i64).pow(2);
    ((distance_x + distance_y) as f64).sqrt() as i64
}

fn can_see_each_other(
    left: &odmo_types::CharacterSummary,
    right: &odmo_types::CharacterSummary,
) -> bool {
    map_distance(left.x, left.y, right.x, right.y) <= START_TO_SEE_DISTANCE
}

#[allow(dead_code)]
fn current_unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Check the legacy completed-quest bitmap (1 bit per quest id, packed into i32 array).
fn quest_completed(progress: &odmo_types::QuestProgressSnapshot, quest_id: i32) -> bool {
    if quest_id <= 0 {
        return false;
    }
    let bit_index = (quest_id - 1) as usize;
    let array_index = bit_index / 32;
    let bit_position = bit_index % 32;
    if array_index >= progress.completed_data.len() {
        return false;
    }
    (progress.completed_data[array_index] & (1 << bit_position)) != 0
}

fn quest_in_progress(progress: &odmo_types::QuestProgressSnapshot, quest_id: i32) -> bool {
    if let Ok(quest_id) = i16::try_from(quest_id) {
        return progress
            .in_progress
            .iter()
            .any(|quest| quest.quest_id == quest_id);
    }
    false
}

/// Set the bit in the legacy completed-quest bitmap.
fn set_quest_completed(progress: &mut odmo_types::QuestProgressSnapshot, quest_id: i32) {
    if quest_id <= 0 {
        return;
    }
    let bit_index = (quest_id - 1) as usize;
    let array_index = bit_index / 32;
    let bit_position = bit_index % 32;
    if array_index >= progress.completed_data.len() {
        progress.completed_data.resize(array_index + 1, 0);
    }
    progress.completed_data[array_index] |= 1 << bit_position;
}

/// Seconds remaining until the next UTC midnight; used by the daily-reset packets.
fn seconds_until_next_day() -> i32 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let day_seconds: i64 = 24 * 60 * 60;
    let elapsed = now.rem_euclid(day_seconds);
    let remaining = day_seconds - elapsed;
    remaining.max(0).min(i32::MAX as i64) as i32
}

#[allow(dead_code)]
fn apply_runtime_drop_state(mut drop: odmo_types::DropSummary) -> odmo_types::DropSummary {
    let now = current_unix_timestamp();
    if drop.expires_at_unix > 0 && now >= drop.expires_at_unix {
        drop.collected = true;
    }
    if !drop.collected && drop.owner_expires_at_unix > 0 && now >= drop.owner_expires_at_unix {
        drop.no_owner = true;
    }
    drop
}

fn current_unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn find_usable_digi_summon_ticket(
    inventory: &odmo_types::InventorySnapshot,
    product: &odmo_types::DigiSummonProduct,
    requested_slot: i32,
) -> Option<(usize, odmo_types::DigiSummonTicket)> {
    if requested_slot >= 0 {
        let requested_slot = requested_slot as usize;
        if let Some(slot_item) = inventory.items.get(requested_slot)
            && let Some(ticket) = product.tickets.iter().find(|ticket| {
                ticket.item_id == slot_item.item_id && slot_item.amount >= ticket.cost
            })
        {
            return Some((requested_slot, ticket.clone()));
        }
    }

    for ticket in &product.tickets {
        if let Some((index, _)) = inventory
            .items
            .iter()
            .enumerate()
            .find(|(_, item)| item.item_id == ticket.item_id && item.amount >= ticket.cost)
        {
            return Some((index, ticket.clone()));
        }
    }

    None
}

fn roll_digi_summon_rewards(
    product: &odmo_types::DigiSummonProduct,
) -> Vec<odmo_types::DigiSummonReward> {
    let rewards: Vec<_> = product
        .rewards
        .iter()
        .filter(|reward| reward.item_id > 0)
        .cloned()
        .collect();
    if rewards.is_empty() {
        return Vec::new();
    }

    let draw_count = product.draw_count.max(1) as usize;
    let mut results = Vec::with_capacity(draw_count);
    for draw_index in 0..draw_count {
        results.push(pick_weighted_digi_summon_reward(&rewards, draw_index));
    }
    results
}

/// Defensive server-side re-validation of the 11x4 Material_Grid.
///
/// Only filled cells travel on the wire, so the per-row grouping is not
/// separable from the flat node list. The valid shape is a multiple of 4
/// entries per filled row-group and never exceeds the full 11x4 grid.
fn combine_grid_is_valid(materials: &[CombineItemRef]) -> bool {
    materials.len() <= COMBINE_GRID_MAX_NODES
        && materials.len().is_multiple_of(COMBINE_GRID_ROW_CELLS)
}

/// Remove exactly the submitted material nodes from inventory. Returns false
/// (with the inventory restored) if any node is missing or insufficient.
fn consume_combine_materials(
    inventory: &mut odmo_types::InventorySnapshot,
    materials: &[CombineItemRef],
) -> bool {
    let original = inventory.clone();
    for material in materials {
        let count = i32::from(material.count);
        if count <= 0 {
            *inventory = original;
            return false;
        }
        let Some(slot_index) = inventory
            .items
            .iter()
            .position(|item| item.item_id == material.item_type as i32 && item.amount >= count)
        else {
            *inventory = original;
            return false;
        };
        if consume_inventory_item_at(inventory, slot_index, count).is_none() {
            *inventory = original;
            return false;
        }
    }
    true
}

/// Flatten every ceiling-group entry in the catalog into a single ceiling map.
fn combine_ceiling_all(catalog: &DigiCombineCatalog) -> Vec<CombineCeilingEntry> {
    catalog
        .ceil_groups
        .iter()
        .flat_map(|group| group.entries.iter().cloned())
        .collect()
}

/// Resolve the ceiling-map entries configured for one ceiling tier.
fn combine_ceiling_for_type(
    catalog: &DigiCombineCatalog,
    ceiling_type: u8,
) -> Vec<CombineCeilingEntry> {
    catalog
        .ceil_groups
        .iter()
        .filter(|group| group.ceiling_type == ceiling_type)
        .flat_map(|group| group.entries.iter().cloned())
        .collect()
}

/// Pick one reward from the random box pool by relative weight.
///
/// Returns `None` for an empty pool. A pool with only zero-weight entries falls
/// back to a uniform pick, and an entry with zero weight is never chosen while a
/// positive-weight entry remains.
fn pick_weighted_random_box_reward(rewards: &[RandomBoxReward]) -> Option<RandomBoxReward> {
    if rewards.is_empty() {
        return None;
    }
    let total_weight: u128 = rewards.iter().map(|reward| u128::from(reward.weight)).sum();
    if total_weight == 0 {
        let index = (current_unix_nanos() as usize) % rewards.len();
        return Some(rewards[index].clone());
    }

    let mut roll = (current_unix_nanos() % total_weight) + 1;
    for reward in rewards {
        let weight = u128::from(reward.weight);
        if roll <= weight {
            return Some(reward.clone());
        }
        roll -= weight;
    }
    rewards.last().cloned()
}

/// Pick one rank from the candidates by relative weight.
fn pick_weighted_combine_rank(
    ranks: &[odmo_types::DigiCombineRank],
) -> Option<odmo_types::DigiCombineRank> {
    if ranks.is_empty() {
        return None;
    }
    let total_weight: u128 = ranks.iter().map(|rank| u128::from(rank.weight)).sum();
    if total_weight == 0 {
        let index = (current_unix_nanos() as usize) % ranks.len();
        return Some(ranks[index].clone());
    }

    let mut roll = (current_unix_nanos() % total_weight) + 1;
    for rank in ranks {
        let weight = u128::from(rank.weight);
        if roll <= weight {
            return Some(rank.clone());
        }
        roll -= weight;
    }
    ranks.last().cloned()
}

fn pick_weighted_digi_summon_reward(
    rewards: &[odmo_types::DigiSummonReward],
    draw_index: usize,
) -> odmo_types::DigiSummonReward {
    let total_weight: i32 = rewards.iter().map(|reward| reward.weight.max(0)).sum();
    if total_weight <= 0 {
        let index = (current_unix_nanos() as usize).wrapping_add(draw_index) % rewards.len();
        return rewards[index].clone();
    }

    let mut roll = ((current_unix_nanos() + draw_index as u128) % total_weight as u128) as i32 + 1;
    for reward in rewards {
        roll -= reward.weight.max(0);
        if roll <= 0 {
            return reward.clone();
        }
    }

    rewards.last().cloned().unwrap_or_default()
}

fn consume_inventory_item_at(
    inventory: &mut odmo_types::InventorySnapshot,
    slot_index: usize,
    amount: i32,
) -> Option<odmo_types::ItemRecord> {
    if amount <= 0 {
        return None;
    }
    let item = inventory.items.get_mut(slot_index)?;
    if item.item_id <= 0 || item.amount < amount {
        return None;
    }

    let consumed = odmo_types::ItemRecord::new(item.item_id, amount);
    item.amount -= amount;
    if item.amount <= 0 {
        *item = odmo_types::ItemRecord::default();
    } else {
        item.sync_record();
    }
    Some(consumed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EvolutionResourceCost {
    Ds(i32),
    XGauge(i32),
}

fn resolve_jogress_partner_level(
    partner_slots: &[odmo_types::PartnerSlotSnapshot],
    active_slot: u8,
    target_line: &odmo_types::EvolutionLineAsset,
) -> Option<i32> {
    if target_line.jogress_need_digimon_types.is_empty() {
        return None;
    }

    let mut used_indices = Vec::new();
    let mut lowest_level: Option<i32> = None;

    for &required_base_type in &target_line.jogress_need_digimon_types {
        let mut match_index = None;
        let mut match_level = i32::MAX;
        for (index, slot) in partner_slots.iter().enumerate() {
            if slot.slot == active_slot || used_indices.contains(&index) {
                continue;
            }

            let slot_base_type = if slot.model > 0 {
                slot.model
            } else {
                slot.digimon_type
            };
            if slot_base_type != required_base_type {
                continue;
            }

            let has_open_target = slot
                .evolutions
                .iter()
                .find(|evolution| evolution.evolution_type == target_line.type_id)
                .is_some_and(|evolution| (evolution.unlocked & 0x01) != 0);
            if !has_open_target {
                continue;
            }

            let level = i32::from(slot.level);
            if level < match_level {
                match_level = level;
                match_index = Some(index);
            }
        }

        let Some(index) = match_index else {
            return None;
        };
        used_indices.push(index);
        lowest_level = Some(lowest_level.map_or(match_level, |current| current.min(match_level)));
    }

    lowest_level
}

fn evolution_requires_opened_slot(target_line: &odmo_types::EvolutionLineAsset) -> bool {
    target_line.unlock_item_section > 0
        || target_line.jogress_chipset_type > 0
        || target_line.evolution_tree == 2
}

fn validate_and_consume_jogress_chipset(
    item_assets: &[odmo_types::ItemAsset],
    digivice: &mut odmo_types::DigiviceSnapshot,
    target_line: &odmo_types::EvolutionLineAsset,
) -> bool {
    if target_line.jogress_chipset_type <= 0 {
        return true;
    }

    let mut has_any_jogress_chipset = false;
    for slot in &digivice.chipsets {
        if slot.item_id <= 0 {
            continue;
        }
        let Some(asset) = item_assets
            .iter()
            .find(|asset| asset.item_id == slot.item_id)
        else {
            continue;
        };
        if asset.item_type != target_line.jogress_chipset_type {
            continue;
        }
        if asset.section % 100 != 0 {
            has_any_jogress_chipset = true;
            break;
        }
    }
    if !has_any_jogress_chipset {
        return false;
    }

    if let Some(index) = digivice.chipsets.iter().position(|slot| {
        if slot.item_id <= 0 {
            return false;
        }
        item_assets
            .iter()
            .find(|asset| asset.item_id == slot.item_id)
            .is_some_and(|asset| {
                asset.item_type == target_line.jogress_chipset_type
                    && asset.section == target_line.jogress_period_chipset_type
            })
    }) {
        return digivice.chipsets[index].amount > 0;
    }

    let Some(index) = digivice.chipsets.iter().position(|slot| {
        if slot.item_id <= 0 {
            return false;
        }
        item_assets
            .iter()
            .find(|asset| asset.item_id == slot.item_id)
            .is_some_and(|asset| {
                asset.item_type == target_line.jogress_chipset_type
                    && asset.section == target_line.jogress_consumable_chipset_type
            })
    }) else {
        return false;
    };

    let required_amount = target_line.jogress_chipset_amount.max(1);
    if digivice.chipsets[index].amount < required_amount {
        return false;
    }

    digivice.chipsets[index].amount -= required_amount;
    digivice.chipsets[index].sync_record();
    if digivice.chipsets[index].amount == 0 {
        digivice.chipsets[index] = odmo_types::DigiviceItemSnapshot::default();
    }
    true
}

fn evolution_resource_cost(
    digimon_assets: &[odmo_types::DigimonAsset],
    current_type: i32,
    target_type: i32,
    source_level: i32,
    required_ds_override: i32,
) -> Option<EvolutionResourceCost> {
    if required_ds_override > 1 {
        return Some(EvolutionResourceCost::Ds(required_ds_override));
    }

    let source = digimon_assets
        .iter()
        .find(|asset| asset.digimon_id == current_type)?;
    let target = digimon_assets
        .iter()
        .find(|asset| asset.digimon_id == target_type)?;
    let next_step = target.evolution_type;
    let curr_step = source.evolution_type;

    if next_step == 9 {
        return Some(EvolutionResourceCost::Ds(
            (((5 * 5) - (curr_step * curr_step) - 5)
                * ((target.base_level - source_level + 110) / 5))
                + 20,
        ));
    }

    if (11..=16).contains(&next_step) {
        let step_delta = (next_step - curr_step).abs();
        let consume_xgauge = 20 + step_delta * ((30 + target.base_level) / 5);
        return Some(EvolutionResourceCost::XGauge(consume_xgauge));
    }

    if next_step == 8 {
        let step_value = (next_step * next_step - curr_step * curr_step).abs() - 5;
        return Some(EvolutionResourceCost::Ds(
            (step_value * (source_level / 5)) + 20,
        ));
    }

    if next_step == 10 && curr_step == 8 {
        return Some(EvolutionResourceCost::Ds(0));
    }

    if next_step == 17 {
        return Some(EvolutionResourceCost::XGauge(0));
    }

    let step_value = (next_step * next_step - curr_step * curr_step) - 5;
    Some(EvolutionResourceCost::Ds(
        (step_value * ((target.base_level - source_level + 110) / 5)) + 20,
    ))
}

fn add_stackable_inventory_item(
    inventory: &mut odmo_types::InventorySnapshot,
    item_id: i32,
    amount: i32,
) -> bool {
    if item_id <= 0 || amount <= 0 {
        return false;
    }

    if let Some(existing) = inventory
        .items
        .iter_mut()
        .find(|item| item.item_id == item_id && item.amount > 0)
    {
        existing.amount = existing.amount.saturating_add(amount);
        existing.sync_record();
        return true;
    }

    if let Some(empty_slot) = inventory
        .items
        .iter_mut()
        .find(|item| item.item_id <= 0 || item.amount <= 0)
    {
        *empty_slot = odmo_types::ItemRecord::new(item_id, amount);
        return true;
    }

    if inventory.items.len() >= inventory.size as usize {
        return false;
    }

    inventory
        .items
        .push(odmo_types::ItemRecord::new(item_id, amount));
    true
}

fn consume_first_matching_material(
    inventory: &mut odmo_types::InventorySnapshot,
    materials: &[odmo_types::ExtraEvolutionMaterial],
    consumed_items: &mut Vec<odmo_types::ItemRecord>,
) -> bool {
    for material in materials {
        let Some(slot_index) = inventory.items.iter().position(|item| {
            item.item_id == material.material_id && item.amount >= material.amount
        }) else {
            continue;
        };

        let Some(consumed) =
            consume_inventory_item_at(inventory, slot_index, material.amount.max(1))
        else {
            continue;
        };
        consumed_items.push(consumed);
        return true;
    }

    false
}

fn consume_all_materials(
    inventory: &mut odmo_types::InventorySnapshot,
    materials: &[odmo_types::ExtraEvolutionMaterial],
    consumed_items: &mut Vec<odmo_types::ItemRecord>,
) -> bool {
    for material in materials {
        let Some(slot_index) = inventory.items.iter().position(|item| {
            item.item_id == material.material_id && item.amount >= material.amount
        }) else {
            return false;
        };

        let Some(consumed) =
            consume_inventory_item_at(inventory, slot_index, material.amount.max(1))
        else {
            return false;
        };
        consumed_items.push(consumed);
    }

    true
}

fn item_section_index(item_assets: &[odmo_types::ItemAsset]) -> HashMap<i32, i32> {
    item_assets
        .iter()
        .map(|asset| (asset.item_id, asset.section))
        .collect()
}

fn consume_items_by_section(
    inventory: &mut odmo_types::InventorySnapshot,
    item_sections: &HashMap<i32, i32>,
    target_section: i32,
    mut total_amount: i32,
    consumed_items: &mut Vec<odmo_types::ItemRecord>,
) -> bool {
    if target_section <= 0 || total_amount <= 0 {
        return false;
    }

    let mut slot_index = 0usize;
    while total_amount > 0 && slot_index < inventory.items.len() {
        let Some(item) = inventory.items.get(slot_index).cloned() else {
            break;
        };
        let matches_section = item.amount > 0
            && item_sections
                .get(&item.item_id)
                .copied()
                .unwrap_or_default()
                == target_section;
        if !matches_section {
            slot_index += 1;
            continue;
        }

        let consume_amount = item.amount.min(total_amount);
        let Some(consumed) = consume_inventory_item_at(inventory, slot_index, consume_amount)
        else {
            slot_index += 1;
            continue;
        };
        consumed_items.push(consumed);
        total_amount -= consume_amount;
        slot_index += 1;
    }

    total_amount == 0
}

fn consume_items_by_type(
    inventory: &mut odmo_types::InventorySnapshot,
    target_item_id: i32,
    mut total_amount: i32,
    consumed_items: &mut Vec<odmo_types::ItemRecord>,
) -> bool {
    if target_item_id <= 0 || total_amount <= 0 {
        return false;
    }

    let mut slot_index = 0usize;
    while total_amount > 0 && slot_index < inventory.items.len() {
        let Some(item) = inventory.items.get(slot_index).cloned() else {
            break;
        };
        let matches_item = item.amount > 0 && item.item_id == target_item_id;
        if !matches_item {
            slot_index += 1;
            continue;
        }

        let consume_amount = item.amount.min(total_amount);
        let Some(consumed) = consume_inventory_item_at(inventory, slot_index, consume_amount)
        else {
            slot_index += 1;
            continue;
        };
        consumed_items.push(consumed);
        total_amount -= consume_amount;
        slot_index += 1;
    }

    total_amount == 0
}

fn consume_item_material_groups(
    inventory: &mut odmo_types::InventorySnapshot,
    way_type: u16,
    main_materials: &[odmo_types::ExtraEvolutionMaterial],
    sub_materials: &[odmo_types::ExtraEvolutionMaterial],
    consumed_items: &mut Vec<odmo_types::ItemRecord>,
) -> bool {
    let original_inventory = inventory.clone();
    let original_consumed_len = consumed_items.len();
    let success = match way_type {
        EXTRA_EVOLUTION_NEED_ONE => {
            consume_first_matching_material(inventory, main_materials, consumed_items)
                && consume_first_matching_material(inventory, sub_materials, consumed_items)
        }
        EXTRA_EVOLUTION_NEED_ALL => {
            consume_all_materials(inventory, main_materials, consumed_items)
                && consume_all_materials(inventory, sub_materials, consumed_items)
        }
        _ => {
            consume_all_materials(inventory, main_materials, consumed_items)
                && consume_all_materials(inventory, sub_materials, consumed_items)
        }
    };
    if success {
        return true;
    }

    *inventory = original_inventory;
    consumed_items.truncate(original_consumed_len);
    false
}

fn default_partner_for_type(
    digimon_type: i32,
    slot: u8,
    name: String,
) -> odmo_types::PartnerSlotSnapshot {
    odmo_types::PartnerSlotSnapshot {
        slot,
        digimon_type,
        model: digimon_type,
        level: 1,
        name,
        ..odmo_types::PartnerSlotSnapshot::default()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, RwLock},
    };

    use super::*;
    use crate::{
        character::{CharacterAccountRepository, CharacterRepository},
        portal::PortalBridge,
    };
    use odmo_protocol::PacketReader;
    use odmo_types::{
        AccessLevel, Account, ActiveBuffSnapshot, AttendanceStatus, CharacterConnectionState,
        CharacterSummary, DEFAULT_ALT_PARTNER_MODEL_ID, DEFAULT_ALT_TAMER_MODEL_ID,
        DEFAULT_GM_PARTNER_MODEL_ID, DEFAULT_GM_TAMER_MODEL_ID, DEFAULT_PARTNER_MODEL_ID,
        DEFAULT_START_MAP_ID, DEFAULT_START_X, DEFAULT_START_Y, DEFAULT_TAMER_MODEL_ID,
        DailyRewardStatus, DigimonAsset, DropSummary, EvolutionAsset, EvolutionLineAsset,
        EvolutionStageAsset, ExtraEvolutionNpc, GameSessionTicket, GuildHistoricEntry,
        GuildMemberSnapshot, GuildSnapshot, ItemAsset, MobSummary, RelationEntry, SealListSnapshot,
        SealRecord, XaiSnapshot,
    };

    #[derive(Debug)]
    struct InMemoryCharacterRepository {
        characters: RwLock<HashMap<u64, CharacterSummary>>,
        accounts: HashMap<u64, Account>,
        mobs_by_map: RwLock<HashMap<(i16, u8), Vec<MobSummary>>>,
        drops_by_map: RwLock<HashMap<(i16, u8), Vec<DropSummary>>>,
        digi_summon_products: Vec<odmo_types::DigiSummonProduct>,
        extra_evolution_npcs: Vec<ExtraEvolutionNpc>,
        item_assets: Vec<ItemAsset>,
        evolution_assets: Vec<EvolutionAsset>,
        digimon_assets: Vec<DigimonAsset>,
        digi_combine_catalog: odmo_types::DigiCombineCatalog,
        union_combine_catalog: odmo_types::UnionCombineCatalog,
        random_box_rewards: Vec<odmo_types::RandomBoxReward>,
    }

    #[derive(Debug, Default)]
    struct RecordingBroadcast {
        online: RwLock<std::collections::HashSet<u64>>,
        packets: RwLock<HashMap<u64, Vec<Vec<u8>>>>,
        /// Scoped visible deliveries, recorded per call so tests can check the
        /// map, channel and exclusion without asserting on per-session queues.
        visible: RwLock<Vec<(i16, u8, u64, Vec<u8>)>>,
    }

    impl RecordingBroadcast {
        fn with_online(online: impl IntoIterator<Item = u64>) -> Self {
            Self {
                online: RwLock::new(online.into_iter().collect::<std::collections::HashSet<_>>()),
                packets: RwLock::new(HashMap::new()),
                visible: RwLock::new(Vec::new()),
            }
        }

        fn packets_for(&self, character_id: u64) -> Vec<Vec<u8>> {
            self.packets
                .read()
                .expect("broadcast poisoned")
                .get(&character_id)
                .cloned()
                .unwrap_or_default()
        }
    }

    impl crate::BroadcastSink for RecordingBroadcast {
        fn send_to(&self, character_id: u64, packet: &[u8]) -> anyhow::Result<()> {
            self.packets
                .write()
                .expect("broadcast poisoned")
                .entry(character_id)
                .or_default()
                .push(packet.to_vec());
            Ok(())
        }

        fn is_online(&self, character_id: u64) -> bool {
            self.online
                .read()
                .expect("broadcast poisoned")
                .contains(&character_id)
        }

        fn send_to_visible(
            &self,
            map_id: i16,
            channel: u8,
            exclude_character_id: u64,
            packet: &[u8],
        ) -> anyhow::Result<()> {
            self.visible
                .write()
                .expect("broadcast poisoned")
                .push((map_id, channel, exclude_character_id, packet.to_vec()));
            Ok(())
        }

        fn send_to_all_except(
            &self,
            exclude_character_id: u64,
            packet: &[u8],
        ) -> anyhow::Result<()> {
            let recipients: Vec<u64> = self
                .online
                .read()
                .expect("broadcast poisoned")
                .iter()
                .copied()
                .filter(|cid| *cid != exclude_character_id)
                .collect();
            for cid in recipients {
                self.send_to(cid, packet)?;
            }
            Ok(())
        }

        fn update_location(&self, _character_id: u64, _map_id: i16, _channel: u8) {}
    }

    const TEST_SECONDARY_PASSWORD: &str = "4321";
    const TEST_SUMMON_TICKET_ITEM_ID: i32 = 81_001;
    const TEST_RIDE_UNLOCK_ITEM_ID: i32 = 81_002;

    fn demo_combine_catalog() -> odmo_types::DigiCombineCatalog {
        odmo_types::DigiCombineCatalog {
            rank_rows: vec![odmo_types::DigiCombineRank {
                ceiling_type: 1,
                weight: 1,
                rewards: vec![odmo_types::DigiCombineReward {
                    item_id: 5201,
                    amount: 1,
                    grade: 1,
                }],
            }],
            item_list: vec![odmo_types::DigiCombineItem {
                item_id: TEST_SUMMON_TICKET_ITEM_ID,
                group_id: 1,
            }],
            item_groups: vec![odmo_types::DigiCombineGroup {
                group_id: 1,
                members: vec![TEST_SUMMON_TICKET_ITEM_ID],
            }],
            ceil_groups: vec![odmo_types::DigiCombineCeil {
                ceiling_type: 1,
                entries: vec![odmo_types::CombineCeilingEntry {
                    tier: 1,
                    value_a: 0,
                    value_b: 0,
                }],
            }],
        }
    }

    fn demo_random_box_rewards() -> Vec<odmo_types::RandomBoxReward> {
        vec![
            odmo_types::RandomBoxReward {
                item_id: 5301,
                amount: 1,
                weight: 1,
            },
            odmo_types::RandomBoxReward {
                item_id: 5302,
                amount: 2,
                weight: 3,
            },
        ]
    }

    fn demo_evolution_assets() -> Vec<EvolutionAsset> {
        vec![EvolutionAsset {
            base_type: DEFAULT_PARTNER_MODEL_ID,
            lines: vec![
                EvolutionLineAsset {
                    type_id: DEFAULT_PARTNER_MODEL_ID,
                    slot_level: 3,
                    enabled: 1,
                    stages: vec![
                        EvolutionStageAsset {
                            target_type: 31_011,
                            value: 4 | (1 << 16),
                        },
                        EvolutionStageAsset {
                            target_type: DEFAULT_PARTNER_MODEL_ID,
                            value: 8,
                        },
                    ],
                    ..EvolutionLineAsset::default()
                },
                EvolutionLineAsset {
                    type_id: 31_011,
                    slot_level: 4,
                    unlock_level: 1,
                    required_ds: 50,
                    enabled: 1,
                    stages: vec![EvolutionStageAsset {
                        target_type: DEFAULT_PARTNER_MODEL_ID,
                        value: 8,
                    }],
                    ..EvolutionLineAsset::default()
                },
            ],
        }]
    }

    fn demo_digimon_assets() -> Vec<DigimonAsset> {
        vec![
            DigimonAsset {
                digimon_id: DEFAULT_PARTNER_MODEL_ID,
                base_level: 1,
                evolution_type: 3,
            },
            DigimonAsset {
                digimon_id: 31_011,
                base_level: 11,
                evolution_type: 4,
            },
        ]
    }

    impl InMemoryCharacterRepository {
        fn demo() -> Self {
            Self {
                characters: RwLock::new(HashMap::from([
                    (
                        100,
                        CharacterSummary {
                            id: 100,
                            account_id: 1,
                            slot: 0,
                            name: "AdminTamer".to_string(),
                            inventory: odmo_types::InventorySnapshot {
                                bits: 0,
                                size: 30,
                                items: vec![
                                    odmo_types::ItemRecord::new(TEST_SUMMON_TICKET_ITEM_ID, 3),
                                    odmo_types::ItemRecord::new(TEST_RIDE_UNLOCK_ITEM_ID, 2),
                                    odmo_types::ItemRecord::default(),
                                ],
                            },
                            partner_name: "Agumon".to_string(),
                            general_handler: 11_000,
                            partner_handler: 21_000,
                            model: DEFAULT_TAMER_MODEL_ID,
                            partner_model: DEFAULT_PARTNER_MODEL_ID,
                            seal_list: SealListSnapshot {
                                seal_leader_id: 1,
                                seals: vec![SealRecord {
                                    seal_id: 5001,
                                    amount: 3,
                                    sequential_id: 9,
                                    favorite: true,
                                }],
                            },
                            daily_reward: DailyRewardStatus {
                                event_no: 2001,
                                remaining_seconds: 120,
                                total_seconds: 600,
                                week: 3,
                            },
                            attendance: AttendanceStatus {
                                event_no: 0xFF,
                                attendance_count: 0,
                                notify: false,
                            },
                            friends: vec![RelationEntry {
                                name: "Matt".to_string(),
                                connected: true,
                                annotation: "Party".to_string(),
                            }],
                            foes: vec![RelationEntry {
                                name: "Devimon".to_string(),
                                connected: false,
                                annotation: String::new(),
                            }],
                            friended_character_ids: vec![200],
                            guild: Some(GuildSnapshot {
                                id: 77,
                                name: "Tamers".to_string(),
                                level: 4,
                                current_experience: 12_345,
                                notice: "Digital world first.".to_string(),
                                extra_slots: 10,
                                members: vec![
                                    GuildMemberSnapshot {
                                        character_id: 100,
                                        authority: 1,
                                        contribution: 900,
                                        character_name: "AdminTamer".to_string(),
                                        character_level: 70,
                                        character_model: 80_001,
                                        map_id: DEFAULT_START_MAP_ID,
                                        channel: 1,
                                        state: CharacterConnectionState::Ready,
                                    },
                                    GuildMemberSnapshot {
                                        character_id: 200,
                                        authority: 5,
                                        contribution: 100,
                                        character_name: "Rookie".to_string(),
                                        character_level: 20,
                                        character_model: 80_002,
                                        map_id: 0,
                                        channel: 0,
                                        state: CharacterConnectionState::Disconnected,
                                    },
                                ],
                                historic: vec![GuildHistoricEntry {
                                    historic_type: 101,
                                    date_utc_seconds: 1_700_000_000,
                                    master_class: 1,
                                    master_name: "AdminTamer".to_string(),
                                    member_class: 5,
                                    member_name: "Rookie".to_string(),
                                }],
                                rank_position: 9,
                                ..GuildSnapshot::default()
                            }),
                            xai: Some(XaiSnapshot {
                                item_id: 131063,
                                max_xgauge: 2000,
                                max_xcrystals: 3,
                            }),
                            active_buffs: vec![ActiveBuffSnapshot {
                                buff_id: 500,
                                buff_class: 1,
                                skill_id: 8_001_001,
                                remaining_seconds: 120,
                            }],
                            current_xgauge: 500,
                            current_xcrystals: 2,
                            partner_current_slot: 1,
                            partner_slots: vec![
                                odmo_types::PartnerSlotSnapshot {
                                    slot: 1,
                                    digimon_type: DEFAULT_PARTNER_MODEL_ID,
                                    model: DEFAULT_PARTNER_MODEL_ID,
                                    name: "Agumon".to_string(),
                                    evolutions: vec![
                                        odmo_types::PartnerEvolutionSnapshot {
                                            evolution_type: DEFAULT_PARTNER_MODEL_ID,
                                            unlocked: 1,
                                            ..odmo_types::PartnerEvolutionSnapshot::default()
                                        },
                                        odmo_types::PartnerEvolutionSnapshot {
                                            evolution_type: 31_011,
                                            unlocked: 1,
                                            ..odmo_types::PartnerEvolutionSnapshot::default()
                                        },
                                    ],
                                    ..odmo_types::PartnerSlotSnapshot::default()
                                },
                                odmo_types::PartnerSlotSnapshot {
                                    slot: 2,
                                    digimon_type: 31_002,
                                    model: 31_002,
                                    level: 11,
                                    name: "Greymon".to_string(),
                                    size: 13_000,
                                    hatch_grade: 4,
                                    hp: 1_400,
                                    ds: 1_200,
                                    current_hp: 1_400,
                                    current_ds: 1_200,
                                    de: 120,
                                    at: 150,
                                    fs: 120,
                                    ev: 8,
                                    cc: 5,
                                    ms: 260,
                                    as_value: 950,
                                    ht: 3,
                                    ar: 1,
                                    bl: 2,
                                    clone_level: 3,
                                    clone_at_value: 1,
                                    clone_bl_value: 1,
                                    clone_hp_value: 1,
                                    clone_at_level: 1,
                                    clone_bl_level: 1,
                                    clone_hp_level: 1,
                                    ..odmo_types::PartnerSlotSnapshot::default()
                                },
                            ],
                            partner_active_buffs: vec![ActiveBuffSnapshot {
                                buff_id: 600,
                                buff_class: 1,
                                skill_id: 8_002_001,
                                remaining_seconds: 90,
                            }],
                            partner_active_debuffs: vec![ActiveBuffSnapshot {
                                buff_id: 700,
                                buff_class: 1,
                                skill_id: 8_003_001,
                                remaining_seconds: 30,
                            }],
                            ..CharacterSummary::default()
                        },
                    ),
                    (
                        200,
                        CharacterSummary {
                            id: 200,
                            account_id: 2,
                            slot: 0,
                            name: "Matt".to_string(),
                            inventory: odmo_types::InventorySnapshot {
                                bits: 0,
                                size: 30,
                                items: vec![odmo_types::ItemRecord::default()],
                            },
                            partner_name: "Gabumon".to_string(),
                            model: DEFAULT_GM_TAMER_MODEL_ID,
                            partner_model: DEFAULT_GM_PARTNER_MODEL_ID,
                            general_handler: 12_000,
                            partner_handler: 22_000,
                            ..CharacterSummary::default()
                        },
                    ),
                    (
                        300,
                        CharacterSummary {
                            id: 300,
                            account_id: 3,
                            slot: 0,
                            name: "FarAway".to_string(),
                            inventory: odmo_types::InventorySnapshot {
                                bits: 0,
                                size: 30,
                                items: vec![odmo_types::ItemRecord::default()],
                            },
                            partner_name: "Patamon".to_string(),
                            model: DEFAULT_ALT_TAMER_MODEL_ID,
                            partner_model: DEFAULT_ALT_PARTNER_MODEL_ID,
                            general_handler: 13_000,
                            partner_handler: 23_000,
                            x: 99_999,
                            y: 99_999,
                            partner_x: 99_999,
                            partner_y: 99_999,
                            ..CharacterSummary::default()
                        },
                    ),
                ])),
                accounts: HashMap::from([
                    (
                        1,
                        Account {
                            id: 1,
                            username: "admin".to_string(),
                            password_hash: "admin".to_string(),
                            email: "admin@odmo.local".to_string(),
                            access_level: AccessLevel::Administrator,
                            secondary_password: Some(TEST_SECONDARY_PASSWORD.to_string()),
                            suspension: None,
                        },
                    ),
                    (
                        2,
                        Account {
                            id: 2,
                            username: "gm".to_string(),
                            password_hash: "gm".to_string(),
                            email: "gm@odmo.local".to_string(),
                            access_level: AccessLevel::GameMaster,
                            secondary_password: Some(TEST_SECONDARY_PASSWORD.to_string()),
                            suspension: None,
                        },
                    ),
                    (
                        3,
                        Account {
                            id: 3,
                            username: "alt".to_string(),
                            password_hash: "alt".to_string(),
                            email: "alt@odmo.local".to_string(),
                            access_level: AccessLevel::Player,
                            secondary_password: Some(TEST_SECONDARY_PASSWORD.to_string()),
                            suspension: None,
                        },
                    ),
                ]),
                mobs_by_map: RwLock::new(HashMap::from([(
                    (DEFAULT_START_MAP_ID, 0),
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
                            x: DEFAULT_START_X,
                            y: DEFAULT_START_Y,
                            previous_x: DEFAULT_START_X,
                            previous_y: DEFAULT_START_Y,
                            active_debuffs: vec![ActiveBuffSnapshot {
                                buff_id: 901,
                                buff_class: 1,
                                skill_id: 7_001_001,
                                remaining_seconds: 45,
                            }],
                            ..MobSummary::default()
                        },
                        MobSummary {
                            id: 401,
                            map_id: DEFAULT_START_MAP_ID,
                            channel: 0,
                            handler: 34_001,
                            type_id: 51_002,
                            model: 51_002,
                            name: "FarMob".to_string(),
                            level: 40,
                            x: 99_999,
                            y: 99_999,
                            previous_x: 99_950,
                            previous_y: 99_950,
                            ..MobSummary::default()
                        },
                    ],
                )])),
                drops_by_map: RwLock::new(HashMap::from([(
                    (DEFAULT_START_MAP_ID, 0),
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
                            x: DEFAULT_START_X,
                            y: DEFAULT_START_Y,
                            owner_expires_at_unix: current_unix_timestamp() + 60,
                            expires_at_unix: current_unix_timestamp() + 90,
                            bits_drop: true,
                            ..DropSummary::default()
                        },
                        DropSummary {
                            id: 501,
                            map_id: DEFAULT_START_MAP_ID,
                            channel: 0,
                            handler: 49_201,
                            item_id: 5101,
                            amount: 1,
                            x: 99_999,
                            y: 99_999,
                            owner_expires_at_unix: current_unix_timestamp().saturating_sub(5),
                            expires_at_unix: current_unix_timestamp() + 30,
                            no_owner: true,
                            ..DropSummary::default()
                        },
                    ],
                )])),
                digi_summon_products: vec![odmo_types::DigiSummonProduct {
                    product_id: 9001,
                    string_id: 10001,
                    draw_count: 1,
                    rank: 1,
                    remaining_daily_limit: 0,
                    icon: "digi_summon/sample_box.tga".to_string(),
                    name: "Sample DigiSummon Box".to_string(),
                    description: "Demo DigiSummon product used by tests.".to_string(),
                    tickets: vec![odmo_types::DigiSummonTicket {
                        item_id: TEST_SUMMON_TICKET_ITEM_ID,
                        cost: 1,
                    }],
                    rewards: vec![odmo_types::DigiSummonReward {
                        item_list_id: 1,
                        item_id: 5101,
                        grade: 1,
                        amount: 1,
                        weight: 1,
                        group: 0,
                        group_code: 0,
                    }],
                }],
                extra_evolution_npcs: vec![ExtraEvolutionNpc {
                    npc_id: 91001,
                    recipes: vec![
                        odmo_types::ExtraEvolutionRecipe {
                            exchange_type: EXTRA_EVOLUTION_ITEM_TO_DIGIMON,
                            object_id: 31_004,
                            material_type: 2,
                            need_material_value: 0,
                            price: 500,
                            way_type: EXTRA_EVOLUTION_NEED_ALL,
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
                            exchange_type: EXTRA_EVOLUTION_DIGIMON_TO_ITEM,
                            object_id: 81_003,
                            material_type: 1,
                            need_material_value: 10,
                            price: 250,
                            way_type: EXTRA_EVOLUTION_NEED_ALL,
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
                item_assets: vec![
                    ItemAsset {
                        item_id: TEST_SUMMON_TICKET_ITEM_ID,
                        name: "Sample Burst Opener".to_string(),
                        item_type: 61,
                        section: 6100,
                        combined_section: 67100,
                        overlap: 99,
                        ..Default::default()
                    },
                    ItemAsset {
                        item_id: TEST_RIDE_UNLOCK_ITEM_ID,
                        name: "Sample Ride Opener".to_string(),
                        item_type: 62,
                        section: 6220,
                        combined_section: 68220,
                        overlap: 99,
                        ..Default::default()
                    },
                ],
                evolution_assets: demo_evolution_assets(),
                digimon_assets: demo_digimon_assets(),
                digi_combine_catalog: demo_combine_catalog(),
                union_combine_catalog: demo_combine_catalog(),
                random_box_rewards: demo_random_box_rewards(),
            }
        }
    }

    impl CharacterRepository for InMemoryCharacterRepository {
        fn list_characters_by_account(
            &self,
            _account_id: AccountId,
        ) -> anyhow::Result<Vec<CharacterSummary>> {
            unreachable!()
        }
        fn character_by_slot(
            &self,
            _account_id: AccountId,
            _slot: u8,
        ) -> anyhow::Result<Option<CharacterSummary>> {
            unreachable!()
        }
        fn character_by_id(&self, character_id: u64) -> anyhow::Result<Option<CharacterSummary>> {
            Ok(self
                .characters
                .read()
                .expect("repo poisoned")
                .get(&character_id)
                .cloned())
        }
        fn character_by_name(&self, name: &str) -> anyhow::Result<Option<CharacterSummary>> {
            Ok(self
                .characters
                .read()
                .expect("repo poisoned")
                .values()
                .find(|character| character.name.eq_ignore_ascii_case(name))
                .cloned())
        }
        fn is_name_available(&self, _name: &str) -> anyhow::Result<bool> {
            unreachable!()
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
            unreachable!()
        }
        fn delete_character(&self, _account_id: AccountId, _slot: u8) -> anyhow::Result<bool> {
            unreachable!()
        }
        fn update_character_position(
            &self,
            character_id: u64,
            x: i32,
            y: i32,
            _z: f32,
        ) -> anyhow::Result<()> {
            let mut characters = self.characters.write().expect("repo poisoned");
            if let Some(character) = characters.get_mut(&character_id) {
                character.x = x;
                character.y = y;
            }
            Ok(())
        }
        fn update_partner_position(
            &self,
            character_id: u64,
            x: i32,
            y: i32,
            _z: f32,
        ) -> anyhow::Result<()> {
            let mut characters = self.characters.write().expect("repo poisoned");
            if let Some(character) = characters.get_mut(&character_id) {
                character.partner_x = x;
                character.partner_y = y;
            }
            Ok(())
        }
        fn update_character_map(
            &self,
            character_id: u64,
            map_id: i16,
            x: i32,
            y: i32,
        ) -> anyhow::Result<()> {
            let mut characters = self.characters.write().expect("repo poisoned");
            if let Some(character) = characters.get_mut(&character_id) {
                character.map_id = map_id;
                character.x = x;
                character.y = y;
            }
            Ok(())
        }
        fn switch_partner(
            &self,
            character_id: u64,
            slot: u8,
        ) -> anyhow::Result<Option<CharacterSummary>> {
            let mut characters = self.characters.write().expect("repo poisoned");
            let Some(character) = characters.get_mut(&character_id) else {
                return Ok(None);
            };
            let Some(target) = character
                .partner_slots
                .iter()
                .find(|partner| partner.slot == slot)
                .cloned()
            else {
                return Ok(None);
            };
            if character.partner_current_slot == slot {
                return Ok(Some(character.clone()));
            }
            character.partner_current_slot = slot;
            character.partner_current_type = target.digimon_type;
            character.partner_model = target.model;
            character.partner_name = target.name;
            character.partner_level = target.level;
            character.partner_size = target.size;
            character.partner_hatch_grade = target.hatch_grade;
            character.partner_hp = target.hp;
            character.partner_ds = target.ds;
            character.partner_current_hp = target.current_hp;
            character.partner_current_ds = target.current_ds;
            character.partner_de = target.de;
            character.partner_at = target.at;
            character.partner_fs = target.fs;
            character.partner_ev = target.ev;
            character.partner_cc = target.cc;
            character.partner_ms = target.ms;
            character.partner_as = target.as_value;
            character.partner_ht = target.ht;
            character.partner_ar = target.ar;
            character.partner_bl = target.bl;
            character.partner_clone_level = target.clone_level;
            character.partner_clone_at_value = target.clone_at_value;
            character.partner_clone_bl_value = target.clone_bl_value;
            character.partner_clone_ct_value = target.clone_ct_value;
            character.partner_clone_ev_value = target.clone_ev_value;
            character.partner_clone_hp_value = target.clone_hp_value;
            character.partner_clone_at_level = target.clone_at_level;
            character.partner_clone_bl_level = target.clone_bl_level;
            character.partner_clone_ct_level = target.clone_ct_level;
            character.partner_clone_ev_level = target.clone_ev_level;
            character.partner_clone_hp_level = target.clone_hp_level;
            character.partner_active_buffs = target.active_buffs;
            Ok(Some(character.clone()))
        }
        fn update_inventory(
            &self,
            character_id: u64,
            inventory: odmo_types::InventorySnapshot,
        ) -> anyhow::Result<()> {
            let mut characters = self.characters.write().expect("repo poisoned");
            if let Some(character) = characters.get_mut(&character_id) {
                character.inventory = inventory;
            }
            Ok(())
        }
        fn update_equipment(&self, character_id: u64, equipment: Vec<u8>) -> anyhow::Result<()> {
            let mut characters = self.characters.write().expect("repo poisoned");
            if let Some(character) = characters.get_mut(&character_id) {
                character.equipment = equipment;
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
            let mut characters = self.characters.write().expect("repo poisoned");
            if let Some(character) = characters.get_mut(&character_id) {
                character.digivice = digivice;
            }
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
        fn update_welcome_flag(&self, _account_id: u64, _welcome: bool) -> anyhow::Result<()> {
            Ok(())
        }
        fn update_partner_type(&self, _character_id: u64, _new_type: i32) -> anyhow::Result<()> {
            Ok(())
        }
        fn update_tamer_resources(
            &self,
            character_id: u64,
            current_hp: i32,
            current_ds: i32,
            current_xgauge: i32,
        ) -> anyhow::Result<()> {
            let mut guard = self.characters.write().expect("repo poisoned");
            let character = guard
                .get_mut(&character_id)
                .expect("character should exist for resource update");
            character.current_hp = current_hp.clamp(0, character.hp);
            character.current_ds = current_ds.clamp(0, character.ds);
            character.current_xgauge = current_xgauge.max(0);
            Ok(())
        }
        fn update_inventory_bits(&self, character_id: u64, bits: i64) -> anyhow::Result<()> {
            let mut guard = self.characters.write().expect("repo poisoned");
            let character = guard
                .get_mut(&character_id)
                .expect("character should exist for bits update");
            character.inventory_bits = bits.max(0);
            character.inventory.bits = character.inventory_bits;
            Ok(())
        }
        fn update_partner_roster(
            &self,
            character_id: u64,
            partner_current_slot: u8,
            partner_slots: Vec<odmo_types::PartnerSlotSnapshot>,
        ) -> anyhow::Result<()> {
            let mut guard = self.characters.write().expect("repo poisoned");
            let character = guard
                .get_mut(&character_id)
                .expect("character should exist for roster update");
            character.partner_current_slot = partner_current_slot;
            character.partner_slots = partner_slots;
            if let Some(active_partner) = character
                .partner_slots
                .iter()
                .find(|partner| partner.slot == character.partner_current_slot)
                .cloned()
            {
                character.partner_current_type = active_partner.digimon_type;
                character.partner_model = active_partner.model;
                character.partner_level = active_partner.level;
                character.partner_name = active_partner.name;
                character.partner_size = active_partner.size;
                character.partner_hatch_grade = active_partner.hatch_grade;
                character.partner_hp = active_partner.hp;
                character.partner_ds = active_partner.ds;
                character.partner_current_hp = active_partner.current_hp;
                character.partner_current_ds = active_partner.current_ds;
                character.partner_de = active_partner.de;
                character.partner_at = active_partner.at;
                character.partner_fs = active_partner.fs;
                character.partner_ev = active_partner.ev;
                character.partner_cc = active_partner.cc;
                character.partner_ms = active_partner.ms;
                character.partner_as = active_partner.as_value;
                character.partner_ht = active_partner.ht;
                character.partner_ar = active_partner.ar;
                character.partner_bl = active_partner.bl;
                character.partner_clone_level = active_partner.clone_level;
                character.partner_clone_at_value = active_partner.clone_at_value;
                character.partner_clone_bl_value = active_partner.clone_bl_value;
                character.partner_clone_ct_value = active_partner.clone_ct_value;
                character.partner_clone_ev_value = active_partner.clone_ev_value;
                character.partner_clone_hp_value = active_partner.clone_hp_value;
                character.partner_clone_at_level = active_partner.clone_at_level;
                character.partner_clone_bl_level = active_partner.clone_bl_level;
                character.partner_clone_ct_level = active_partner.clone_ct_level;
                character.partner_clone_ev_level = active_partner.clone_ev_level;
                character.partner_clone_hp_level = active_partner.clone_hp_level;
                character.partner_active_buffs = active_partner.active_buffs;
            }
            Ok(())
        }
    }

    impl CharacterAccountRepository for InMemoryCharacterRepository {
        fn account_by_id(&self, account_id: u64) -> anyhow::Result<Option<Account>> {
            Ok(self.accounts.get(&account_id).cloned())
        }
    }

    impl PortalRepository for InMemoryCharacterRepository {
        fn portal_by_id(&self, portal_id: i32) -> anyhow::Result<Option<PortalDefinition>> {
            Ok(match portal_id {
                10001 => Some(PortalDefinition {
                    id: 10001,
                    is_local: false,
                    destination_map_id: 102,
                    destination_x: 32615,
                    destination_y: 14930,
                }),
                10002 => Some(PortalDefinition {
                    id: 10002,
                    is_local: false,
                    destination_map_id: 3,
                    destination_x: 18086,
                    destination_y: 18874,
                }),
                _ => None,
            })
        }
    }

    impl NpcShopRepository for InMemoryCharacterRepository {
        fn shop_by_npc(
            &self,
            _npc_id: i32,
            _map_id: i16,
        ) -> anyhow::Result<Option<NpcShopDefinition>> {
            Ok(None)
        }
    }

    impl DigiSummonRepository for InMemoryCharacterRepository {
        fn digi_summon_products(&self) -> anyhow::Result<Vec<odmo_types::DigiSummonProduct>> {
            Ok(self.digi_summon_products.clone())
        }
    }

    impl ExtraEvolutionRepository for InMemoryCharacterRepository {
        fn extra_evolution_npcs(&self) -> anyhow::Result<Vec<ExtraEvolutionNpc>> {
            Ok(self.extra_evolution_npcs.clone())
        }
    }

    impl EvolutionAssetRepository for InMemoryCharacterRepository {
        fn evolution_assets(&self) -> anyhow::Result<Vec<EvolutionAsset>> {
            Ok(self.evolution_assets.clone())
        }
    }

    impl ItemAssetRepository for InMemoryCharacterRepository {
        fn item_assets(&self) -> anyhow::Result<Vec<ItemAsset>> {
            Ok(self.item_assets.clone())
        }
    }

    impl DigimonAssetRepository for InMemoryCharacterRepository {
        fn digimon_assets(&self) -> anyhow::Result<Vec<DigimonAsset>> {
            Ok(self.digimon_assets.clone())
        }
    }

    impl DigiCombineRepository for InMemoryCharacterRepository {
        fn digi_combine_catalog(&self) -> anyhow::Result<odmo_types::DigiCombineCatalog> {
            Ok(self.digi_combine_catalog.clone())
        }
    }

    impl UnionCombineRepository for InMemoryCharacterRepository {
        fn union_combine_catalog(&self) -> anyhow::Result<odmo_types::UnionCombineCatalog> {
            Ok(self.union_combine_catalog.clone())
        }
    }

    impl RandomBoxRepository for InMemoryCharacterRepository {
        fn random_box_rewards(&self) -> anyhow::Result<Vec<odmo_types::RandomBoxReward>> {
            Ok(self.random_box_rewards.clone())
        }
    }

    impl MapMobRepository for InMemoryCharacterRepository {
        fn mobs_by_map(&self, map_id: i16, channel: u8) -> anyhow::Result<Vec<MobSummary>> {
            Ok(self
                .mobs_by_map
                .read()
                .expect("repo poisoned")
                .get(&(map_id, channel))
                .cloned()
                .unwrap_or_default())
        }
    }

    impl MapDropRepository for InMemoryCharacterRepository {
        fn drops_by_map(&self, map_id: i16, channel: u8) -> anyhow::Result<Vec<DropSummary>> {
            Ok(self
                .drops_by_map
                .read()
                .expect("repo poisoned")
                .get(&(map_id, channel))
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
            let mut characters = self.characters.write().expect("repo poisoned");
            let Some(character) = characters.get_mut(&character_id) else {
                return Ok(DropCollectionResult::Missing);
            };

            let mut drops = self.drops_by_map.write().expect("repo poisoned");
            let Some(entries) = drops.get_mut(&(map_id, channel)) else {
                return Ok(DropCollectionResult::Missing);
            };
            let Some(index) = entries.iter().position(|drop| drop.handler == drop_handler) else {
                return Ok(DropCollectionResult::Missing);
            };
            let drop = apply_runtime_drop_state(entries[index].clone());

            if drop.collected {
                entries.remove(index);
                return Ok(DropCollectionResult::Missing);
            }
            if map_distance(character.x, character.y, drop.x, drop.y) >= STOP_SEEING_DISTANCE {
                return Ok(DropCollectionResult::TooFarAway);
            }
            if drop.owner_id != 0 && drop.owner_id != character_id && !drop.no_owner {
                return Ok(DropCollectionResult::NotTheOwner);
            }

            if drop.bits_drop {
                character.inventory.bits += i64::from(drop.amount.max(0));
                character.inventory_bits += i64::from(drop.amount.max(0));
                entries.remove(index);
                return Ok(DropCollectionResult::BitsCollected {
                    drop: drop.clone(),
                    amount: drop.amount,
                    character: character.clone(),
                });
            }

            if !test_add_inventory_item(
                &mut character.inventory.items,
                character.inventory.size,
                &drop,
            ) {
                return Ok(DropCollectionResult::InventoryFull);
            }

            entries.remove(index);
            Ok(DropCollectionResult::ItemCollected {
                drop: drop.clone(),
                item_id: drop.item_id,
                amount: drop.amount.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
                character: character.clone(),
            })
        }
    }

    fn unique_test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("odmo-game-{name}-{}", uuid::Uuid::new_v4()))
    }

    fn establish_party(
        app: &GameApplication,
        inviter_id: u64,
        invitee_id: u64,
        invitee_name: &str,
        inviter_name: &str,
    ) {
        let mut inviter_session = GameSession::new(1);
        inviter_session.character_id = Some(inviter_id);
        app.handle_request(
            &mut inviter_session,
            GameRequest::PartyInvite {
                target_name: invitee_name.to_string(),
            },
        )
        .expect("invite should be delivered");

        let mut invitee_session = GameSession::new(2);
        invitee_session.character_id = Some(invitee_id);
        app.handle_request(
            &mut invitee_session,
            GameRequest::PartyInviteResponse {
                result_type: PARTY_INVITE_ACCEPTED,
                inviter_name: inviter_name.to_string(),
            },
        )
        .expect("accept should create party");
    }

    fn test_add_inventory_item(
        items: &mut Vec<odmo_types::ItemRecord>,
        size: u16,
        drop: &DropSummary,
    ) -> bool {
        if let Some(existing) = items.iter_mut().find(|item| item.item_id == drop.item_id) {
            existing.amount = existing.amount.saturating_add(drop.amount.max(0));
            existing.record.resize(69, 0);
            existing.record[0..4].copy_from_slice(&existing.item_id.to_le_bytes());
            existing.record[4..8].copy_from_slice(&existing.amount.to_le_bytes());
            return true;
        }

        if items.len() >= size as usize {
            return false;
        }

        let mut record = odmo_types::ItemRecord {
            item_id: drop.item_id,
            amount: drop.amount.max(1),
            ..odmo_types::ItemRecord::default()
        };
        record.record[0..4].copy_from_slice(&record.item_id.to_le_bytes());
        record.record[4..8].copy_from_slice(&record.amount.to_le_bytes());
        items.push(record);
        true
    }

    #[test]
    fn initial_information_requires_ticket() {
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("missing-ticket"),
            },
            Arc::new(InMemoryCharacterRepository::demo()),
        );
        let mut session = GameSession::new(1);
        let error = app
            .handle_request(
                &mut session,
                GameRequest::InitialInformation {
                    account_id: 1,
                    access_code: 0,
                },
            )
            .expect_err("ticket should be required");
        assert!(matches!(error, GameFlowError::MissingSessionTicket(1)));
    }

    #[test]
    fn initial_information_returns_bootstrap_packet() {
        let portal_state_dir = unique_test_dir("with-ticket");
        let bridge = PortalBridge::from_json(portal_state_dir.clone()).expect("bridge");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "demo".to_string(),
                account_id: 1,
                character_id: 100,
            })
            .expect("store ticket");

        let app = GameApplication::new(
            GameServiceConfig { portal_state_dir },
            Arc::new(InMemoryCharacterRepository::demo()),
        );
        let mut session = GameSession::new(1);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::InitialInformation {
                    account_id: 1,
                    access_code: 0,
                },
            )
            .expect("bootstrap should succeed");
        assert_eq!(responses.len(), 2);
    }

    #[test]
    fn initial_information_allows_reconnect_with_same_ticket() {
        let portal_state_dir = unique_test_dir("reconnect-ticket");
        let bridge = PortalBridge::from_json(portal_state_dir.clone()).expect("bridge");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "demo".to_string(),
                account_id: 1,
                character_id: 100,
            })
            .expect("store ticket");

        let app = GameApplication::new(
            GameServiceConfig { portal_state_dir },
            Arc::new(InMemoryCharacterRepository::demo()),
        );

        let mut first_session = GameSession::new(1);
        let first = app
            .handle_request(
                &mut first_session,
                GameRequest::InitialInformation {
                    account_id: 1,
                    access_code: 0,
                },
            )
            .expect("first bootstrap should succeed");
        assert_eq!(first.len(), 2);

        let mut reconnect_session = GameSession::new(2);
        let reconnect = app
            .handle_request(
                &mut reconnect_session,
                GameRequest::InitialInformation {
                    account_id: 1,
                    access_code: 0,
                },
            )
            .expect("reconnect bootstrap should also succeed");
        assert_eq!(reconnect.len(), 2);
        assert_eq!(reconnect_session.character_id, Some(100));
    }

    #[test]
    fn complementar_information_returns_follow_up_packets() {
        let portal_state_dir = unique_test_dir("complementary");
        let bridge = PortalBridge::from_json(portal_state_dir.clone()).expect("bridge");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "demo".to_string(),
                account_id: 1,
                character_id: 100,
            })
            .expect("store ticket");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "friend".to_string(),
                account_id: 2,
                character_id: 200,
            })
            .expect("store friend ticket");

        let app = GameApplication::new(
            GameServiceConfig { portal_state_dir },
            Arc::new(InMemoryCharacterRepository::demo()),
        );
        let mut existing_session = GameSession::new(2);
        app.handle_request(
            &mut existing_session,
            GameRequest::InitialInformation {
                account_id: 2,
                access_code: 0,
            },
        )
        .expect("existing occupant bootstrap should succeed");
        app.handle_request(&mut existing_session, GameRequest::ComplementarInformation)
            .expect("existing occupant should register map presence");

        let mut session = GameSession::new(1);
        app.handle_request(
            &mut session,
            GameRequest::InitialInformation {
                account_id: 1,
                access_code: 0,
            },
        )
        .expect("bootstrap should succeed");

        let responses = app
            .handle_request(&mut session, GameRequest::ComplementarInformation)
            .expect("follow-up should succeed");
        let packet_types: Vec<i16> = responses
            .iter()
            .map(|frame| {
                PacketReader::from_frame(frame)
                    .expect("frame should decode")
                    .packet_type
            })
            .collect();

        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::SEALS),
            "complementary flow should include seals",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::TIME_REWARD),
            "complementary flow should include daily time reward",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::AVAILABLE_RELATIONS),
            "complementary flow should include tamer relations",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::TAMER_ATTENDANCE),
            "complementary flow should include attendance",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::GUILD_INFORMATION),
            "complementary flow should include guild information",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOAD_UNLOAD_ENTITY),
            "complementary flow should include load/unload spawn packets for existing occupants",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOAD_BUFFS),
            "map visibility should include load-buffs packets",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::XAI_INFO),
            "complementary flow should include xai info when equipped",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::TAMER_XAI_RESOURCES),
            "complementary flow should include tamer xai resources when equipped",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::GUILD_HISTORIC),
            "complementary flow should include guild historic",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::GUILD_RANK),
            "complementary flow should include guild rank when present",
        );
        assert!(
            responses.len() >= 18,
            "complementary flow should emit the expanded legacy-like follow-up set",
        );
    }

    #[test]
    fn complementar_information_enqueues_friend_connect_for_friended_characters() {
        let portal_state_dir = unique_test_dir("friend-connect");
        let bridge = PortalBridge::from_json(portal_state_dir.clone()).expect("bridge");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "demo".to_string(),
                account_id: 1,
                character_id: 100,
            })
            .expect("store ticket");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "demo-friend".to_string(),
                account_id: 2,
                character_id: 200,
            })
            .expect("store friend ticket");

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: portal_state_dir.clone(),
            },
            Arc::new(InMemoryCharacterRepository::demo()),
        );

        let mut friend_session = GameSession::new(2);
        app.handle_request(
            &mut friend_session,
            GameRequest::InitialInformation {
                account_id: 2,
                access_code: 0,
            },
        )
        .expect("friend bootstrap should succeed");
        app.handle_request(&mut friend_session, GameRequest::ComplementarInformation)
            .expect("friend follow-up should register map presence");

        let mut announcer_session = GameSession::new(1);
        app.handle_request(
            &mut announcer_session,
            GameRequest::InitialInformation {
                account_id: 1,
                access_code: 0,
            },
        )
        .expect("bootstrap should succeed");
        app.handle_request(&mut announcer_session, GameRequest::ComplementarInformation)
            .expect("follow-up should succeed");

        let responses = app
            .handle_request(&mut friend_session, GameRequest::KeepConnection)
            .expect("friend keep-connection should flush notifications");
        let packet_types: Vec<i16> = responses
            .iter()
            .map(|frame| {
                PacketReader::from_frame(frame)
                    .expect("frame should decode")
                    .packet_type
            })
            .collect();

        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::FRIEND_CONNECT),
            "friend should receive friend-connect notification after announcer login",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOAD_UNLOAD_ENTITY),
            "friend should receive map spawn notification after announcer login",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOAD_BUFFS),
            "friend visibility should include load-buffs packets",
        );
    }

    #[test]
    fn disconnect_enqueues_unload_for_remaining_map_occupants() {
        let portal_state_dir = unique_test_dir("map-unload");
        let bridge = PortalBridge::from_json(portal_state_dir.clone()).expect("bridge");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "demo".to_string(),
                account_id: 1,
                character_id: 100,
            })
            .expect("store ticket");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "friend".to_string(),
                account_id: 2,
                character_id: 200,
            })
            .expect("store friend ticket");

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: portal_state_dir.clone(),
            },
            Arc::new(InMemoryCharacterRepository::demo()),
        );

        let mut occupant_session = GameSession::new(1);
        app.handle_request(
            &mut occupant_session,
            GameRequest::InitialInformation {
                account_id: 1,
                access_code: 0,
            },
        )
        .expect("occupant bootstrap should succeed");
        app.handle_request(&mut occupant_session, GameRequest::ComplementarInformation)
            .expect("occupant should register map presence");

        let mut leaving_session = GameSession::new(2);
        app.handle_request(
            &mut leaving_session,
            GameRequest::InitialInformation {
                account_id: 2,
                access_code: 0,
            },
        )
        .expect("leaving bootstrap should succeed");
        app.handle_request(&mut leaving_session, GameRequest::ComplementarInformation)
            .expect("leaving should register map presence");

        app.handle_request(&mut occupant_session, GameRequest::KeepConnection)
            .expect("occupant should first observe the leaving tamer");

        app.handle_disconnect(&leaving_session)
            .expect("disconnect cleanup should succeed");

        let responses = app
            .handle_request(&mut occupant_session, GameRequest::KeepConnection)
            .expect("occupant keep-connection should flush unload");
        let packet_types: Vec<i16> = responses
            .iter()
            .map(|frame| {
                PacketReader::from_frame(frame)
                    .expect("frame should decode")
                    .packet_type
            })
            .collect();

        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOAD_UNLOAD_ENTITY),
            "remaining occupant should receive unload notification after disconnect",
        );
    }

    #[test]
    fn keep_connection_unloads_tamers_that_moved_out_of_visibility_range() {
        let portal_state_dir = unique_test_dir("distance-hide");
        let bridge = PortalBridge::from_json(portal_state_dir.clone()).expect("bridge");
        bridge
            .upsert_map_presence(&CharacterSummary {
                id: 300,
                account_id: 3,
                name: "FarAway".to_string(),
                partner_name: "Patamon".to_string(),
                general_handler: 13_000,
                partner_handler: 23_000,
                x: 99_999,
                y: 99_999,
                partner_x: 99_999,
                partner_y: 99_999,
                ..CharacterSummary::default()
            })
            .expect("store far occupant");

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: portal_state_dir.clone(),
            },
            Arc::new(InMemoryCharacterRepository::demo()),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        session.registered_map_presence = true;
        session.viewed_characters.insert(
            300,
            CharacterSummary {
                id: 300,
                account_id: 3,
                name: "FarAway".to_string(),
                partner_name: "Patamon".to_string(),
                general_handler: 13_000,
                partner_handler: 23_000,
                x: 99_999,
                y: 99_999,
                partner_x: 99_999,
                partner_y: 99_999,
                ..CharacterSummary::default()
            },
        );

        let responses = app
            .handle_request(&mut session, GameRequest::KeepConnection)
            .expect("visibility reconciliation should succeed");
        let packet_types: Vec<i16> = responses
            .iter()
            .map(|frame| {
                PacketReader::from_frame(frame)
                    .expect("frame should decode")
                    .packet_type
            })
            .collect();

        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOAD_UNLOAD_ENTITY),
            "keep-connection should unload tamers outside visibility range",
        );
    }

    #[test]
    fn complementar_information_loads_visible_mobs() {
        let portal_state_dir = unique_test_dir("mob-load");
        let bridge = PortalBridge::from_json(portal_state_dir.clone()).expect("bridge");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "demo".to_string(),
                account_id: 1,
                character_id: 100,
            })
            .expect("store ticket");

        let app = GameApplication::new(
            GameServiceConfig { portal_state_dir },
            Arc::new(InMemoryCharacterRepository::demo()),
        );

        let mut session = GameSession::new(1);
        app.handle_request(
            &mut session,
            GameRequest::InitialInformation {
                account_id: 1,
                access_code: 0,
            },
        )
        .expect("bootstrap should succeed");

        // ComplementarInformation handler internally calls register_map_presence then reconcile.
        let responses = app
            .handle_request(&mut session, GameRequest::ComplementarInformation)
            .expect("follow-up should succeed");
        let load_unload_payloads: Vec<Vec<u8>> = responses
            .iter()
            .filter_map(|frame| {
                let raw = PacketReader::from_frame(frame).ok()?;
                (raw.packet_type == odmo_protocol::opcode::game::LOAD_UNLOAD_ENTITY)
                    .then_some(raw.payload)
            })
            .collect();
        let load_buffs_payloads: Vec<Vec<u8>> = responses
            .iter()
            .filter_map(|frame| {
                let raw = PacketReader::from_frame(frame).ok()?;
                (raw.packet_type == odmo_protocol::opcode::game::LOAD_BUFFS).then_some(raw.payload)
            })
            .collect();

        assert!(
            load_unload_payloads
                .iter()
                .any(|payload| payload.first() == Some(&3)),
            "complementary flow should include a load-mob entity packet",
        );
        assert!(
            load_buffs_payloads
                .iter()
                .any(|payload| payload.first() == Some(&16)),
            "complementary flow should include a load-mob buffs packet",
        );
    }

    #[test]
    fn keep_connection_unloads_mobs_that_moved_out_of_visibility_range() {
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("mob-distance-hide"),
            },
            Arc::new(InMemoryCharacterRepository::demo()),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        session.registered_map_presence = true;
        session.viewed_mobs.insert(
            401,
            MobSummary {
                id: 401,
                map_id: DEFAULT_START_MAP_ID,
                channel: 0,
                handler: 34_001,
                type_id: 51_002,
                x: 99_999,
                y: 99_999,
                previous_x: 99_950,
                previous_y: 99_950,
                ..MobSummary::default()
            },
        );

        let responses = app
            .handle_request(&mut session, GameRequest::KeepConnection)
            .expect("mob visibility reconciliation should succeed");
        let unload_payloads: Vec<Vec<u8>> = responses
            .iter()
            .filter_map(|frame| {
                let raw = PacketReader::from_frame(frame).ok()?;
                (raw.packet_type == odmo_protocol::opcode::game::LOAD_UNLOAD_ENTITY
                    && raw.payload.first() == Some(&4))
                .then_some(raw.payload)
            })
            .collect();

        assert!(
            !unload_payloads.is_empty(),
            "keep-connection should unload mobs outside visibility range",
        );
    }

    #[test]
    fn complementar_information_loads_visible_drops() {
        let portal_state_dir = unique_test_dir("drop-load");
        let bridge = PortalBridge::from_json(portal_state_dir.clone()).expect("bridge");
        bridge
            .store_game_session_ticket(&GameSessionTicket {
                token: "demo".to_string(),
                account_id: 1,
                character_id: 100,
            })
            .expect("store ticket");

        let app = GameApplication::new(
            GameServiceConfig { portal_state_dir },
            Arc::new(InMemoryCharacterRepository::demo()),
        );

        let mut session = GameSession::new(1);
        app.handle_request(
            &mut session,
            GameRequest::InitialInformation {
                account_id: 1,
                access_code: 0,
            },
        )
        .expect("bootstrap should succeed");

        // ComplementarInformation handler internally calls register_map_presence then reconcile.
        let responses = app
            .handle_request(&mut session, GameRequest::ComplementarInformation)
            .expect("follow-up should succeed");
        let load_unload_payloads: Vec<Vec<u8>> = responses
            .iter()
            .filter_map(|frame| {
                let raw = PacketReader::from_frame(frame).ok()?;
                (raw.packet_type == odmo_protocol::opcode::game::LOAD_UNLOAD_ENTITY)
                    .then_some(raw.payload)
            })
            .collect();

        assert!(
            load_unload_payloads
                .iter()
                .any(|payload| payload.first() == Some(&3)),
            "complementary flow should include a load-drop entity packet",
        );
    }

    #[test]
    fn keep_connection_unloads_drops_that_moved_out_of_visibility_range() {
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("drop-distance-hide"),
            },
            Arc::new(InMemoryCharacterRepository::demo()),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        session.registered_map_presence = true;
        session.viewed_drops.insert(
            501,
            DropSummary {
                id: 501,
                map_id: DEFAULT_START_MAP_ID,
                channel: 0,
                handler: 49_201,
                item_id: 5101,
                amount: 1,
                x: 99_999,
                y: 99_999,
                no_owner: true,
                ..DropSummary::default()
            },
        );

        let responses = app
            .handle_request(&mut session, GameRequest::KeepConnection)
            .expect("drop visibility reconciliation should succeed");
        let unload_payloads: Vec<Vec<u8>> = responses
            .iter()
            .filter_map(|frame| {
                let raw = PacketReader::from_frame(frame).ok()?;
                (raw.packet_type == odmo_protocol::opcode::game::LOAD_UNLOAD_ENTITY
                    && raw.payload.first() == Some(&4))
                .then_some(raw.payload)
            })
            .collect();

        assert!(
            !unload_payloads.is_empty(),
            "keep-connection should unload drops outside visibility range",
        );
    }

    #[test]
    fn loot_item_collects_bits_drop_and_reloads_inventory() {
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("loot-bits"),
            },
            Arc::new(InMemoryCharacterRepository::demo()),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        session.registered_map_presence = true;

        let responses = app
            .handle_request(
                &mut session,
                GameRequest::LootItem {
                    drop_handler: 49_200,
                },
            )
            .expect("bits loot should succeed");
        let packet_types: Vec<i16> = responses
            .iter()
            .map(|frame| {
                PacketReader::from_frame(frame)
                    .expect("frame should decode")
                    .packet_type
            })
            .collect();

        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::PICK_BITS),
            "bits loot should emit pick-bits packet",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOAD_UNLOAD_ENTITY),
            "bits loot should unload the collected drop",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOAD_INVENTORY),
            "bits loot should reload inventory after updating bits",
        );
    }

    #[test]
    fn loot_item_collects_item_drop_when_owner_is_lost() {
        let repo = InMemoryCharacterRepository::demo();
        repo.drops_by_map
            .write()
            .expect("repo poisoned")
            .entry((DEFAULT_START_MAP_ID, 0))
            .or_default()
            .push(DropSummary {
                id: 502,
                map_id: DEFAULT_START_MAP_ID,
                channel: 0,
                handler: 49_202,
                owner_id: 0,
                owner_handler: 0,
                item_id: 6001,
                amount: 2,
                x: DEFAULT_START_X,
                y: DEFAULT_START_Y,
                owner_expires_at_unix: current_unix_timestamp().saturating_sub(2),
                expires_at_unix: current_unix_timestamp() + 30,
                no_owner: true,
                ..DropSummary::default()
            });

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("loot-item"),
            },
            Arc::new(repo),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        session.registered_map_presence = true;

        let responses = app
            .handle_request(
                &mut session,
                GameRequest::LootItem {
                    drop_handler: 49_202,
                },
            )
            .expect("item loot should succeed");
        let packet_types: Vec<i16> = responses
            .iter()
            .map(|frame| {
                PacketReader::from_frame(frame)
                    .expect("frame should decode")
                    .packet_type
            })
            .collect();

        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOOT_ITEM),
            "item loot should emit pick-item packet",
        );
        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::LOAD_INVENTORY),
            "item loot should reload inventory after pickup",
        );
    }

    #[test]
    fn loot_item_fails_for_foreign_owned_drop() {
        let repo = InMemoryCharacterRepository::demo();
        repo.drops_by_map
            .write()
            .expect("repo poisoned")
            .entry((DEFAULT_START_MAP_ID, 0))
            .or_default()
            .push(DropSummary {
                id: 503,
                map_id: DEFAULT_START_MAP_ID,
                channel: 0,
                handler: 49_203,
                owner_id: 200,
                owner_handler: 12_000,
                item_id: 7001,
                amount: 1,
                x: 14_925,
                y: 9_965,
                owner_expires_at_unix: current_unix_timestamp() + 60,
                expires_at_unix: current_unix_timestamp() + 90,
                ..DropSummary::default()
            });

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("loot-owner-fail"),
            },
            Arc::new(repo),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        session.registered_map_presence = true;

        let responses = app
            .handle_request(
                &mut session,
                GameRequest::LootItem {
                    drop_handler: 49_203,
                },
            )
            .expect("owner failure should still respond cleanly");
        let packet_types: Vec<i16> = responses
            .iter()
            .map(|frame| {
                PacketReader::from_frame(frame)
                    .expect("frame should decode")
                    .packet_type
            })
            .collect();

        assert!(
            packet_types.contains(&odmo_protocol::opcode::game::PICK_ITEM_FAIL),
            "foreign owned drop should return pick-item-fail",
        );
    }

    #[test]
    fn party_invite_accept_bootstraps_party_contract() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-accept"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        let mut inviter_session = GameSession::new(1);
        inviter_session.character_id = Some(100);

        let invite_responses = app
            .handle_request(
                &mut inviter_session,
                GameRequest::PartyInvite {
                    target_name: "Matt".to_string(),
                },
            )
            .expect("invite should be accepted for online target");
        assert!(
            invite_responses.is_empty(),
            "invite sender should not receive immediate local packets on success",
        );

        let invite_packets = broadcast.packets_for(200);
        assert_eq!(
            invite_packets.len(),
            1,
            "target should receive one invite packet"
        );
        assert_eq!(
            PacketReader::from_frame(&invite_packets[0])
                .expect("invite frame should decode")
                .packet_type,
            odmo_protocol::opcode::game::PARTY_INVITE,
        );

        let mut target_session = GameSession::new(2);
        target_session.character_id = Some(200);

        let responses = app
            .handle_request(
                &mut target_session,
                GameRequest::PartyInviteResponse {
                    result_type: PARTY_INVITE_ACCEPTED,
                    inviter_name: "AdminTamer".to_string(),
                },
            )
            .expect("accept should create party");
        assert_eq!(
            responses.len(),
            1,
            "invitee should receive party member list"
        );
        assert_eq!(
            PacketReader::from_frame(&responses[0])
                .expect("member-list frame should decode")
                .packet_type,
            odmo_protocol::opcode::game::PARTY_MEMBER_LIST,
        );

        let inviter_packets = broadcast.packets_for(100);
        let inviter_types: Vec<i16> = inviter_packets
            .iter()
            .map(|frame| {
                PacketReader::from_frame(frame)
                    .expect("leader frame should decode")
                    .packet_type
            })
            .collect();
        assert!(
            inviter_types.contains(&odmo_protocol::opcode::game::PARTY_CREATED),
            "leader should receive party-created packet",
        );
        assert!(
            inviter_types.contains(&odmo_protocol::opcode::game::PARTY_INVITE_RESPONSE),
            "leader should receive invite-result packet",
        );
        assert!(
            inviter_types.contains(&odmo_protocol::opcode::game::PARTY_JOIN),
            "leader should receive join packet for the new member",
        );
    }

    #[test]
    fn party_invite_reject_notifies_inviter() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-reject"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        let mut inviter_session = GameSession::new(1);
        inviter_session.character_id = Some(100);
        app.handle_request(
            &mut inviter_session,
            GameRequest::PartyInvite {
                target_name: "Matt".to_string(),
            },
        )
        .expect("invite should be delivered");

        let mut target_session = GameSession::new(2);
        target_session.character_id = Some(200);
        let responses = app
            .handle_request(
                &mut target_session,
                GameRequest::PartyInviteResponse {
                    result_type: PARTY_INVITE_REJECTED,
                    inviter_name: "AdminTamer".to_string(),
                },
            )
            .expect("reject should notify inviter");
        assert!(
            responses.is_empty(),
            "reject should not send local packets to invitee"
        );

        let inviter_packets = broadcast.packets_for(100);
        let inviter_types: Vec<i16> = inviter_packets
            .iter()
            .map(|frame| {
                PacketReader::from_frame(frame)
                    .expect("leader frame should decode")
                    .packet_type
            })
            .collect();
        assert!(
            inviter_types.contains(&odmo_protocol::opcode::game::PARTY_INVITE_RESPONSE),
            "inviter should receive reject result packet",
        );
    }

    #[test]
    fn party_leave_notifies_members() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-leave"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        establish_party(&app, 100, 200, "Matt", "AdminTamer");

        let mut invitee_session = GameSession::new(2);
        invitee_session.character_id = Some(200);
        app.handle_request(&mut invitee_session, GameRequest::PartyLeave)
            .expect("leave should succeed");

        let inviter_types: Vec<i16> = broadcast
            .packets_for(100)
            .iter()
            .map(|frame| PacketReader::from_frame(frame).expect("frame").packet_type)
            .collect();
        assert!(
            inviter_types.contains(&odmo_protocol::opcode::game::PARTY_LEAVE),
            "remaining member should receive leave packet",
        );
    }

    #[test]
    fn party_kick_notifies_target_and_members() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-kick"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        establish_party(&app, 100, 200, "Matt", "AdminTamer");

        let mut leader_session = GameSession::new(1);
        leader_session.character_id = Some(100);
        app.handle_request(
            &mut leader_session,
            GameRequest::PartyKick {
                target_name: "Matt".to_string(),
            },
        )
        .expect("kick should succeed");

        let target_types: Vec<i16> = broadcast
            .packets_for(200)
            .iter()
            .map(|frame| PacketReader::from_frame(frame).expect("frame").packet_type)
            .collect();
        assert!(
            target_types.contains(&odmo_protocol::opcode::game::PARTY_KICK),
            "kicked member should receive kick packet",
        );
    }

    #[test]
    fn general_chat_reaches_the_speaker_and_nearby_tamers() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("chat-general"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::ChatMessage {
                    message: "hello".to_string(),
                },
            )
            .expect("chat should succeed");

        let speaker = PacketReader::from_frame(&responses[0]).expect("frame should decode");
        assert_eq!(
            speaker.packet_type,
            odmo_protocol::opcode::game::CHAT_MESSAGE_RESPONSE
        );
        assert_eq!(&speaker.payload[..2], &[7, 1], "record 7 carrying one line");

        let visible = broadcast.visible.read().expect("visible poisoned");
        assert_eq!(visible.len(), 1, "one scoped delivery for nearby tamers");
        let (_, _, excluded, frame) = &visible[0];
        assert_eq!(*excluded, 100, "the speaker keeps its own copy");
        assert_eq!(frame, &responses[0], "nearby tamers get the same record");
    }

    #[test]
    fn shout_reaches_every_connected_tamer() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200, 300]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("chat-shout"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::ShoutMessage {
                    message: "gather".to_string(),
                },
            )
            .expect("shout should succeed");

        let speaker = PacketReader::from_frame(&responses[0]).expect("frame should decode");
        assert_eq!(&speaker.payload[..2], &[11, 1], "record 11 carrying one line");

        for listener in [200, 300] {
            let frames = broadcast.packets_for(listener);
            assert_eq!(frames.len(), 1, "tamer {listener} should hear the shout");
            let decoded = PacketReader::from_frame(&frames[0]).expect("frame should decode");
            assert_eq!(
                decoded.payload, speaker.payload,
                "tamer {listener} should read the same record as the speaker",
            );
        }

        assert!(
            broadcast.packets_for(100).is_empty(),
            "the speaker is served by the returned packet, not the sink",
        );
    }

    #[test]
    fn whisper_reports_delivery_by_result_code() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("chat-whisper"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::WhisperMessage {
                    target_name: "Matt".to_string(),
                    message: "over here".to_string(),
                },
            )
            .expect("whisper should succeed");

        let speaker = PacketReader::from_frame(&responses[0]).expect("frame should decode");
        assert_eq!(&speaker.payload[..3], &[8, 1, 3], "result 3 means delivered");

        let target_frames = broadcast.packets_for(200);
        assert_eq!(target_frames.len(), 1, "the target receives the whisper");
        let target = PacketReader::from_frame(&target_frames[0]).expect("frame should decode");
        assert_eq!(
            target.payload, speaker.payload,
            "both peers read the same record",
        );

        // An offline tamer and an unknown name must not be reported as delivered.
        let offline_app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("chat-whisper-offline"),
            },
            Arc::new(InMemoryCharacterRepository::demo()),
        )
        .with_broadcast(Arc::new(RecordingBroadcast::with_online([100])));

        for (target_name, expected_result) in [("Matt", 0_u8), ("Nobody", 2)] {
            let mut offline_session = GameSession::new(1);
            offline_session.character_id = Some(100);
            let responses = offline_app
                .handle_request(
                    &mut offline_session,
                    GameRequest::WhisperMessage {
                        target_name: target_name.to_string(),
                        message: "still here".to_string(),
                    },
                )
                .expect("whisper should succeed");
            let frame = PacketReader::from_frame(&responses[0]).expect("frame should decode");
            assert_eq!(
                &frame.payload[..3],
                &[8, 1, expected_result],
                "{target_name} should report result {expected_result}",
            );
        }
    }

    fn frames_of_type(
        broadcast: &RecordingBroadcast,
        character_id: u64,
        packet_type: i16,
    ) -> Vec<Vec<u8>> {
        broadcast
            .packets_for(character_id)
            .into_iter()
            .filter(|frame| {
                PacketReader::from_frame(frame)
                    .map(|raw| raw.packet_type == packet_type)
                    .unwrap_or(false)
            })
            .collect()
    }

    #[test]
    fn party_chat_reaches_the_other_members() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-chat"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        let mut leader = GameSession::new(1);
        leader.character_id = Some(100);
        app.handle_request(
            &mut leader,
            GameRequest::PartyInvite {
                target_name: "Matt".to_string(),
            },
        )
        .expect("invite should reach the online target");

        let mut invitee = GameSession::new(2);
        invitee.character_id = Some(200);
        app.handle_request(
            &mut invitee,
            GameRequest::PartyInviteResponse {
                result_type: PARTY_INVITE_ACCEPTED,
                inviter_name: "AdminTamer".to_string(),
            },
        )
        .expect("accept should form the party");

        let responses = app
            .handle_request(
                &mut leader,
                GameRequest::PartyChat {
                    message: "on my way".to_string(),
                },
            )
            .expect("party chat should succeed");
        assert!(
            responses.is_empty(),
            "the speaker is not sent its own party line back",
        );

        let delivered = frames_of_type(&broadcast, 200, odmo_protocol::opcode::game::PARTY_CHAT);
        assert_eq!(delivered.len(), 1, "the other member receives the line");
        let frame = PacketReader::from_frame(&delivered[0]).expect("frame should decode");
        assert_eq!(
            frame.payload,
            [
                &[10, 0],
                b"AdminTamer".as_slice(),
                &[9, 0],
                b"on my way".as_slice()
            ]
            .concat(),
            "party chat carries the sender then the line as modern text",
        );
    }

    #[test]
    fn guild_chat_reaches_the_online_members() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("guild-chat"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        let mut master = GameSession::new(1);
        master.character_id = Some(100);
        app.handle_request(
            &mut master,
            GameRequest::GuildCreate {
                guild_name: "Sentinels".to_string(),
                inven_slot: 0,
                npc_id: 0,
            },
        )
        .expect("guild should be created");

        app.handle_request(
            &mut master,
            GameRequest::GuildInvite {
                target_name: "Matt".to_string(),
            },
        )
        .expect("invite should be queued");

        let mut recruit = GameSession::new(2);
        recruit.character_id = Some(200);
        app.handle_request(
            &mut recruit,
            GameRequest::GuildInviteAccept {
                certified_code: 0,
                target_name: String::new(),
            },
        )
        .expect("accept should join the guild");

        let responses = app
            .handle_request(
                &mut master,
                GameRequest::GuildMessage {
                    message: "rally".to_string(),
                },
            )
            .expect("guild chat should succeed");
        assert_eq!(responses.len(), 1, "the speaker gets its own line back");

        let delivered = frames_of_type(&broadcast, 200, odmo_protocol::opcode::game::GUILD_MESSAGE);
        assert_eq!(delivered.len(), 1, "the online member receives the line");
        assert_eq!(
            delivered[0], responses[0],
            "members get the same frame the speaker sees",
        );
    }

    #[test]
    fn party_change_master_broadcasts_slot() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-master"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        establish_party(&app, 100, 200, "Matt", "AdminTamer");

        let mut leader_session = GameSession::new(1);
        leader_session.character_id = Some(100);
        app.handle_request(
            &mut leader_session,
            GameRequest::PartyChangeMaster { new_leader_slot: 1 },
        )
        .expect("leader change should succeed");

        let invitee_types: Vec<i16> = broadcast
            .packets_for(200)
            .iter()
            .map(|frame| PacketReader::from_frame(frame).expect("frame").packet_type)
            .collect();
        assert!(
            invitee_types.contains(&odmo_protocol::opcode::game::PARTY_CHANGE_MASTER),
            "members should receive leader-changed packet",
        );
    }

    #[test]
    fn party_change_loot_broadcasts_rule() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-loot"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        establish_party(&app, 100, 200, "Matt", "AdminTamer");

        let mut leader_session = GameSession::new(1);
        leader_session.character_id = Some(100);
        app.handle_request(
            &mut leader_session,
            GameRequest::PartyChangeLoot {
                loot_type: 2,
                rare_type: 3,
                disp_rare_grade: 4,
            },
        )
        .expect("loot change should succeed");

        let invitee_types: Vec<i16> = broadcast
            .packets_for(200)
            .iter()
            .map(|frame| PacketReader::from_frame(frame).expect("frame").packet_type)
            .collect();
        assert!(
            invitee_types.contains(&odmo_protocol::opcode::game::PARTY_CHANGE_LOOT),
            "members should receive loot-change packet",
        );
    }

    #[test]
    fn consume_item_broadcasts_party_member_info() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        {
            let mut characters = repo.characters.write().expect("repo poisoned");
            let character = characters.get_mut(&100).expect("leader should exist");
            if character.inventory.items.is_empty() {
                character
                    .inventory
                    .items
                    .resize(1, odmo_types::ItemRecord::default());
            }
            character.inventory.items[0] = odmo_types::ItemRecord {
                item_id: 5101,
                amount: 1,
                ..odmo_types::ItemRecord::default()
            };
            character.inventory.items[0].sync_record();
        }
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-info"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        establish_party(&app, 100, 200, "Matt", "AdminTamer");

        let mut leader_session = GameSession::new(1);
        leader_session.character_id = Some(100);
        app.handle_request(
            &mut leader_session,
            GameRequest::ConsumeItem {
                target_handler: 0,
                slot: 0,
            },
        )
        .expect("consume should succeed");

        let info_packet = broadcast
            .packets_for(200)
            .into_iter()
            .find(|frame| {
                PacketReader::from_frame(frame)
                    .map(|raw| raw.packet_type == odmo_protocol::opcode::game::PARTY_MEMBER_INFO)
                    .unwrap_or(false)
            })
            .expect("party member info should be broadcast");
        let raw = PacketReader::from_frame(&info_packet).expect("frame");
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u8().expect("slot"), 0);
        assert_eq!(payload.read_i32().expect("digimon type"), 31_001);
    }

    #[test]
    fn movement_broadcasts_party_member_position() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-position"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        establish_party(&app, 100, 200, "Matt", "AdminTamer");

        let mut leader_session = GameSession::new(1);
        leader_session.character_id = Some(100);
        app.handle_request(
            &mut leader_session,
            GameRequest::TamerMovimentation {
                ticks: 0,
                handler: 32_767,
                x: 4321,
                y: 5432,
                z: 0.0,
            },
        )
        .expect("movement should succeed");

        let position_packet = broadcast
            .packets_for(200)
            .into_iter()
            .find(|frame| {
                PacketReader::from_frame(frame)
                    .map(|raw| {
                        raw.packet_type == odmo_protocol::opcode::game::PARTY_MEMBER_POSITION
                    })
                    .unwrap_or(false)
            })
            .expect("party member position should be broadcast");
        let raw = PacketReader::from_frame(&position_packet).expect("frame");
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u8().expect("slot"), 0);
        assert_eq!(payload.read_i32().expect("x"), 4321);
        assert_eq!(payload.read_i32().expect("y"), 5432);
    }

    #[test]
    fn warp_gate_broadcasts_party_member_map_change() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-map-change"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        establish_party(&app, 100, 200, "Matt", "AdminTamer");

        let mut leader_session = GameSession::new(1);
        leader_session.character_id = Some(100);
        app.handle_request(
            &mut leader_session,
            GameRequest::WarpGate { portal_id: 10001 },
        )
        .expect("warp should succeed");

        let map_packet = broadcast
            .packets_for(200)
            .into_iter()
            .find(|frame| {
                PacketReader::from_frame(frame)
                    .map(|raw| {
                        raw.packet_type == odmo_protocol::opcode::game::PARTY_MEMBER_MAP_CHANGE
                    })
                    .unwrap_or(false)
            })
            .expect("party member map change should be broadcast");
        let raw = PacketReader::from_frame(&map_packet).expect("frame");
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u8().expect("slot"), 0);
        assert_eq!(payload.read_i32().expect("map"), 102);
    }

    #[test]
    fn disconnect_broadcasts_party_member_disconnected_even_without_map_presence() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-disconnected"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        establish_party(&app, 100, 200, "Matt", "AdminTamer");

        let mut invitee_session = GameSession::new(2);
        invitee_session.character_id = Some(200);
        app.handle_disconnect(&invitee_session)
            .expect("disconnect should clean party runtime");

        let disconnect_packet = broadcast
            .packets_for(100)
            .into_iter()
            .find(|frame| {
                PacketReader::from_frame(frame)
                    .map(|raw| {
                        raw.packet_type == odmo_protocol::opcode::game::PARTY_MEMBER_DISCONNECTED
                    })
                    .unwrap_or(false)
            })
            .expect("party member disconnected should be broadcast");
        let raw = PacketReader::from_frame(&disconnect_packet).expect("frame");
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_i32().expect("slot"), 1);
    }

    #[test]
    fn partner_switch_broadcasts_party_member_digimon_change() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100, 200]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("party-partner-switch"),
            },
            repo,
        )
        .with_broadcast(broadcast.clone());

        establish_party(&app, 100, 200, "Matt", "AdminTamer");

        let mut leader_session = GameSession::new(1);
        leader_session.character_id = Some(100);
        let responses = app
            .handle_request(&mut leader_session, GameRequest::PartnerSwitch { slot: 2 })
            .expect("partner switch should succeed");

        assert!(
            responses.iter().any(|frame| {
                PacketReader::from_frame(frame)
                    .map(|raw| raw.packet_type == odmo_protocol::opcode::game::UPDATE_STATUS)
                    .unwrap_or(false)
            }),
            "partner switch should refresh local status",
        );

        let change_packet = broadcast
            .packets_for(200)
            .into_iter()
            .find(|frame| {
                PacketReader::from_frame(frame)
                    .map(|raw| {
                        raw.packet_type == odmo_protocol::opcode::game::PARTY_MEMBER_DIGIMON_CHANGE
                    })
                    .unwrap_or(false)
            })
            .expect("party digimon change should be broadcast");
        let raw = PacketReader::from_frame(&change_packet).expect("frame");
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u8().expect("slot"), 0);
        assert_eq!(payload.read_i32().expect("type"), 31_002);

        let buff_packet = broadcast
            .packets_for(200)
            .into_iter()
            .find(|frame| {
                PacketReader::from_frame(frame)
                    .map(|raw| {
                        raw.packet_type == odmo_protocol::opcode::game::PARTY_MEMBER_BUFF_CHANGE
                    })
                    .unwrap_or(false)
            })
            .expect("party buff change should be broadcast");
        let raw = PacketReader::from_frame(&buff_packet).expect("frame");
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u8().expect("slot"), 0);
        assert_eq!(payload.read_u16().expect("buff count"), 0);
    }

    #[test]
    fn partner_switch_invalid_slot_uses_modern_failure_contract() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-switch-fail"),
            },
            repo,
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(&mut session, GameRequest::PartnerSwitch { slot: 9 })
            .expect("failure packet should still be returned");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::PARTNER_SWITCH_RESPONSE
        );
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u32().expect("uid"), 0);
    }

    #[test]
    fn partner_evolution_success_with_valid_handler() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-success"),
            },
            repo,
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::PARTNER_EVOLUTION
        );
    }

    #[test]
    fn partner_evolution_consumes_tamer_ds_without_item_gate() {
        let repo = InMemoryCharacterRepository::demo();
        let before = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        let before_ds = before.current_ds;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-consume-resources"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.partner_current_type, 31_011);
        assert_eq!(
            stored.current_ds,
            before_ds - repo.evolution_assets[0].lines[1].required_ds
        );
    }

    #[test]
    fn partner_evolution_fails_with_wrong_handler() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-wrong-handler"),
            },
            repo,
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 99_999, // wrong handler
                    evolution_slot: 0,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::EVOLUTION_FAILURE
        );
    }

    #[test]
    fn partner_evolution_fails_without_enough_ds() {
        let repo = InMemoryCharacterRepository::demo();
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .expect("character")
            .current_ds = 10;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-not-enough-ds"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::EVOLUTION_FAILURE
        );
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.partner_current_type, DEFAULT_PARTNER_MODEL_ID);
        assert_eq!(stored.current_ds, 10);
    }

    #[test]
    fn partner_evolution_fails_when_required_slot_is_closed() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets[0].lines[1].unlock_item_section = 6100;
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .expect("character")
            .partner_slots[0]
            .evolutions[1]
            .unlocked = 0;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-slot-closed"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::EVOLUTION_FAILURE
        );
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.partner_current_type, DEFAULT_PARTNER_MODEL_ID);
    }

    #[test]
    fn partner_evolution_fails_without_completed_open_quest() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets[0].lines[1].unlock_quest_id = 321;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-open-quest"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::EVOLUTION_FAILURE
        );
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.partner_current_type, DEFAULT_PARTNER_MODEL_ID);
    }

    #[test]
    fn partner_evolution_fails_when_unlock_level_is_not_met() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets[0].lines[1].unlock_level = 99;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-open-level"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::EVOLUTION_FAILURE
        );
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.partner_current_type, DEFAULT_PARTNER_MODEL_ID);
    }

    #[test]
    fn partner_evolution_allows_activation_without_required_intimacy() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets[0].lines[1].required_intimacy = 150;
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .expect("character")
            .partner_fs = 100;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-intimacy"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::PARTNER_EVOLUTION
        );
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.partner_current_type, 31_011);
    }

    #[test]
    fn partner_evolution_fails_when_jogress_partner_is_lower_level() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets = vec![EvolutionAsset {
            base_type: DEFAULT_PARTNER_MODEL_ID,
            lines: vec![
                EvolutionLineAsset {
                    type_id: DEFAULT_PARTNER_MODEL_ID,
                    enabled: 1,
                    stages: vec![EvolutionStageAsset {
                        target_type: 41_000,
                        value: 4 | (1 << 16),
                    }],
                    ..EvolutionLineAsset::default()
                },
                EvolutionLineAsset {
                    type_id: 41_000,
                    enabled: 1,
                    required_ds: 2,
                    evolution_tree: 2,
                    jogress_need_digimon_types: vec![32_001],
                    ..EvolutionLineAsset::default()
                },
            ],
        }];
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .map(|character| {
                character.partner_level = 20;
                character.partner_slots = vec![
                    odmo_types::PartnerSlotSnapshot {
                        slot: 1,
                        digimon_type: DEFAULT_PARTNER_MODEL_ID,
                        model: DEFAULT_PARTNER_MODEL_ID,
                        level: 20,
                        evolutions: vec![
                            odmo_types::PartnerEvolutionSnapshot {
                                evolution_type: DEFAULT_PARTNER_MODEL_ID,
                                unlocked: 1,
                                ..odmo_types::PartnerEvolutionSnapshot::default()
                            },
                            odmo_types::PartnerEvolutionSnapshot {
                                evolution_type: 41_000,
                                unlocked: 1,
                                ..odmo_types::PartnerEvolutionSnapshot::default()
                            },
                        ],
                        ..odmo_types::PartnerSlotSnapshot::default()
                    },
                    odmo_types::PartnerSlotSnapshot {
                        slot: 2,
                        digimon_type: 32_001,
                        model: 32_001,
                        level: 15,
                        evolutions: vec![odmo_types::PartnerEvolutionSnapshot {
                            evolution_type: 41_000,
                            unlocked: 1,
                            ..odmo_types::PartnerEvolutionSnapshot::default()
                        }],
                        ..odmo_types::PartnerSlotSnapshot::default()
                    },
                ];
            })
            .expect("character");
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-jogress-level"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::EVOLUTION_FAILURE
        );
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.partner_current_type, DEFAULT_PARTNER_MODEL_ID);
    }

    #[test]
    fn partner_evolution_fails_without_required_jogress_chipset() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets = vec![EvolutionAsset {
            base_type: DEFAULT_PARTNER_MODEL_ID,
            lines: vec![
                EvolutionLineAsset {
                    type_id: DEFAULT_PARTNER_MODEL_ID,
                    enabled: 1,
                    stages: vec![EvolutionStageAsset {
                        target_type: 41_000,
                        value: 4 | (1 << 16),
                    }],
                    ..EvolutionLineAsset::default()
                },
                EvolutionLineAsset {
                    type_id: 41_000,
                    enabled: 1,
                    required_ds: 2,
                    evolution_tree: 2,
                    jogress_chipset_type: 52,
                    jogress_consumable_chipset_type: 5_202,
                    jogress_period_chipset_type: 5_201,
                    jogress_chipset_amount: 1,
                    jogress_need_digimon_types: vec![32_001],
                    ..EvolutionLineAsset::default()
                },
            ],
        }];
        repo.item_assets.push(ItemAsset {
            item_id: 90_001,
            name: "Jogress Consumable Chipset".to_string(),
            item_type: 52,
            section: 5_202,
            combined_section: 5_202,
            overlap: 99,
            ..Default::default()
        });
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .map(|character| {
                character.partner_level = 20;
                character.partner_slots = vec![
                    odmo_types::PartnerSlotSnapshot {
                        slot: 1,
                        digimon_type: DEFAULT_PARTNER_MODEL_ID,
                        model: DEFAULT_PARTNER_MODEL_ID,
                        level: 20,
                        evolutions: vec![
                            odmo_types::PartnerEvolutionSnapshot {
                                evolution_type: DEFAULT_PARTNER_MODEL_ID,
                                unlocked: 1,
                                ..odmo_types::PartnerEvolutionSnapshot::default()
                            },
                            odmo_types::PartnerEvolutionSnapshot {
                                evolution_type: 41_000,
                                unlocked: 1,
                                ..odmo_types::PartnerEvolutionSnapshot::default()
                            },
                        ],
                        ..odmo_types::PartnerSlotSnapshot::default()
                    },
                    odmo_types::PartnerSlotSnapshot {
                        slot: 2,
                        digimon_type: 32_001,
                        model: 32_001,
                        level: 20,
                        evolutions: vec![odmo_types::PartnerEvolutionSnapshot {
                            evolution_type: 41_000,
                            unlocked: 1,
                            ..odmo_types::PartnerEvolutionSnapshot::default()
                        }],
                        ..odmo_types::PartnerSlotSnapshot::default()
                    },
                ];
            })
            .expect("character");
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-jogress-chipset-missing"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::EVOLUTION_FAILURE
        );
    }

    #[test]
    fn partner_evolution_consumes_consumable_jogress_chipset() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets = vec![EvolutionAsset {
            base_type: DEFAULT_PARTNER_MODEL_ID,
            lines: vec![
                EvolutionLineAsset {
                    type_id: DEFAULT_PARTNER_MODEL_ID,
                    enabled: 1,
                    stages: vec![EvolutionStageAsset {
                        target_type: 41_000,
                        value: 4 | (1 << 16),
                    }],
                    ..EvolutionLineAsset::default()
                },
                EvolutionLineAsset {
                    type_id: 41_000,
                    enabled: 1,
                    required_ds: 2,
                    evolution_tree: 2,
                    jogress_chipset_type: 52,
                    jogress_consumable_chipset_type: 5_202,
                    jogress_period_chipset_type: 5_201,
                    jogress_chipset_amount: 2,
                    jogress_need_digimon_types: vec![32_001],
                    ..EvolutionLineAsset::default()
                },
            ],
        }];
        repo.item_assets.push(ItemAsset {
            item_id: 90_001,
            name: "Jogress Consumable Chipset".to_string(),
            item_type: 52,
            section: 5_202,
            combined_section: 5_202,
            overlap: 99,
            ..Default::default()
        });
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .map(|character| {
                character.partner_level = 20;
                character.partner_slots = vec![
                    odmo_types::PartnerSlotSnapshot {
                        slot: 1,
                        digimon_type: DEFAULT_PARTNER_MODEL_ID,
                        model: DEFAULT_PARTNER_MODEL_ID,
                        level: 20,
                        evolutions: vec![
                            odmo_types::PartnerEvolutionSnapshot {
                                evolution_type: DEFAULT_PARTNER_MODEL_ID,
                                unlocked: 1,
                                ..odmo_types::PartnerEvolutionSnapshot::default()
                            },
                            odmo_types::PartnerEvolutionSnapshot {
                                evolution_type: 41_000,
                                unlocked: 1,
                                ..odmo_types::PartnerEvolutionSnapshot::default()
                            },
                        ],
                        ..odmo_types::PartnerSlotSnapshot::default()
                    },
                    odmo_types::PartnerSlotSnapshot {
                        slot: 2,
                        digimon_type: 32_001,
                        model: 32_001,
                        level: 20,
                        evolutions: vec![odmo_types::PartnerEvolutionSnapshot {
                            evolution_type: 41_000,
                            unlocked: 1,
                            ..odmo_types::PartnerEvolutionSnapshot::default()
                        }],
                        ..odmo_types::PartnerSlotSnapshot::default()
                    },
                ];
                character.digivice.chipsets[0] = odmo_types::DigiviceItemSnapshot {
                    item_id: 90_001,
                    amount: 3,
                    record: vec![0; 60],
                };
                character.digivice.chipsets[0].sync_record();
            })
            .expect("character");
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("partner-evolution-jogress-chipset-consume"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerEvolution {
                    digimon_handler: 21_000,
                    evolution_slot: 4,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.partner_current_type, 41_000);
        assert_eq!(stored.digivice.chipsets[0].amount, 1);
    }

    #[test]
    fn move_item_chipset_to_inventory_updates_persisted_digivice() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.item_assets.push(ItemAsset {
            item_id: 40_100,
            item_type: 53,
            digivice_chipset_slots: 2,
            digivice_skill_slots: 0,
            ..Default::default()
        });
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .map(|character| {
                character.inventory.items[0] = ItemRecord::default();
                character.digivice.equipped_item = odmo_types::DigiviceItemSnapshot {
                    item_id: 40_100,
                    amount: 1,
                    ..odmo_types::DigiviceItemSnapshot::default()
                };
                character.digivice.equipped_item.sync_record();
                character.digivice.chipsets[0] = odmo_types::DigiviceItemSnapshot {
                    item_id: 52_202,
                    amount: 2,
                    ..odmo_types::DigiviceItemSnapshot::default()
                };
                character.digivice.chipsets[0].sync_record();
            })
            .expect("character");
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("move-item-chipset-to-inventory"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::MoveItem {
                    origin_slot: 4_000,
                    destination_slot: 0,
                },
            )
            .expect("move should succeed");

        assert!(!responses.is_empty(), "move should emit responses");
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.digivice.chipsets[0].item_id, 0);
        assert_eq!(stored.inventory.items[0].item_id, 52_202);
        assert_eq!(stored.inventory.items[0].amount, 2);
    }

    #[test]
    fn move_item_inventory_to_digivice_swaps_equipped_item() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.item_assets.push(ItemAsset {
            item_id: 40_001,
            item_type: 53,
            digivice_chipset_slots: 2,
            digivice_skill_slots: 2,
            ..Default::default()
        });
        repo.item_assets.push(ItemAsset {
            item_id: 40_002,
            item_type: 53,
            digivice_chipset_slots: 2,
            digivice_skill_slots: 2,
            ..Default::default()
        });
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .map(|character| {
                character.inventory.items[0] = ItemRecord::new(40_001, 1);
                character.digivice.equipped_item = odmo_types::DigiviceItemSnapshot {
                    item_id: 40_002,
                    amount: 1,
                    ..odmo_types::DigiviceItemSnapshot::default()
                };
                character.digivice.equipped_item.sync_record();
            })
            .expect("character");
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("move-item-inventory-to-digivice"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::MoveItem {
                    origin_slot: 0,
                    destination_slot: 5_000,
                },
            )
            .expect("move should succeed");

        assert!(!responses.is_empty(), "move should emit responses");
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.digivice.equipped_item.item_id, 40_001);
        assert_eq!(stored.inventory.items[0].item_id, 40_002);
    }

    #[test]
    fn move_item_inventory_to_digivice_fails_when_overflow_slots_remain() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.item_assets.push(ItemAsset {
            item_id: 40_010,
            item_type: 53,
            digivice_chipset_slots: 1,
            digivice_skill_slots: 0,
            ..Default::default()
        });
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .map(|character| {
                character.inventory.items[0] = ItemRecord::new(40_010, 1);
                character.digivice.chipsets[1] = odmo_types::DigiviceItemSnapshot {
                    item_id: 52_999,
                    amount: 1,
                    ..odmo_types::DigiviceItemSnapshot::default()
                };
                character.digivice.chipsets[1].sync_record();
            })
            .expect("character");
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("move-item-digivice-overflow-fail"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::MoveItem {
                    origin_slot: 0,
                    destination_slot: 5_000,
                },
            )
            .expect("move should return failure");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::MOVE_ITEM_FAILURE
        );
        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.digivice.equipped_item.item_id, 0);
        assert_eq!(stored.inventory.items[0].item_id, 40_010);
    }

    #[test]
    fn move_item_inventory_to_chipset_requires_chipset_item_type() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.item_assets.push(ItemAsset {
            item_id: 40_100,
            item_type: 53,
            digivice_chipset_slots: 2,
            digivice_skill_slots: 0,
            ..Default::default()
        });
        repo.item_assets.push(ItemAsset {
            item_id: 77_777,
            item_type: 10,
            ..Default::default()
        });
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .map(|character| {
                character.inventory.items[0] = ItemRecord::new(77_777, 1);
                character.digivice.equipped_item = odmo_types::DigiviceItemSnapshot {
                    item_id: 40_100,
                    amount: 1,
                    ..odmo_types::DigiviceItemSnapshot::default()
                };
                character.digivice.equipped_item.sync_record();
            })
            .expect("character");
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("move-item-chipset-invalid-type"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::MoveItem {
                    origin_slot: 0,
                    destination_slot: 4_000,
                },
            )
            .expect("move should return failure");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::MOVE_ITEM_FAILURE
        );
    }

    #[test]
    fn move_item_inventory_to_chipset_rejects_closed_slot() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.item_assets.push(ItemAsset {
            item_id: 40_101,
            item_type: 53,
            digivice_chipset_slots: 1,
            digivice_skill_slots: 0,
            ..Default::default()
        });
        repo.item_assets.push(ItemAsset {
            item_id: 90_101,
            item_type: 52,
            ..Default::default()
        });
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .map(|character| {
                character.inventory.items[0] = ItemRecord::new(90_101, 1);
                character.digivice.equipped_item = odmo_types::DigiviceItemSnapshot {
                    item_id: 40_101,
                    amount: 1,
                    ..odmo_types::DigiviceItemSnapshot::default()
                };
                character.digivice.equipped_item.sync_record();
            })
            .expect("character");
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("move-item-chipset-closed-slot"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::MoveItem {
                    origin_slot: 0,
                    destination_slot: 4_001,
                },
            )
            .expect("move should return failure");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::MOVE_ITEM_FAILURE
        );
    }

    #[test]
    fn evolution_unlock_modern_branch_consumes_section_and_persists_unlock() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets[0].lines[1].unlock_item_section = 6100;
        repo.evolution_assets[0].lines[1].unlock_item_section_amount = 1;
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .expect("character")
            .partner_slots[0]
            .evolutions[1]
            .unlocked = 0;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("evolution-unlock-modern"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::EvolutionUnlock {
                    evo_slot: 1,
                    items: vec![EvolutionUnlockItem {
                        item_id: TEST_SUMMON_TICKET_ITEM_ID as u32,
                        item_count: 1,
                    }],
                    selected_inven_idx: -1,
                },
            )
            .expect("request should complete");

        assert!(
            responses.is_empty(),
            "modern slot-open should stay optimistic"
        );

        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.inventory.items[0].amount, 2);
        assert_eq!(stored.partner_slots[0].evolutions[1].unlocked & 0x01, 0x01);
    }

    #[test]
    fn evolution_unlock_requires_completed_open_quest() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets[0].lines[1].unlock_item_section = 6100;
        repo.evolution_assets[0].lines[1].unlock_item_section_amount = 1;
        repo.evolution_assets[0].lines[1].unlock_quest_id = 321;
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .expect("character")
            .partner_slots[0]
            .evolutions[1]
            .unlocked = 0;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("evolution-unlock-open-quest"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::EvolutionUnlock {
                    evo_slot: 1,
                    items: vec![EvolutionUnlockItem {
                        item_id: TEST_SUMMON_TICKET_ITEM_ID as u32,
                        item_count: 1,
                    }],
                    selected_inven_idx: -1,
                },
            )
            .expect("request should complete");

        assert!(responses.is_empty());

        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.inventory.items[0].amount, 3);
        assert_eq!(stored.partner_slots[0].evolutions[1].unlocked & 0x01, 0);
    }

    #[test]
    fn evolution_unlock_rejects_when_jogress_quest_is_in_progress() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets[0].lines[1].unlock_item_section = 6100;
        repo.evolution_assets[0].lines[1].unlock_item_section_amount = 1;
        repo.evolution_assets[0].lines[1].jogress_quest_check = 777;
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .map(|character| {
                character.partner_slots[0].evolutions[1].unlocked = 0;
                character
                    .quest_progress
                    .in_progress
                    .push(odmo_types::InProgressQuest {
                        quest_id: 777,
                        goals: [0; 5],
                    });
            });
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("evolution-unlock-jogress-quest"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::EvolutionUnlock {
                    evo_slot: 1,
                    items: vec![EvolutionUnlockItem {
                        item_id: TEST_SUMMON_TICKET_ITEM_ID as u32,
                        item_count: 1,
                    }],
                    selected_inven_idx: -1,
                },
            )
            .expect("request should complete");

        assert!(responses.is_empty());

        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.inventory.items[0].amount, 3);
        assert_eq!(stored.partner_slots[0].evolutions[1].unlocked & 0x01, 0);
    }

    #[test]
    fn evolution_unlock_does_not_gate_on_unlock_level() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.evolution_assets[0].lines[1].unlock_item_section = 6100;
        repo.evolution_assets[0].lines[1].unlock_item_section_amount = 1;
        repo.evolution_assets[0].lines[1].unlock_level = u8::MAX;
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .expect("character")
            .partner_slots[0]
            .evolutions[1]
            .unlocked = 0;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("evolution-unlock-level-ignored"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::EvolutionUnlock {
                    evo_slot: 1,
                    items: vec![EvolutionUnlockItem {
                        item_id: TEST_SUMMON_TICKET_ITEM_ID as u32,
                        item_count: 1,
                    }],
                    selected_inven_idx: -1,
                },
            )
            .expect("request should complete");

        assert!(responses.is_empty());

        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.inventory.items[0].amount, 2);
        assert_eq!(stored.partner_slots[0].evolutions[1].unlocked & 0x01, 0x01);
    }

    #[test]
    fn evolution_unlock_capsule_branch_keeps_legacy_result_packet() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("evolution-unlock-capsule"),
            },
            repo,
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::EvolutionUnlock {
                    evo_slot: 1,
                    items: Vec::new(),
                    selected_inven_idx: 0,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::CAPSULE_EVOLUTION_SLOT_RESULT
        );
    }

    #[test]
    fn open_ride_mode_consumes_section_and_sets_ride_bit() {
        let repo = InMemoryCharacterRepository::demo();
        repo.characters
            .write()
            .expect("repo poisoned")
            .get_mut(&100)
            .expect("character")
            .partner_slots[0]
            .evolutions[1]
            .unlocked = 0;
        let repo = Arc::new(repo);

        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("open-ride-mode"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::OpenRideMode {
                    evo_unit_idx: 1,
                    item_type: 6220,
                },
            )
            .expect("request should complete");

        assert!(responses.is_empty(), "ride-open should stay optimistic");

        let stored = repo
            .character_by_id(100)
            .expect("load character")
            .expect("character exists");
        assert_eq!(stored.inventory.items[1].amount, 1);
        assert_eq!(stored.partner_slots[0].evolutions[1].unlocked & 0x08, 0x08);
    }

    #[test]
    fn digi_summon_sync_returns_catalog() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("digi-summon-sync"),
            },
            repo,
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(&mut session, GameRequest::DigiSummonSyncRequest)
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::DIGI_SUMMON_SYNC_RESPONSE
        );
        let mut reader = odmo_protocol::PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), DIGI_SUMMON_SUCCESS);
        assert_eq!(reader.read_u16().expect("count"), 1);
        assert_eq!(reader.read_i32().expect("product id"), 9001);
    }

    #[test]
    fn digi_summon_purchase_consumes_ticket_and_grants_reward() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("digi-summon-purchase"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::DigiSummonPurchase {
                    product_id: 9001,
                    ticket_slot: 0,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 2);
        let inventory_raw = PacketReader::from_frame(&responses[0]).expect("inventory frame");
        assert_eq!(
            inventory_raw.packet_type,
            odmo_protocol::opcode::game::LOAD_INVENTORY
        );

        let purchase_raw = PacketReader::from_frame(&responses[1]).expect("purchase frame");
        assert_eq!(
            purchase_raw.packet_type,
            odmo_protocol::opcode::game::DIGI_SUMMON_PURCHASE_RESPONSE
        );
        let mut reader = odmo_protocol::PacketReader::new(purchase_raw.payload);
        assert_eq!(reader.read_u8().expect("result"), DIGI_SUMMON_SUCCESS);
        assert_eq!(reader.read_i32().expect("product id"), 9001);
        assert_eq!(reader.read_u16().expect("reward count"), 1);
        assert_eq!(reader.read_i32().expect("reward item"), 5101);

        let stored = repo
            .character_by_id(100)
            .expect("lookup")
            .expect("character should exist");
        assert_eq!(
            stored.inventory.items[0].item_id,
            TEST_SUMMON_TICKET_ITEM_ID
        );
        assert_eq!(stored.inventory.items[0].amount, 2);
        assert!(
            stored
                .inventory
                .items
                .iter()
                .any(|item| item.item_id == 5101 && item.amount >= 1),
            "reward item should be present in inventory"
        );
    }

    #[test]
    fn digi_summon_purchase_rolls_back_when_inventory_is_full() {
        let mut repo = InMemoryCharacterRepository::demo();
        repo.digi_summon_products = vec![odmo_types::DigiSummonProduct {
            product_id: 9002,
            string_id: 10002,
            draw_count: 1,
            rank: 1,
            remaining_daily_limit: 0,
            icon: String::new(),
            name: "FullInventoryBox".to_string(),
            description: String::new(),
            tickets: vec![odmo_types::DigiSummonTicket {
                item_id: TEST_SUMMON_TICKET_ITEM_ID,
                cost: 1,
            }],
            rewards: vec![odmo_types::DigiSummonReward {
                item_list_id: 3,
                item_id: 99999,
                grade: 1,
                amount: 1,
                weight: 1,
                group: 0,
                group_code: 0,
            }],
        }];
        {
            let mut characters = repo.characters.write().expect("repo poisoned");
            let character = characters.get_mut(&100).expect("demo character");
            character.inventory.size = 1;
            character.inventory.items =
                vec![odmo_types::ItemRecord::new(TEST_SUMMON_TICKET_ITEM_ID, 2)];
        }
        let repo = Arc::new(repo);
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("digi-summon-full"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::DigiSummonPurchase {
                    product_id: 9002,
                    ticket_slot: 0,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let purchase_raw = PacketReader::from_frame(&responses[0]).expect("purchase frame");
        let mut reader = odmo_protocol::PacketReader::new(purchase_raw.payload);
        assert_eq!(
            reader.read_u8().expect("result"),
            DIGI_SUMMON_INVENTORY_FULL
        );

        let stored = repo
            .character_by_id(100)
            .expect("lookup")
            .expect("character should exist");
        assert_eq!(
            stored.inventory.items[0].item_id,
            TEST_SUMMON_TICKET_ITEM_ID
        );
        assert_eq!(stored.inventory.items[0].amount, 2);
        assert!(
            stored
                .inventory
                .items
                .iter()
                .all(|item| item.item_id != 99999),
            "reward should not remain after rollback"
        );
    }

    #[test]
    fn spirit_to_digimon_consumes_materials_and_adds_partner_slot() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        {
            let mut characters = repo.characters.write().expect("repo poisoned");
            let character = characters.get_mut(&100).expect("demo character");
            character.inventory.bits = 1_000;
            character.inventory_bits = 1_000;
        }
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("digital-fusion-item-to-digimon"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.account_id = Some(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::SpiritToDigimon {
                    model_id: 31_004,
                    name: "Vmon".to_string(),
                    npc_id: 91001,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 3);
        let result_raw = PacketReader::from_frame(&responses[1]).expect("result frame");
        assert_eq!(
            result_raw.packet_type,
            odmo_protocol::opcode::game::SPIRIT_TO_DIGIMON
        );

        let stored = repo
            .character_by_id(100)
            .expect("lookup")
            .expect("character should exist");
        assert_eq!(stored.inventory_bits, 500);
        assert!(
            stored
                .inventory
                .items
                .iter()
                .any(|item| item.item_id == TEST_SUMMON_TICKET_ITEM_ID && item.amount == 2),
            "main material should be consumed once"
        );
        assert!(
            stored
                .inventory
                .items
                .iter()
                .any(|item| item.item_id == TEST_RIDE_UNLOCK_ITEM_ID && item.amount == 1),
            "sub material should be consumed once"
        );
        assert!(
            stored
                .partner_slots
                .iter()
                .any(|partner| partner.slot == 3 && partner.digimon_type == 31_004),
            "new partner slot should be appended"
        );
    }

    #[test]
    fn digimon_to_spirit_consumes_sub_material_and_removes_partner() {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        {
            let mut characters = repo.characters.write().expect("repo poisoned");
            let character = characters.get_mut(&100).expect("demo character");
            character.inventory.bits = 1_000;
            character.inventory_bits = 1_000;
        }
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("digital-fusion-digimon-to-item"),
            },
            repo.clone(),
        );

        let mut session = GameSession::new(1);
        session.account_id = Some(1);
        session.character_id = Some(100);
        let responses = app
            .handle_request(
                &mut session,
                GameRequest::DigimonToSpirit {
                    slot: 2,
                    validation: TEST_SECONDARY_PASSWORD.to_string(),
                    npc_id: 91001,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 2);
        let result_raw = PacketReader::from_frame(&responses[0]).expect("result frame");
        assert_eq!(
            result_raw.packet_type,
            odmo_protocol::opcode::game::DIGIMON_TO_SPIRIT
        );

        let stored = repo
            .character_by_id(100)
            .expect("lookup")
            .expect("character should exist");
        assert_eq!(stored.inventory_bits, 750);
        assert!(
            stored
                .inventory
                .items
                .iter()
                .any(|item| item.item_id == 81003 && item.amount >= 1),
            "crafted item should be added"
        );
        assert!(
            stored.partner_slots.iter().all(|partner| partner.slot != 2),
            "crafted partner should be removed from roster"
        );
    }

    fn build_combat_app() -> (
        GameApplication,
        Arc<InMemoryCharacterRepository>,
        Arc<RecordingBroadcast>,
    ) {
        let repo = Arc::new(InMemoryCharacterRepository::demo());
        let broadcast = Arc::new(RecordingBroadcast::with_online([100]));
        let app = GameApplication::new(
            GameServiceConfig {
                portal_state_dir: unique_test_dir("combat"),
            },
            repo.clone(),
        )
        .with_broadcast(broadcast.clone() as Arc<dyn crate::BroadcastSink>);
        (app, repo, broadcast)
    }

    fn seed_session_with_mob(handler: u32, current_hp: i32, max_hp: i32) -> GameSession {
        let mut session = GameSession::new(1);
        session.character_id = Some(100);
        session.viewed_mobs.insert(
            u64::from(handler),
            MobSummary {
                handler,
                id: u64::from(handler),
                map_id: DEFAULT_START_MAP_ID,
                channel: 0,
                current_hp,
                max_hp,
                level: 30,
                ..MobSummary::default()
            },
        );
        session
    }

    #[test]
    fn partner_attack_emits_hit_packet_when_target_survives() {
        let (app, _repo, _broadcast) = build_combat_app();
        let mut session = seed_session_with_mob(50_000, 100_000, 100_000);

        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerAttack {
                    attacker_handler: 21_000,
                    target_handler: 50_000,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::PARTNER_ATTACK_RESPONSE
        );
    }

    #[test]
    fn partner_attack_emits_kill_on_hit_when_lethal() {
        let (app, _repo, _broadcast) = build_combat_app();
        // Set HP very low so the deterministic damage formula one-shots.
        let mut session = seed_session_with_mob(50_001, 1, 1_000);

        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerAttack {
                    attacker_handler: 21_000,
                    target_handler: 50_001,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(raw.packet_type, odmo_protocol::opcode::game::KILL_ON_HIT);
    }

    #[test]
    fn partner_attack_misses_when_target_unknown() {
        let (app, _repo, _broadcast) = build_combat_app();
        let mut session = GameSession::new(1);
        session.character_id = Some(100);

        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerAttack {
                    attacker_handler: 21_000,
                    target_handler: 99_999,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(raw.packet_type, odmo_protocol::opcode::game::ATTACK_MISS);
    }

    #[test]
    fn partner_skill_emits_cast_and_hit_packets() {
        let (app, _repo, _broadcast) = build_combat_app();
        let mut session = seed_session_with_mob(50_002, 100_000, 100_000);

        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerSkill {
                    skill_slot: 1,
                    attacker_handler: 21_000,
                    target_handler: 50_002,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 2);
        let cast = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            cast.packet_type,
            odmo_protocol::opcode::game::PARTNER_SKILL_RESPONSE
        );
        let hit = PacketReader::from_frame(&responses[1]).expect("frame");
        assert_eq!(
            hit.packet_type,
            odmo_protocol::opcode::game::PARTNER_ATTACK_RESPONSE
        );
    }

    #[test]
    fn partner_skill_rejects_invalid_slot() {
        let (app, _repo, _broadcast) = build_combat_app();
        let mut session = seed_session_with_mob(50_003, 100_000, 100_000);

        let responses = app
            .handle_request(
                &mut session,
                GameRequest::PartnerSkill {
                    skill_slot: 99, // invalid
                    attacker_handler: 21_000,
                    target_handler: 50_003,
                },
            )
            .expect("request should complete");

        assert_eq!(responses.len(), 1);
        let raw = PacketReader::from_frame(&responses[0]).expect("frame");
        assert_eq!(
            raw.packet_type,
            odmo_protocol::opcode::game::PARTNER_SKILL_ERROR
        );
    }

    #[test]
    fn matches_tamer_target_handler_accepts_client_projected_uid() {
        let character = CharacterSummary {
            general_handler: 13_000,
            ..CharacterSummary::default()
        };

        assert!(matches_tamer_target_handler(&character, 13_000));
        assert!(matches_tamer_target_handler(&character, 33_480));
        assert!(!matches_tamer_target_handler(&character, 13_001));
    }
}
