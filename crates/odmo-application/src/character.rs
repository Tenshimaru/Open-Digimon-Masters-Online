use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;

use odmo_protocol::character::{
    AvailableNamePacket, CharacterConnectionPacket, CharacterCreatedPacket,
    CharacterCreationFailedPacket, CharacterCreationFailure, CharacterDeletedPacket,
    CharacterListPacket, CharacterRequest, ConnectGameServerInfoPacket, ConnectGameServerPacket,
    DeleteCharacterResult,
};
use odmo_types::{
    Account, AccountId, CharacterSummary, DEFAULT_START_MAP_ID, DEFAULT_START_X, DEFAULT_START_Y,
    GameServerTarget, GameSessionTicket,
};
use uuid::Uuid;

use crate::portal::PortalBridge;

const HANDSHAKE_DEGREE: i16 = 32321;
const HANDSHAKE_STAMP_MASK: u32 = 0xFFFF;

#[derive(Debug, Clone)]
pub struct CharacterServiceConfig {
    pub game_server: GameServerTarget,
    pub portal_state_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CharacterSession {
    pub handshake_seed: i16,
    pub account_id: Option<AccountId>,
    pub selected_character_id: Option<u64>,
}

impl CharacterSession {
    pub fn new(handshake_seed: i16) -> Self {
        Self {
            handshake_seed,
            account_id: None,
            selected_character_id: None,
        }
    }
}

pub trait CharacterRepository: Send + Sync {
    fn list_characters_by_account(
        &self,
        account_id: AccountId,
    ) -> anyhow::Result<Vec<CharacterSummary>>;
    fn character_by_slot(
        &self,
        account_id: AccountId,
        slot: u8,
    ) -> anyhow::Result<Option<CharacterSummary>>;
    fn character_by_id(&self, character_id: u64) -> anyhow::Result<Option<CharacterSummary>>;
    fn character_by_name(&self, name: &str) -> anyhow::Result<Option<CharacterSummary>>;
    fn is_name_available(&self, name: &str) -> anyhow::Result<bool>;
    fn create_character(
        &self,
        account_id: AccountId,
        slot: u8,
        tamer_name: String,
        tamer_model: i32,
        partner_name: String,
        partner_model: i32,
    ) -> anyhow::Result<CharacterSummary>;
    fn delete_character(&self, account_id: AccountId, slot: u8) -> anyhow::Result<bool>;
    fn update_character_position(
        &self,
        character_id: u64,
        x: i32,
        y: i32,
        z: f32,
    ) -> anyhow::Result<()>;
    fn update_partner_position(
        &self,
        character_id: u64,
        x: i32,
        y: i32,
        z: f32,
    ) -> anyhow::Result<()>;
    fn switch_partner(
        &self,
        character_id: u64,
        slot: u8,
    ) -> anyhow::Result<Option<CharacterSummary>>;
    fn update_character_map(
        &self,
        character_id: u64,
        map_id: i16,
        x: i32,
        y: i32,
    ) -> anyhow::Result<()>;
    fn update_inventory(
        &self,
        character_id: u64,
        inventory: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()>;
    fn update_equipment(&self, character_id: u64, equipment: Vec<u8>) -> anyhow::Result<()>;
    fn update_digivice(
        &self,
        _character_id: u64,
        _digivice: odmo_types::DigiviceSnapshot,
    ) -> anyhow::Result<()> {
        Ok(())
    }
    fn update_extra_inventory(
        &self,
        character_id: u64,
        extra_inventory: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()>;
    fn update_warehouse(
        &self,
        character_id: u64,
        warehouse: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()>;
    fn update_account_warehouse(
        &self,
        character_id: u64,
        account_warehouse: odmo_types::InventorySnapshot,
    ) -> anyhow::Result<()>;
    fn update_character_map_region(
        &self,
        character_id: u64,
        map_id: i16,
        unlocked: bool,
    ) -> anyhow::Result<()>;
    fn update_character_state(&self, character_id: u64, state: u8) -> anyhow::Result<()>;
    fn update_welcome_flag(&self, account_id: AccountId, welcome: bool) -> anyhow::Result<()>;
    fn update_partner_type(&self, character_id: u64, new_type: i32) -> anyhow::Result<()>;
    fn update_partner_roster(
        &self,
        _character_id: u64,
        _partner_current_slot: u8,
        _partner_slots: Vec<odmo_types::PartnerSlotSnapshot>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    // ---- Extended persistence (Onda completa) -----------------------------

    /// Replace the character's quest progress.
    fn update_quest_progress(
        &self,
        _character_id: u64,
        _progress: odmo_types::QuestProgressSnapshot,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's encyclopedia.
    fn update_encyclopedia(
        &self,
        _character_id: u64,
        _encyclopedia: odmo_types::EncyclopediaSnapshot,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's friend list.
    fn update_friend_list(
        &self,
        _character_id: u64,
        _friends: Vec<odmo_types::FriendListEntry>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's cash shop history.
    fn update_cash_shop_history(
        &self,
        _character_id: u64,
        _history: Vec<odmo_types::CashShopHistoryEntry>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's digimon archive (storage of dormant partners).
    fn update_digimon_archive(
        &self,
        _character_id: u64,
        _archive: Vec<odmo_types::DigimonArchiveEntry>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's hatch state.
    fn update_hatch_state(
        &self,
        _character_id: u64,
        _state: odmo_types::HatchState,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Set the character's damage skin id.
    fn update_damage_skin(&self, _character_id: u64, _skin_id: i32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Set the character's currently equipped title.
    fn update_current_title(&self, _character_id: u64, _title_id: u16) -> anyhow::Result<()> {
        Ok(())
    }

    /// Set the character's owned titles.
    fn update_owned_titles(&self, _character_id: u64, _owned: Vec<i16>) -> anyhow::Result<()> {
        Ok(())
    }

    /// Set the character's tamer model id.
    fn update_tamer_model(&self, _character_id: u64, _model_id: i32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Set the character's name.
    fn update_tamer_name(&self, _character_id: u64, _new_name: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Set the character's HP/DS state (current and max).
    fn update_tamer_resources(
        &self,
        _character_id: u64,
        _current_hp: i32,
        _current_ds: i32,
        _current_xgauge: i32,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's bits balance.
    fn update_inventory_bits(&self, _character_id: u64, _bits: i64) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's premium / silk balances.
    fn update_currencies(
        &self,
        _character_id: u64,
        _premium: i32,
        _silk: i32,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's seal list.
    fn update_seal_list(
        &self,
        _character_id: u64,
        _seal_list: odmo_types::SealListSnapshot,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's active buffs.
    fn update_active_buffs(
        &self,
        _character_id: u64,
        _buffs: Vec<odmo_types::ActiveBuffSnapshot>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's deck buff id.
    fn update_deck_buff(&self, _character_id: u64, _deck_buff_id: i32) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's reward storage.
    fn update_reward_storage(
        &self,
        _character_id: u64,
        _items: Vec<odmo_types::ItemRecord>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's gift storage.
    fn update_gift_storage(
        &self,
        _character_id: u64,
        _items: Vec<odmo_types::ItemRecord>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's NPC repurchase log.
    fn update_npc_repurchase_log(
        &self,
        _character_id: u64,
        _items: Vec<odmo_types::ItemRecord>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's tamer-shop listings.
    fn update_tamer_shop(
        &self,
        _character_id: u64,
        _listings: Vec<odmo_types::ConsignedShopListing>,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the character's season pass state.
    fn update_season_pass(
        &self,
        _character_id: u64,
        _state: odmo_types::SeasonPassState,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the partner's HP/DS state.
    fn update_partner_resources(
        &self,
        _character_id: u64,
        _current_hp: i32,
        _current_ds: i32,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the partner's name.
    fn update_partner_name(&self, _character_id: u64, _new_name: &str) -> anyhow::Result<()> {
        Ok(())
    }

    /// Replace the partner's memory skill at the given slot.
    fn update_partner_memory_skills(
        &self,
        _character_id: u64,
        _skills: [i32; 4],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    /// Read the D-Unit (Union hacking tool) slots installed on a character.
    /// Each row represents one unlocked slot. New characters return an empty list.
    fn union_hack_slots(
        &self,
        _character_id: u64,
    ) -> anyhow::Result<Vec<odmo_types::UnionHackSlotRow>> {
        Ok(Vec::new())
    }

    /// Replace the part installed in a D-Unit slot. Returns `true` if the row
    /// was created/updated, `false` if the slot index is out of range.
    fn update_union_hack_slot(
        &self,
        _character_id: u64,
        _slot: u8,
        _part_id: i32,
        _grade: i16,
    ) -> anyhow::Result<bool> {
        Ok(false)
    }

    // ---- Cross-character lookups -----------------------------------------

    /// Search by partial name fragment for friend search.
    fn search_characters_by_name(
        &self,
        _name_fragment: &str,
        _limit: u32,
    ) -> anyhow::Result<Vec<CharacterSummary>> {
        Ok(Vec::new())
    }
}

pub trait CharacterAccountRepository: Send + Sync {
    fn account_by_id(&self, account_id: AccountId) -> anyhow::Result<Option<Account>>;
}

#[derive(Clone)]
pub struct CharacterApplication {
    config: CharacterServiceConfig,
    portal_bridge: PortalBridge,
    repository: Arc<dyn CharacterRepository>,
    account_repository: Arc<dyn CharacterAccountRepository>,
    state: Arc<RwLock<CharacterState>>,
}

#[derive(Debug)]
struct CharacterState {
    authorized_accounts: HashSet<AccountId>,
}

impl CharacterApplication {
    pub fn new(
        config: CharacterServiceConfig,
        repository: Arc<dyn CharacterRepository>,
        account_repository: Arc<dyn CharacterAccountRepository>,
    ) -> Self {
        let portal_bridge = PortalBridge::from_json(config.portal_state_dir.clone())
            .expect("portal bridge should initialize");
        Self {
            portal_bridge,
            config,
            repository,
            account_repository,
            state: Arc::new(RwLock::new(CharacterState {
                authorized_accounts: HashSet::new(),
            })),
        }
    }

    pub fn handle_request(
        &self,
        session: &mut CharacterSession,
        request: CharacterRequest,
    ) -> Result<Vec<Vec<u8>>, CharacterFlowError> {
        match request {
            CharacterRequest::Connection { .. } => Ok(vec![
                CharacterConnectionPacket {
                    handshake: session.handshake_seed ^ HANDSHAKE_DEGREE,
                }
                .encode(),
            ]),
            CharacterRequest::KeepConnection => Ok(Vec::new()),
            CharacterRequest::RequestCharacters { account_id } => {
                self.authorize_account(account_id)?;
                session.account_id = Some(account_id);

                let characters = self
                    .repository
                    .list_characters_by_account(account_id)
                    .map_err(|error| CharacterFlowError::Storage(error.to_string()))?;

                Ok(vec![CharacterListPacket { characters }.encode()])
            }
            CharacterRequest::CreateCharacter {
                slot,
                tamer_model,
                tamer_name,
                partner_model,
                partner_name,
            } => {
                let account_id = self.require_authorized(session)?;

                if tamer_name.trim().is_empty() || partner_name.trim().is_empty() {
                    return Ok(vec![
                        CharacterCreationFailedPacket {
                            result: CharacterCreationFailure::Generic,
                        }
                        .encode(),
                    ]);
                }

                if !self
                    .repository
                    .is_name_available(&tamer_name)
                    .map_err(|error| CharacterFlowError::Storage(error.to_string()))?
                {
                    return Ok(vec![
                        CharacterCreationFailedPacket {
                            result: CharacterCreationFailure::ConflictingTamerName,
                        }
                        .encode(),
                    ]);
                }

                let created = self
                    .repository
                    .create_character(
                        account_id,
                        slot,
                        tamer_name,
                        tamer_model,
                        partner_name,
                        partner_model,
                    )
                    .map_err(|error| CharacterFlowError::Storage(error.to_string()))?;

                let handshake = (unix_timestamp() & HANDSHAKE_STAMP_MASK) as i16;
                Ok(vec![
                    CharacterCreatedPacket {
                        character: created,
                        handshake,
                    }
                    .encode(),
                ])
            }
            CharacterRequest::DeleteCharacter { slot, validation } => {
                let account_id = self.require_authorized(session)?;
                let account = self
                    .account_repository
                    .account_by_id(account_id)
                    .map_err(|error| CharacterFlowError::Storage(error.to_string()))?
                    .ok_or(CharacterFlowError::Unauthenticated)?;

                let result = if validation == account.email
                    || account.secondary_password.as_deref() == Some(validation.as_str())
                {
                    if self
                        .repository
                        .delete_character(account_id, slot)
                        .map_err(|error| CharacterFlowError::Storage(error.to_string()))?
                    {
                        DeleteCharacterResult::Deleted
                    } else {
                        DeleteCharacterResult::Error
                    }
                } else {
                    DeleteCharacterResult::ValidationFail
                };

                Ok(vec![CharacterDeletedPacket { result }.encode()])
            }
            CharacterRequest::GetCharacterPosition { slot } => {
                let account_id = self.require_authorized(session)?;
                let mut character = self
                    .repository
                    .character_by_slot(account_id, slot)
                    .map_err(|error| CharacterFlowError::Storage(error.to_string()))?
                    .ok_or(CharacterFlowError::CharacterNotFound(slot))?;

                // Normalize legacy map positions: if character is on map 0 or 1, move to modern spawn.
                if character.map_id <= 1 {
                    character.map_id = DEFAULT_START_MAP_ID;
                    character.x = DEFAULT_START_X;
                    character.y = DEFAULT_START_Y;
                    let _ = self.repository.update_character_map(
                        character.id,
                        DEFAULT_START_MAP_ID,
                        DEFAULT_START_X,
                        DEFAULT_START_Y,
                    );
                }

                session.selected_character_id = Some(character.id);
                let _ = self
                    .portal_bridge
                    .store_game_session_ticket(&GameSessionTicket {
                        token: Uuid::new_v4().to_string(),
                        account_id,
                        character_id: character.id,
                    });

                Ok(vec![ConnectGameServerInfoPacket {
                    address: self.config.game_server.address.clone(),
                    port: self.config.game_server.port,
                    map_id: character.map_id,
                }
                .encode()])
            }
            CharacterRequest::ConnectGameServer => {
                self.require_authorized(session)?;
                Ok(vec![ConnectGameServerPacket.encode()])
            }
            CharacterRequest::CheckNameDuplicity { name } => {
                let available = self
                    .repository
                    .is_name_available(&name)
                    .map_err(|error| CharacterFlowError::Storage(error.to_string()))?;
                Ok(vec![AvailableNamePacket { available }.encode()])
            }
        }
    }

    fn authorize_account(&self, account_id: AccountId) -> Result<(), CharacterFlowError> {
        let Some(ticket) = self
            .portal_bridge
            .consume_transfer_ticket(account_id)
            .map_err(|_| CharacterFlowError::PortalBridgeUnavailable)?
        else {
            return Err(CharacterFlowError::MissingTransferTicket(account_id));
        };

        let mut state = self.write_state();
        state.authorized_accounts.insert(ticket.account_id);
        Ok(())
    }

    fn require_authorized(
        &self,
        session: &CharacterSession,
    ) -> Result<AccountId, CharacterFlowError> {
        let account_id = session
            .account_id
            .ok_or(CharacterFlowError::Unauthenticated)?;
        if !self.read_state().authorized_accounts.contains(&account_id) {
            return Err(CharacterFlowError::MissingTransferTicket(account_id));
        }
        Ok(account_id)
    }

    fn read_state(&self) -> std::sync::RwLockReadGuard<'_, CharacterState> {
        self.state.read().expect("character state poisoned")
    }

    fn write_state(&self) -> std::sync::RwLockWriteGuard<'_, CharacterState> {
        self.state.write().expect("character state poisoned")
    }
}

#[derive(Debug)]
pub struct CharacterSessionFactory {
    next_seed: std::sync::atomic::AtomicI16,
}

impl Default for CharacterSessionFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl CharacterSessionFactory {
    pub fn new() -> Self {
        Self {
            next_seed: std::sync::atomic::AtomicI16::new(2_000),
        }
    }

    pub fn create(&self) -> CharacterSession {
        let seed = self
            .next_seed
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        CharacterSession::new(seed)
    }
}

#[derive(Debug, Error)]
pub enum CharacterFlowError {
    #[error("request requires authenticated account")]
    Unauthenticated,
    #[error("missing transfer ticket for account {0}")]
    MissingTransferTicket(AccountId),
    #[error("portal bridge unavailable")]
    PortalBridgeUnavailable,
    #[error("character slot {0} not found")]
    CharacterNotFound(u8),
    #[error("storage error: {0}")]
    Storage(String),
}

fn unix_timestamp() -> u32 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as u32
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{Arc, RwLock},
    };

    use super::*;
    use crate::portal::PortalBridge;
    use odmo_types::{
        AccessLevel, DEFAULT_GM_PARTNER_MODEL_ID, DEFAULT_GM_TAMER_MODEL_ID,
        DEFAULT_PARTNER_MODEL_ID, DEFAULT_TAMER_MODEL_ID, TransferTicket,
    };

    const TEST_SECONDARY_PASSWORD: &str = "4321";

    fn unique_test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("odmo-{name}-{}", uuid::Uuid::new_v4()))
    }

    #[derive(Debug)]
    struct InMemoryCharacterRepository {
        characters_by_account: RwLock<HashMap<AccountId, Vec<CharacterSummary>>>,
    }

    impl InMemoryCharacterRepository {
        fn demo() -> Self {
            Self {
                characters_by_account: RwLock::new(HashMap::from([
                    (
                        1,
                        vec![CharacterSummary {
                            id: 100,
                            account_id: 1,
                            slot: 0,
                            name: "AdminTamer".to_string(),
                            partner_name: "Agumon".to_string(),
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
                            partner_name: "Gabumon".to_string(),
                            general_handler: 12_000,
                            partner_handler: 22_000,
                            model: DEFAULT_GM_TAMER_MODEL_ID,
                            partner_model: DEFAULT_GM_PARTNER_MODEL_ID,
                            ..CharacterSummary::default()
                        }],
                    ),
                ])),
            }
        }
    }

    impl CharacterRepository for InMemoryCharacterRepository {
        fn list_characters_by_account(
            &self,
            account_id: AccountId,
        ) -> anyhow::Result<Vec<CharacterSummary>> {
            Ok(self
                .characters_by_account
                .read()
                .expect("repo poisoned")
                .get(&account_id)
                .cloned()
                .unwrap_or_default())
        }

        fn character_by_slot(
            &self,
            account_id: AccountId,
            slot: u8,
        ) -> anyhow::Result<Option<CharacterSummary>> {
            Ok(self
                .characters_by_account
                .read()
                .expect("repo poisoned")
                .get(&account_id)
                .and_then(|characters| characters.iter().find(|character| character.slot == slot))
                .cloned())
        }

        fn character_by_id(&self, character_id: u64) -> anyhow::Result<Option<CharacterSummary>> {
            Ok(self
                .characters_by_account
                .read()
                .expect("repo poisoned")
                .values()
                .flatten()
                .find(|character| character.id == character_id)
                .cloned())
        }

        fn character_by_name(&self, name: &str) -> anyhow::Result<Option<CharacterSummary>> {
            Ok(self
                .characters_by_account
                .read()
                .expect("repo poisoned")
                .values()
                .flatten()
                .find(|character| character.name.eq_ignore_ascii_case(name))
                .cloned())
        }

        fn is_name_available(&self, name: &str) -> anyhow::Result<bool> {
            Ok(!self
                .characters_by_account
                .read()
                .expect("repo poisoned")
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
            let mut guard = self.characters_by_account.write().expect("repo poisoned");
            let next_id = guard
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
                partner_slots: vec![odmo_types::PartnerSlotSnapshot {
                    slot: 1,
                    digimon_type: partner_model,
                    model: partner_model,
                    name: partner_name,
                    ..odmo_types::PartnerSlotSnapshot::default()
                }],
                general_handler: next_id as u32 + 10_000,
                partner_handler: next_id as u32 + 20_000,
                model: tamer_model,
                ..CharacterSummary::default()
            };
            guard.entry(account_id).or_default().push(character.clone());
            Ok(character)
        }

        fn delete_character(&self, account_id: AccountId, slot: u8) -> anyhow::Result<bool> {
            let mut guard = self.characters_by_account.write().expect("repo poisoned");
            let Some(characters) = guard.get_mut(&account_id) else {
                return Ok(false);
            };
            let original_len = characters.len();
            characters.retain(|character| character.slot != slot);
            Ok(original_len != characters.len())
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
        fn update_character_map(
            &self,
            _character_id: u64,
            _map_id: i16,
            _x: i32,
            _y: i32,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        fn switch_partner(
            &self,
            character_id: u64,
            slot: u8,
        ) -> anyhow::Result<Option<CharacterSummary>> {
            let mut guard = self.characters_by_account.write().expect("repo poisoned");
            for characters in guard.values_mut() {
                if let Some(character) = characters.iter_mut().find(|c| c.id == character_id) {
                    let Some(target) = character
                        .partner_slots
                        .iter()
                        .find(|partner| partner.slot == slot)
                        .cloned()
                    else {
                        return Ok(None);
                    };
                    character.partner_current_slot = slot;
                    character.partner_current_type = target.digimon_type;
                    character.partner_model = target.model;
                    character.partner_name = target.name.clone();
                    character.partner_level = target.level;
                    character.partner_hp = target.hp;
                    character.partner_ds = target.ds;
                    character.partner_current_hp = target.current_hp;
                    character.partner_current_ds = target.current_ds;
                    character.partner_active_buffs = target.active_buffs.clone();
                    return Ok(Some(character.clone()));
                }
            }
            Ok(None)
        }
        fn update_inventory(
            &self,
            _character_id: u64,
            _inventory: odmo_types::InventorySnapshot,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        fn update_equipment(&self, _character_id: u64, _equipment: Vec<u8>) -> anyhow::Result<()> {
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
        fn update_welcome_flag(
            &self,
            _account_id: AccountId,
            _welcome: bool,
        ) -> anyhow::Result<()> {
            Ok(())
        }
        fn update_partner_type(&self, _character_id: u64, _new_type: i32) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct InMemoryAccountRepository {
        accounts: HashMap<AccountId, Account>,
    }

    impl InMemoryAccountRepository {
        fn demo() -> Self {
            Self {
                accounts: HashMap::from([
                    (
                        1,
                        Account {
                            id: 1,
                            username: "admin".to_string(),
                            password_hash: "admin".to_string(),
                            email: "admin@odmo.local".to_string(),
                            access_level: AccessLevel::Administrator,
                            secondary_password: None,
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
                ]),
            }
        }
    }

    impl CharacterAccountRepository for InMemoryAccountRepository {
        fn account_by_id(&self, account_id: AccountId) -> anyhow::Result<Option<Account>> {
            Ok(self.accounts.get(&account_id).cloned())
        }
    }

    #[test]
    fn request_characters_requires_transfer_ticket() {
        let app = CharacterApplication::new(
            CharacterServiceConfig {
                game_server: odmo_types::GameServerTarget {
                    address: "127.0.0.1".to_string(),
                    port: 7003,
                },
                portal_state_dir: unique_test_dir("character-no-ticket"),
            },
            Arc::new(InMemoryCharacterRepository::demo()),
            Arc::new(InMemoryAccountRepository::demo()),
        );
        let mut session = CharacterSession::new(1);
        let error = app
            .handle_request(
                &mut session,
                CharacterRequest::RequestCharacters { account_id: 1 },
            )
            .expect_err("ticket should be required");
        assert!(matches!(
            error,
            CharacterFlowError::MissingTransferTicket(1)
        ));
    }

    #[test]
    fn request_characters_consumes_transfer_ticket_and_returns_list() {
        let portal_state_dir = unique_test_dir("character-with-ticket");
        let bridge =
            PortalBridge::from_json(portal_state_dir.clone()).expect("bridge should initialize");
        bridge
            .store_transfer_ticket(&TransferTicket {
                token: "demo".to_string(),
                account_id: 1,
                server_id: 1,
            })
            .expect("ticket should be stored");

        let app = CharacterApplication::new(
            CharacterServiceConfig {
                game_server: odmo_types::GameServerTarget {
                    address: "127.0.0.1".to_string(),
                    port: 7003,
                },
                portal_state_dir,
            },
            Arc::new(InMemoryCharacterRepository::demo()),
            Arc::new(InMemoryAccountRepository::demo()),
        );
        let mut session = CharacterSession::new(1);
        let responses = app
            .handle_request(
                &mut session,
                CharacterRequest::RequestCharacters { account_id: 1 },
            )
            .expect("ticketed request should succeed");
        assert_eq!(responses.len(), 1);
    }

    #[test]
    fn create_character_returns_created_packet() {
        let portal_state_dir = unique_test_dir("character-create");
        let bridge =
            PortalBridge::from_json(portal_state_dir.clone()).expect("bridge should initialize");
        bridge
            .store_transfer_ticket(&TransferTicket {
                token: "demo".to_string(),
                account_id: 1,
                server_id: 1,
            })
            .expect("ticket should be stored");

        let app = CharacterApplication::new(
            CharacterServiceConfig {
                game_server: odmo_types::GameServerTarget {
                    address: "127.0.0.1".to_string(),
                    port: 7003,
                },
                portal_state_dir,
            },
            Arc::new(InMemoryCharacterRepository::demo()),
            Arc::new(InMemoryAccountRepository::demo()),
        );
        let mut session = CharacterSession::new(1);
        app.handle_request(
            &mut session,
            CharacterRequest::RequestCharacters { account_id: 1 },
        )
        .expect("authorization should succeed");
        let responses = app
            .handle_request(
                &mut session,
                CharacterRequest::CreateCharacter {
                    slot: 1,
                    tamer_model: odmo_types::DEFAULT_ALT_TAMER_MODEL_ID,
                    tamer_name: "NewTamer".to_string(),
                    partner_model: odmo_types::DEFAULT_ALT_PARTNER_MODEL_ID,
                    partner_name: "Patamon".to_string(),
                },
            )
            .expect("creation should succeed");
        assert_eq!(responses.len(), 1);
    }
}
