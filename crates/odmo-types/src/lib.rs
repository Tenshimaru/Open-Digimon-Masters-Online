use serde::{Deserialize, Serialize};

pub type AccountId = u64;
pub type CharacterId = u64;
pub type DigimonId = u64;

pub const DEFAULT_START_MAP_ID: i16 = 105;
pub const DEFAULT_START_X: i32 = 39_375;
pub const DEFAULT_START_Y: i32 = 30_935;
pub const DEFAULT_TAMER_MODEL_ID: i32 = 80_001;
pub const DEFAULT_GM_TAMER_MODEL_ID: i32 = 80_010;
pub const DEFAULT_ALT_TAMER_MODEL_ID: i32 = 80_011;
pub const DEFAULT_PARTNER_MODEL_ID: i32 = 31_001;
pub const DEFAULT_GM_PARTNER_MODEL_ID: i32 = 31_002;
pub const DEFAULT_ALT_PARTNER_MODEL_ID: i32 = 31_003;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccessLevel {
    Player,
    GameMaster,
    Administrator,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub username: String,
    pub password_hash: String,
    pub email: String,
    pub access_level: AccessLevel,
    pub secondary_password: Option<String>,
    pub suspension: Option<AccountSuspension>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSuspension {
    pub remaining_seconds: u32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerDescriptor {
    pub id: u32,
    pub name: String,
    pub maintenance: bool,
    pub overloaded: bool,
    pub is_new: bool,
    pub character_count: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterServerTarget {
    pub address: String,
    pub port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferTicket {
    pub token: String,
    pub account_id: AccountId,
    pub server_id: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameSessionTicket {
    pub token: String,
    pub account_id: AccountId,
    pub character_id: CharacterId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ItemRecord {
    pub item_id: i32,
    pub amount: i32,
    pub record: Vec<u8>,
}

impl Default for ItemRecord {
    fn default() -> Self {
        Self {
            item_id: 0,
            amount: 0,
            record: vec![0; 69],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiviceItemSnapshot {
    pub item_id: i32,
    pub amount: i32,
    pub record: Vec<u8>,
}

impl Default for DigiviceItemSnapshot {
    fn default() -> Self {
        Self {
            item_id: 0,
            amount: 0,
            record: vec![0; 60],
        }
    }
}

impl DigiviceItemSnapshot {
    /// Sync the packed 60-byte cItemData/cItemInfo record used by the modern
    /// digivice state into the raw buffer.
    pub fn sync_record(&mut self) {
        if self.record.len() < 60 {
            self.record.resize(60, 0);
        }
        let packed =
            (self.item_id.max(0) as u32 & 0x1FFFF) | ((self.amount.max(0) as u32 & 0x7FFF) << 17);
        self.record[0..4].copy_from_slice(&packed.to_le_bytes());
    }

    pub fn from_record(mut record: Vec<u8>) -> Self {
        if record.len() < 60 {
            record.resize(60, 0);
        }
        let packed = u32::from_le_bytes([record[0], record[1], record[2], record[3]]);
        Self {
            item_id: (packed & 0x1FFFF) as i32,
            amount: ((packed >> 17) & 0x7FFF) as i32,
            record,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiviceSnapshot {
    pub equipped_item: DigiviceItemSnapshot,
    pub chipsets: Vec<DigiviceItemSnapshot>,
    pub tamer_skills: Vec<DigiviceItemSnapshot>,
}

impl Default for DigiviceSnapshot {
    fn default() -> Self {
        Self {
            equipped_item: DigiviceItemSnapshot::default(),
            chipsets: vec![DigiviceItemSnapshot::default(); 12],
            tamer_skills: vec![DigiviceItemSnapshot::default(); 5],
        }
    }
}

impl DigiviceSnapshot {
    pub fn normalize(&mut self) {
        self.equipped_item.sync_record();
        self.chipsets.resize_with(12, DigiviceItemSnapshot::default);
        self.tamer_skills
            .resize_with(5, DigiviceItemSnapshot::default);
        for slot in &mut self.chipsets {
            slot.sync_record();
        }
        for slot in &mut self.tamer_skills {
            slot.sync_record();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct ItemAsset {
    pub item_id: i32,
    pub name: String,
    pub item_type: i32,
    pub section: i32,
    pub combined_section: i32,
    pub overlap: i16,
    pub use_time_group: i32,
    pub use_time_seconds: i32,
    pub quest_requirements: Vec<i32>,
    pub use_character: i32,
    pub bound_type: i32,
    pub use_time_type: i32,
    pub skill_code: i64,
    pub tamer_min_level: u8,
    pub tamer_max_level: u8,
    pub digimon_min_level: u8,
    pub digimon_max_level: u8,
    pub sell_price: i64,
    pub scan_price: i32,
    pub digicore_price: i32,
    pub event_price_id: i32,
    pub event_price_amount: i32,
    pub digivice_skill_slots: u8,
    pub digivice_chipset_slots: u8,
    pub usage_time_minutes: i32,
    pub do_not_use_type: i32,
    pub use_battle: i32,
}

impl ItemRecord {
    /// Sync `item_id` and `amount` into the leading `[u32 item_id][u16 amount]`
    /// fields of the 69-byte item record buffer.
    pub fn sync_record(&mut self) {
        if self.record.len() < 69 {
            self.record.resize(69, 0);
        }
        let item_id = self.item_id.max(0) as u32;
        let amount = self.amount.clamp(0, u16::MAX as i32) as u16;
        self.record[0..4].copy_from_slice(&item_id.to_le_bytes());
        self.record[4..6].copy_from_slice(&amount.to_le_bytes());
    }

    /// Create a new ItemRecord for a given item_id and amount, with record bytes synced.
    pub fn new(item_id: i32, amount: i32) -> Self {
        let mut rec = Self {
            item_id,
            amount,
            record: vec![0; 69],
        };
        rec.sync_record();
        rec
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InventorySnapshot {
    pub bits: i64,
    pub size: u16,
    pub items: Vec<ItemRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChannelAvailability {
    pub channel: u8,
    pub load: u8,
}

impl Default for ChannelAvailability {
    fn default() -> Self {
        Self {
            channel: 0,
            load: 1,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SealRecord {
    pub seal_id: i32,
    pub amount: i16,
    pub sequential_id: i16,
    pub favorite: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SealListSnapshot {
    pub seal_leader_id: i16,
    pub seals: Vec<SealRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DailyRewardStatus {
    pub event_no: i32,
    pub remaining_seconds: i32,
    pub total_seconds: i32,
    pub week: u8,
}

impl Default for DailyRewardStatus {
    fn default() -> Self {
        Self {
            event_no: 0,
            remaining_seconds: 0,
            total_seconds: 0,
            week: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AttendanceStatus {
    pub event_no: u8,
    pub attendance_count: u8,
    pub notify: bool,
}

impl Default for AttendanceStatus {
    fn default() -> Self {
        Self {
            event_no: u8::MAX,
            attendance_count: 0,
            notify: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RelationEntry {
    pub name: String,
    pub connected: bool,
    pub annotation: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildAuthoritySnapshot {
    pub class: u8,
    pub title: String,
    pub duty: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharacterConnectionState {
    Disconnected = 0,
    Loading = 1,
    Connected = 2,
    Ready = 3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildMemberSnapshot {
    pub character_id: CharacterId,
    pub authority: u8,
    pub contribution: i32,
    pub character_name: String,
    pub character_level: u8,
    pub character_model: i32,
    pub map_id: i16,
    pub channel: u8,
    pub state: CharacterConnectionState,
}

impl Default for GuildMemberSnapshot {
    fn default() -> Self {
        Self {
            character_id: 0,
            authority: 5,
            contribution: 0,
            character_name: String::new(),
            character_level: 1,
            character_model: 80_001,
            map_id: 0,
            channel: 0,
            state: CharacterConnectionState::Disconnected,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildHistoricEntry {
    pub historic_type: u8,
    pub date_utc_seconds: u32,
    pub master_class: u8,
    pub master_name: String,
    pub member_class: u8,
    pub member_name: String,
}

impl Default for GuildHistoricEntry {
    fn default() -> Self {
        Self {
            historic_type: 100,
            date_utc_seconds: 0,
            master_class: 1,
            master_name: String::new(),
            member_class: 5,
            member_name: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GuildSnapshot {
    pub id: u32,
    pub name: String,
    pub level: u8,
    pub current_experience: i32,
    pub notice: String,
    pub extra_slots: i32,
    pub authorities: Vec<GuildAuthoritySnapshot>,
    pub members: Vec<GuildMemberSnapshot>,
    pub historic: Vec<GuildHistoricEntry>,
    pub rank_position: i16,
}

impl Default for GuildSnapshot {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            level: 1,
            current_experience: 0,
            notice: "Welcome to the guild!!!".to_string(),
            extra_slots: 0,
            authorities: vec![
                GuildAuthoritySnapshot {
                    class: 1,
                    title: "Master".to_string(),
                    duty: "Master".to_string(),
                },
                GuildAuthoritySnapshot {
                    class: 2,
                    title: "SubMaster".to_string(),
                    duty: "SubMaster".to_string(),
                },
                GuildAuthoritySnapshot {
                    class: 3,
                    title: "DatsMember".to_string(),
                    duty: "DatsMember".to_string(),
                },
                GuildAuthoritySnapshot {
                    class: 4,
                    title: "Member".to_string(),
                    duty: "Member".to_string(),
                },
                GuildAuthoritySnapshot {
                    class: 5,
                    title: "NewMember".to_string(),
                    duty: "NewMember".to_string(),
                },
            ],
            members: Vec::new(),
            historic: Vec::new(),
            rank_position: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct XaiSnapshot {
    pub item_id: i32,
    pub max_xgauge: i32,
    pub max_xcrystals: i16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ActiveBuffSnapshot {
    pub buff_id: u16,
    pub buff_class: u16,
    pub skill_id: i32,
    pub remaining_seconds: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartnerEvolutionSnapshot {
    pub evolution_type: i32,
    pub unlocked: u8,
    pub skill_experience: i32,
    pub skill_points: u8,
    pub skill_mastery: u8,
    pub skill_levels: Vec<u8>,
    pub max_skill_levels: Vec<u8>,
}

impl Default for PartnerEvolutionSnapshot {
    fn default() -> Self {
        Self {
            evolution_type: 0,
            unlocked: 0,
            skill_experience: 0,
            skill_points: 0,
            skill_mastery: 0,
            skill_levels: vec![0; 5],
            max_skill_levels: vec![0; 5],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct EvolutionStageAsset {
    pub target_type: i32,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct EvolutionLineAsset {
    pub type_id: i32,
    pub slot_level: u8,
    pub unlock_level: u8,
    pub unlock_quest_id: i32,
    pub unlock_item_section: i32,
    pub unlock_item_section_amount: i32,
    pub required_item: i32,
    pub required_amount: i32,
    pub required_ds: i32,
    pub enabled: u8,
    pub open_qualification: u16,
    pub use_item_hint: u16,
    pub required_intimacy: u16,
    pub open_crest: u16,
    pub evolution_tree: i32,
    pub icon_pos_x: i32,
    pub icon_pos_y: i32,
    pub jogress_quest_check: i32,
    pub jogress_chipset_type: i32,
    pub jogress_consumable_chipset_type: i32,
    pub jogress_chipset_amount: i32,
    pub jogress_period_chipset_type: i32,
    pub jogress_need_digimon_types: Vec<i32>,
    pub stages: Vec<EvolutionStageAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct EvolutionAsset {
    pub base_type: i32,
    pub lines: Vec<EvolutionLineAsset>,
}

/// Minimal server-owned Digimon table slice needed for faithful evolution-cost
/// and gating rules derived from the modern client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct DigimonAsset {
    pub digimon_id: i32,
    pub base_level: i32,
    pub evolution_type: i32,
}

/// One skill row shown in the partner detail skill grid.
///
/// Carried in the tamer-detail snapshot as an `{id, value}` pair. `value` holds
/// the current skill level (or mastery) for the slot; `skill_id` is 0 for an
/// empty slot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DetailSkillEntry {
    pub skill_id: i32,
    pub value: i32,
}

/// One enchant/attribute row shown in the partner detail enchant grid.
///
/// Carried in the tamer-detail snapshot as a `{kind, value}` pair.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DetailEnchantEntry {
    pub kind: i32,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PartnerSlotSnapshot {
    pub slot: u8,
    pub digimon_type: i32,
    pub model: i32,
    pub level: u8,
    pub name: String,
    pub size: i16,
    pub hatch_grade: u8,
    pub hp: i32,
    pub ds: i32,
    pub current_hp: i32,
    pub current_ds: i32,
    pub de: i32,
    pub at: i32,
    pub fs: i32,
    pub ev: i32,
    pub cc: i32,
    pub ms: i32,
    pub as_value: i32,
    pub ht: i32,
    pub ar: i32,
    pub bl: i32,
    pub clone_level: u16,
    pub clone_at_value: u16,
    pub clone_bl_value: u16,
    pub clone_ct_value: u16,
    pub clone_ev_value: u16,
    pub clone_hp_value: u16,
    pub clone_at_level: u16,
    pub clone_bl_level: u16,
    pub clone_ct_level: u16,
    pub clone_ev_level: u16,
    pub clone_hp_level: u16,
    pub active_buffs: Vec<ActiveBuffSnapshot>,
    pub evolutions: Vec<PartnerEvolutionSnapshot>,
    /// Skills shown in the partner-detail skill grid (detail view).
    pub detail_skills: Vec<DetailSkillEntry>,
    /// Enchant attributes shown in the partner-detail enchant grid.
    pub detail_enchants: Vec<DetailEnchantEntry>,
}

impl Default for PartnerSlotSnapshot {
    fn default() -> Self {
        Self {
            slot: 1,
            digimon_type: 31_001,
            model: DEFAULT_PARTNER_MODEL_ID,
            level: 1,
            name: String::new(),
            size: 12_000,
            hatch_grade: 3,
            hp: 1_000,
            ds: 1_000,
            current_hp: 1_000,
            current_ds: 1_000,
            de: 100,
            at: 100,
            fs: 100,
            ev: 0,
            cc: 0,
            ms: 250,
            as_value: 1_000,
            ht: 0,
            ar: 0,
            bl: 0,
            clone_level: 0,
            clone_at_value: 0,
            clone_bl_value: 0,
            clone_ct_value: 0,
            clone_ev_value: 0,
            clone_hp_value: 0,
            clone_at_level: 0,
            clone_bl_level: 0,
            clone_ct_level: 0,
            clone_ev_level: 0,
            clone_hp_level: 0,
            active_buffs: Vec::new(),
            evolutions: Vec::new(),
            detail_skills: Vec::new(),
            detail_enchants: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MobSummary {
    pub id: u64,
    pub map_id: i16,
    pub channel: u8,
    pub handler: u32,
    pub type_id: i32,
    pub model: i32,
    pub name: String,
    pub level: u8,
    pub x: i32,
    pub y: i32,
    pub previous_x: i32,
    pub previous_y: i32,
    pub current_hp: i32,
    pub max_hp: i32,
    pub grow_stack: u8,
    pub disposed_objects: u8,
    pub respawn: bool,
    pub active_debuffs: Vec<ActiveBuffSnapshot>,
}

impl Default for MobSummary {
    fn default() -> Self {
        Self {
            id: 0,
            map_id: DEFAULT_START_MAP_ID,
            channel: 0,
            handler: 0,
            type_id: DEFAULT_PARTNER_MODEL_ID,
            model: DEFAULT_PARTNER_MODEL_ID,
            name: String::new(),
            level: 1,
            x: DEFAULT_START_X,
            y: DEFAULT_START_Y,
            previous_x: DEFAULT_START_X,
            previous_y: DEFAULT_START_Y,
            current_hp: 1_000,
            max_hp: 1_000,
            grow_stack: 0,
            disposed_objects: 0,
            respawn: false,
            active_debuffs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DropSummary {
    pub id: u64,
    pub map_id: i16,
    pub channel: u8,
    pub handler: u32,
    pub owner_id: u64,
    pub owner_handler: u32,
    pub item_id: i32,
    pub amount: i32,
    pub x: i32,
    pub y: i32,
    pub owner_expires_at_unix: u64,
    pub expires_at_unix: u64,
    pub bits_drop: bool,
    pub no_owner: bool,
    pub collected: bool,
}

impl Default for DropSummary {
    fn default() -> Self {
        Self {
            id: 0,
            map_id: DEFAULT_START_MAP_ID,
            channel: 0,
            handler: 0,
            owner_id: 0,
            owner_handler: 0,
            item_id: 0,
            amount: 1,
            x: DEFAULT_START_X,
            y: DEFAULT_START_Y,
            owner_expires_at_unix: 0,
            expires_at_unix: 0,
            bits_drop: false,
            no_owner: false,
            collected: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterSummary {
    pub id: CharacterId,
    pub account_id: AccountId,
    pub slot: u8,
    pub map_id: i16,
    pub x: i32,
    pub y: i32,
    pub z: f32,
    pub channel: u8,
    pub general_handler: u32,
    pub model: i32,
    pub level: u8,
    pub name: String,
    pub current_experience: i64,
    pub hp: i32,
    pub ds: i32,
    pub current_hp: i32,
    pub current_ds: i32,
    pub fatigue: i32,
    pub at: i32,
    pub de: i32,
    pub ms: i32,
    pub proper_ms: i16,
    pub current_condition: i32,
    pub partner_condition: i32,
    pub inventory_bits: i64,
    pub inventory_size: u16,
    pub warehouse_size: u16,
    pub account_warehouse_size: u16,
    pub extra_inventory_size: u16,
    pub inventory: InventorySnapshot,
    pub warehouse: InventorySnapshot,
    pub account_warehouse: Option<InventorySnapshot>,
    pub extra_inventory: InventorySnapshot,
    pub digimon_slots: u8,
    pub current_title: u16,
    pub map_region: Vec<u8>,
    pub archive_slots: i32,
    pub deck_buff_id: i32,
    pub equipment: Vec<u8>,
    pub digivice: DigiviceSnapshot,
    pub shop_name: String,
    pub size: i16,
    pub active_buffs: Vec<ActiveBuffSnapshot>,
    pub seal_list: SealListSnapshot,
    pub daily_reward: DailyRewardStatus,
    pub attendance: AttendanceStatus,
    pub server_experience: i32,
    pub premium: i32,
    pub silk: i32,
    pub membership_seconds: u32,
    pub available_channels: Vec<ChannelAvailability>,
    pub friends: Vec<RelationEntry>,
    pub foes: Vec<RelationEntry>,
    pub friended_character_ids: Vec<CharacterId>,
    pub guild: Option<GuildSnapshot>,
    pub xai: Option<XaiSnapshot>,
    pub current_xgauge: i32,
    pub current_xcrystals: i16,
    pub partner_x: i32,
    pub partner_y: i32,
    pub partner_z: f32,
    pub partner_current_slot: u8,
    pub partner_current_type: i32,
    pub partner_slots: Vec<PartnerSlotSnapshot>,
    pub partner_active_buffs: Vec<ActiveBuffSnapshot>,
    pub partner_active_debuffs: Vec<ActiveBuffSnapshot>,
    pub partner_model: i32,
    pub partner_level: u8,
    pub partner_name: String,
    pub partner_handler: u32,
    pub partner_size: i16,
    pub partner_hatch_grade: u8,
    pub partner_current_experience: i64,
    pub partner_transcendence_experience: i64,
    pub partner_hp: i32,
    pub partner_ds: i32,
    pub partner_de: i32,
    pub partner_at: i32,
    pub partner_current_hp: i32,
    pub partner_current_ds: i32,
    pub partner_fs: i32,
    pub partner_ev: i32,
    pub partner_cc: i32,
    pub partner_ms: i32,
    pub partner_as: i32,
    pub partner_ht: i32,
    pub partner_ar: i32,
    pub partner_bl: i32,
    pub partner_clone_level: u16,
    pub partner_clone_at_value: u16,
    pub partner_clone_bl_value: u16,
    pub partner_clone_ct_value: u16,
    pub partner_clone_ev_value: u16,
    pub partner_clone_hp_value: u16,
    pub partner_clone_at_level: u16,
    pub partner_clone_bl_level: u16,
    pub partner_clone_ct_level: u16,
    pub partner_clone_ev_level: u16,
    pub partner_clone_hp_level: u16,

    // ---- Extended state (Onda completa) ------------------------------------
    /// Quest progress (in-progress quests + bitmap of completed).
    pub quest_progress: QuestProgressSnapshot,
    /// Encyclopedia entries.
    pub encyclopedia: EncyclopediaSnapshot,
    /// Friend list with annotations and favorite flags.
    pub friend_list: Vec<FriendListEntry>,
    /// Cash shop purchase history (most recent first).
    pub cash_shop_history: Vec<CashShopHistoryEntry>,
    /// Digimon archive (dormant partner storage).
    pub digimon_archive: Vec<DigimonArchiveEntry>,
    /// Hatch incubator state.
    pub hatch_state: HatchState,
    /// Damage skin id currently equipped.
    pub damage_skin_id: i32,
    /// Tamer's currently equipped skill memory chips (slot index -> skill id).
    pub partner_memory_skills: [i32; 4],
    /// Active deck buff id (encyclopedia deck buff currently chosen).
    pub active_deck_buff: i32,
    /// Owned titles bitmap (title id -> owned).
    pub owned_titles: Vec<i16>,
    /// Reroll counters consumed today.
    pub jump_booster_count: i32,
    /// Season pass state.
    pub season_pass: SeasonPassState,
    /// Personal/tamer shop listings posted.
    pub tamer_shop_listings: Vec<ConsignedShopListing>,
    /// Repurchase log (last 10 sold items).
    pub npc_repurchase_log: Vec<ItemRecord>,
    /// Reward storage (mission/event rewards waiting to be claimed).
    pub reward_storage: Vec<ItemRecord>,
    /// Gift storage (premium gifts).
    pub gift_storage: Vec<ItemRecord>,
    /// D-Unit (Union hacking tool) installed parts, one per unlocked slot.
    pub union_hack_slots: Vec<UnionHackSlotRow>,
}

impl Default for CharacterSummary {
    fn default() -> Self {
        Self {
            id: 0,
            account_id: 0,
            slot: 0,
            map_id: DEFAULT_START_MAP_ID,
            x: DEFAULT_START_X,
            y: DEFAULT_START_Y,
            z: 0.0,
            channel: 0,
            general_handler: 0,
            model: DEFAULT_TAMER_MODEL_ID,
            level: 1,
            name: String::new(),
            current_experience: 0,
            hp: 1_000,
            ds: 1_000,
            current_hp: 1_000,
            current_ds: 1_000,
            fatigue: 0,
            at: 100,
            de: 100,
            ms: 300,
            proper_ms: 300,
            current_condition: 0,
            partner_condition: 0,
            inventory_bits: 0,
            inventory_size: 30,
            warehouse_size: 21,
            account_warehouse_size: 14,
            extra_inventory_size: 200,
            inventory: InventorySnapshot {
                bits: 0,
                size: 30,
                items: Vec::new(),
            },
            warehouse: InventorySnapshot {
                bits: 0,
                size: 21,
                items: Vec::new(),
            },
            account_warehouse: Some(InventorySnapshot {
                bits: 0,
                size: 14,
                items: Vec::new(),
            }),
            extra_inventory: InventorySnapshot {
                bits: 0,
                size: 200,
                items: Vec::new(),
            },
            digimon_slots: 7,
            current_title: 0,
            map_region: vec![0; 255],
            archive_slots: 7,
            deck_buff_id: 0,
            equipment: vec![0; 16 * 69],
            digivice: DigiviceSnapshot::default(),
            shop_name: String::new(),
            size: 12_000,
            active_buffs: Vec::new(),
            seal_list: SealListSnapshot::default(),
            daily_reward: DailyRewardStatus::default(),
            attendance: AttendanceStatus::default(),
            server_experience: 1000,
            premium: 0,
            silk: 0,
            membership_seconds: 0,
            available_channels: vec![ChannelAvailability {
                channel: 0,
                load: 1,
            }],
            friends: Vec::new(),
            foes: Vec::new(),
            friended_character_ids: Vec::new(),
            guild: None,
            xai: None,
            current_xgauge: 0,
            current_xcrystals: 0,
            partner_x: DEFAULT_START_X,
            partner_y: DEFAULT_START_Y,
            partner_z: 0.0,
            partner_current_slot: 1,
            partner_current_type: 31_001,
            partner_slots: vec![PartnerSlotSnapshot::default()],
            partner_active_buffs: Vec::new(),
            partner_active_debuffs: Vec::new(),
            partner_model: DEFAULT_PARTNER_MODEL_ID,
            partner_level: 1,
            partner_name: String::new(),
            partner_handler: 0,
            partner_size: 12_000,
            partner_hatch_grade: 3,
            partner_current_experience: 0,
            partner_transcendence_experience: 0,
            partner_hp: 1_000,
            partner_ds: 1_000,
            partner_de: 100,
            partner_at: 100,
            partner_current_hp: 1_000,
            partner_current_ds: 1_000,
            partner_fs: 100,
            partner_ev: 0,
            partner_cc: 0,
            partner_ms: 250,
            partner_as: 1_000,
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

            quest_progress: QuestProgressSnapshot::default(),
            encyclopedia: EncyclopediaSnapshot::default(),
            friend_list: Vec::new(),
            cash_shop_history: Vec::new(),
            digimon_archive: Vec::new(),
            hatch_state: HatchState::default(),
            damage_skin_id: 0,
            partner_memory_skills: [0; 4],
            active_deck_buff: 0,
            owned_titles: Vec::new(),
            jump_booster_count: 0,
            season_pass: SeasonPassState::default(),
            tamer_shop_listings: Vec::new(),
            npc_repurchase_log: Vec::new(),
            reward_storage: Vec::new(),
            gift_storage: Vec::new(),
            union_hack_slots: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameServerTarget {
    pub address: String,
    pub port: u16,
}

// ---------------------------------------------------------------------------
// Quest progress (per character)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InProgressQuest {
    pub quest_id: i16,
    /// Goals 0..4. Each value is the cumulative progress count for that goal index.
    pub goals: [i16; 5],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct QuestProgressSnapshot {
    pub in_progress: Vec<InProgressQuest>,
    /// Bitmap of completed quests (one bit per quest id, packed into i32 array).
    pub completed_data: Vec<i32>,
    /// Bitmap of daily quests already cleared today.
    pub daily_quest_data: Vec<i32>,
    /// Last day-of-year that the daily set was reset.
    pub daily_reset_day: i32,
}

impl Default for QuestProgressSnapshot {
    fn default() -> Self {
        Self {
            in_progress: Vec::new(),
            completed_data: vec![0; 288],
            daily_quest_data: vec![0; 32],
            daily_reset_day: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// Encyclopedia (per character)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EncyclopediaEvolutionEntry {
    pub digimon_base_type: i32,
    pub slot_level: u8,
    pub unlocked: bool,
}

impl Default for EncyclopediaEvolutionEntry {
    fn default() -> Self {
        Self {
            digimon_base_type: 0,
            slot_level: 1,
            unlocked: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EncyclopediaEntry {
    pub digimon_evolution_id: i64,
    pub level: u8,
    pub enchant_at: i16,
    pub enchant_bl: i16,
    pub enchant_ct: i16,
    pub enchant_ev: i16,
    pub enchant_hp: i16,
    pub size: i16,
    pub reward_allowed: bool,
    pub reward_received: bool,
    pub deck_buff: bool,
    pub evolutions: Vec<EncyclopediaEvolutionEntry>,
}

impl Default for EncyclopediaEntry {
    fn default() -> Self {
        Self {
            digimon_evolution_id: 0,
            level: 1,
            enchant_at: 0,
            enchant_bl: 0,
            enchant_ct: 0,
            enchant_ev: 0,
            enchant_hp: 0,
            size: 12_000,
            reward_allowed: false,
            reward_received: false,
            deck_buff: false,
            evolutions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EncyclopediaSnapshot {
    pub entries: Vec<EncyclopediaEntry>,
}

// ---------------------------------------------------------------------------
// Friend list (per character)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FriendListEntry {
    pub character_id: CharacterId,
    pub name: String,
    pub annotation: String,
    pub favorite: bool,
}

// ---------------------------------------------------------------------------
// Cash shop history (per character/account)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CashShopHistoryEntry {
    pub order_id: u32,
    pub product_id: i32,
    pub amount: i16,
    pub price: i32,
    pub purchased_at_unix: u64,
}

// ---------------------------------------------------------------------------
// Cash shop catalog (server-wide)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CashShopProduct {
    pub product_id: i32,
    pub item_id: i32,
    pub amount: i16,
    pub price_premium: i32,
    pub price_silk: i32,
}

impl Default for CashShopProduct {
    fn default() -> Self {
        Self {
            product_id: 0,
            item_id: 0,
            amount: 1,
            price_premium: 0,
            price_silk: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// DigiSummon catalog (server-wide random-box data)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiSummonTicket {
    pub item_id: i32,
    pub cost: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiSummonReward {
    pub item_list_id: i32,
    pub item_id: i32,
    pub grade: i32,
    pub amount: i32,
    pub weight: i32,
    pub group: i32,
    pub group_code: i32,
}

impl Default for DigiSummonReward {
    fn default() -> Self {
        Self {
            item_list_id: 0,
            item_id: 0,
            grade: 0,
            amount: 1,
            weight: 0,
            group: 0,
            group_code: 0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiSummonProduct {
    pub product_id: i32,
    pub string_id: i32,
    pub draw_count: i32,
    pub rank: i32,
    pub remaining_daily_limit: i32,
    pub icon: String,
    pub name: String,
    pub description: String,
    pub tickets: Vec<DigiSummonTicket>,
    pub rewards: Vec<DigiSummonReward>,
}

/// One reward in the random box pool: an item stack and its relative selection weight.
///
/// The box rolls a single reward by weight over the pool. `weight` is the
/// relative chance of this entry; an entry with zero weight is never picked
/// while a positive-weight entry remains.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RandomBoxReward {
    /// Item id granted when this entry is rolled.
    pub item_id: i32,
    /// Stack amount granted.
    pub amount: u16,
    /// Relative weight for selecting this entry during a roll.
    pub weight: u32,
}

impl Default for RandomBoxReward {
    fn default() -> Self {
        Self {
            item_id: 0,
            amount: 1,
            weight: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// D-Unit (Union hacking tool) — per character
// ---------------------------------------------------------------------------

/// One row in the modern D-Unit hacking grid: the part installed in a given
/// slot, the grade rolled on it, and whether the slot is locked against
/// further edits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct UnionHackSlotRow {
    /// Catalog part id installed in the slot (mirrors `CSTable_Union`).
    pub part_id: i32,
    /// Grade/level rolled on the installed part; contributes to the total
    /// rating shown on the hacking tool header.
    pub grade: i16,
    /// Whether the slot is currently locked.
    pub locked: bool,
}

// ---------------------------------------------------------------------------
// Digi/Union Combine catalog (server-wide gacha-combine data)
// ---------------------------------------------------------------------------

/// A material item reference the client submits when combining and the server echoes back.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CombineItemRef {
    /// Unique inventory id of the submitted material.
    pub item_uid: u32,
    /// Item type/class of the material.
    pub item_type: u16,
    /// Stack count contributed by this material node.
    pub count: u16,
}

/// One entry of the combine ceiling map: a tier key and its two progress values.
///
/// The widths are fixed (`u8` key, `u8`, `u2`); the precise meaning of the two
/// values is not yet decoded and is left to the reward/ceiling logic.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CombineCeilingEntry {
    /// Ceiling tier key.
    pub tier: u8,
    /// First ceiling value (one byte on the wire).
    pub value_a: u8,
    /// Second ceiling value (two bytes on the wire).
    pub value_b: u16,
}

/// A reward item granted by a combine roll (leading decoded fields of the reward node).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiCombineReward {
    /// Item id granted.
    pub item_id: i32,
    /// Stack amount granted.
    pub amount: u16,
    /// Reward grade/tier.
    pub grade: u8,
}

impl Default for DigiCombineReward {
    fn default() -> Self {
        Self {
            item_id: 0,
            amount: 1,
            grade: 0,
        }
    }
}

/// One rank-tier row: the reward pool and selection weight for a ceiling tier.
///
/// Pack-table columns beyond the fields needed to drive a roll are deferred.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiCombineRank {
    /// Ceiling tier this rank belongs to; matches the leading combine byte and the ceiling map key.
    pub ceiling_type: u8,
    /// Relative weight for selecting this rank during a roll.
    pub weight: u32,
    /// Reward pool granted when this rank is rolled.
    pub rewards: Vec<DigiCombineReward>,
}

/// One material-item row: an item allowed as a combine material and the group it belongs to.
///
/// Pack-table columns beyond the membership fields needed to validate a roll are deferred.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiCombineItem {
    /// Item id accepted as a combine material.
    pub item_id: i32,
    /// Group this material belongs to.
    pub group_id: i32,
}

/// One item-group row: a named group of materials the combine roll selects from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiCombineGroup {
    /// Group identifier referenced by material rows.
    pub group_id: i32,
    /// Item ids that make up this group.
    pub members: Vec<i32>,
}

/// One ceiling-group row: the ceiling-map entries emitted for a tier.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiCombineCeil {
    /// Ceiling tier this row configures.
    pub ceiling_type: u8,
    /// Ceiling-map entries emitted for this tier.
    pub entries: Vec<CombineCeilingEntry>,
}

/// The Digi Combine catalog: rank, material, group, and ceiling rows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigiCombineCatalog {
    /// Rank-tier rows and their reward pools.
    pub rank_rows: Vec<DigiCombineRank>,
    /// Allowed combine materials.
    pub item_list: Vec<DigiCombineItem>,
    /// Material groupings.
    pub item_groups: Vec<DigiCombineGroup>,
    /// Ceiling-group configuration.
    pub ceil_groups: Vec<DigiCombineCeil>,
}

/// The Union Combine catalog.
///
/// Union shares byte-identical node layouts with Digi Combine, so it reuses the
/// same catalog and sub-types rather than duplicating them.
pub type UnionCombineCatalog = DigiCombineCatalog;

// ---------------------------------------------------------------------------
// Extra evolution / digital fusion catalog
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtraEvolutionMaterial {
    pub material_id: i32,
    pub amount: i32,
}

impl Default for ExtraEvolutionMaterial {
    fn default() -> Self {
        Self {
            material_id: 0,
            amount: 1,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtraEvolutionRecipe {
    pub exchange_type: u16,
    pub object_id: i32,
    pub material_type: u16,
    pub need_material_value: i32,
    pub price: i64,
    pub way_type: u16,
    pub main_materials: Vec<ExtraEvolutionMaterial>,
    pub sub_materials: Vec<ExtraEvolutionMaterial>,
}

impl Default for ExtraEvolutionRecipe {
    fn default() -> Self {
        Self {
            exchange_type: 0,
            object_id: 0,
            material_type: 0,
            need_material_value: 0,
            price: 0,
            way_type: 1,
            main_materials: Vec::new(),
            sub_materials: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtraEvolutionNpc {
    pub npc_id: i32,
    pub recipes: Vec<ExtraEvolutionRecipe>,
}

// ---------------------------------------------------------------------------
// Tamer/personal shop state (consigned shop = warehouse-shop)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConsignedShopListing {
    pub listing_id: u32,
    pub seller_id: CharacterId,
    pub seller_name: String,
    pub item_id: i32,
    pub amount: i16,
    pub price_per_unit: i64,
    pub created_at_unix: u64,
}

// ---------------------------------------------------------------------------
// Trade session
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TradeSlotEntry {
    pub trade_slot: u8,
    pub item_record: ItemRecord,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TradeSideState {
    pub character_id: CharacterId,
    pub items: Vec<TradeSlotEntry>,
    pub money: i64,
    pub locked: bool,
    pub final_confirmed: bool,
}

// ---------------------------------------------------------------------------
// Hatch (incubator) state (per character)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct HatchState {
    /// Whether an egg is currently in the incubator.
    pub egg_inserted: bool,
    /// Item id of the inserted egg.
    pub egg_item_id: i32,
    /// 0..3 = increase percentage step.
    pub increase_level: i8,
    /// True when the egg is on the backup slot.
    pub backup_active: bool,
}

// ---------------------------------------------------------------------------
// Digimon archive (storage of dormant partners)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DigimonArchiveEntry {
    pub archive_slot: u8,
    pub partner: PartnerSlotSnapshot,
}

// ---------------------------------------------------------------------------
// Buff state (active buffs/debuffs)
// ---------------------------------------------------------------------------
//
// `ActiveBuffSnapshot` already exists. Below is the helper for buff updates that
// the application layer uses to add/remove/extend buffs.

// ---------------------------------------------------------------------------
// Membership / season pass / time-charge state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeasonPassState {
    pub current_level: i32,
    pub current_experience: i32,
    pub purchased_premium: bool,
    pub claimed_mission_ids: Vec<i32>,
    pub claimed_season_levels: Vec<i32>,
}

impl Default for SeasonPassState {
    fn default() -> Self {
        Self {
            current_level: 1,
            current_experience: 0,
            purchased_premium: false,
            claimed_mission_ids: Vec::new(),
            claimed_season_levels: Vec::new(),
        }
    }
}
