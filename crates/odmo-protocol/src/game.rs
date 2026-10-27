use odmo_types::{
    ActiveBuffSnapshot, AttendanceStatus, ChannelAvailability, CharacterSummary,
    CombineCeilingEntry, CombineItemRef, DailyRewardStatus, DigiCombineReward, DigiSummonProduct,
    DigiSummonReward, DropSummary, GuildHistoricEntry, GuildSnapshot, InventorySnapshot,
    ItemRecord, MobSummary, RelationEntry, SealListSnapshot, XaiSnapshot,
};

use std::collections::BTreeMap;

use crate::{
    error::ProtocolError,
    opcode::game,
    reader::{PacketReader, RawPacket},
    writer::PacketWriter,
};

/// Client → server game requests, one variant per opcode.
///
/// Wire notation used by the per-variant comments: `text` is
/// `[u2 byte-length][UTF-8 bytes]` with no terminator, bounded at 512 bytes;
/// `wstring` is the older `[u1 byte-length][bytes][0x00]` form.
#[derive(Debug, Clone, PartialEq)]
pub enum GameRequest {
    Connection {
        kind: u8,
    },
    KeepConnection,
    InitialInformation {
        account_id: u64,
        access_code: u32,
    },
    ComplementarInformation,
    TamerMovimentation {
        ticks: u32,
        handler: u32,
        x: i32,
        y: i32,
        z: f32,
    },
    WarpGate {
        portal_id: i32,
    },
    ConsumeItem {
        target_handler: i32,
        slot: u16,
    },
    MoveItem {
        origin_slot: u16,
        destination_slot: u16,
    },
    SplitItem {
        origin_slot: u16,
        destination_slot: u16,
        amount: u16,
    },
    RemoveItem {
        slot: u16,
        x: i32,
        y: i32,
        amount: u16,
    },
    NpcPurchase {
        vip: u8,
        npc_id: i32,
        marker: u8,
        shop_slot: i32,
        purchase_count: u16,
    },
    NpcSell {
        vip: u8,
        npc_id: i32,
        marker: u8,
        item_slot: u8,
        sell_amount: u16,
    },
    LootItem {
        drop_handler: u32,
    },
    DigiSummonSyncRequest,
    ChannelInfo,
    Membership,
    Emoticon {
        emoticon_type: i32,
        value: i32,
    },
    FriendlyInfo {
        target_handler: u32,
    },
    ExtraInventoryMove {
        category: u16,
        extra_slot: u16,
        inventory_slot: u16,
    },
    ExtraInventoryBatchMove {
        category: u8,
    },
    ExtraInventorySort {
        category: u8,
    },
    ExtraInventoryUse {
        category: u8,
        extra_slot: u16,
    },
    /// `pGame::Chat` C→S — opcode 1008. `[text message]`.
    ChatMessage {
        message: String,
    },
    /// `pGame::Whisper` C→S — opcode 1009. `[text target_name][text message]`.
    WhisperMessage {
        target_name: String,
        message: String,
    },
    /// Shout delivery (sync record 11) is proven by the client reader; the simple
    /// shout request has no proven opcode in the modern build (op 1056 at
    /// 0x00DA6F50 reads one dword and only opens a dialog), so nothing on the wire
    /// constructs this variant today.
    ShoutMessage {
        message: String,
    },
    /// `pGame::MsgAllByItem` C→S — opcode 1074.
    /// `[u1 mode][u4 item_code][text message][u2 item_slot]`.
    /// Mode 0 burns a megaphone from `item_slot`; an entitlement shout sends mode 1
    /// with a fixed item code and `0xfffe` in the slot because it owns no slot.
    MegaphoneMessage {
        mode: u8,
        item_code: u32,
        message: String,
        item_slot: u16,
    },
    TamerReaction {
        reaction_type: i32,
    },
    PartnerStop {
        uid: u32,
    },
    PartnerEvolution {
        digimon_handler: u32,
        evolution_slot: u8,
    },
    PartnerAttack {
        attacker_handler: u32,
        target_handler: u32,
    },
    PartnerSkill {
        skill_slot: u8,
        attacker_handler: u32,
        target_handler: u32,
    },
    PartnerSwitch {
        slot: u8,
    },
    PartnerDelete {
        slot: u8,
        validation: String,
    },
    EvolutionUnlock {
        evo_slot: i32,
        items: Vec<EvolutionUnlockItem>,
        selected_inven_idx: i16,
    },
    RideModeStart,
    RideModeStop,
    OpenRideMode {
        evo_unit_idx: u32,
        item_type: i32,
    },
    /// `pGame::SetTarget` C→S — opcode 1016. `[u4 attacker_handler][u4 target_handler]`.
    SetTarget {
        attacker_handler: u32,
        target_handler: u32,
    },
    /// `pGame::StatUp` C→S — opcode 1030. `[u4 uid][u1 stat]`.
    StatUp {
        uid: u32,
        stat: u8,
    },
    /// `pGame::RefreshScreen` C→S — opcode 1046. Empty payload.
    RefreshScreen,
    /// `pGame::AwayTime` C→S — opcode 1069. Empty payload.
    AwayTime,
    /// `pDigimon::ChangeName` C→S — opcode 3206. `[i4 inven_slot][text new_name]`.
    DigimonChangeName {
        inven_slot: i32,
        new_name: String,
    },
    /// `pGame::HatchIn` C→S — opcode 1036. `[u1 vip][u4 inven_slot][i4 npc_idx]`.
    ///
    /// The leading byte is always emitted; the builder derives it from the
    /// hatch dialog state (comparison against 2), so it is a 0/1 flag.
    HatchInsertEgg {
        vip: u8,
        inven_slot: u32,
        npc_idx: i32,
    },
    /// `pGame::HatchUp` C→S — opcode 1037. `[u1 vip][i4 npc_idx][u1 data_level]`.
    HatchIncrease {
        vip: u8,
        npc_idx: i32,
        data_level: i8,
    },
    /// `pGame::HatchOut` C→S — opcode 1038. `[u1 flag][u4 portable_pos][text name][i4 npc_idx]`.
    ///
    /// The builder writes a constant 1 into `flag` instead of a VIP state, so
    /// the byte is carried through without a resolved meaning.
    HatchFinish {
        flag: u8,
        portable_pos: u32,
        name: String,
        npc_idx: i32,
    },
    /// `pGame::HatchCancel` C→S — opcode 1039. `[u1 vip][i4 npc_idx]`.
    HatchRemoveEgg {
        vip: u8,
        npc_idx: i32,
    },
    /// `pItem::HatchBackupDiskAdd` C→S — opcode 3946. `[u1 vip][u4 inven_slot][i4 npc_idx]`.
    HatchBackupInsert {
        vip: u8,
        inven_slot: u32,
        npc_idx: i32,
    },
    /// `pItem::HatchBackupDiskCancel` C→S — opcode 3947. `[u1 vip][i4 npc_idx]`.
    HatchBackupCancel {
        vip: u8,
        npc_idx: i32,
    },
    IncubatorClose,
    DigimonArchiveMove {
        vip: u8,
        slot1: i32,
        slot2: i32,
        npc_type: u32,
    },
    DigimonArchiveList {
        vip: u8,
        inven_idx: u32,
        npc_type: u32,
    },
    DigimonArchiveSwap {
        npc_idx: u32,
        archive_type: i32,
        src_arr: u8,
        dst_arr: u8,
    },
    InventorySort {
        sort_type: u8,
    },
    ItemIdentify {
        item_slot: i16,
    },
    ItemCraft {
        recipe_slot: i16,
    },
    ItemReroll {
        item_slot: i16,
    },
    /// `pItem::SocketIn` C→S — opcode 3926.
    /// Binary-verified wire (sender 0xF24F0):
    /// `[u1 vip][u4 inven_portable_pos][u4 npc_idx][u2 src][u2 dst][u1 socket_order]`.
    ItemSocketIn {
        vip: u8,
        inven_portable_pos: u32,
        npc_idx: i32,
        src_inven_pos: u16,
        dst_inven_pos: u16,
        socket_order: u8,
    },
    /// `pItem::SocketOut` C→S — opcode 3927. Same shape as SocketIn (binary-verified, sender 0xF2630).
    ItemSocketOut {
        vip: u8,
        inven_portable_pos: u32,
        npc_idx: i32,
        src_inven_pos: u16,
        dst_inven_pos: u16,
        socket_order: u8,
    },
    /// `pItem::SocketClear` C→S — opcode 3928. `[i32 npc_idx][u2 inven_pos][u1 socket_order]`.
    /// `pItem::Analysis` C→S — opcode 3929. Binary-verified wire (sender 0xF2770):
    /// `[u1 vip][u4 npc_idx][u4 inven_portable_pos][u2 inven_pos]`.
    ItemSocketIdentify {
        vip: u8,
        npc_idx: i32,
        inven_portable_pos: u32,
        inven_pos: u16,
    },
    ItemReturn {
        item_slot: i16,
    },
    ItemScan {
        item_slot: i16,
    },
    LoadGiftStorage,
    GiftStorageRetrieve {
        item_slot: i16,
    },
    LoadRewardStorage,
    RecompenseGain {
        reward_id: i32,
    },
    TamerShopOpen,
    TamerShopClose,
    TamerShopBuy {
        item_id: i32,
        amount: i16,
    },
    ConsignedShopOpen,
    ConsignedShopView {
        shop_id: i32,
    },
    ConsignedShopPurchase {
        item_id: i32,
        amount: i16,
    },
    ConsignedShopRetrieve {
        item_slot: i16,
    },
    CashShopBuy {
        amount: u8,
        total_price: i32,
        order_id: u64,
        product_ids: Vec<i32>,
    },
    CashShopReload,
    QuestAvailableList {
        npc_id: i32,
    },
    QuestAccept {
        quest_id: i16,
    },
    QuestDeliver {
        quest_id: i16,
    },
    QuestGiveUp {
        quest_id: i16,
    },
    QuestUpdate {
        quest_id: i16,
        cond_index: u8,
        value: u8,
    },
    DieConfirm,
    RemoveBuff {
        buff_id: i32,
    },
    DamageSkinChange {
        skin_id: i32,
    },
    /// `Seal::Open` C→S — opcode 3971. `[u2 seal_idx]`.
    SealOpen {
        seal_idx: i16,
    },
    /// `Seal::Close` C→S — opcode 3972. `[u2 seal_idx][u4 value]`.
    SealClose {
        seal_idx: i16,
        value: u32,
    },
    SealSetLeader {
        card_code: u16,
    },
    SealRemoveLeader,
    SealSetFavorite {
        card_code: u16,
        bookmark: u8,
    },
    EncyclopediaLoad,
    EncyclopediaGetReward {
        digimon_id: u32,
    },
    EncyclopediaDeckBuff {
        deck_idx: u16,
    },
    OtherTamerDetailInfo {
        target_handler: u32,
    },
    ArenaDailyPoints {
        item_slot: i16,
        points: i16,
        item_id: i16,
    },
    ArenaDailyRanking,
    ArenaRequestRank {
        ranking_type: u8,
    },
    ArenaRequestOldRank {
        ranking_type: u8,
    },
    ArenaRequestFame {
        season: u16,
    },
    ArenaRequestFameList,
    DungeonNextStage,
    DungeonSurrender,
    BurningEvent,
    DailyCheckEvent,
    DailyCheckEventRequest {
        event_no: i32,
    },
    JoinEventQueue {
        event_id: i32,
    },
    RegionUnlock {
        region_idx: i16,
    },
    SetTitle {
        title_id: i16,
    },
    ChangeTamerModel {
        model_id: i32,
        inven_slot: i32,
    },
    /// `pTamer::ChangeName` C→S — opcode 1311. `[i4 inven_slot][text new_name]`.
    /// The slot is the rename item's inventory position; the client always sends it.
    TamerNameChange {
        inven_slot: i32,
        new_name: String,
    },
    RareMachineOpen {
        npc_idx: u32,
    },
    RareMachineRun {
        npc_idx: u32,
        inven_idx: u32,
        reset_count: u32,
    },
    PartyInvite {
        target_name: String,
    },
    PartyInviteResponse {
        result_type: i32,
        inviter_name: String,
    },
    /// `pParty::Chat` C→S — opcode 2304. `[text message]`.
    PartyChat {
        message: String,
    },
    PartyKick {
        target_name: String,
    },
    PartyLeave,
    PartyChangeMaster {
        new_leader_slot: i32,
    },
    PartyChangeLoot {
        loot_type: i32,
        rare_type: u8,
        disp_rare_grade: u8,
    },
    PartyDismiss,
    /// `GuildCreate` C→S — opcode 2101. `[text guild_name][u2 inven_slot][u4 npc_id]`.
    GuildCreate {
        guild_name: String,
        inven_slot: u16,
        npc_id: u32,
    },
    GuildDelete,
    /// `GuildReqAdd` C→S — opcode 2109. `[text target_name]`.
    GuildInvite {
        target_name: String,
    },
    /// `GuildAllow` C→S — opcode 2103. `[u4 certified_code][text tamer_name]`.
    GuildInviteAccept {
        certified_code: u32,
        target_name: String,
    },
    /// `GuildReject` C→S — opcode 2105. `[u4 certified_code][text tamer_name]`.
    GuildInviteDeny {
        certified_code: u32,
        target_name: String,
    },
    /// `GuildDelete` C→S — opcode 2106 (KICK). `[text tamer_name]`.
    GuildKick {
        target_name: String,
    },
    GuildLeave,
    /// `pGuild::Chat` C→S — opcode 2114. `[text message]`.
    GuildMessage {
        message: String,
    },
    /// `pGuild::ChangeNotice` C→S — opcode 2126. `[text notice]`.
    GuildNotice {
        notice: String,
    },
    GuildHistory,
    GuildSetTitle {
        authority_class: i32,
        title: String,
        duty: String,
    },
    TradeRequest {
        target_handler: u32,
    },
    TradeAccept {
        accepter_handler: u32,
    },
    TradeCancel,
    /// `TradeAddItem` C→S — opcode 1508. Modern client wire: `[u2 inven_pos][u2 amount]`.
    TradeAddItem {
        inven_pos: u16,
        amount: u16,
    },
    /// `TradeCancelItem` C→S — opcode 1531. Modern client wire: `[i1 trade_slot]`.
    TradeRemoveItem {
        trade_slot: i8,
    },
    /// `TradeAddMoney` C→S — opcode 1509. Modern client wire: `[u4 money]`.
    TradeAddMoney {
        amount: u32,
    },
    TradeConfirm,
    TradeLock,
    TradeUnlock,
    SeasonPassDetails,
    SeasonPassPurchaseExp {
        purchase_count: i32,
    },
    SeasonPassMissionReward {
        mission_id: i32,
    },
    SeasonPassSeasonReward {
        level: i32,
    },
    /// `pGame::ChangeChannel` C→S — opcode 1050. `[u4 channel]`.
    ChangeChannel {
        channel: u32,
    },
    ChannelSwitchConfirm,
    TamerShopList,
    ConsignedWarehouse,
    ConsignedWarehouseRetrieve {
        item_slot: i16,
    },
    CashShopBuyHistory,
    /// `pBuddy::Add` C→S — opcode 2401. `[text friend_name]`.
    AddFriend {
        friend_name: String,
    },
    /// `pBuddy::Delete` C→S — opcode 2402. `[text friend_name]`.
    DeleteFriend {
        friend_name: String,
    },
    /// `pBuddy::Memo` C→S — opcode 2405. `[text tamer_name][text memo]`.
    FriendMemo {
        tamer_name: String,
        memo: String,
    },
    /// `pGuild::ReqAddFailure` C→S — opcode 2110. `[text inviter_name]`: the
    /// sender could not take the pending invite, so the notice goes back to the
    /// tamer who sent it.
    GuildInviteFailure {
        inviter_name: String,
    },
    FriendList,
    /// `GuildToMaster` C→S — opcode 2119. `[text tamer_name]`.
    GuildAuthorityMaster {
        target_name: String,
    },
    /// `GuildToSubMaster` C→S — opcode 2118. `[text tamer_name]`.
    GuildAuthoritySubMaster {
        target_name: String,
    },
    /// `GuildToMember` C→S — opcode 2116. `[text tamer_name]`.
    GuildAuthorityMember {
        target_name: String,
    },
    /// `GuildToSubMember` C→S — opcode 2115. `[text tamer_name]`.
    GuildAuthorityNewMember {
        target_name: String,
    },
    /// `GuildToDatsMember` C→S — opcode 2117. `[text tamer_name]`.
    GuildAuthorityDats {
        target_name: String,
    },
    /// `pDigimon::SpiritToDigimon` C→S — opcode 3239.
    /// `[i32 model_id][wstring name][i32 npc_id]`.
    SpiritToDigimon {
        model_id: i32,
        name: String,
        npc_id: i32,
    },
    DigiSummonPurchase {
        product_id: i32,
        ticket_slot: i32,
    },
    /// Digivice combine: open/sync the gacha window (bare body).
    DigiCombineSyncRequest,
    /// Digivice combine: submit the selected materials for a roll.
    DigiCombine {
        ceiling_type: u8,
        materials: Vec<CombineItemRef>,
    },
    /// Digivice combine: claim the reward for a resolved ceiling tier.
    DigiCombineRewardClaim {
        ceiling_type: u8,
    },
    /// Union combine: open/sync the gacha window (bare body).
    UnionCombineSyncRequest,
    /// Union combine: submit the selected materials for a roll.
    UnionCombine {
        ceiling_type: u8,
        materials: Vec<CombineItemRef>,
    },
    /// Union combine: claim the reward for a resolved ceiling tier.
    UnionCombineRewardClaim {
        ceiling_type: u8,
    },
    /// D-Unit: open the hacking grid window — returns the unlocked slot count
    /// and the equipped parts per slot (opcode 4311).
    UnionHackOpenRequest,
    /// D-Unit: replace the part installed in a given slot (opcode 4312).
    UnionHackModify {
        slot: u8,
        part_id: i32,
        grade: i16,
    },
    /// Random box: open/sync the box window (5-byte body).
    RandomBoxList {
        flag: u8,
        index: i32,
    },
    /// Random box: purchase a box entry (15-byte body).
    RandomBoxPurchase {
        flag: u8,
        product_id: i32,
        item_uid: i32,
        count: u16,
        state: i32,
    },
    LoadAccountWarehouse,
    RetrieveAccountWarehouse {
        item_slot: i16,
    },
    PartyMemberDisconnect,
    MonsterRespawnTimer,
    JumpBooster,
    SkillLevelUp {
        uid: u32,
        evo_unit_idx: u8,
        skill_idx: u8,
    },
    TamerChargeXCrystal,
    TamerConsumeXCrystal {
        amount: i32,
    },
    /// `pSkill::Summon` C→S — opcode 1114, discriminated by its leading case byte.
    TamerSummon(TamerSummonRequest),
    TamerSkillRequest {
        skill_idx: u32,
        target_uid: u32,
    },
    TranscendenceReceiveExp,
    TranscendenceSuccess,
    TimeChargeResult {
        charge_type: u8,
    },
    WarpGateDungeon,
    /// `pDigimon::DigimonToSpirit` C→S — opcode 3240.
    /// `[u8 slot][string validation][i32 npc_id]`. The validation string carries
    /// the account secondary secret and is checked before any mutation.
    DigimonToSpirit {
        slot: u8,
        validation: String,
        npc_id: i32,
    },
}

/// Bodies the client frames under `pSkill::Summon`; the leading case byte chooses one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TamerSummonRequest {
    /// `[u1 1][text target_name][u4 skill_index]` — summon through a named tamer skill.
    ByName {
        target_name: String,
        skill_index: u32,
    },
    /// `[u1 4][u1 1][u1]` or `[u1 4][u1 2][u4]` — the summon UI reports a state value.
    StateCheck {
        submode: u8,
        value: u32,
    },
    /// `[u1 case][u1 value]` — a caller-selected case with no payload beyond the value.
    Accept {
        case: u8,
        value: u8,
    },
}

/// One record of the sync stream. Chat text has no opcode of its own: the client's router
/// loops `[u1 type]` records inside opcode 1006 and dispatches 7 chat, 8 whisper and 11
/// shout into the chat presenter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncChatLine {
    pub handler: u32,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncWhisperLine {
    pub result: u8,
    pub sender: String,
    pub receiver: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncNamedLine {
    pub sender: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncChatRecord {
    /// `[u1 7][u1 count][count x {u4 handler, text message}]`
    Chat(Vec<SyncChatLine>),
    /// `[u1 8][u1 count][count x {u1 result, text sender, text receiver, text message}]`
    Whisper(Vec<SyncWhisperLine>),
    /// `[u1 11][u1 count][count x {text sender, text message}]`
    Shout(Vec<SyncNamedLine>),
}

impl SyncChatRecord {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::CHAT_MESSAGE_RESPONSE);
        match self {
            SyncChatRecord::Chat(lines) => {
                writer.write_u8(7);
                writer.write_u8(clamp_u8_len(lines.len()));
                for line in lines {
                    writer.write_u32(line.handler);
                    writer.write_modern_utf8_string(&line.message);
                }
            }
            SyncChatRecord::Whisper(lines) => {
                writer.write_u8(8);
                writer.write_u8(clamp_u8_len(lines.len()));
                for line in lines {
                    writer.write_u8(line.result);
                    writer.write_modern_utf8_string(&line.sender);
                    writer.write_modern_utf8_string(&line.receiver);
                    writer.write_modern_utf8_string(&line.message);
                }
            }
            SyncChatRecord::Shout(lines) => {
                writer.write_u8(11);
                writer.write_u8(clamp_u8_len(lines.len()));
                for line in lines {
                    writer.write_modern_utf8_string(&line.sender);
                    writer.write_modern_utf8_string(&line.message);
                }
            }
        }
        // A sync frame lists records until a zero type ends it, so the reader never
        // runs past the last record looking for one.
        writer.write_u8(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolutionUnlockItem {
    pub item_id: u32,
    pub item_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameConnectionPacket {
    pub handshake: i16,
}

impl GameConnectionPacket {
    /// The proactive handshake sent on TCP connect.
    /// Uses opcode -1 (65535 as u16).
    /// Proactive handshake sent on TCP connect (opcode -1 / 65535).
    /// Wire: [FF,FF] [HS] [0,0] [0,0] [0,0] [0,0] [0,0] [CK] = 18 bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(-1);
        writer.write_i16(self.handshake);
        writer.write_i16(0);
        writer.write_i16(0);
        writer.write_i16(0);
        writer.write_i16(0);
        writer.write_i16(0);
        writer.finalize() // adds 1 zero→checksum, total = 18 bytes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GameInitialInfoPacket {
    pub character: CharacterSummary,
}

impl GameInitialInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::INITIAL_INFO_RESPONSE);
        writer.write_i32(1);
        writer.write_i32(self.character.x);
        writer.write_i32(self.character.y);
        writer.write_i32(non_zero_handler(
            self.character.general_handler,
            self.character.id,
        ));
        writer.write_u8(1);
        writer.write_i32(self.character.model);
        writer.write_string(&self.character.name);
        writer.write_i64(self.character.current_experience.saturating_mul(100));
        writer.write_i16(self.character.level as i16);
        writer.write_i32(self.character.hp);
        writer.write_i32(self.character.ds);
        writer.write_i32(self.character.current_hp);
        writer.write_i32(self.character.current_ds);
        writer.write_i32(self.character.fatigue);
        writer.write_i32(self.character.at);
        writer.write_i32(self.character.de);
        writer.write_i32(self.character.ms);
        write_empty_item_records(&mut writer, 16);
        write_empty_item_records(&mut writer, 12);
        write_empty_item_records(&mut writer, 1);
        write_empty_item_records(&mut writer, 5);
        writer.write_zeroes(1_292);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.write_i32(-1);
        writer.write_i32(0);
        writer.write_i32(-1);
        writer.write_i64(self.character.inventory_bits);
        writer.write_u16(self.character.inventory_size);
        writer.write_u16(self.character.warehouse_size);
        writer.write_i16(0);
        writer.write_u8(self.character.digimon_slots);
        writer.write_i32(non_zero_handler(
            self.character.partner_handler,
            self.character.id.saturating_add(10_000),
        ));
        writer.write_i32(self.character.partner_model);
        writer.write_string(&self.character.partner_name);
        writer.write_u8(self.character.partner_hatch_grade);
        writer.write_i16(self.character.partner_size);
        writer.write_i64(
            self.character
                .partner_current_experience
                .saturating_mul(100),
        );
        writer.write_i64(self.character.partner_transcendence_experience);
        writer.write_i16(self.character.partner_level as i16);
        writer.write_i32(self.character.partner_hp);
        writer.write_i32(self.character.partner_ds);
        writer.write_i32(self.character.partner_de);
        writer.write_i32(self.character.partner_at);
        writer.write_i32(self.character.partner_current_hp);
        writer.write_i32(self.character.partner_current_ds);
        writer.write_i32(self.character.partner_fs);
        writer.write_i32(0);
        writer.write_i32(self.character.partner_ev);
        writer.write_i32(self.character.partner_cc);
        writer.write_i32(self.character.partner_ms);
        writer.write_i32(self.character.partner_as);
        writer.write_i32(0);
        writer.write_i32(self.character.partner_ht);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.write_i32(self.character.partner_ar);
        writer.write_i32(self.character.partner_bl);
        writer.write_i32(self.character.partner_model);
        writer.write_u8(0);
        write_zero_i16s(&mut writer, 15);
        writer.write_u16(0);
        write_zero_i16s(&mut writer, 13);
        writer.write_i32(non_zero_handler(
            self.character.partner_handler,
            self.character.id.saturating_add(10_000),
        ));
        writer.write_u8(0);
        writer.write_u8(99);
        writer.write_i32(0);
        writer.write_i32(self.character.channel as i32);
        write_map_region(&mut writer, &self.character.map_region);
        writer.write_i32(self.character.archive_slots);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.write_u8(0);
        writer.write_u8(0);
        writer.write_u8(0);
        writer.write_u8(99);
        writer.write_i16(self.character.current_title as i16);
        for _ in 0..32 {
            writer.write_i32(0);
        }
        writer.write_i32(0);
        writer.write_i32(2);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.write_u8(0);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.write_u8(0);
        writer.write_i16(0);
        writer.write_u8(0);
        writer.write_u8(0);
        writer.write_i32(self.character.deck_buff_id);
        writer.write_u8(0);
        writer.write_i32(0);
        writer.write_zeroes(29);
        writer.write_u8(0);
        writer.write_u8(0);
        writer.write_i32(0);
        writer.write_u8(0);
        writer.write_i32(0);
        writer.write_u8(0);
        writer.write_u8(0);
        writer.finalize()
    }
}

// --- Item operation packets ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemMoveSuccessPacket {
    pub origin_slot: u16,
    pub destination_slot: u16,
}

impl ItemMoveSuccessPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MOVE_ITEM);
        writer.write_u16(self.origin_slot);
        writer.write_u16(self.destination_slot);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemMoveFailPacket {
    pub origin_slot: u16,
    pub destination_slot: u16,
}

impl ItemMoveFailPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MOVE_ITEM_FAILURE);
        writer.write_u16(self.origin_slot);
        writer.write_u16(self.destination_slot);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitItemPacket {
    pub origin_slot: u16,
    pub destination_slot: u16,
    pub amount: u16,
}

impl SplitItemPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SPLIT_ITEM);
        writer.write_u16(self.origin_slot);
        writer.write_u16(self.destination_slot);
        writer.write_u16(self.amount);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemConsumeFailPacket {
    pub slot: u16,
    pub item_id: i32,
    pub result: u8,
}

impl ItemConsumeFailPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::CONSUME_ITEM);
        writer.write_u8(0); // fail
        writer.write_u16(self.slot);
        writer.write_i32(self.item_id);
        writer.write_u8(self.result);
        writer.finalize()
    }
}

// --- NPC shop packets ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcPurchaseResultPacket {
    pub success: bool,
    pub remaining_bits: i64,
}

impl NpcPurchaseResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NPC_PURCHASE);
        writer.write_u8(if self.success { 1 } else { 0 });
        writer.write_i64(self.remaining_bits);
        writer.finalize()
    }
}

/// One sold-item row on the sell result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcSellRow {
    pub kind: u8,    // unresolved
    pub serial: u32, // unresolved
    pub stack: u16,  // unresolved
}

/// `NpcSellResult` — opcode 3916 (mirrored from the C2S sell request). Modern receive
/// parser reads the i64 money field, a u1 row count and one {u1 kind; u4 serial; u2
/// stack} row per counted item, pings event 89 with the money amount and keeps the sold
/// rows briefly for the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcSellResultPacket {
    pub remaining_bits: i64,
    pub rows: Vec<NpcSellRow>,
}

impl NpcSellResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(self.rows.len() <= u8::MAX as usize, "sell rows overflow u1");
        let mut writer = PacketWriter::new(game::NPC_SELL);
        writer.write_i64(self.remaining_bits);
        writer.write_u8(self.rows.len() as u8);
        for row in &self.rows {
            writer.write_u8(row.kind);
            writer.write_u32(row.serial);
            writer.write_u16(row.stack);
        }
        writer.finalize()
    }
}

/// `HotTimeEvent` — opcode 3135: one u4 value handed to event 454 together
/// with the op marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HotTimeEventPacket {
    pub value: u32, // unresolved
}

impl HotTimeEventPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::EVENT_HOT_TIME);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `ItemTerm` — opcode 3139: one u2 term handed to event 527 together with
/// the op marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemTermPacket {
    pub term: u16, // unresolved
}

impl ItemTermPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::EVENT_ITEM_TERM);
        writer.write_u16(self.term);
        writer.finalize()
    }
}

/// `GotchaItem` — opcode 3959 and `IncMember` — opcode 2130: a single u4
/// forwarded to event 492 / event 261 respectively.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SingleValueNoticePacket {
    pub value: u32, // unresolved
}

impl SingleValueNoticePacket {
    pub fn encode_on(&self, opcode: i16) -> Vec<u8> {
        let mut writer = PacketWriter::new(opcode);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `Option` — opcode 1313: {u4 target; u4 value}; the client resolves the
/// target handler and applies the option through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TamerOptionPacket {
    pub target: u32, // entity handle the option applies to
    pub value: u32,  // unresolved
}

impl TamerOptionPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TAMER_OPTION);
        writer.write_u32(self.target);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `ResultSkillChip` — opcode 1122: {u4 handler; u4 skill_id}; a skill-row
/// kind of 1 or 47 pops the buff feedback message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillResultChipPacket {
    pub handler: u32,  // entity carrying the chip
    pub skill_id: u32, // keyed into CSkillTable
}

impl SkillResultChipPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_RESULT_CHIP);
        writer.write_u32(self.handler);
        writer.write_u32(self.skill_id);
        writer.finalize()
    }
}

/// `CrossInvenSlotSize` — opcode 3966: {u4; u2; u4; u4} handed to event 491.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CrossInvenSlotSizePacket {
    pub first: u32,  // unresolved
    pub second: u16, // unresolved
    pub third: u32,  // unresolved
    pub fourth: u32, // unresolved
}

impl CrossInvenSlotSizePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_CROSS_INVEN_SLOT_SIZE);
        writer.write_u32(self.first);
        writer.write_u16(self.second);
        writer.write_u32(self.third);
        writer.write_u32(self.fourth);
        writer.finalize()
    }
}

/// `MoveClear` — opcode 3962: {u4 handler; u4 item_id}; the client gates on
/// the handler and pops the removal visual keyed by the item table row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemMoveClearPacket {
    pub handler: u32, // entity gate
    pub item_id: u32, // CItemTable key
}

impl ItemMoveClearPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_MOVE_CLEAR);
        writer.write_u32(self.handler);
        writer.write_u32(self.item_id);
        writer.finalize()
    }
}

/// `CashItemTradeTimeOut` — opcode 3984: {u2; u4; u4} handed to event 467.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CashTradeTimeoutPacket {
    pub trade: u16, // unresolved
    pub first: u32, // unresolved
    pub second: u32, // unresolved
}

impl CashTradeTimeoutPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_CASH_TRADE_TIMEOUT);
        writer.write_u16(self.trade);
        writer.write_u32(self.first);
        writer.write_u32(self.second);
        writer.finalize()
    }
}

/// `Make` — opcode 3983: {u1 result kind; u1 slot}; kind 0 clears a digimon
/// skill-table row, 1/3/4 fire event 586 with 1000/3000/4000 plus the slot,
/// kind 2 fires event 68 with the slot, kind 5 fires event 586 with 5000 and
/// kind 9 fires event 68 with 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ItemMakeResultPacket {
    pub kind: u8, // switch selector
    pub slot: u8, // unresolved
}

impl ItemMakeResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_MAKE_RESULT);
        writer.write_u8(self.kind);
        writer.write_u8(self.slot);
        writer.finalize()
    }
}

/// Tamer item-skill notices. `ItemSkillApply` — opcode 1330 carries {u4
/// index; u4 skill ref} applied through the client slide-list; the expire
/// (1331) and remove (1332) gates carry {u1 count; count x u4 skill id}
/// batches refreshed against CSkillTable rows.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemSkillBatchPacket {
    pub index: u32,      // apply gate only
    pub skill_ids: Vec<u32>, // one u16-keyed row per count
}

impl ItemSkillBatchPacket {
    pub fn encode_on(&self, opcode: i16) -> Vec<u8> {
        assert!(
            self.skill_ids.len() <= u8::MAX as usize,
            "skill ids overflow u1"
        );
        let mut writer = PacketWriter::new(opcode);
        if opcode == game::TAMER_ITEM_SKILL_APPLY {
            writer.write_u32(self.index);
            writer.write_u32(*self.skill_ids.first().unwrap_or(&0));
        } else {
            writer.write_u8(self.skill_ids.len() as u8);
            for skill_id in &self.skill_ids {
                writer.write_u32(*skill_id);
            }
        }
        writer.finalize()
    }
}

/// `PinochiVote` — opcode 3127: {u2 count; count x {u2 slot; u4 value}};
/// every entry decrements one 69-byte digimon skill-table row addr + 4
/// (zeroing it on underflow) and the tail pops the 10019 window.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PinochiVotePacket {
    pub entries: Vec<PinochiVoteRow>,
}

/// One vote row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinochiVoteRow {
    pub slot: u16,  // 69-byte skill-table index (<= 198)
    pub value: u16, // decrement applied to the row counter
}

impl PinochiVotePacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.entries.len() <= u16::MAX as usize,
            "vote entries overflow u2"
        );
        let mut writer = PacketWriter::new(game::EVENT_PINOCHI_VOTE);
        writer.write_u16(self.entries.len() as u16);
        for row in &self.entries {
            writer.write_u16(row.slot);
            writer.write_u32(row.value as u32);
        }
        writer.finalize()
    }
}

/// `StoreSummonUse` — opcode 3953: {u1 count; count x {u4; u4}} handed to
/// event 466 after the client-side filler render.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoreSummonUsePacket {
    pub items: Vec<StoreSummonRow>,
}

/// One summon-use row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StoreSummonRow {
    pub first: u32,  // unresolved
    pub second: u32, // unresolved
}

impl StoreSummonUsePacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.items.len() <= u8::MAX as usize,
            "summon rows overflow u1"
        );
        let mut writer = PacketWriter::new(game::ITEM_STORE_SUMMON_USE);
        writer.write_u8(self.items.len() as u8);
        for row in &self.items {
            writer.write_u32(row.first);
            writer.write_u32(row.second);
        }
        writer.finalize()
    }
}

/// `Skill::Error` — opcode 1106: {u1 kind; u2 code} plus a dead trailing u2
/// consumed only when kind == 255. Kind 95 resets on code 0, kind 100 closes
/// the skill-cast window on code 1, other kinds fall through to the digimon
/// reset slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillErrorPacket {
    pub kind: u8, // 95 / 100 / 255 with distinct behavior
    pub code: u16,
}

impl SkillErrorPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_ERROR);
        writer.write_u8(self.kind);
        writer.write_u16(self.code);
        if self.kind == 255 {
            // Dead discriminator tail consumed by the kind-255 branch.
            writer.write_u16(0);
        }
        writer.finalize()
    }
}

/// `Event::Active` — opcode 3138: one u4 value, four u1 flags, a u1 row
/// count and one 71-byte row per count (68 opaque bytes, a u2 and a u1
/// tail). The client bonds the rows into a list and pings event 130 with
/// the assembled struct.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventActivePacket {
    pub value: u32, // unresolved
    pub flags: [u8; 4], // unresolved
    pub rows: Vec<[u8; 71]>,
}

impl EventActivePacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(self.rows.len() <= u8::MAX as usize, "active rows overflow u1");
        let mut writer = PacketWriter::new(game::EVENT_ACTIVE);
        writer.write_u32(self.value);
        for flag in &self.flags {
            writer.write_u8(*flag);
        }
        writer.write_u8(self.rows.len() as u8);
        for row in &self.rows {
            for byte in row {
                writer.write_u8(*byte);
            }
        }
        writer.finalize()
    }
}

/// `SkillSuccess` — opcode 1328: {u4 skill index; u4 argument}; the client
/// resolves the slide-list row and rebuilds the skill presentation struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TamerSkillSuccessPacket {
    pub index: u32, // slide-list row index
    pub arg: u32,   // unresolved
}

impl TamerSkillSuccessPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TAMER_SKILL_SUCCESS);
        writer.write_u32(self.index);
        writer.write_u32(self.arg);
        writer.finalize()
    }
}

/// `PartnerScale` — opcode 9942: {u4 handler; u2 scale; u4 arg}; the client
/// resolves the entity and applies the scale through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PartnerScalePacket {
    pub handler: u32, // entity handle to look up
    pub scale: u16,   // unresolved
    pub arg: u32,     // unresolved
}

impl PartnerScalePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_PARTNER_SCALE);
        writer.write_u32(self.handler);
        writer.write_u16(self.scale);
        writer.write_u32(self.arg);
        writer.finalize()
    }
}

/// Master/GM debug notices. `MasterConn` — opcode 9963: one u4 count
/// rendered as the "Connect" text; `MasterPayCash` — opcode 9964: two i64
/// amounts (silk plus premium cash) rendered as the payout text; `GMQuest` —
/// opcode 9972: a fixed 0x598-byte blob copied verbatim into the client
/// table at +0x766C.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MasterConnPacket {
    pub count: u32,
}

impl MasterConnPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_CONN_COUNT);
        writer.write_u32(self.count);
        writer.finalize()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MasterPayCashPacket {
    pub silk: i64, // unresolved
    pub cash: i64, // unresolved
}

impl MasterPayCashPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_PAY_CASH);
        writer.write_i64(self.silk);
        writer.write_i64(self.cash);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GmQuestPacket {
    pub blob: Vec<u8>, // fixed 0x598 bytes
}

impl GmQuestPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert_eq!(self.blob.len(), 0x598, "gm quest blob is fixed size");
        let mut writer = PacketWriter::new(game::MASTER_GM_QUEST);
        for byte in &self.blob {
            writer.write_u8(*byte);
        }
        writer.finalize()
    }
}

/// Guild name-flow notices. `MemberLevelUp` — opcode 2122 consumes one dead
/// u8 then a modern string, popped to event 268; `ChangeTName` — opcode 2123
/// reads two modern strings and pops event 275 with the first; `DeletedTamer`
/// — opcode 2124 reads one modern string and pops event 274.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuildMemberLevelUpPacket {
    pub reserved: u8, // consumed and unused
    pub name: String, // unresolved (event 268 payload)
}

impl GuildMemberLevelUpPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_MEMBER_LEVEL_UP);
        writer.write_u8(self.reserved);
        writer.write_modern_utf8_string(&self.name);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuildChangeTNamePacket {
    pub name: String, // popped via event 275
    pub title: String,
}

impl GuildChangeTNamePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_CHANGE_TNAME);
        writer.write_modern_utf8_string(&self.name);
        writer.write_modern_utf8_string(&self.title);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GuildDeletedTamerPacket {
    pub name: String, // popped via event 274
}

impl GuildDeletedTamerPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_DELETED_TAMER);
        writer.write_modern_utf8_string(&self.name);
        writer.finalize()
    }
}

/// `GMHatchTry` — opcode 9966: six u4 success counters, five u4 fail
/// counters, five u4 break counters and five u4 success-average floats; the
/// client renders them into `_HatchTry.txt` and opens notepad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GmHatchTryPacket {
    pub succ_count: [u32; 6],
    pub fail_count: [u32; 5],
    pub break_count: [u32; 5],
    pub succ_average: [u32; 5], // raw float bits
}

impl GmHatchTryPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GM_HATCH_TRY);
        for slot in self.succ_count.iter().chain(&self.fail_count).chain(&self.break_count).chain(&self.succ_average) {
            writer.write_u32(*slot);
        }
        writer.finalize()
    }
}

/// `PinochiGetInfo` — opcode 3126: eleven fixed u4 values parked into the
/// container global slots (+0x48 first, then +0x60..+0x70, then +0x4C..+0x5C).
/// The client refreshes the render list through the 0x3138 slots; when the
/// container lacks the window it falls back to an event-42 lookup text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PinochiGetInfoPacket {
    pub values: [u32; 11], // container slot parking, wire order preserved
}

impl PinochiGetInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::EVENT_PINOCHI_GET_INFO);
        for value in &self.values {
            writer.write_u32(*value);
        }
        writer.finalize()
    }
}

/// `Party::Exp` — opcode 2311: three i64 amounts, one u4 gap, another three
/// i64 amounts, one final u4 and a modern label string. The client hands the
/// numeric block to event 196; the label string is consumed and discarded.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PartyExpPacket {
    pub first: i64,  // unresolved
    pub second: i64, // unresolved
    pub third: i64,  // unresolved
    pub first_lo: u32,   // unresolved
    pub fifth: i64,  // unresolved
    pub sixth: i64,  // unresolved
    pub seventh: i64, // unresolved
    pub second_lo: u32,  // unresolved
    pub label: String,   // consumed, unused by the event
}

impl PartyExpPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_EXP);
        writer.write_i64(self.first);
        writer.write_i64(self.second);
        writer.write_i64(self.third);
        writer.write_u32(self.first_lo);
        writer.write_i64(self.fifth);
        writer.write_i64(self.sixth);
        writer.write_i64(self.seventh);
        writer.write_u32(self.second_lo);
        writer.write_modern_utf8_string(&self.label);
        writer.finalize()
    }
}

/// `EquipItem` — opcode 1310: {u2 handler; u1 kind; u4 x3; u8 dead; u8}.
/// Kind 5 pushes the equip record through the resolved entity slot; kind 10
/// swaps the partner aura, gating on item-table row kind 31 with the aura
/// visuals and the level-up jingle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TamerEquipItemPacket {
    pub handler: u16, // u2 container key
    pub kind: u8,     // 5 equip / 10 aura override
    pub first: u32,   // unresolved
    pub second: u32,  // unresolved
    pub third: u32,   // unresolved
    pub filler: u8,   // consumed and unused
    pub tail: u8,     // unresolved
}

impl TamerEquipItemPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TAMER_EQUIP_ITEM);
        writer.write_u16(self.handler);
        writer.write_u8(self.kind);
        writer.write_u32(self.first);
        writer.write_u32(self.second);
        writer.write_u32(self.third);
        writer.write_u8(self.filler);
        writer.write_u8(self.tail);
        writer.finalize()
    }
}

/// `Master::UserInfoNotFound` — opcode 9921: one modern name string; the
/// client prints the "(name) is not found" console line through event 42.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterUserInfoNotFoundPacket {
    pub name: String,
}

impl MasterUserInfoNotFoundPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_USER_INFO_NOT_FOUND);
        writer.write_modern_utf8_string(&self.name);
        writer.finalize()
    }
}

/// `Master::Init2ndPass` — opcode 9945: one u2 code. Code 0 resolves misc
/// table id 34 (CHAT_SECOND_PASSWORD_RESET_SUCCESS) and code 1 id 17
/// (CHAT_NOT_EXIST_ID); the resolved text pops as an event-42 console line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MasterInit2ndPassPacket {
    pub code: u16, // 0 = reset success, 1 = id not found
}

impl MasterInit2ndPassPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_INIT_2ND_PASS);
        writer.write_u16(self.code);
        writer.finalize()
    }
}

/// `Master::LieKillMonster` — opcode 9958: {u4 count; count x i64 money}. Each
/// entry carries the money credited for one kill record.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterLieKillMonsterPacket {
    pub entries: Vec<u64>, // kill money per entry
}

impl MasterLieKillMonsterPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.entries.len() <= u32::MAX as usize,
            "kill entries overflow u4"
        );
        let mut writer = PacketWriter::new(game::MASTER_LIE_KILL_MONSTER);
        writer.write_u32(self.entries.len() as u32);
        for entry in &self.entries {
            writer.write_i64(*entry as i64);
        }
        writer.finalize()
    }
}

/// `Master::InsertItemToShop` — opcode 9944: one u2 code resolving a misc
/// console message (30 added to cash shop, 23 item code missing, 29 store
/// full, plus further failure ids) popped through event 42.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MasterInsertItemToShopPacket {
    pub code: u16, // misc message id selector
}

impl MasterInsertItemToShopPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_INSERT_ITEM_TO_SHOP);
        writer.write_u16(self.code);
        writer.finalize()
    }
}

/// `Master::CheatLevel` — opcode 9932: {u4 result; s2 name}. Result 2
/// resolves misc table id 24 (CHAT_CHEAT_LEVEL_CHANGED); other results
/// resolve failure ids such as 23 (CHAT_RESULT_VALUE_ERROR) and pop through
/// event 42 with the target name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterCheatLevelPacket {
    pub result: u32,
    pub name: String,
}

impl MasterCheatLevelPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_CHEAT_LEVEL);
        writer.write_u32(self.result);
        writer.write_modern_utf8_string(&self.name);
        writer.finalize()
    }
}

/// `Master::GMCoolReSet` — opcode 9975: a fixed blob of 102 u4 slot
/// cooldown modifiers. Entry `i` adjusts the cooldown of slot `i + 1`
/// across the tamer and partner skill icon lists; the client then pings
/// event 42 with a static reset text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GmCoolResetPacket {
    pub entries: [u32; 102], // slot cooldown modifiers, index 0 -> slot 1
}

impl GmCoolResetPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_COOL_RESET);
        for entry in &self.entries {
            writer.write_u32(*entry);
        }
        writer.finalize()
    }
}

/// `Master::ChatBanUser` — opcode 9983: {u4 duration; s2 name}. Duration 0
/// resolves misc id 31 (unlimited server ban); any other value resolves id
/// 29 (limited server ban) with the duration split into hours through the
/// #Hour# message template, both popped through event 42.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterChatBanUserPacket {
    pub duration: u32, // ban seconds; 0 = unlimited
    pub name: String,
}

impl MasterChatBanUserPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_CHAT_BAN_USER);
        writer.write_u32(self.duration);
        writer.write_modern_utf8_string(&self.name);
        writer.finalize()
    }
}

/// `Master::InitSkillStat` — opcode 9941: {u4 kind; s2 tamer; s2 digimon}.
/// The client renders the misc id 37 line with the "{tamer} tn({second})
/// dn({third})" template, walks the per-slot stat reset switch, and pops
/// the console line through event 42.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterInitSkillStatPacket {
    pub kind: u32, // stat reset slot selector
    pub tamer: String,
    pub digimon: String,
}

impl MasterInitSkillStatPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_INIT_SKILL_STAT);
        writer.write_u32(self.kind);
        writer.write_modern_utf8_string(&self.tamer);
        writer.write_modern_utf8_string(&self.digimon);
        writer.finalize()
    }
}

/// `Master::UserInfo` — opcode 9920: {s2; s2; u4; u2; u1; r4 pair as i64;
/// u4; u1}. The two strings and the numeric block feed the console info
/// branches (each popping through event 42); the field semantics remain
/// unresolved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterUserInfoPacket {
    pub first: String, // unresolved
    pub second: String, // unresolved
    pub first_flag: u32,  // unresolved
    pub class: u16,       // unresolved
    pub head: u8,         // unresolved
    pub value: i64,       // unresolved
    pub tail: u32,        // unresolved
    pub trailing: u8,     // unresolved
}

impl MasterUserInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_USER_INFO);
        writer.write_modern_utf8_string(&self.first);
        writer.write_modern_utf8_string(&self.second);
        writer.write_u32(self.first_flag);
        writer.write_u16(self.class);
        writer.write_u8(self.head);
        writer.write_i64(self.value);
        writer.write_u32(self.tail);
        writer.write_u8(self.trailing);
        writer.finalize()
    }
}

/// `Master::MachineRun` — opcode 9981: `{u4 count; count x u4 item_code}`. A zero count is the
/// wrong-input reply; a code that misses the item table is the bad-data reply. Otherwise the
/// client resolves each code through the item table and writes the winning-count census to its
/// rare-machine report file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterMachineRunPacket {
    pub codes: Vec<u32>, // item codes, count encoded on the wire
}

impl MasterMachineRunPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.codes.len() <= u32::MAX as usize,
            "machine codes overflow u4"
        );
        let mut writer = PacketWriter::new(game::MASTER_MACHINE_RUN);
        writer.write_u32(self.codes.len() as u32);
        for code in &self.codes {
            writer.write_u32(*code);
        }
        writer.finalize()
    }
}

/// `Master::RankItemRun` — opcode 9982: `{u2 selector; u4 count; count x u4 item_code}`. A zero
/// selector ends the frame after the selector itself; it is the wrong-input reply. A code that
/// misses the item table is the item-rank error, and resolved codes roll into the item-rank
/// census file.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterRankItemRunPacket {
    pub selector: u16, // rank tier selector; zero means "no census follows"
    pub codes: Vec<u32>,
}

impl MasterRankItemRunPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.codes.len() <= u32::MAX as usize,
            "rank codes overflow u4"
        );
        let mut writer = PacketWriter::new(game::MASTER_RANK_ITEM_RUN);
        writer.write_u16(self.selector);
        if self.selector == 0 {
            return writer.finalize();
        }
        writer.write_u32(self.codes.len() as u32);
        for code in &self.codes {
            writer.write_u32(*code);
        }
        writer.finalize()
    }
}

/// `Skill::Range_PersistEnd` — opcode 1124: one u4 id ending the tracked
/// range-persist effect entry whose entity carries the id, respawning its
/// tail particle and refreshing the active list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillRangePersistEndPacket {
    pub id: u32,
}

impl SkillRangePersistEndPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_RANGE_PERSIST_END);
        writer.write_u32(self.id);
        writer.finalize()
    }
}

/// `Skill::ChipCoolTime` — opcode 1121: {u4 tamer_slot; u4 partner_slot}.
/// The tamer skill icon list is scanned for the first slot and the partner
/// skill list for the second, and each cooldown is re-anchored from its
/// skill-table row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillChipCoolTimePacket {
    pub tamer_slot: u32,
    pub partner_slot: u32,
}

impl SkillChipCoolTimePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_CHIP_COOL_TIME);
        writer.write_u32(self.tamer_slot);
        writer.write_u32(self.partner_slot);
        writer.finalize()
    }
}

/// `Skill::RemoveSkillChip` — opcode 1119: one u4 chip id. Erases the chip
/// from the tamer skill icon list, zeroes its chip slots in the +0x74C7
/// table, and stops the row effect when the matching CSkillTable row
/// carries one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillRemoveChipPacket {
    pub chip_id: u32,
}

impl SkillRemoveChipPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_REMOVE_SKILL_CHIP);
        writer.write_u32(self.chip_id);
        writer.finalize()
    }
}

/// `Skill::AddSkillChip` — opcode 1118: {u4 entity_slot; u4 chip_id;
/// u4 item_id}. Anchors the entity through the first slot, registers the
/// chip id in the chip table, feeds its item-table row into the buff
/// particle load, and pops the console line through the 477 event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillAddChipPacket {
    pub entity_slot: u32,
    pub chip_id: u32,
    pub item_id: u32,
}

impl SkillAddChipPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_ADD_SKILL_CHIP);
        writer.write_u32(self.entity_slot);
        writer.write_u32(self.chip_id);
        writer.write_u32(self.item_id);
        writer.finalize()
    }
}

/// `Skill::ActiveSkillEff` — opcode 1117: {u4 slot; u4 skill_id; u2 level}.
/// Resolves the entity through the slot and the CSkillTable row through the
/// skill id, then branches on the handler kind and pops message lines
/// through event 42.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillActiveEffectPacket {
    pub slot: u32,
    pub skill_id: u32,
    pub level: u16,
}

impl SkillActiveEffectPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_ACTIVE_SKILL_EFF);
        writer.write_u32(self.slot);
        writer.write_u32(self.skill_id);
        writer.write_u16(self.level);
        writer.finalize()
    }
}

/// `Skill::MonsterBuff` — opcode 1115: {u4 slot; u4 monster_key; u4 param}.
/// Resolves the entity through the slot and the CMonsterTable row through
/// the key, branches on the row kind (4/13, 14, or fallback message),
/// applies the 0x9CE2 buff text with the scaled float, and pops the console
/// line through event 42.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SkillMonsterBuffPacket {
    pub slot: u32,
    pub monster_key: u32,
    pub param: u32, // unresolved buff amplitude
}

impl SkillMonsterBuffPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_MONSTER_BUFF);
        writer.write_u32(self.slot);
        writer.write_u32(self.monster_key);
        writer.write_u32(self.param);
        writer.finalize()
    }
}

/// `Skill::MonsterSkillEffect` — opcode 1123: {u4 caster; u4 skill_id; u4
/// count; count x {u4 handler; u4 x; u4 y}}. The trailing reads only exist
/// on the CMonsterTable row kind-18 branch: each loop iteration resolves
/// the target through the handler and spawns the raid dot-damage circle at
/// the integer tile coordinates. Row kind 19 stops the effect; other kinds
/// pop the event 42 error line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillMonsterSkillTarget {
    pub handler: u32,
    pub x: u32, // integer tile coordinate, cast to f32 client-side
    pub y: u32, // integer tile coordinate, cast to f32 client-side
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillMonsterSkillEffectPacket {
    pub caster: u32,
    pub skill_id: u32,
    pub targets: Vec<SkillMonsterSkillTarget>, // kind-18 branch
}

impl SkillMonsterSkillEffectPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.targets.len() <= u32::MAX as usize,
            "monster skill targets overflow u4"
        );
        let mut writer = PacketWriter::new(game::SKILL_MONSTER_SKILL_EFFECT);
        writer.write_u32(self.caster);
        writer.write_u32(self.skill_id);
        writer.write_u32(self.targets.len() as u32);
        for target in &self.targets {
            writer.write_u32(target.handler);
            writer.write_u32(target.x);
            writer.write_u32(target.y);
        }
        writer.finalize()
    }
}

/// `Skill::AroundSkill` — opcode 1110: {u4 caster; u4 skill_id; u2 count;
/// u4 param; count x {u1 flag; u4 handler; u4 value; u1 level}}. Each hit
/// is resolved through the handler and pushed into the damage dispatch
/// together with the third field; flagged hits also pop the wide-skill
/// console line (event 42) and refresh the caster marker.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillAroundHit {
    pub flag: u8,    // wide-skill console flag on the hit
    pub handler: u32,
    pub value: u32, // damage/name payload carried into the dispatch
    pub level: u8,  // hit level subtracted client-side
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillAroundSkillPacket {
    pub caster: u32,
    pub skill_id: u32,
    pub attack_param: u32, // forwarded to the damage dispatch unmodified
    pub hits: Vec<SkillAroundHit>,
}

impl SkillAroundSkillPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.hits.len() <= u16::MAX as usize,
            "around-skill hits overflow u2"
        );
        let mut writer = PacketWriter::new(game::SKILL_AROUND_SKILL);
        writer.write_u32(self.caster);
        writer.write_u32(self.skill_id);
        writer.write_u16(self.hits.len() as u16);
        writer.write_u32(self.attack_param);
        for hit in &self.hits {
            writer.write_u8(hit.flag);
            writer.write_u32(hit.handler);
            writer.write_u32(hit.value);
            writer.write_u8(hit.level);
        }
        writer.finalize()
    }
}

/// `Skill::RangeSkillDmg` — opcode 1116: {u4 caster; u2 atk_count; u1 flag;
/// u4 primary_target; atk_count x attacker; u2 tgt_count; tgt_count x
/// target}. Each attacker row carries a 40-byte stat blob plus a
/// self-terminated list of `slot,value` pairs whose final u1 sentinel
/// (>= 0x12) closes it; target rows carry only the handler and the blob.
/// Both row kinds feed the damage dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRangeAttacker {
    pub actor: u32,
    pub pad: u8, // unresolved per-row flag
    pub blob: [u8; 40],
    pub stats: Vec<(u8, u32)>, // (slot 0..0x11, value)
    pub stats_end: u8,         // terminating sentinel slot (>= 0x12)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRangeTarget {
    pub handler: u32,
    pub blob: [u8; 40],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRangeSkillDmgPacket {
    pub caster: u32,
    pub flag: u8,
    pub primary_target: u32,
    pub attackers: Vec<SkillRangeAttacker>,
    pub targets: Vec<SkillRangeTarget>,
}

impl Default for SkillRangeSkillDmgPacket {
    fn default() -> Self {
        Self {
            caster: 0,
            flag: 0,
            primary_target: 0,
            attackers: Vec::new(),
            targets: Vec::new(),
        }
    }
}

impl SkillRangeSkillDmgPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.attackers.len() <= u16::MAX as usize,
            "range-skill attackers overflow u2"
        );
        assert!(
            self.targets.len() <= u16::MAX as usize,
            "range-skill targets overflow u2"
        );
        let mut writer = PacketWriter::new(game::SKILL_RANGE_SKILL_DMG);
        writer.write_u32(self.caster);
        writer.write_u16(self.attackers.len() as u16);
        writer.write_u8(self.flag);
        writer.write_u32(self.primary_target);
        for attacker in &self.attackers {
            writer.write_u32(attacker.actor);
            writer.write_u8(attacker.pad);
            for byte in &attacker.blob {
                writer.write_u8(*byte);
            }
            for (slot, value) in &attacker.stats {
                writer.write_u8(*slot);
                writer.write_u32(*value);
            }
            writer.write_u8(attacker.stats_end);
        }
        writer.write_u16(self.targets.len() as u16);
        for target in &self.targets {
            writer.write_u32(target.handler);
            for byte in &target.blob {
                writer.write_u8(*byte);
            }
        }
        writer.finalize()
    }
}

/// `Master::ConnCountInfo` — opcode 9938: {u2 group_count; group_count x
/// chain of (u1 code; u2 count) closed by a lone u1 0xFF}. The chain is a
/// sequence of counted segments: the first code opens it, each following
/// count is reported with the "%04d - %02d : %03d" line through misc id
/// 3334 and event 42, and the terminator code reads back without a count.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterConnCountGroup {
    pub opens: u8,                            // first code read
    pub entries: Vec<(u16, u8)>,              // (count, following code)
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterConnCountInfoPacket {
    pub groups: Vec<MasterConnCountGroup>,
}

impl MasterConnCountInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.groups.len() <= u16::MAX as usize,
            "conn-count groups overflow u2"
        );
        let mut writer = PacketWriter::new(game::MASTER_CONN_COUNT_INFO);
        writer.write_u16(self.groups.len() as u16);
        for group in &self.groups {
            writer.write_u8(group.opens);
            for (count, code) in &group.entries {
                writer.write_u16(*count);
                writer.write_u8(*code);
            }
            writer.write_u8(0xFF);
        }
        writer.finalize()
    }
}

/// `Item::CapsuleToItem` — opcode 3937: {u4 count; per row {u1 slot; u2
/// code; u4 param; u4 next_count}}. The loop is a self-terminating chain:
/// each row carries the following count on the wire, so the trailing count
/// of the last row is zero. Rows insert into the +0x9DC inventory list
/// through event 11, and a scene item 901107/901108 additionally triggers
/// the 901109 cast.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemCapsuleRow {
    pub slot: u8,
    pub code: u16,
    pub param: u32, // unresolved per-row value
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemCapsuleToItemPacket {
    pub rows: Vec<ItemCapsuleRow>,
}

impl ItemCapsuleToItemPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_CAPSULE_TO_ITEM);
        let remaining = self.rows.len() as u32;
        writer.write_u32(remaining);
        for (index, row) in self.rows.iter().enumerate() {
            writer.write_u8(row.slot);
            writer.write_u16(row.code);
            writer.write_u32(row.param);
            writer.write_u32(remaining - index as u32 - 1);
        }
        writer.finalize()
    }
}

/// `Item::Timeout` — opcode 3933: {u1 kind_cc; u2 code; u4 row_key; u1
/// kind_c8}. Resolves the CItemTable row through the key: rows of kind 3
/// hitting a special +0x18 value drop the matching slot from the +0x7664
/// list, while other rows stop the 0x4E3D effect, open the timed-removal
/// window filled with the two kinds plus the code, and pop event 687.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemTimeoutPacket {
    pub kind_cc: u8, // unresolved; stored into the window field +0xCC
    pub code: u16,   // item code surfaced in the window
    pub row_key: u32, // CItemTable row key
    pub kind_c8: u8, // unresolved; stored into the window field +0xC8
}

impl ItemTimeoutPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_TIMEOUT);
        writer.write_u8(self.kind_cc);
        writer.write_u16(self.code);
        writer.write_u32(self.row_key);
        writer.write_u8(self.kind_c8);
        writer.finalize()
    }
}

/// `Item::PartyCrop` — opcode 3939: {u2 code; u4 param; u2 sub_out; u1
/// mode; s2 reason; u1 signed member count; count x {u1 kind; s2 name}}.
/// Each member row becomes a list node consumed by the delete-list
/// handler, and event 197 pops the code after the list settles.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemPartyCropMember {
    pub kind: u8,
    pub name: String, // modern utf8 string, u16 length prefix
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ItemPartyCropPacket {
    pub code: u16,
    pub param: u32,
    pub sub_out: u16,
    pub mode: u8,
    pub reason: String, // modern utf8 string, u16 length prefix
    pub members: Vec<ItemPartyCropMember>, // count is a signed u1
}

impl ItemPartyCropPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.members.len() <= i8::MAX as usize,
            "party-crop members overflow s1"
        );
        let mut writer = PacketWriter::new(game::ITEM_PARTY_CROP);
        writer.write_u16(self.code);
        writer.write_u32(self.param);
        writer.write_u16(self.sub_out);
        writer.write_u8(self.mode);
        writer.write_modern_utf8_string(&self.reason);
        writer.write_u8(self.members.len() as u8);
        for member in &self.members {
            writer.write_u8(member.kind);
            writer.write_modern_utf8_string(&member.name);
        }
        writer.finalize()
    }
}

/// `Skill::AroundSkillAni` — opcode 1111: {u1 flag; u4 seconds}. Nonzero
/// flags apply the around-skill cooldown animation, converting the value
/// to float seconds against the frame rate before storing it into the
/// timer slot (and popping the warning popup when the dialog singleton is
/// up). A zero flag hands the cursor to an embedded animation-state
/// parser (a u4 pair plus a chained u4 handler list) first — that tail is
/// unresolved — and in every bail-out path consumes the trailing u4.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillAroundSkillAniPacket {
    pub flag: u8,
    pub seconds: u32, // integer seconds, cast to f32 client-side
}

impl SkillAroundSkillAniPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_AROUND_SKILL_ANI);
        writer.write_u8(self.flag);
        writer.write_u32(self.seconds);
        writer.finalize()
    }
}

/// 14-byte record read inside the rows payload: one u64, one u32 and one
/// u16 back to back; field semantics are unresolved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DigimonDataChangeRecord14 {
    pub qword: u64,
    pub dword: u32,
    pub word: u16,
}

fn write_record14(writer: &mut PacketWriter, record: &DigimonDataChangeRecord14) {
    writer.write_u64(record.qword);
    writer.write_u32(record.dword);
    writer.write_u16(record.word);
}

/// Tag-specific body of `Digimon::DigimonDataChange` (opcode 3242) after
/// the shared `{u4 tag; u1 a; u1 b; u4 magic}` header. Window feeds client
/// event 79, Rows feeds event 80, Blobs feeds event 81. The Blobs counts
/// are raw bytes (`u1`), each covering exactly that many 28-byte records
/// memcpy'd on the client, which reserves 19 slots per list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigimonDataChangePayload {
    Window {
        c: u8,
        d: u16,
        e: u8,
        f: u16,
    }, // client event 79
    Rows {
        g: u16,
        first: DigimonDataChangeRecord14,
        second: DigimonDataChangeRecord14,
        h: u16,
        third: DigimonDataChangeRecord14,
        fourth: DigimonDataChangeRecord14,
    }, // client event 80
    Blobs {
        first: Vec<[u8; 28]>,
        second: Vec<[u8; 28]>,
    }, // client event 81
}

/// `Digimon::DigimonDataChange` — opcode 3242: {u4 tag; u1 a; u1 b; u4
/// magic; tag body}. The magic field must read 30803 (0x7853); any other
/// value pops the reject line through client event 82 before the body is
/// parsed. Tags 0, 1 and 2 select the payloads above.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigimonDataChangePacket {
    pub tag: u32, // 0, 1 or 2; other tags pop the reject line client-side
    pub field_a: u8,
    pub field_b: u8,
    pub magic: u32,
    pub payload: DigimonDataChangePayload,
}

impl DigimonDataChangePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGIMON_DATA_CHANGE);
        writer.write_u32(self.tag);
        writer.write_u8(self.field_a);
        writer.write_u8(self.field_b);
        writer.write_u32(self.magic);
        match &self.payload {
            DigimonDataChangePayload::Window { c, d, e, f } => {
                writer.write_u8(*c);
                writer.write_u16(*d);
                writer.write_u8(*e);
                writer.write_u16(*f);
            }
            DigimonDataChangePayload::Rows {
                g,
                first,
                second,
                h,
                third,
                fourth,
            } => {
                writer.write_u16(*g);
                write_record14(&mut writer, first);
                write_record14(&mut writer, second);
                writer.write_u16(*h);
                write_record14(&mut writer, third);
                write_record14(&mut writer, fourth);
            }
            DigimonDataChangePayload::Blobs { first, second } => {
                writer.write_u8(first.len() as u8);
                for record in first {
                    writer.write_bytes(record);
                }
                writer.write_u8(second.len() as u8);
                for record in second {
                    writer.write_bytes(record);
                }
            }
        }
        writer.finalize()
    }
}

/// One answer row of `Game::MacroQuestion` — opcode 1064: {u4 id; s2
/// text}; mirrored by the nsMacroProtector 32-byte element object.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameMacroQuestionAnswer {
    pub id: u32,
    pub text: String, // modern utf8 string, u16 length prefix
}

/// `Game::MacroQuestion` — opcode 1064: {u2 a; u2 b; u1 c; u4 d; s2
/// prompt; u2 count; count x {u4 id; s2 text}; u4 x4}. Assembles the
/// image-question dialog (GS2C_RES_MACRO_IMAGE_QUESTION) with counted
/// answer rows and pops event 357. The two leading u2, the u1, the u4 and
/// the four trailing u4 stay on the dialog object unresolved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameMacroQuestionPacket {
    pub a: u16, // unresolved dialog field +0x8
    pub b: u16, // unresolved dialog field +0xA
    pub c: u8,  // unresolved dialog field +0xC
    pub d: u32, // unresolved dialog field +0x10
    pub prompt: String, // modern utf8 string, u16 length prefix
    pub answers: Vec<GameMacroQuestionAnswer>,
    pub tail: [u32; 4], // unresolved dialog fields +0x38..+0x44
}

impl GameMacroQuestionPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_MACRO_QUESTION);
        writer.write_u16(self.a);
        writer.write_u16(self.b);
        writer.write_u8(self.c);
        writer.write_u32(self.d);
        writer.write_modern_utf8_string(&self.prompt);
        assert!(
            self.answers.len() <= u16::MAX as usize,
            "macro-question answers overflow u2"
        );
        writer.write_u16(self.answers.len() as u16);
        for answer in &self.answers {
            writer.write_u32(answer.id);
            writer.write_modern_utf8_string(&answer.text);
        }
        for value in &self.tail {
            writer.write_u32(*value);
        }
        writer.finalize()
    }
}

/// `Master::SetMoney` — opcode 9907: {u8 money}. Stores the amount into
/// the local wallet singleton slot (+60) and pings event 38.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterSetMoneyPacket {
    pub money: u64,
}

impl MasterSetMoneyPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MASTER_SET_MONEY);
        writer.write_u64(self.money);
        writer.finalize()
    }
}

/// `Master::GMGuildSkillReset` — opcode 9990: empty payload. The parser
/// performs no cursor reads and only pings event 284.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterGmGuildSkillResetPacket;

impl MasterGmGuildSkillResetPacket {
    pub fn encode(&self) -> Vec<u8> {
        PacketWriter::new(game::MASTER_GM_GUILD_SKILL_RESET).finalize()
    }
}

/// `Event::MastersMatching` — opcode 3125: empty payload. The parser
/// performs no cursor reads and fires event 572 carrying {3125, 0}.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventMastersMatchingPacket;

impl EventMastersMatchingPacket {
    pub fn encode(&self) -> Vec<u8> {
        PacketWriter::new(game::EVENT_MASTERS_MATCHING).finalize()
    }
}

/// `Buff::CreateBuff` — opcode 4000: {u4 target; u2 a; u2 b; u4 c; u4 d}.
/// The client resolves the target by entity kind and hands the five
/// fields to the slot creator; field semantics beyond target are
/// unresolved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuffCreatePacket {
    pub target: u32,
    pub a: u16, // resolved kind 11/12 buff slot
    pub b: u16,
    pub c: u32,
    pub d: u32,
}

impl BuffCreatePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUFF_CREATE);
        writer.write_u32(self.target);
        writer.write_u16(self.a);
        writer.write_u16(self.b);
        writer.write_u32(self.c);
        writer.write_u32(self.d);
        writer.finalize()
    }
}

/// `Buff::BuffChange` — opcode 4001: {u4 target; u2 a; u2 b; u2 c; u4 d;
/// u4 e}. Third u2 and second u4 extend the create envelope.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuffChangePacket {
    pub target: u32,
    pub a: u16,
    pub b: u16,
    pub c: u16,
    pub d: u32,
    pub e: u32,
}

impl BuffChangePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUFF_BUFF_CHANGE);
        writer.write_u32(self.target);
        writer.write_u16(self.a);
        writer.write_u16(self.b);
        writer.write_u16(self.c);
        writer.write_u32(self.d);
        writer.write_u32(self.e);
        writer.finalize()
    }
}

/// `Buff::BuffClear` — opcode 4002: {u4 target; u2 count; count x u2} —
/// each u2 clears one buff id from the resolved entity's buff list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuffClearPacket {
    pub target: u32,
    pub buffs: Vec<u16>,
}

impl BuffClearPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUFF_BUFF_CLEAR);
        writer.write_u32(self.target);
        writer.write_u16(self.buffs.len() as u16);
        for buff in &self.buffs {
            writer.write_u16(*buff);
        }
        writer.finalize()
    }
}

/// `Buff::ReturnFail` — opcode 4009: {u1 kind; u2 code; u4 upgrade when
/// kind==1}; a kind of 1 routes the u4 parameter onward, kind 2 stops
/// reading, and codes 1/3/4 pop the inert dialog flags.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuffReturnFailPacket {
    pub kind: u8,
    pub code: u16,
    pub upgrade: Option<u32>,
}

impl BuffReturnFailPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUFF_RETURN_FAIL);
        writer.write_u8(self.kind);
        writer.write_u16(self.code);
        if let Some(value) = self.upgrade {
            writer.write_u32(value);
        }
        writer.finalize()
    }
}

/// `Buff::AbnormalDamage` — opcode 4011: {u4 handler; u4 actor; u2
/// target; u1 flag; u4 pair; u1 kind}. All five cursor reads happen up
/// front; the remaining logic applies the abnormal damage without further
/// wire access.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuffAbnormalDamagePacket {
    pub handler: u32,
    pub actor: u32,
    pub target: u16,
    pub flag: u8,
    pub pair: [u32; 2],
    pub kind: u8,
}

impl BuffAbnormalDamagePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUFF_ABNORMAL_DAMAGE);
        writer.write_u32(self.handler);
        writer.write_u32(self.actor);
        writer.write_u16(self.target);
        writer.write_u8(self.flag);
        for value in &self.pair {
            writer.write_u32(*value);
        }
        writer.write_u8(self.kind);
        writer.finalize()
    }
}

/// `Buff::BuffSkill` — opcode 4012: {u4 target; u2 slot; u1 flag; u4 a;
/// u4 b}. The client RTTI-casts the target to the tamer type and applies
/// the buff skill entry through the buff slot.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuffBuffSkillPacket {
    pub target: u32,
    pub slot: u16,
    pub flag: u8,
    pub a: u32,
    pub b: u32,
}

impl BuffBuffSkillPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUFF_BUFF_SKILL);
        writer.write_u32(self.target);
        writer.write_u16(self.slot);
        writer.write_u8(self.flag);
        writer.write_u32(self.a);
        writer.write_u32(self.b);
        writer.finalize()
    }
}

/// `Buff::Stun` — opcode 4013: {u4 actor; u4 skill; u2 duration; u4
/// value}. The stun table row (kind 0xF) applies through the buff slot
/// and the failure path pops event 42 carrying message 3391.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BuffStunPacket {
    pub actor: u32,
    pub skill: u32,
    pub duration: u16,
    pub value: u32,
}

impl BuffStunPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUFF_STUN);
        writer.write_u32(self.actor);
        writer.write_u32(self.skill);
        writer.write_u16(self.duration);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `CashShop::BuyRequest` — opcode 3401: {u4 a; u4 b}; the pair flows
/// straight into the event 433 report.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CashShopBuyRequestPacket {
    pub a: u32,
    pub b: u32,
}

impl CashShopBuyRequestPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::CASH_SHOP_BUY_REQUEST);
        writer.write_u32(self.a);
        writer.write_u32(self.b);
        writer.finalize()
    }
}

/// `CashShop::Gift` — opcode 3403: {u2 a; u4 b; u4 c; s2 name; u4 d};
/// event 430 carries the fields with the recipient name string.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CashShopGiftPacket {
    pub a: u16, // unresolved field
    pub b: u32, // unresolved field
    pub c: u32, // unresolved field
    pub name: String, // modern utf8 string, u16 length prefix
    pub d: u32, // unresolved field
}

impl CashShopGiftPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::CASH_SHOP_GIFT);
        writer.write_u16(self.a);
        writer.write_u32(self.b);
        writer.write_u32(self.c);
        writer.write_modern_utf8_string(&self.name);
        writer.write_u32(self.d);
        writer.finalize()
    }
}

/// `CashShop::Cart` — opcode 3405: a skipped 48-byte blob; the client
/// advances the cursor past it and pops event 426.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashShopCartPacket {
    pub blob: [u8; 48],
}

impl Default for CashShopCartPacket {
    fn default() -> Self {
        Self { blob: [0u8; 48] }
    }
}

impl CashShopCartPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::CASH_SHOP_CART);
        writer.write_bytes(&self.blob);
        writer.finalize()
    }
}

/// One row of `Quest::Canceled` — opcode 11008: {u4 kind; u2 slot; u4 a;
/// u4 b}; kind 2 or 9 routes the row to the direct-cancel handler.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestCancelRow {
    pub kind: u32,
    pub slot: u16,
    pub a: u32, // unresolved field
    pub b: u32, // unresolved field
}

/// `Quest::Canceled` — opcode 11008: {u2 count; count x rows}.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QuestCanceledPacket {
    pub rows: Vec<QuestCancelRow>,
}

impl QuestCanceledPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::QUEST_CANCELED);
        assert!(
            self.rows.len() <= u16::MAX as usize,
            "canceled quest rows overflow u2"
        );
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            writer.write_u32(row.kind);
            writer.write_u16(row.slot);
            writer.write_u32(row.a);
            writer.write_u32(row.b);
        }
        writer.finalize()
    }
}

/// `InsMap::DungeonFail` — opcode 4121: {u4 skipped}; the parser advances
/// past the u4 without storing it, flags the map singleton byte +28584,
/// and the inventory associates dispatch id 262 with it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InsMapDungeonFailPacket {
    pub skipped: u32,
}

impl InsMapDungeonFailPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::INSMAP_DUNGEON_FAIL);
        writer.write_u32(self.skipped);
        writer.finalize()
    }
}

/// `Svr::SelectPortalFailure` — opcode 1710: {u4 skipped}; the u4 is
/// consumed unstored, and the portal-error flag gates event 124. The flag
/// clears afterwards either way.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SvrSelectPortalFailurePacket {
    pub skipped: u32,
}

impl SvrSelectPortalFailurePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SVR_SELECT_PORTAL_FAILURE);
        writer.write_u32(self.skipped);
        writer.finalize()
    }
}

/// `Monster::NatualExp` — opcode 1602: {u1 a; u1 b; u2 c}; event 576
/// forwards the opcode id followed by the three fields untranslated.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MonsterNatualExpPacket {
    pub a: u8,
    pub b: u8,
    pub c: u16,
}

impl MonsterNatualExpPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MONSTER_NATUAL_EXP);
        writer.write_u8(self.a);
        writer.write_u8(self.b);
        writer.write_u16(self.c);
        writer.finalize()
    }
}

/// `Event::HotTimeEvent` — opcode 3134: {u1 a; u1 b; u1 c; u1 d; u4 e;
/// u4 f}. The client rollers the four leading bytes into {u16, u16}
/// accumulator slots after the opcode id and pings event 453.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventHotTimePacket {
    pub a: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u32,
    pub f: u32,
}

impl EventHotTimePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::EVENT_HOT_TIME_EVENT);
        writer.write_u8(self.a);
        writer.write_u8(self.b);
        writer.write_u8(self.c);
        writer.write_u8(self.d);
        writer.write_u32(self.e);
        writer.write_u32(self.f);
        writer.finalize()
    }
}

/// `Event::ItemType` — opcode 3140: {s2 name; u4 a; u2 b}; event 528
/// carries the name string with the two trailing fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventItemTypePacket {
    pub name: String, // modern utf8 string, u16 length prefix
    pub a: u32,
    pub b: u16,
}

impl EventItemTypePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::EVENT_ITEM_TYPE);
        writer.write_modern_utf8_string(&self.name);
        writer.write_u32(self.a);
        writer.write_u16(self.b);
        writer.finalize()
    }
}

/// One roster row of `Monster::MsgRaidRankingList` — opcode 1604: {u4 a;
/// s2 name1; s2 name2; u4 b; u4 c}; unresolved fields keep their node
/// offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MsgRaidRankingRow {
    pub a: u32,
    pub name1: String, // modern utf8 string, u16 length prefix
    pub name2: String, // modern utf8 string, u16 length prefix
    pub b: u32,
    pub c: u32,
}

/// `Monster::MsgRaidRankingList` — opcode 1604: {u4 count; count x rows}.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MsgRaidRankingListPacket {
    pub rows: Vec<MsgRaidRankingRow>,
}

impl MsgRaidRankingListPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MONSTER_MSG_RAID_RANKING_LIST);
        assert!(
            self.rows.len() <= u32::MAX as usize,
            "raid ranking rows overflow u4"
        );
        writer.write_u32(self.rows.len() as u32);
        for row in &self.rows {
            writer.write_u32(row.a);
            writer.write_modern_utf8_string(&row.name1);
            writer.write_modern_utf8_string(&row.name2);
            writer.write_u32(row.b);
            writer.write_u32(row.c);
        }
        writer.finalize()
    }
}

/// One clear row of `Msg::DungeonClearInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonClearEntry {
    pub flag: u8,
    pub value: u32,
}

/// One reward row of `Msg::DungeonClearInfo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonClearReward {
    pub value: u32,
    pub item: u16,
}

/// `Msg::DungeonClearInfo` — opcode 14006: {u4 value; u1 clear count;
/// clear count x {u1 flag; u4 value}; u1 reward count;
/// reward count x {u4 value; u2 item}}.
///
/// Both counts are read as signed bytes, so a value above 127 would end the
/// lists; the server keeps them within that range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DungeonClearInfoPacket {
    pub value: u32,
    pub clears: Vec<DungeonClearEntry>,
    pub rewards: Vec<DungeonClearReward>,
}

impl DungeonClearInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MSG_DUNGEON_CLEAR_INFO);
        assert!(
            self.clears.len() <= i8::MAX as usize && self.rewards.len() <= i8::MAX as usize,
            "dungeon clear lists overflow a signed u1"
        );
        writer.write_u32(self.value);
        writer.write_u8(self.clears.len() as u8);
        for entry in &self.clears {
            writer.write_u8(entry.flag);
            writer.write_u32(entry.value);
        }
        writer.write_u8(self.rewards.len() as u8);
        for reward in &self.rewards {
            writer.write_u32(reward.value);
            writer.write_u16(reward.item);
        }
        writer.finalize()
    }
}

/// One stat row of a `Skill::ApplyAround` melee cast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillStatRow {
    pub value: i32,
    pub extra: u32,
    pub stat: u8, // the chain ends when this reaches 18
}

/// One tick row of a `Skill::ApplyAround` damage-over-time broadcast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkillDotRow {
    pub handler: u32,
    pub buff: u16,
    pub stat: u8,
    pub value: i32,
    pub second_value: u32,
}

/// Body of `Skill::ApplyAround` — opcode 1102, selected by its leading tag.
///
/// Tag 1 is deliberately absent: its remaining fields are unbound, and encoding
/// it on a guess would shift every later read on the client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillApplyAroundBody {
    /// Tag 0: melee cast.
    Melee {
        caster: u32,
        skill_id: u32,
        target: u32,
        flag: u8,
        /// 40 bytes read only when the caster is nonzero.
        caster_block: Option<[u8; 40]>,
        caster_stats: Vec<SkillStatRow>,
        /// Rows that fill the target table, indexed by their own stat selector.
        target_stats: Vec<SkillStatRow>,
        tail: u32,
    },
    /// Tag 2: damage-over-time ticks.
    Dot {
        source: u32,
        flag: u8,
        rows: Vec<SkillDotRow>,
    },
    /// Tag 3: scene cast.
    Scene { source: u32, index: u32 },
}

/// Small item notices — opcodes 5005 through 5009.
///
/// Each carries one flag byte followed by one or two words; the shapes differ
/// per opcode, so the encoder writes only what that opcode declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemNoticeBody {
    /// 5005 and 5009: {u1 flag; u2 value}.
    FlagWord { flag: u8, value: u16 },
    /// 5006: {u1 flag; u2 value; u2 second value}.
    FlagTwoWords {
        flag: u8,
        value: u16,
        second_value: u16,
    },
    /// 5007: {u2 value}.
    Word { value: u16 },
    /// 5008: {u1 flag}.
    Flag { flag: u8 },
}

/// `Item::SmallNotice` — opcodes 5005 to 5009.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSmallNoticePacket {
    pub opcode: i16,
    pub body: ItemNoticeBody,
}

impl ItemSmallNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(self.opcode);
        match self.body {
            ItemNoticeBody::FlagWord { flag, value } => {
                assert!(
                    self.opcode == game::ITEM_NOTICE_5005 || self.opcode == game::ITEM_NOTICE_5009,
                    "the flag+word body belongs to opcodes 5005 and 5009"
                );
                writer.write_u8(flag);
                writer.write_u16(value);
            }
            ItemNoticeBody::FlagTwoWords {
                flag,
                value,
                second_value,
            } => {
                assert!(
                    self.opcode == game::ITEM_NOTICE_5006,
                    "the flag+two-word body belongs to opcode 5006"
                );
                writer.write_u8(flag);
                writer.write_u16(value);
                writer.write_u16(second_value);
            }
            ItemNoticeBody::Word { value } => {
                assert!(
                    self.opcode == game::ITEM_NOTICE_5007,
                    "the bare word body belongs to opcode 5007"
                );
                writer.write_u16(value);
            }
            ItemNoticeBody::Flag { flag } => {
                assert!(
                    self.opcode == game::ITEM_NOTICE_5008,
                    "the bare flag body belongs to opcode 5008"
                );
                writer.write_u8(flag);
            }
        }
        writer.finalize()
    }
}

/// `Item::RefreshMacroProtectorResult` — opcode 5206: {u4 value; u2 count;
/// count x u1 bytes; u1 tail}.
///
/// The byte run is a blob the client stores verbatim, so it stays opaque here
/// rather than being reinterpreted as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshMacroProtectorResultPacket {
    pub value: u32,
    pub blob: Vec<u8>,
    pub tail: u8,
}

impl RefreshMacroProtectorResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::RESULT_REFRESH_MACRO_PROTECTOR);
        assert!(
            self.blob.len() <= u16::MAX as usize,
            "macro protector blob overflows u2"
        );
        writer.write_u32(self.value);
        writer.write_u16(self.blob.len() as u16);
        writer.write_bytes(&self.blob);
        writer.write_u8(self.tail);
        writer.finalize()
    }
}

/// `Game::SingleByteNotice` — opcode 2318: {u1 value}.
///
/// The handler consumes the byte and drops it, so the value carries no state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingleByteNoticePacket {
    pub value: u8,
}

impl SingleByteNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SINGLE_BYTE_NOTICE_2318);
        writer.write_u8(self.value);
        writer.finalize()
    }
}

/// Single-word notice — opcodes 1309, 1315, 9801, 9804 and 9806.
///
/// All carry one unbound word; the encoding is identical and only the opcode
/// distinguishes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingleWordNoticePacket {
    pub opcode: i16,
    pub value: u32,
}

impl SingleWordNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `Digimon::LevelUpInIncubator` — opcode 3250: {u4 digimon; u4 level}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelUpInIncubatorPacket {
    pub digimon: u32,
    pub level: u32,
}

impl LevelUpInIncubatorPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LEVELUP_DIGIMON_IN_INCUBATER);
        writer.write_u32(self.digimon);
        writer.write_u32(self.level);
        writer.finalize()
    }
}

/// `Item::ExtraInvenSlotExpansion` — opcode 3702: {u1 flag; u2 slots}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtraInvenSlotExpansionPacket {
    pub flag: u8,
    pub slots: u16,
}

impl ExtraInvenSlotExpansionPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NTF_EXTRAINVEN_SLOTEXPANSION);
        writer.write_u8(self.flag);
        writer.write_u16(self.slots);
        writer.finalize()
    }
}

/// `Item::MixedWidthNotice` — opcodes 4125 and 4136: {u4 value; u1 flag;
/// u4 second value; u2 first short; u2 second short; u4 third value}.
///
/// The widths alternate, so a flat run of words would put every field after the
/// first byte at the wrong offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MixedWidthNoticePacket {
    pub opcode: i16,
    pub value: u32,
    pub flag: u8,
    pub second_value: u32,
    pub first_short: u16,
    pub second_short: u16,
    pub third_value: u32,
}

impl MixedWidthNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            matches!(self.opcode, game::NOTICE_4125 | game::NOTICE_4136),
            "the mixed-width body belongs to opcodes 4125 and 4136"
        );
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_u32(self.value);
        writer.write_u8(self.flag);
        writer.write_u32(self.second_value);
        writer.write_u16(self.first_short);
        writer.write_u16(self.second_short);
        writer.write_u32(self.third_value);
        writer.finalize()
    }
}

/// `Game::ShortWordNotice` — opcode 3601: {u4 value; u2 first short;
/// u4 second value; u4 third value; u4 fourth value}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortWordNoticePacket {
    pub value: u32,
    pub first_short: u16,
    pub second_value: u32,
    pub third_value: u32,
    pub fourth_value: u32,
}

impl ShortWordNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NOTICE_3601);
        writer.write_u32(self.value);
        writer.write_u16(self.first_short);
        writer.write_u32(self.second_value);
        writer.write_u32(self.third_value);
        writer.write_u32(self.fourth_value);
        writer.finalize()
    }
}

/// `Game::WideFieldNotice` — opcode 3512: {u4 value; u8 wide value;
/// u2 short; u8 second wide value}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WideFieldNoticePacket {
    pub value: u32,
    pub wide_value: u64,
    pub short: u16,
    pub second_wide_value: u64,
}

impl WideFieldNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NOTICE_3512);
        writer.write_u32(self.value);
        writer.write_u32(self.wide_value as u32);
        writer.write_u32((self.wide_value >> 32) as u32);
        writer.write_u16(self.short);
        writer.write_u32(self.second_wide_value as u32);
        writer.write_u32((self.second_wide_value >> 32) as u32);
        writer.finalize()
    }
}

/// `Item::FlagDoubleWordNotice` — opcode 4132: {u1 flag; u8 value; u4 tail}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagDoubleWordNoticePacket {
    pub flag: u8,
    pub value: u64,
    pub tail: u32,
}

impl FlagDoubleWordNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NOTICE_4132);
        writer.write_u8(self.flag);
        writer.write_u32(self.value as u32);
        writer.write_u32((self.value >> 32) as u32);
        writer.write_u32(self.tail);
        writer.finalize()
    }
}

/// `Item::ExItemMoveResult` — opcode 3801: {u4 result}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExItemMoveResultPacket {
    pub result: u32,
}

impl ExItemMoveResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::RES_EXITEM_MOVE);
        writer.write_u32(self.result);
        writer.finalize()
    }
}

/// `Item::FlagWordNotice` — opcode 4138: {u1 flag; u4 value}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagWordNoticePacket {
    pub flag: u8,
    pub value: u32,
}

impl FlagWordNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NOTICE_4138);
        writer.write_u8(self.flag);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `Item::FlagTwoWordNotice` — opcode 4137: {u1 flag; u4 value; u4 second}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlagTwoWordNoticePacket {
    pub flag: u8,
    pub value: u32,
    pub second_value: u32,
}

impl FlagTwoWordNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NOTICE_4137);
        writer.write_u8(self.flag);
        writer.write_u32(self.value);
        writer.write_u32(self.second_value);
        writer.finalize()
    }
}

/// `Game::TwoWordPair` — opcodes 3501 and 3514: {u4 value; u4 second}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwoWordPairPacket {
    pub opcode: i16,
    pub value: u32,
    pub second_value: u32,
}

impl TwoWordPairPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            matches!(self.opcode, game::RESPONSE_3501 | game::RESPONSE_3514),
            "the two-word pair belongs to opcodes 3501 and 3514"
        );
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_u32(self.value);
        writer.write_u32(self.second_value);
        writer.finalize()
    }
}

/// `Game::FixedBlockNotice` — opcode 16035: {1024-byte block; u4 value}.
///
/// The block is a fixed-size payload the client copies whole, so it stays opaque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedBlockNoticePacket {
    pub block: [u8; 1024],
    pub value: u32,
}

impl FixedBlockNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NOTICE_16035);
        writer.write_bytes(&self.block);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `Game::TwoWordReply` — opcode 3603: {u4 value; u4 second value}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwoWordReplyPacket {
    pub value: u32,
    pub second_value: u32,
}

impl TwoWordReplyPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::RESPONSE_3603);
        writer.write_u32(self.value);
        writer.write_u32(self.second_value);
        writer.finalize()
    }
}

/// `Item::ChangeSkinResult` — opcode 3154: {u1 result}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeSkinResultPacket {
    pub result: u8,
}

impl ChangeSkinResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::RES_CHANGE_SKIN_RESULT);
        writer.write_u8(self.result);
        writer.finalize()
    }
}

/// `Item::AcquireSkin` — opcode 3152: {u1 flag; u4 skin}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcquireSkinPacket {
    pub flag: u8,
    pub skin: u32,
}

impl AcquireSkinPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NTF_ACQUIRE_SKIN);
        writer.write_u8(self.flag);
        writer.write_u32(self.skin);
        writer.finalize()
    }
}

/// `Party::RecruitMember` — one recruiter member row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRecruitMember {
    pub flag: u8,
    pub tamer: u32,
    pub level: u32,
    pub digimon: u32,
    pub icon: u16,
    pub name: String,
}

impl PartyRecruitMember {
    fn encode_into(&self, writer: &mut PacketWriter) {
        writer.write_u8(self.flag);
        writer.write_u32(self.tamer);
        writer.write_u32(self.level);
        writer.write_u32(self.digimon);
        writer.write_u16(self.icon);
        writer.write_modern_utf8_string(&self.name);
    }
}

/// `Party::RecruitMemberNotice` — opcodes 2506 and 2517: one member row.
///
/// Both notices read exactly one member, not a counted list, so a server that
/// prefixed a count would push every field one byte out of place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRecruitMemberNoticePacket {
    pub opcode: i16,
    pub member: PartyRecruitMember,
}

impl PartyRecruitMemberNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            matches!(
                self.opcode,
                game::PARTY_RECRUIT_NEW_MEMBER | game::PARTY_RECRUIT_SYNC_MEMBER
            ),
            "the single-member body belongs to opcodes 2506 and 2517"
        );
        let mut writer = PacketWriter::new(self.opcode);
        self.member.encode_into(&mut writer);
        writer.finalize()
    }
}

/// `Party::RecruitView` — one row of the recruiter query list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRecruitView {
    pub first_value: u32,
    pub second_value: u32,
    pub tamer: u32,
    pub digimon: u32,
    pub name: String,
    pub flag: u8,
    pub note: String,
    pub tail: u32,
}

impl PartyRecruitView {
    fn encode_into(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.first_value);
        writer.write_u32(self.second_value);
        writer.write_u32(self.tamer);
        writer.write_u32(self.digimon);
        writer.write_modern_utf8_string(&self.name);
        writer.write_u8(self.flag);
        writer.write_modern_utf8_string(&self.note);
        writer.write_u32(self.tail);
    }
}

/// `Party::RecruitQueryList` — opcode 2518: {u4 value; u4 count; u4 value;
/// count x view row; u4 value; u4 value; u4 value}.
///
/// The count is a word, not a byte: the rows are read in a loop bounded by it,
/// and both strings inside a row carry their own word-long length prefix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRecruitQueryListPacket {
    pub value: u32,
    pub second_value: u32,
    pub views: Vec<PartyRecruitView>,
    pub third_value: u32,
    pub fourth_value: u32,
    pub fifth_value: u32,
}

impl PartyRecruitQueryListPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.views.len() <= u32::MAX as usize,
            "recruiter view count overflows u4"
        );
        let mut writer = PacketWriter::new(game::PARTY_RECRUIT_QUERY_LIST);
        writer.write_u32(self.value);
        writer.write_u32(self.second_value);
        writer.write_u32(self.views.len() as u32);
        for view in &self.views {
            view.encode_into(&mut writer);
        }
        writer.write_u32(self.third_value);
        writer.write_u32(self.fourth_value);
        writer.write_u32(self.fifth_value);
        writer.finalize()
    }
}

/// `Item::SkinGroup` — one group of the skin snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkinGroup {
    pub flag: u8,
    pub items: Vec<u32>,
}

/// `Item::InitSkinData` — opcode 3151: {u2 group count; count x group;
/// u4 value}.
///
/// Two levels of counting: the outer count bounds the groups and each group
/// carries its own item count. Both counts are words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitSkinDataPacket {
    pub groups: Vec<SkinGroup>,
    pub tail: u32,
}

impl InitSkinDataPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.groups.len() <= u16::MAX as usize,
            "skin group count overflows u2"
        );
        let mut writer = PacketWriter::new(game::NTF_INIT_SKIN_DATA);
        writer.write_u16(self.groups.len() as u16);
        for group in &self.groups {
            assert!(
                group.items.len() <= u16::MAX as usize,
                "skin item count overflows u2"
            );
            writer.write_u8(group.flag);
            writer.write_u16(group.items.len() as u16);
            for item in &group.items {
                writer.write_u32(*item);
            }
        }
        writer.write_u32(self.tail);
        writer.finalize()
    }
}

/// `Season::Row` — one seasonal row: a double word head and a short tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeasonRow {
    pub value: u64,
    pub tail: u16,
}

/// `Season::Notice` — opcodes 3507 and 3508: {u4 value; u8 value; u4 value;
/// u2 value; u8 value; u2 count; count x row}.
///
/// The count sits after a double word, so a reader that aligned the count to a
/// word boundary would take the wrong two bytes for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonNoticePacket {
    pub opcode: i16,
    pub value: u32,
    pub wide_value: u64,
    pub second_value: u32,
    pub short_value: u16,
    pub second_wide_value: u64,
    pub rows: Vec<SeasonRow>,
}

impl SeasonNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            matches!(
                self.opcode,
                game::NTF_SEASON_3507 | game::NTF_SEASON_3508
            ),
            "the seasonal notice body belongs to opcodes 3507 and 3508"
        );
        assert!(
            self.rows.len() <= u16::MAX as usize,
            "seasonal row count overflows u2"
        );
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_u32(self.value);
        writer.write_u32(self.wide_value as u32);
        writer.write_u32((self.wide_value >> 32) as u32);
        writer.write_u32(self.second_value);
        writer.write_u16(self.short_value);
        writer.write_u32(self.second_wide_value as u32);
        writer.write_u32((self.second_wide_value >> 32) as u32);
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            writer.write_u32(row.value as u32);
            writer.write_u32((row.value >> 32) as u32);
            writer.write_u16(row.tail);
        }
        writer.finalize()
    }
}

/// `Item::CircuitModuleNotice` — opcodes 4504, 4505 and 4506.
///
/// All three open with a flag byte and a slot word; 4506 also carries a double
/// word before the trailing byte run, and the run's count is a word over plain
/// bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircuitModuleNoticePacket {
    pub opcode: i16,
    pub flag: u8,
    pub slot: u32,
    pub second_flag: u8,
    pub wide_value: Option<u64>,
    pub payload: Vec<u8>,
}

impl CircuitModuleNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            matches!(
                self.opcode,
                game::RES_DS_CIRCUIT_MODULE_EQUIP
                    | game::RES_DS_CIRCUIT_MODULE_MOVE
                    | game::RES_DS_CIRCUIT_MODULE_UNEQUIP
            ),
            "the circuit module body belongs to opcodes 4504 to 4506"
        );
        if self.opcode != game::RES_DS_CIRCUIT_MODULE_UNEQUIP {
            assert!(
                self.wide_value.is_none(),
                "only the unequip body carries a double word"
            );
        }
        assert!(
            self.payload.len() <= u16::MAX as usize,
            "circuit payload count overflows u2"
        );
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_u8(self.flag);
        writer.write_u32(self.slot);
        writer.write_u8(self.second_flag);
        if let Some(wide_value) = self.wide_value {
            writer.write_u32(wide_value as u32);
            writer.write_u32((wide_value >> 32) as u32);
        }
        writer.write_u16(self.payload.len() as u16);
        writer.write_bytes(&self.payload);
        writer.finalize()
    }
}

/// `Item::CreateBreakthroughData` — opcode 3672: {u4 item; u4 flag;
/// {u2 count; count x u4 value}}.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateBreakthroughDataPacket {
    pub item: u32,
    pub flag: u32,
    pub options: Vec<u32>,
}

impl CreateBreakthroughDataPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.options.len() <= u16::MAX as usize,
            "breakthrough option count overflows u2"
        );
        let mut writer = PacketWriter::new(game::RES_CREATE_BREAKTHROUGH_DATA);
        writer.write_u32(self.item);
        writer.write_u32(self.flag);
        writer.write_u16(self.options.len() as u16);
        for option in &self.options {
            writer.write_u32(*option);
        }
        writer.finalize()
    }
}

/// `Account::SyncVerification` — opcode 16072: {u2 count; count x u4 value}.
///
/// The count bounds a run of plain words with no per-row nesting, so the body is
/// reachable without following a virtual call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncVerificationPacket {
    pub values: Vec<u32>,
}

impl SyncVerificationPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.values.len() <= u16::MAX as usize,
            "sync verification count overflows u2"
        );
        let mut writer = PacketWriter::new(game::RES_SYNC_VERIFICATION);
        writer.write_u16(self.values.len() as u16);
        for value in &self.values {
            writer.write_u32(*value);
        }
        writer.finalize()
    }
}

/// `Mail::DeleteRun` — one counted run of mail references.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailDeleteRun {
    pub entries: Vec<(u32, u32)>,
}

impl MailDeleteRun {
    fn encode_into(&self, writer: &mut PacketWriter) {
        assert!(
            self.entries.len() <= u16::MAX as usize,
            "mail delete run overflows u2"
        );
        writer.write_u16(self.entries.len() as u16);
        for (first, second) in &self.entries {
            writer.write_u32(*first);
            writer.write_u32(*second);
        }
    }
}

/// `Mail::DeleteResult` — opcode 16054: two counted runs back to back.
///
/// Each run is its own count followed by that many pairs, so a single count for
/// the whole body would leave the second run read as list rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailDeleteResultPacket {
    pub first: MailDeleteRun,
    pub second: MailDeleteRun,
}

impl MailDeleteResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::RES_MAIL_DELETE);
        self.first.encode_into(&mut writer);
        self.second.encode_into(&mut writer);
        writer.finalize()
    }
}

/// `Dungeon::RankingStageClear` — opcode 14059: {u2 stage; u1 flag; u4 value;
/// u4 value; u2 value}.
///
/// The body opens with a short and then a single byte, so a reader expecting
/// words would take the stage and flag as one field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingStageClearPacket {
    pub stage: u16,
    pub flag: u8,
    pub value: u32,
    pub second_value: u32,
    pub tail: u16,
}

impl RankingStageClearPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NTF_RANKING_STAGE_CLEAR);
        writer.write_u16(self.stage);
        writer.write_u8(self.flag);
        writer.write_u32(self.value);
        writer.write_u32(self.second_value);
        writer.write_u16(self.tail);
        writer.finalize()
    }
}

/// `Party::RecruitReply` — opcodes 2501, 2503, 2508, 2510, 2511 and 2514:
/// {u4 result}.
///
/// The recruiter operations all answer with one result word, so one type covers
/// them and the opcode selects which operation the client reports on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyRecruitReplyPacket {
    pub opcode: i16,
    pub result: u32,
}

impl PartyRecruitReplyPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            matches!(
                self.opcode,
                game::PARTY_RECRUIT_MAKE
                    | game::PARTY_RECRUIT_APPLY
                    | game::PARTY_RECRUIT_CHAT
                    | game::PARTY_RECRUIT_KICKOUT
                    | game::PARTY_RECRUIT_WITHDRAW
                    | game::PARTY_RECRUIT_CREATE_PARTY
                    | game::REPLY_2513
            ),
            "the single-result body belongs to the recruiter reply opcodes"
        );
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_u32(self.result);
        writer.finalize()
    }
}

/// `Party::RecruitLeaveMember` — opcode 2512: {u1 type; u4 member}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartyRecruitLeaveMemberPacket {
    pub member_type: u8,
    pub member: u32,
}

impl PartyRecruitLeaveMemberPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_RECRUIT_LEAVE_MEMBER);
        writer.write_u8(self.member_type);
        writer.write_u32(self.member);
        writer.finalize()
    }
}

/// `Game::RawNotice` — notices whose body is a single fixed-width run.
///
/// 3305, 5354, 3172 and 14058 carry one word; 16055 carries a double word; 9851
/// carries a fixed 32-byte run the client copies verbatim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawNoticeBody {
    Word(u32),
    DoubleWord(u64),
    Fixed32([u8; 32]),
}

/// `Game::RawNotice` — see [`RawNoticeBody`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawNoticePacket {
    pub opcode: i16,
    pub body: RawNoticeBody,
}

impl RawNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(self.opcode);
        match self.body {
            RawNoticeBody::Word(value) => {
                assert!(
                    matches!(
                        self.opcode,
                        game::NOTICE_3305
                            | game::NOTICE_5354
                            | game::NOTICE_3172
                            | game::NOTICE_14058
                    ),
                    "the word body belongs to opcodes 3305, 5354, 3172 and 14058"
                );
                writer.write_u32(value);
            }
            RawNoticeBody::DoubleWord(value) => {
                assert!(
                    self.opcode == game::NOTICE_16055,
                    "the double-word body belongs to opcode 16055"
                );
                writer.write_u32(value as u32);
                writer.write_u32((value >> 32) as u32);
            }
            RawNoticeBody::Fixed32(block) => {
                assert!(
                    self.opcode == game::NOTICE_9851,
                    "the 32-byte body belongs to opcode 9851"
                );
                writer.write_bytes(&block);
            }
        }
        writer.finalize()
    }
}

/// `Game::HalfWordNotice` — opcode 10003: {u2 value}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HalfWordNoticePacket {
    pub value: u16,
}

impl HalfWordNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::NOTICE_10003);
        writer.write_u16(self.value);
        writer.finalize()
    }
}

/// `Game::ProgressNotice` — opcode 905: {u4 value; u8 value}.
///
/// Both fields are unbound, so they keep positional names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressNoticePacket {
    pub value: u32,
    pub second_value: u64,
}

impl ProgressNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PROGRESS_NOTICE_905);
        writer.write_u32(self.value);
        writer.write_u32(self.second_value as u32);
        writer.write_u32((self.second_value >> 32) as u32);
        writer.finalize()
    }
}

/// `Game::SkillApplyAround` — opcode 1102: {u1 tag; tag body}.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillApplyAroundPacket {
    pub body: SkillApplyAroundBody,
}

impl SkillApplyAroundPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_APPLY_AROUND);
        match &self.body {
            SkillApplyAroundBody::Melee {
                caster,
                skill_id,
                target,
                flag,
                caster_block,
                caster_stats,
                target_stats,
                tail,
            } => {
                assert!(
                    (caster_block.is_some() == (*caster != 0)),
                    "the caster block rides only when the caster is nonzero"
                );
                assert!(
                    caster_stats.iter().all(|row| row.stat < 18)
                        && target_stats.iter().all(|row| row.stat < 18),
                    "a stat row at or above 18 would terminate the chain early"
                );
                writer.write_u8(0);
                writer.write_u32(*caster);
                writer.write_u32(*skill_id);
                writer.write_u32(*target);
                writer.write_u8(*flag);
                if let Some(block) = caster_block {
                    writer.write_bytes(block);
                }
                writer.write_u8(1); // one-byte field between the block and the chain
                for row in caster_stats {
                    writer.write_u32(row.value as u32);
                    writer.write_u32(row.extra);
                    writer.write_u8(row.stat);
                }
                writer.write_u8(18); // caster chain terminator
                for row in target_stats {
                    writer.write_u32(row.value as u32);
                    writer.write_u32(row.extra);
                    writer.write_u8(row.stat);
                }
                writer.write_u8(18); // target chain terminator
                writer.write_u32(*tail);
            }
            SkillApplyAroundBody::Dot { source, flag, rows } => {
                assert!(rows.len() <= u8::MAX as usize, "dot rows overflow u1");
                assert!(
                    rows.iter().all(|row| row.stat < 18),
                    "a dot row stat at or above 18 would terminate the chain early"
                );
                writer.write_u8(2);
                writer.write_u32(*source);
                writer.write_u8(*flag);
                writer.write_u8(rows.len() as u8);
                for row in rows {
                    writer.write_u32(row.handler);
                    writer.write_u16(row.buff);
                    writer.write_u8(row.stat);
                    writer.write_u32(row.value as u32);
                    writer.write_u32(row.second_value);
                }
            }
            SkillApplyAroundBody::Scene { source, index } => {
                writer.write_u8(3);
                writer.write_u32(*source);
                writer.write_u32(*index);
            }
        }
        writer.finalize()
    }
}

/// One member row of a dungeon schedule entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DungeonScheduleMember {
    pub name: String,
    pub value: u64,
}

/// One schedule row of `Msg::DungeonScheduleList`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DungeonScheduleEntry {
    pub kind: u8,
    pub value: u64,
    pub second_value: u32,
    pub short_value: u16,
    pub title: String,
    pub third_value: u32,
    pub second_short: u16,
    pub note: String,
    pub members: Vec<DungeonScheduleMember>,
}

/// Opaque 69-byte row of `Msg::DungeonScheduleList`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonScheduleRow {
    pub head: u16,
    pub tail: u16,
    pub body: [u8; 69],
}

/// One reward row of `Msg::DungeonScheduleList`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonScheduleGift {
    pub value: u32,
    pub item: u16,
}

/// One row of `User::SettingData`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSettingEntry {
    pub prefix: u16,
    pub value: String,
}

/// `User::SettingData` — opcode 60001: {u2 count; count x {u2 prefix;
/// modern string}}.
///
/// The text is the length-prefixed UTF-8 form, so the row has no terminator and
/// the next row starts right after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserSettingDataPacket {
    pub entries: Vec<UserSettingEntry>,
}

impl UserSettingDataPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::USER_SETTING_DATA);
        assert!(
            self.entries.len() <= u16::MAX as usize,
            "user setting rows overflow u2"
        );
        writer.write_u16(self.entries.len() as u16);
        for entry in &self.entries {
            writer.write_u16(entry.prefix);
            writer.write_modern_utf8_string(&entry.value);
        }
        writer.finalize()
    }
}

/// `Item::CashBalanceNotice` — opcode 1056: {u4 value}.
///
/// The client only opens its cash-shop refresh when the value is 1, so the
/// server sends anything else to make the notice inert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CashBalanceNoticePacket {
    pub value: u32,
}

impl CashBalanceNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::CASH_BALANCE_NOTICE);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// One row of `Item::MakeLimitData`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemMakeLimitEntry {
    pub value: u32,
    pub limit: u32,
}

/// `Item::MakeLimitData` — opcode 5301: {u2 count; count x {u4 value; u4 limit}}.
///
/// The count is unsigned here, unlike the msg-scope notices, so a full 65,535
/// rows stays representable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemMakeLimitDataPacket {
    pub entries: Vec<ItemMakeLimitEntry>,
}

impl ItemMakeLimitDataPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_MAKE_LIMIT_DATA);
        assert!(
            self.entries.len() <= u16::MAX as usize,
            "make-limit rows overflow u2"
        );
        writer.write_u16(self.entries.len() as u16);
        for entry in &self.entries {
            writer.write_u32(entry.value);
            writer.write_u32(entry.limit);
        }
        writer.finalize()
    }
}

/// `Item::MakeLimitReset` — opcode 5302: {u2 count; count x u4}.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MakeLimitResetPacket {
    pub values: Vec<u32>,
}

impl MakeLimitResetPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MAKE_LIMIT_RESET);
        assert!(
            self.values.len() <= u16::MAX as usize,
            "reset rows overflow u2"
        );
        writer.write_u16(self.values.len() as u16);
        for value in &self.values {
            writer.write_u32(*value);
        }
        writer.finalize()
    }
}

/// `Item::RefreshMacroProtector` — opcode 4145: no payload.
pub fn encode_refresh_macro_protector() -> Vec<u8> {
    PacketWriter::new(game::REFRESH_MACRO_PROTECTOR).finalize()
}

/// `Msg::DungeonScheduleList` — opcode 14010: {u4 dungeon id; u8 value; u8 value;
/// u1 clear count; clear count x {u1 flag; u4 value};
/// u1 schedule count; schedule count x {u1 kind; u8 value; u4 value; u2 value;
/// entry title; u4 value; u2 value; entry note; u1 member count;
/// member count x {member name; u8 value}};
/// u1 row count; row count x {u2 head; u2 tail; 69B body};
/// u1 gift count; gift count x {u4 value; u2 item}}.
///
/// The nested counts are signed bytes on the wire, which bounds each list at
/// 127 rows. Text fields use the length-prefixed UTF-8 form the modern client
/// reads, not the legacy terminated form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DungeonScheduleListPacket {
    pub dungeon_id: u32,
    pub value: u64,
    pub second_value: u64,
    pub clears: Vec<DungeonClearEntry>,
    pub schedules: Vec<DungeonScheduleEntry>,
    pub rows: Vec<DungeonScheduleRow>,
    pub gifts: Vec<DungeonScheduleGift>,
}

impl DungeonScheduleListPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MSG_DUNGEON_SCHEDULE_LIST);
        writer.write_u32(self.dungeon_id);
        writer.write_u64(self.value);
        writer.write_u64(self.second_value);

        assert!(
            self.clears.len() <= i8::MAX as usize
                && self.schedules.len() <= i8::MAX as usize
                && self.rows.len() <= i8::MAX as usize
                && self.gifts.len() <= i8::MAX as usize,
            "dungeon schedule lists overflow a signed u1"
        );

        writer.write_u8(self.clears.len() as u8);
        for entry in &self.clears {
            writer.write_u8(entry.flag);
            writer.write_u32(entry.value);
        }

        writer.write_u8(self.schedules.len() as u8);
        for schedule in &self.schedules {
            assert!(
                schedule.members.len() <= i8::MAX as usize,
                "dungeon schedule members overflow a signed u1"
            );
            writer.write_u8(schedule.kind);
            writer.write_u64(schedule.value);
            writer.write_u32(schedule.second_value);
            writer.write_u16(schedule.short_value);
            writer.write_modern_utf8_string(&schedule.title);
            writer.write_u32(schedule.third_value);
            writer.write_u16(schedule.second_short);
            writer.write_modern_utf8_string(&schedule.note);
            writer.write_u8(schedule.members.len() as u8);
            for member in &schedule.members {
                writer.write_modern_utf8_string(&member.name);
                writer.write_u64(member.value);
            }
        }

        writer.write_u8(self.rows.len() as u8);
        for row in &self.rows {
            writer.write_u16(row.head);
            writer.write_u16(row.tail);
            writer.write_bytes(&row.body);
        }

        writer.write_u8(self.gifts.len() as u8);
        for gift in &self.gifts {
            writer.write_u32(gift.value);
            writer.write_u16(gift.item);
        }

        writer.finalize()
    }
}

/// `Msg::DungeonStageInfo` — opcode 14001: {u4 slot; u4 id; modern string;
/// u2 value; u4 value; u8 value}.
///
/// The text sits between the fixed words rather than at the end, so the trailing
/// fields only line up when it is written as a length-prefixed string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DungeonStageInfoPacket {
    pub slot: u32,
    pub id: u32,
    pub name: String,
    pub third: u16,
    pub fourth: u32,
    pub fifth: u64,
}

impl DungeonStageInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MSG_DUNGEON_STAGE_INFO);
        writer.write_u32(self.slot);
        writer.write_u32(self.id);
        writer.write_modern_utf8_string(&self.name);
        writer.write_u16(self.third);
        writer.write_u32(self.fourth);
        writer.write_u32(self.fifth as u32);
        writer.write_u32((self.fifth >> 32) as u32);
        writer.finalize()
    }
}

/// `Msg::DungeonMatchState` — opcode 14007: {u4 value; u1 flag; u1 flag;
/// u4 value; u2 value}.
///
/// Four of the five fields are unbound, so they keep positional names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonMatchStatePacket {
    pub value: u32,
    pub flag: u8,
    pub second_flag: u8,
    pub second_value: u32,
    pub trailing: u16,
}

impl DungeonMatchStatePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MSG_DUNGEON_MATCH_STATE);
        writer.write_u32(self.value);
        writer.write_u8(self.flag);
        writer.write_u8(self.second_flag);
        writer.write_u32(self.second_value);
        writer.write_u16(self.trailing);
        writer.finalize()
    }
}

/// `Msg::DungeonMatchResult` — opcode 14008: {u4 value; u1 result; u4 value}.
///
/// The middle byte is the match result; the two u4 fields stay unbound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonMatchResultPacket {
    pub value: u32,
    pub result: u8,
    pub second_value: u32,
}

impl DungeonMatchResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MSG_DUNGEON_MATCH_RESULT);
        writer.write_u32(self.value);
        writer.write_u8(self.result);
        writer.write_u32(self.second_value);
        writer.finalize()
    }
}

/// `Msg::DungeonMatchCounter` — opcode 14009: {u4 value; u8 value; u8 value}.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonMatchCounterPacket {
    pub value: u32,
    pub first: u64,
    pub second: u64,
}

impl DungeonMatchCounterPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MSG_DUNGEON_MATCH_COUNTER);
        writer.write_u32(self.value);
        writer.write_u32(self.first as u32);
        writer.write_u32((self.first >> 32) as u32);
        writer.write_u32(self.second as u32);
        writer.write_u32((self.second >> 32) as u32);
        writer.finalize()
    }
}

/// `Msg::DungeonMatchEnd` — opcode 14005: no payload.
///
/// The notice only signals that the matchmaking session ended; the client reads
/// nothing from the frame.
pub fn encode_dungeon_match_end() -> Vec<u8> {
    PacketWriter::new(game::MSG_DUNGEON_MATCH_END).finalize()
}

/// `Msg::DungeonMatchCancel` — opcode 14015: no payload.
pub fn encode_dungeon_match_cancel() -> Vec<u8> {
    PacketWriter::new(game::MSG_DUNGEON_MATCH_CANCEL).finalize()
}

/// `Msg::DungeonReEnterResult` — opcode 14016: {u1 result}.
///
/// The client stores the flag and fires one notification per frame; no other
/// field rides the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonReEnterResultPacket {
    pub result: u8,
}

impl DungeonReEnterResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MSG_DUNGEON_RE_ENTER_RESULT);
        writer.write_u8(self.result);
        writer.finalize()
    }
}

/// One stage row of `Msg::RankingPlayProgress`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RankingPlayProgressStage {
    pub stage: u16,
    pub wave: u16,
    pub value: u32,
}

/// `Msg::RankingPlayProgress` — opcode 14057: {u2 dungeon; u4 value; u1 flag;
/// u4 value; u2 value; u4 value; u2 value; u4 value; u2 count;
/// count x {u2 stage; u2 wave; u4 value}}.
///
/// Seven fixed fields precede the stage list; their individual meanings are not
/// yet bound, so they keep positional names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankingPlayProgressPacket {
    pub dungeon_id: u16,
    /// Positional run after the dungeon id: u4, u1, u4, u2, u4, u2.
    pub fixed: [u32; 6],
    pub stages: Vec<RankingPlayProgressStage>,
}

impl RankingPlayProgressPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MSG_RANKING_PLAY_PROGRESS);
        writer.write_u16(self.dungeon_id);
        writer.write_u32(self.fixed[0]);
        writer.write_u8(self.fixed[1] as u8);
        writer.write_u32(self.fixed[2]);
        writer.write_u16(self.fixed[3] as u16);
        writer.write_u32(self.fixed[4]);
        writer.write_u16(self.fixed[5] as u16);
        assert!(
            self.stages.len() <= u16::MAX as usize,
            "ranking stages overflow u2"
        );
        writer.write_u16(self.stages.len() as u16);
        for stage in &self.stages {
            writer.write_u16(stage.stage);
            writer.write_u16(stage.wave);
            writer.write_u32(stage.value);
        }
        writer.finalize()
    }
}

/// Body of the `GameTwo::AllMsg` announcement selected by its case word.
///
/// The case is a signed 16-bit selector and only four values carry a body; any
/// other value leaves the rest of the payload untouched. Each body is written in
/// the order the announcement consumer expects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameTwoAllMsgBody {
    /// Case 1: a tamer used a level-up item; the announcement names the tamer,
    /// the item row and the resulting level.
    ItemLevelUp {
        tamer: String, // modern utf8 string, u16 length prefix
        item_id: u32,  // CItemTable row key
        level: u32,
    },
    /// Case 1605: a raid monster died. An item id of zero makes the client
    /// substitute its own peace text instead of an item name.
    RaidMonsterDie {
        monster_id: u32, // CMonsterTable row key
        tamer: String,   // modern utf8 string, u16 length prefix
        digimon_id: u32, // CDigimonTable row key
        item_id: u32,    // CItemTable row key, 0 = no item
    },
    /// Case 1606: a random-regen monster appeared on the map.
    RandomMonsterCreate { monster_id: u32, map_id: u32 },
    /// Case 1609: a random-regen monster was removed from the map.
    RandomMonsterEnd { monster_id: u32, map_id: u32 },
}

/// `GameTwo::AllMsg` — opcode 16006: {s2 case; case body}.
///
/// The monster, item and digimon names are resolved from the file tables by the
/// consumer; a missing row falls back to its own "unknown" text. The finished
/// announcement goes to the global message window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameTwoAllMsgPacket {
    pub body: GameTwoAllMsgBody,
}

impl GameTwoAllMsgPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_ALL_MSG);
        match &self.body {
            GameTwoAllMsgBody::ItemLevelUp {
                tamer,
                item_id,
                level,
            } => {
                writer.write_u16(1);
                writer.write_modern_utf8_string(tamer);
                writer.write_u32(*item_id);
                writer.write_u32(*level);
            }
            GameTwoAllMsgBody::RaidMonsterDie {
                monster_id,
                tamer,
                digimon_id,
                item_id,
            } => {
                writer.write_u16(1605);
                writer.write_u32(*monster_id);
                writer.write_modern_utf8_string(tamer);
                writer.write_u32(*digimon_id);
                writer.write_u32(*item_id);
            }
            GameTwoAllMsgBody::RandomMonsterCreate { monster_id, map_id } => {
                writer.write_u16(1606);
                writer.write_u32(*monster_id);
                writer.write_u32(*map_id);
            }
            GameTwoAllMsgBody::RandomMonsterEnd { monster_id, map_id } => {
                writer.write_u16(1609);
                writer.write_u32(*monster_id);
                writer.write_u32(*map_id);
            }
        }
        writer.finalize()
    }
}

/// One hit entry of a `GameTwo::MonsterAreaAttack` broadcast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonsterAreaTarget {
    pub first: u32,
    pub second: u32,
}

/// Per-monster-kind body of a `GameTwo::MonsterAreaAttack` broadcast.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MonsterAreaBody {
    /// Kind 0: the attack only plays on the caster and carries no rows.
    CasterOnly,
    /// Kind 1: one u4 per target.
    Targets(Vec<u32>),
    /// Kinds 2, 4 and 7: one coordinate/value pair per target.
    TargetPairs(Vec<MonsterAreaTarget>),
}

/// `GameTwo::MonsterAreaAttack` — opcode 16010: {u4 caster; u4 monster id;
/// per-kind body}.
///
/// The body is selected by the monster row's kind field. An unknown kind ends
/// the payload after the header. The row count is a u4 and a zero count ends the
/// packet before any row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameTwoMonsterAreaAttackPacket {
    pub caster: u32,
    pub monster_id: u32, // CMonsterTable row key
    pub body: MonsterAreaBody,
}

impl GameTwoMonsterAreaAttackPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_MONSTER_AREA_ATTACK);
        writer.write_u32(self.caster);
        writer.write_u32(self.monster_id);
        match &self.body {
            MonsterAreaBody::CasterOnly => {}
            MonsterAreaBody::Targets(targets) => {
                assert!(
                    targets.len() <= u32::MAX as usize,
                    "area-attack targets overflow u4"
                );
                writer.write_u32(targets.len() as u32);
                for target in targets {
                    writer.write_u32(*target);
                }
            }
            MonsterAreaBody::TargetPairs(pairs) => {
                assert!(
                    pairs.len() <= u32::MAX as usize,
                    "area-attack rows overflow u4"
                );
                writer.write_u32(pairs.len() as u32);
                for pair in pairs {
                    writer.write_u32(pair.first);
                    writer.write_u32(pair.second);
                }
            }
        }
        writer.finalize()
    }
}

/// `GameTwo::DoorObjectOpen` — opcode 16007: {u4 monster id; u1 opened}.
/// The client logs "ActorObject - MonsterID / Opend" and spawns the door
/// actor from the pair, pinging event 42 with the formatted log string
/// and event 538 with the spawn node.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameTwoDoorObjectOpenPacket {
    pub monster_id: u32,
    pub opened: u8,
}

impl GameTwoDoorObjectOpenPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_DOOR_OBJECT_OPEN);
        writer.write_u32(self.monster_id);
        writer.write_u8(self.opened);
        writer.finalize()
    }
}

/// One guard entry of `GameTwo::GameGuardCheck` — opcode 16011: {u1 kind;
/// u4 c; u4 d; u1 e}. The dispatcher traces c through the guard path and
/// builds the event-666 message from c; unresolved fields keep their
/// accumulator offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameGuardRow {
    pub kind: u8,
    pub c: u32,
    pub d: u32,
    pub e: u8,
}

/// `GameTwo::GameGuardCheck` — opcode 16011: {u4 a; u4 b; u2 count;
/// count x rows}. The leading a is matched against the selected/main
/// monster handler to enable the guard path.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameTwoGameGuardCheckPacket {
    pub a: u32,
    pub b: u32,
    pub rows: Vec<GameGuardRow>,
}

impl GameTwoGameGuardCheckPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_GAME_GUARD_CHECK);
        assert!(
            self.rows.len() <= u16::MAX as usize,
            "game guard rows overflow u2"
        );
        writer.write_u32(self.a);
        writer.write_u32(self.b);
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            writer.write_u8(row.kind);
            writer.write_u32(row.c);
            writer.write_u32(row.d);
            writer.write_u8(row.e);
        }
        writer.finalize()
    }
}

/// `GameTwo::SkillAction` — opcode 16012: {u4 kind}; the client triggers
/// the skill action only when kind is 4.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameTwoSkillActionPacket {
    pub kind: u32,
}

impl GameTwoSkillActionPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_SKILL_ACTION);
        writer.write_u32(self.kind);
        writer.finalize()
    }
}

/// `GameTwo::XGuageUpdate` — opcode 16034: {u2 a; s2 name; u4 b; u4 c};
/// event 423 carries the whole envelope unparsed.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameTwoXGuageUpdatePacket {
    pub a: u16,
    pub name: String, // modern utf8 string, u16 length prefix
    pub b: u32,
    pub c: u32,
}

impl GameTwoXGuageUpdatePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_X_GUAGE_UPDATE);
        writer.write_u16(self.a);
        writer.write_modern_utf8_string(&self.name);
        writer.write_u32(self.b);
        writer.write_u32(self.c);
        writer.finalize()
    }
}

/// `GameTwo::TimeChargeItemList` — opcode 16043: {u4 a; u2 b; u4 c; u4 d;
/// u8 e; u8[69] charge row; u4 f}; the whole envelope flows through
/// event 510. Unresolved fields keep their accumulator offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeChargeItemListPacket {
    pub a: u32,
    pub b: u16,
    pub c: u32,
    pub d: u32,
    pub e: u64,
    pub charge_row: [u8; 69],
    pub f: u32,
}

impl Default for TimeChargeItemListPacket {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            charge_row: [0u8; 69],
            f: 0,
        }
    }
}

impl TimeChargeItemListPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_TIME_CHARGE_ITEM_LIST);
        writer.write_u32(self.a);
        writer.write_u16(self.b);
        writer.write_u32(self.c);
        writer.write_u32(self.d);
        writer.write_u64(self.e);
        writer.write_bytes(&self.charge_row);
        writer.write_u32(self.f);
        writer.finalize()
    }
}

/// One {u2; u2} slot entry of `GameTwo::TimeChargeNormal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeChargeSlot {
    pub e: u16,
    pub f: u16,
}

/// `GameTwo::TimeChargeNormal` — opcode 16044: {u4 a; u4 b; u4 c; u8[69]
/// charge row; u8 d; u2 count; count x slots}; the slot list feeds
/// event 511.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeChargeNormalPacket {
    pub a: u32,
    pub b: u32,
    pub c: u32,
    pub charge_row: [u8; 69],
    pub d: u64,
    pub rows: Vec<TimeChargeSlot>,
}

impl Default for TimeChargeNormalPacket {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            charge_row: [0u8; 69],
            d: 0,
            rows: Vec::new(),
        }
    }
}

impl TimeChargeNormalPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_TIME_CHARGE_NORMAL);
        assert!(
            self.rows.len() <= u16::MAX as usize,
            "time charge slots overflow u2"
        );
        writer.write_u32(self.a);
        writer.write_u32(self.b);
        writer.write_u32(self.c);
        writer.write_bytes(&self.charge_row);
        writer.write_u64(self.d);
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            writer.write_u16(row.e);
            writer.write_u16(row.f);
        }
        writer.finalize()
    }
}

/// `GameTwo::TimeChargeImmediate` — opcode 16045: {u4 a; u2 b; u4 c;
/// u8[69] charge row}; event 513 pops with the envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeChargeImmediatePacket {
    pub a: u32,
    pub b: u16,
    pub c: u32,
    pub charge_row: [u8; 69],
}

impl Default for TimeChargeImmediatePacket {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            charge_row: [0u8; 69],
        }
    }
}

impl TimeChargeImmediatePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_TIME_CHARGE_IMMEDIATE);
        writer.write_u32(self.a);
        writer.write_u16(self.b);
        writer.write_u32(self.c);
        writer.write_bytes(&self.charge_row);
        writer.finalize()
    }
}

/// `GameTwo::TimeChargeCancel` — opcode 16046: {u4 a; u2 b; u4 c; u8[69]
/// charge row}; event 515 pops with the envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeChargeCancelPacket {
    pub a: u32,
    pub b: u16,
    pub c: u32,
    pub charge_row: [u8; 69],
}

impl Default for TimeChargeCancelPacket {
    fn default() -> Self {
        Self {
            a: 0,
            b: 0,
            c: 0,
            charge_row: [0u8; 69],
        }
    }
}

impl TimeChargeCancelPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_TIME_CHARGE_CANCEL);
        writer.write_u32(self.a);
        writer.write_u16(self.b);
        writer.write_u32(self.c);
        writer.write_bytes(&self.charge_row);
        writer.finalize()
    }
}

/// `GameTwo::TimeChargeWithdraw` — opcode 16047: {u4 a; u8 c; u2 d};
/// event 517 pops with the whole envelope.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TimeChargeWithdrawPacket {
    pub a: u32,
    pub c: u64,
    pub d: u16,
}

impl TimeChargeWithdrawPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_TIME_CHARGE_WITHDRAW);
        writer.write_u32(self.a);
        writer.write_u64(self.c);
        writer.write_u16(self.d);
        writer.finalize()
    }
}

/// `MiniEvent::PiemonGameStart` — opcode 5001: {u8 a; u4 b; u4 c};
/// event 75 pops with the three fields.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PiemonGameStartPacket {
    pub a: u64,
    pub b: u32,
    pub c: u32,
}

impl PiemonGameStartPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MINIEVENT_PIEMON_GAME_START);
        writer.write_u64(self.a);
        writer.write_u32(self.b);
        writer.write_u32(self.c);
        writer.finalize()
    }
}

/// `MiniEvent::PiemonGameEnd` — opcode 5002: {u4 value}; the value is
/// forwarded raw through event 76.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PiemonGameEndPacket {
    pub value: u32,
}

impl PiemonGameEndPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MINIEVENT_PIEMON_GAME_END);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `Mgr::PlayTimeSet` — opcode 9715: {u4 skipped}; the parser advances
/// past the u4 without storing or notifying anything.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MgrPlayTimeSetPacket {
    pub skipped: u32,
}

impl MgrPlayTimeSetPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MGR_PLAY_TIME_SET);
        writer.write_u32(self.skipped);
        writer.finalize()
    }
}

/// `Debug::InsufficientRange` — opcode 3002: {u4 first; u4 second}. The
/// first field feeds the #Distance# chat substitution and the second the
/// #Range# one; event 42 pops the formatted chat string.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DebugInsufficientRangePacket {
    pub first: u32,
    pub second: u32,
}

impl DebugInsufficientRangePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DEBUG_INSUFFICIENT_RANGE);
        writer.write_u32(self.first);
        writer.write_u32(self.second);
        writer.finalize()
    }
}

/// `Debug::LoseTarget` — opcode 3001: {u4 skipped}; the u4 is consumed
/// unstored with no store, no event, and no notification.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DebugLoseTargetPacket {
    pub skipped: u32,
}

impl DebugLoseTargetPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DEBUG_LOSE_TARGET);
        writer.write_u32(self.skipped);
        writer.finalize()
    }
}

/// `Scanner::Read` — opcode 2204: {u2 anchor; u4 mode}; the body extends
/// with {u4 value} once mode is neither 0 nor -1 (the microscope link
/// modes) and with a final {u4 value2} when mode is 30001. Unresolved
/// fields keep their read offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScannerReadPacket {
    pub anchor: u16,
    pub mode: u32,
    pub value: Option<u32>,
    pub value2: Option<u32>,
}

impl ScannerReadPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SCANNER_READ);
        writer.write_u16(self.anchor);
        writer.write_u32(self.mode);
        if let Some(value) = self.value {
            writer.write_u32(value);
        }
        if let Some(value) = self.value2 {
            writer.write_u32(value);
        }
        writer.finalize()
    }
}

/// `Scanner::Release` — opcode 2206: {u4 flag; u4 a}; event 591 pops the
/// one-byte {1} notice through the world handler when the flag is
/// nonzero.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScannerReleasePacket {
    pub flag: u32,
    pub a: u32,
}

impl ScannerReleasePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SCANNER_RELEASE);
        writer.write_u32(self.flag);
        writer.write_u32(self.a);
        writer.finalize()
    }
}

/// `Scanner::Error` — opcode 2207: {u4 skipped}; the u4 is consumed
/// unstored, no store, no event, and no notification.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScannerErrorPacket {
    pub skipped: u32,
}

impl ScannerErrorPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SCANNER_ERROR);
        writer.write_u32(self.skipped);
        writer.finalize()
    }
}

/// Unnamed modern op 1342: an empty payload; the parser only drives the
/// singleton label setter at param 35 plus the idle-action tick from
/// persistent client state, never the message body.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unknown1342Packet {}

impl Unknown1342Packet {
    pub fn encode(&self) -> Vec<u8> {
        PacketWriter::new(game::UNKNOWN_1342).finalize()
    }
}

/// Unnamed modern op 1343: an empty payload like 1342; the driver runs
/// with label param 36.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unknown1343Packet {}

impl Unknown1343Packet {
    pub fn encode(&self) -> Vec<u8> {
        PacketWriter::new(game::UNKNOWN_1343).finalize()
    }
}

/// Unnamed modern op 2140 — parser @0x00D95910, event 564.
/// `{u4 a; u8[71] blob; s2 name}`: the reader takes one word, copies 71 raw
/// bytes (17 dwords, one word and one byte), then the text. The block's
/// content and the words' meanings are unresolved, so they stay opaque.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Modern2140Packet {
    pub a: u32,
    pub blob: [u8; 71],
    pub name: String, // modern utf8 string, u16 length prefix
}

impl Default for Modern2140Packet {
    fn default() -> Self {
        Self {
            a: 0,
            blob: [0u8; 71],
            name: String::new(),
        }
    }
}

impl Modern2140Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MODERN_2140);
        writer.write_u32(self.a);
        writer.write_bytes(&self.blob);
        writer.write_modern_utf8_string(&self.name);
        writer.finalize()
    }
}

/// Unnamed modern op 2141 — parser @0x00D956F0, event 565.
/// `{u4 a; s2 name}`: the reader takes one word then the text; the word's
/// meaning is unresolved.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern2141Packet {
    pub a: u32,
    pub name: String, // modern utf8 string, u16 length prefix
}

impl Modern2141Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MODERN_2141);
        writer.write_u32(self.a);
        writer.write_modern_utf8_string(&self.name);
        writer.finalize()
    }
}

/// Unnamed modern op 2143 — parser @0x00D95A50, event 566.
/// `{u1 count; count x u4 value; s2 name}`: the reader takes one count byte,
/// loops count words into a list node each, then the text. The values are
/// unresolved, so they stay opaque.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern2143Packet {
    pub values: Vec<u32>,
    pub name: String, // modern utf8 string, u16 length prefix
}

impl Modern2143Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MODERN_2143);
        writer.write_u8(self.values.len() as u8);
        for value in &self.values {
            writer.write_u32(*value);
        }
        writer.write_modern_utf8_string(&self.name);
        writer.finalize()
    }
}

/// Unnamed modern op 3247: {u4 handler; u2 a; u4 b; u2 skipped; u1 c;
/// u4 d} drives the item-table lookup and either the linked notice or
/// the value toggle, closing with an empty event 476 ping. Unresolved
/// fields keep their read offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unknown3247Packet {
    pub handler: u32,
    pub a: u16, // unresolved field
    pub b: u32, // unresolved field
    pub skipped: u16,
    pub c: u8, // unresolved field
    pub d: u32, // unresolved field
}

impl Unknown3247Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::UNKNOWN_3247);
        writer.write_u32(self.handler);
        writer.write_u16(self.a);
        writer.write_u32(self.b);
        writer.write_u16(self.skipped);
        writer.write_u8(self.c);
        writer.write_u32(self.d);
        writer.finalize()
    }
}

/// Unnamed modern op 3248: {u4 handler; u4 value}; the entity list is
/// scanned for the handler and the same toggler as 3247 runs, with an
/// empty event 476 ping when the main monster matches.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unknown3248Packet {
    pub handler: u32,
    pub value: u32,
}

impl Unknown3248Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::UNKNOWN_3248);
        writer.write_u32(self.handler);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// Unnamed modern op 3249: {u4 a; u4 b; u1 c; u1 d; u1 e; u1 f} drives
/// the evolution-list mutation and pops empty event 479. Unresolved
/// fields keep their read offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unknown3249Packet {
    pub a: u32, // unresolved field
    pub b: u32, // unresolved field
    pub c: u8,  // unresolved field
    pub d: u8,  // unresolved field
    pub e: u8,  // unresolved field
    pub f: u8,  // unresolved field
}

impl Unknown3249Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::UNKNOWN_3249);
        writer.write_u32(self.a);
        writer.write_u32(self.b);
        writer.write_u8(self.c);
        writer.write_u8(self.d);
        writer.write_u8(self.e);
        writer.write_u8(self.f);
        writer.finalize()
    }
}

/// Row of modern op 3415: {u4 c; u2 d}. Row fields are unresolved; they
/// keep their read offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern3415Row {
    pub c: u32, // unresolved field
    pub d: u16, // unresolved field
}

/// Unnamed modern op 3415: {u4 a; u4 b; u2 count; count x rows} drives
/// the 16-byte list build and pops event 438.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern3415Packet {
    pub a: u32, // unresolved field
    pub b: u32, // unresolved field
    pub rows: Vec<Modern3415Row>,
}

impl Modern3415Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MODERN_3415);
        writer.write_u32(self.a);
        writer.write_u32(self.b);
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            writer.write_u32(row.c);
            writer.write_u16(row.d);
        }
        writer.finalize()
    }
}

/// Row of modern op 3417: {u4 c; u4 d; u4 e; u1 f; u2 g; u2 h; u2 i;
/// u1 j; u1 k; u1 l; u1 m; u1 n} built into the 36-byte list node. Row
/// fields are unresolved; they keep their read offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern3417Row {
    pub c: u32, // unresolved field
    pub d: u32, // unresolved field
    pub e: u32, // unresolved field
    pub f: u8,  // unresolved field
    pub g: u16, // unresolved field
    pub h: u16, // unresolved field
    pub i: u16, // unresolved field
    pub j: u8,  // unresolved field
    pub k: u8,  // unresolved field
    pub l: u8,  // unresolved field
    pub m: u8,  // unresolved field
    pub n: u8,  // unresolved field
}

impl Modern3417Row {
    pub fn encode_into(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.c);
        writer.write_u32(self.d);
        writer.write_u32(self.e);
        writer.write_u8(self.f);
        writer.write_u16(self.g);
        writer.write_u16(self.h);
        writer.write_u16(self.i);
        writer.write_u8(self.j);
        writer.write_u8(self.k);
        writer.write_u8(self.l);
        writer.write_u8(self.m);
        writer.write_u8(self.n);
    }
}

/// Unnamed modern op 3417: {u4 a; u4 b; u2 count; count x rows} drives
/// the 24-byte-row list build and pops event 439.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern3417Packet {
    pub a: u32, // unresolved field
    pub b: u32, // unresolved field
    pub rows: Vec<Modern3417Row>,
}

impl Modern3417Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MODERN_3417);
        writer.write_u32(self.a);
        writer.write_u32(self.b);
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            row.encode_into(&mut writer);
        }
        writer.finalize()
    }
}

/// Row of modern op 3418: {u4 a; u2 b}. Row fields are unresolved; they
/// keep their read offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern3418Row {
    pub a: u32, // unresolved field
    pub b: u16, // unresolved field
}

/// Unnamed modern op 3418: {u2 count; count x rows} drives the 16-byte
/// list build and pops event 440.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern3418Packet {
    pub rows: Vec<Modern3418Row>,
}

impl Modern3418Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MODERN_3418);
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            writer.write_u32(row.a);
            writer.write_u16(row.b);
        }
        writer.finalize()
    }
}

/// Unnamed modern op 3419: {u4 a; u2 count; count x {u4 b}} drives the
/// 16-byte list build of bare u4 entries and pops event 441.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern3419Packet {
    pub a: u32, // unresolved field
    pub entries: Vec<u32>,
}

impl Modern3419Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MODERN_3419);
        writer.write_u32(self.a);
        writer.write_u16(self.entries.len() as u16);
        for entry in &self.entries {
            writer.write_u32(*entry);
        }
        writer.finalize()
    }
}

/// Chain entry of `GameTwo::RaidSkillChain` (opcode 16027).
///
/// Every entry rides one opcode and one coordinate pair that the client uses to
/// spawn the raid-skill effect on the target entity; the pair is signed on the
/// wire so the client can express a negative placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaidSkillChainPosition {
    pub x: i32,
    pub y: i32,
}

/// One link of a `GameTwo::RaidSkillChain` walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaidSkillChainEntry {
    pub value: u32, // accumulated per link; feeds the effect strength
    pub next: u32,  // next link, 0 closes the chain
    pub position: RaidSkillChainPosition,
}

/// `GameTwo::RaidSkillChain` — opcode 16027: {u4 handler; u4 skill id;
/// u2 entry count; entry count x {u4 value; u4 next; i32 x; i32 y}}.
///
/// The last entry closes the chain with a zero `next`, so the entry count is the
/// chain length rather than an independent field. The walk starts at the first
/// entry and stops when `next` is zero.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameTwoRaidSkillChainPacket {
    pub handler: u32,
    pub skill_id: u32, // resolved against the monster row the handler carries
    pub entries: Vec<RaidSkillChainEntry>,
}

impl GameTwoRaidSkillChainPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_RAID_SKILL_CHAIN);
        assert!(
            self.entries.len() <= u16::MAX as usize,
            "raid chain entries overflow u2"
        );
        writer.write_u32(self.handler);
        writer.write_u32(self.skill_id);
        writer.write_u16(self.entries.len() as u16);
        for entry in &self.entries {
            writer.write_u32(entry.value);
            writer.write_u32(entry.next);
            writer.write_u32(entry.position.x as u32);
            writer.write_u32(entry.position.y as u32);
        }
        writer.finalize()
    }
}

/// One hall-of-fame row of `GameTwo::ArenaFameList` (opcode 16025).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArenaFameRow {
    pub rank: u8,
    pub name: String,  // modern utf8 string, u16 length prefix
    pub guild: String, // modern utf8 string, u16 length prefix
    pub point: u32,
}

/// `GameTwo::ArenaFameList` — opcode 16025: {u2 season; u1 result; u1 count;
/// count x {u1 rank; s2 name; s2 guild; u4 point}}.
///
/// One list node is built per row and event 387 fires with the season and result
/// attached to the whole list. A season of zero asks for the current season.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArenaFameListPacket {
    pub season: u16,
    pub result: u8,
    pub rows: Vec<ArenaFameRow>,
}

impl ArenaFameListPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ARENA_REQUEST_FAME);
        assert!(self.rows.len() <= u8::MAX as usize, "fame rows overflow u1");
        writer.write_u16(self.season);
        writer.write_u8(self.result);
        writer.write_u8(self.rows.len() as u8);
        for row in &self.rows {
            writer.write_u8(row.rank);
            writer.write_modern_utf8_string(&row.name);
            writer.write_modern_utf8_string(&row.guild);
            writer.write_u32(row.point);
        }
        writer.finalize()
    }
}

/// RegionBuff: {u4 a; u8 blob; u1 flag; u2 count; count x 69-byte
/// rows}; the flag routes the buffer (0 -> event 464, 2 or 9 -> event
/// 465) and the 69-byte blocks stay opaque; unresolved content keeps
/// its read offsets.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GamTwoRegionBuffPacket {
    pub a: u32,
    pub blob: [u8; 8],
    pub flag: u8,
    pub rows: Vec<[u8; 69]>,
}

impl GamTwoRegionBuffPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAMETWO_REGION_BUFF);
        writer.write_u32(self.a);
        writer.write_bytes(&self.blob);
        writer.write_u8(self.flag);
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            writer.write_bytes(row);
        }
        writer.finalize()
    }
}

/// Unnamed modern op 3990: {u1 flag}; event 469 pops with the pair.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Unknown3990Packet {
    pub flag: u8,
}

impl Unknown3990Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::UNKNOWN_3990);
        writer.write_u8(self.flag);
        writer.finalize()
    }
}

/// Row of modern op 3420; the read shape matches the 3417 24-byte row
/// exactly, so the type is reused under a distinct name.
pub type Modern3420Row = Modern3417Row;

/// Unnamed modern op 3420: {u4 a; u2 count; count x rows} shares the
/// 3417 row build and pops event 442.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Modern3420Packet {
    pub a: u32, // unresolved field
    pub rows: Vec<Modern3420Row>,
}

impl Modern3420Packet {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MODERN_3420);
        writer.write_u32(self.a);
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            row.encode_into(&mut writer);
        }
        writer.finalize()
    }
}

/// One slot/value row of the pinochi item-delete flows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PinochiDeleteRow {
    pub slot: u16,  // table row index (<= 198)
    pub value: u16, // decrement / delete amount
}

/// One counted {slot; value} block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PinochiDeleteList {
    pub rows: Vec<PinochiDeleteRow>,
}

impl PinochiDeleteList {
    fn encode_on(&self, writer: &mut PacketWriter) {
        assert!(self.rows.len() <= u8::MAX as usize, "delete rows overflow u2");
        writer.write_u16(self.rows.len() as u16);
        for row in &self.rows {
            writer.write_u16(row.slot);
            writer.write_u32(row.value as u32);
        }
    }
}

/// `PinochiItemDelete` — opcode 3128: {u4 key; three counted blocks of
/// {u2 slot; u4 value}}. Block 1 decrements digimon skill-table rows
/// (zeroing the 69-byte row on underflow), blocks 2 and 3 are limit-checked
/// and popped to events 59 and 60 respectively; the tail walks the
/// special-event NPC list and refreshes the message box.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PinochiItemDeletePacket {
    pub key: u32, // unresolved NPC/table key
    pub lists: [PinochiDeleteList; 3],
}

impl PinochiItemDeletePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::EVENT_PINOCHI_ITEM_DELETE);
        writer.write_u32(self.key);
        for list in &self.lists {
            list.encode_on(&mut writer);
        }
        writer.finalize()
    }
}

/// One user row of the master user list.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterUserRow {
    pub first: String,  // account-ish column
    pub second: String, // character-ish column, unresolved
}

/// `Master::UserList` — opcode 9922: {u2 count; count x {s2; s2}}; each row
/// appends a "{index:03} {first} {second}" line to `uList.txt` and pops it
/// through event 42; the tail opens the file in notepad.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MasterUserListPacket {
    pub users: Vec<MasterUserRow>,
}

impl MasterUserListPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(
            self.users.len() <= u16::MAX as usize,
            "user rows overflow u2"
        );
        let mut writer = PacketWriter::new(game::MASTER_USER_LIST);
        writer.write_u16(self.users.len() as u16);
        for user in &self.users {
            writer.write_modern_utf8_string(&user.first);
            writer.write_modern_utf8_string(&user.second);
        }
        writer.finalize()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InventoryType {
    Inventory = 0,
    Warehouse = 1,
    AccountWarehouse = 2,
    ExtraInventory = 3,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadInventoryPacket {
    pub inventory: InventorySnapshot,
    pub inventory_type: InventoryType,
}

impl LoadInventoryPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LOAD_INVENTORY);
        writer.write_i32(0);
        writer.write_i64(self.inventory.bits);
        writer.write_u8(self.inventory_type as u8);
        writer.write_i16(self.inventory.size as i16);

        for slot in 0..self.inventory.size as usize {
            let record = self
                .inventory
                .items
                .get(slot)
                .map_or_else(|| ItemRecord::default().record, normalize_item_record);
            writer.write_bytes(&record);
        }

        writer.finalize()
    }
}

// --- 1006 entity-load framing -------------------------------------------
//
// A 1006 entity block is `[u1 action][u2 count](entry)*count[u1 end]`. Each
// entry opens with a 16-byte header `[u4][u4][u4 kind_handle][u4]` whose third
// dword packs the entity kind in its high word and the 16-bit map handle in its
// low word; the client picks a per-kind body parser from that kind. Strings in
// entity bodies are `[u2 len LE][ASCII]` with no terminator.

/// Entity kinds packed into the high word of an entry header's third dword.
const ENTITY_KIND_DIGIMON: u16 = 1;
const ENTITY_KIND_TAMER: u16 = 2;
const ENTITY_KIND_ITEM: u16 = 3;
const ENTITY_KIND_MONSTER: u16 = 4;

/// Lifecycle action prefixing a 1006 entity block.
const ENTITY_ACTION_NEW: u8 = 1;
const ENTITY_ACTION_IN: u8 = 3;

/// Terminator byte that ends the 1006 dispatcher loop.
const ENTITY_BLOCK_END: u8 = 0;

/// Equipment slot records carried in a tamer body, plus a trailing visual record.
const TAMER_EQUIPMENT_SLOTS: usize = 16;
/// Byte length of one visual slot record.
const VISUAL_SLOT_LEN: usize = 69;
/// Number of extend-stat levels a digimon body carries, after the enchant level.
const DIGIMON_EXTEND_STATS: usize = 7;
/// Maximum string length the client accepts in an entity body.
const ENTITY_STRING_MAX: usize = 0x200;

/// Write a 1006 entity-body string as `[u2 len LE][ASCII]` (no terminator).
fn write_entity_string(writer: &mut PacketWriter, value: &str) {
    let bytes = value.as_bytes();
    let len = bytes.len().min(ENTITY_STRING_MAX);
    writer.write_u16(len as u16);
    writer.write_bytes(&bytes[..len]);
}

/// Write the 16-byte entry header: position pair, packed kind+handle, entity type.
///
/// The last word is the entity type the client feeds to its object factory
/// (monster/digimon type id, tamer model, item id); it is not reserved.
fn write_entity_header(
    writer: &mut PacketWriter,
    kind: u16,
    handle: u16,
    entity_type: u32,
    x: i32,
    y: i32,
) {
    writer.write_i32(x);
    writer.write_i32(y);
    writer.write_u32((u32::from(kind) << 16) | u32::from(handle));
    writer.write_u32(entity_type);
}

/// Resolve a 16-bit map handle, falling back to the entity id when unset.
fn entity_handle(raw: u32, fallback_id: u64) -> u16 {
    non_zero_handler(raw, fallback_id) as u16
}

/// Health-bar fill percentage the entity record carries; an unknown maximum reads as full.
fn hp_rate_percent(current: i32, max: i32) -> u8 {
    if max <= 0 {
        return 100;
    }
    let percent = (i64::from(current.max(0)) * 100) / i64::from(max);
    percent.clamp(0, 100) as u8
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadTamerPacket {
    pub character: CharacterSummary,
}

impl LoadTamerPacket {
    /// Encode a peer tamer and its partner digimon as a 1006 `In` block. The
    /// block carries two typed entries (tamer + digimon) followed by the
    /// dispatcher terminator the client loop requires.
    pub fn encode(&self) -> Vec<u8> {
        let c = &self.character;
        let mut writer = PacketWriter::new(game::LOAD_UNLOAD_ENTITY);
        writer.write_u8(ENTITY_ACTION_IN);
        writer.write_u16(2);

        let tamer_handle = entity_handle(c.general_handler, c.id);
        let partner_handle = entity_handle(c.partner_handler, c.id.saturating_add(10_000));

        // --- Tamer entry ---
        write_entity_header(
            &mut writer,
            ENTITY_KIND_TAMER,
            tamer_handle,
            c.model as u32,
            c.x,
            c.y,
        );
        writer.write_i32(c.x);
        writer.write_i32(c.y);
        write_entity_string(&mut writer, &c.name);
        writer.write_u8(c.level);
        writer.write_f32(0.0); // facing direction is owned by the client
        writer.write_u16(c.ms.clamp(0, u16::MAX as i32) as u16);
        writer.write_u8(hp_rate_percent(c.current_hp, c.hp));
        writer.write_zeroes(TAMER_EQUIPMENT_SLOTS * VISUAL_SLOT_LEN);
        writer.write_zeroes(VISUAL_SLOT_LEN);
        let condition = c.current_condition as u32;
        writer.write_u32(condition);
        writer.write_u32(0);
        writer.write_u32(u32::from(partner_handle));
        writer.write_u16(c.size as u16);
        writer.write_u8(0); // no guild
        writer.write_u16(0); // achievement
        writer.write_u8(0); // match team
        writer.write_u16(c.seal_list.seal_leader_id as u16);
        if condition & 0x4 != 0 {
            write_entity_string(&mut writer, &c.shop_name);
        }
        writer.write_u32(0); // no costume

        // --- Partner digimon entry ---
        write_entity_header(
            &mut writer,
            ENTITY_KIND_DIGIMON,
            partner_handle,
            c.partner_current_type as u32,
            c.partner_x,
            c.partner_y,
        );
        writer.write_i32(c.partner_x);
        writer.write_i32(c.partner_y);
        write_entity_string(&mut writer, &c.partner_name);
        writer.write_u16(c.partner_size as u16);
        writer.write_u8(c.partner_level);
        writer.write_f32(0.0); // facing direction is owned by the client
        writer.write_u16(c.partner_ms.clamp(0, u16::MAX as i32) as u16);
        writer.write_u16(c.partner_as.clamp(0, u16::MAX as i32) as u16);
        writer.write_u8(0); // spawn flag
        writer.write_u32(u32::from(tamer_handle));
        writer.write_u8(hp_rate_percent(c.partner_current_hp, c.partner_hp));
        writer.write_u32(c.partner_condition as u32);
        // Enchant level plus the extend-stat levels. Clone stats use a different
        // scale and have no counterpart here, so the record reports an unenchanted
        // partner rather than repurposing them.
        writer.write_u16(0); // enchant level
        for _ in 0..DIGIMON_EXTEND_STATS {
            writer.write_u16(0);
        }
        writer.write_u32(0); // no appearance item

        writer.write_u8(ENTITY_BLOCK_END);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadMobsPacket {
    pub mob: MobSummary,
}

impl LoadMobsPacket {
    /// Encode a monster as a single-entry 1006 block. `New` marks a fresh spawn,
    /// `In` an entity already present when the viewer arrives.
    pub fn encode(&self) -> Vec<u8> {
        let m = &self.mob;
        let mut writer = PacketWriter::new(game::LOAD_UNLOAD_ENTITY);
        writer.write_u8(if m.respawn {
            ENTITY_ACTION_NEW
        } else {
            ENTITY_ACTION_IN
        });
        writer.write_u16(1);

        let handle = entity_handle(m.handler, m.id);
        write_entity_header(
            &mut writer,
            ENTITY_KIND_MONSTER,
            handle,
            m.type_id as u32,
            m.previous_x,
            m.previous_y,
        );
        writer.write_i32(m.x);
        writer.write_i32(m.y);
        writer.write_u8(hp_rate_percent(m.current_hp, m.max_hp));
        writer.write_u8(m.level);
        writer.write_u32(0); // no monster growth skill
        writer.write_u32(0); // growth stack
        writer.write_u32(0); // condition
        // Seed count; the client reads it unconditionally after the condition,
        // then one placement quadruple per seed.
        writer.write_u32(0); // no skill/seed records
        writer.write_u8(ENTITY_BLOCK_END);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadDropsPacket {
    pub drop: DropSummary,
    pub viewer_handler: u32,
}

impl LoadDropsPacket {
    /// Encode a ground item as a single-entry 1006 `In` block. The item id
    /// travels in the entry header; the client resolves the drop's visuals from
    /// that id, so the body only carries the owner handle and the state byte.
    pub fn encode(&self) -> Vec<u8> {
        let d = &self.drop;
        let mut writer = PacketWriter::new(game::LOAD_UNLOAD_ENTITY);
        writer.write_u8(ENTITY_ACTION_IN);
        writer.write_u16(1);

        let handle = entity_handle(d.handler, d.id);
        write_entity_header(
            &mut writer,
            ENTITY_KIND_ITEM,
            handle,
            d.item_id as u32,
            d.x,
            d.y,
        );
        writer.write_u32(0); // no owner
        writer.write_u8(0); // item form/state flags
        writer.write_u8(ENTITY_BLOCK_END);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickItemPacket {
    pub appearance_handler: u32,
    pub item_id: i32,
    pub amount: i16,
}

impl PickItemPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LOOT_ITEM);
        writer.write_u32(self.appearance_handler);
        writer.write_i32(self.item_id);
        writer.write_i16(self.amount);
        writer.write_u8(0);
        writer.write_i32(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickBitsPacket {
    pub appearance_handler: u32,
    pub value: i32,
}

impl PickBitsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PICK_BITS);
        writer.write_u32(self.appearance_handler);
        writer.write_i32(self.value);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickItemFailReason {
    Unknown = 1,
    NotTheOwner = 2,
    TooFarAway = 3,
    InventoryFull = 4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickItemFailPacket {
    pub reason: PickItemFailReason,
}

impl PickItemFailPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PICK_ITEM_FAIL);
        writer.write_i32(self.reason as i32);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnloadTamerPacket {
    pub character: CharacterSummary,
}

impl UnloadTamerPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LOAD_UNLOAD_ENTITY);
        writer.write_u8(4);
        writer.write_i16(2);
        writer.write_i32(non_zero_handler(
            self.character.general_handler,
            self.character.id,
        ));
        writer.write_i32(self.character.x);
        writer.write_i32(self.character.y);
        writer.write_i32(non_zero_handler(
            self.character.partner_handler,
            self.character.id.saturating_add(10_000),
        ));
        writer.write_i32(self.character.partner_x);
        writer.write_i32(self.character.partner_y);
        writer.write_i32(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnloadMobsPacket {
    pub mob: MobSummary,
}

impl UnloadMobsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LOAD_UNLOAD_ENTITY);
        writer.write_u8(4);
        writer.write_i16(1);
        writer.write_u32(non_zero_handler(self.mob.handler, self.mob.id) as u32);
        writer.write_i32(self.mob.x);
        writer.write_i32(self.mob.y);
        writer.write_i32(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnloadDropsPacket {
    pub drop: DropSummary,
}

impl UnloadDropsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LOAD_UNLOAD_ENTITY);
        writer.write_u8(4);
        writer.write_i16(1);
        writer.write_u32(non_zero_handler(self.drop.handler, self.drop.id) as u32);
        writer.write_i32(self.drop.x);
        writer.write_i32(self.drop.y);
        writer.write_i32(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LoadBuffsPacket {
    pub character: CharacterSummary,
}

impl LoadBuffsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LOAD_BUFFS);
        writer.write_u8(16);
        writer.write_i16(1);
        writer.write_i32(non_zero_handler(
            self.character.general_handler,
            self.character.id,
        ));
        writer.write_u8(clamp_u8_len(self.character.active_buffs.len()));
        for buff in &self.character.active_buffs {
            write_active_buff(&mut writer, buff);
        }

        writer.write_i16(1);
        writer.write_i32(non_zero_handler(
            self.character.partner_handler,
            self.character.id.saturating_add(10_000),
        ));
        writer.write_u8(clamp_u8_len(
            self.character.partner_active_buffs.len() + self.character.partner_active_debuffs.len(),
        ));

        for buff in &self.character.partner_active_buffs {
            write_active_buff(&mut writer, buff);
        }
        for buff in &self.character.partner_active_debuffs {
            write_active_buff(&mut writer, buff);
        }

        writer.write_i16(0);
        writer.write_u8(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadMobBuffsPacket {
    pub mob: MobSummary,
}

impl LoadMobBuffsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LOAD_BUFFS);
        writer.write_u8(16);
        writer.write_i16(0);
        writer.write_i16(0);
        writer.write_i16(1);
        writer.write_u32(non_zero_handler(self.mob.handler, self.mob.id) as u32);
        writer.write_u8(clamp_u8_len(self.mob.active_debuffs.len()));
        for buff in self
            .mob
            .active_debuffs
            .iter()
            .take(clamp_u8_len(self.mob.active_debuffs.len()) as usize)
        {
            write_active_buff(&mut writer, buff);
        }
        writer.write_u8(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerExperiencePacket {
    pub experience: i32,
}

impl ServerExperiencePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SERVER_EXPERIENCE);
        writer.write_i32(1);
        writer.write_i32(self.experience);
        writer.write_i32(1);
        writer.write_i32(0);
        writer.write_i32(self.experience);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MembershipPacket {
    pub remaining_seconds: u32,
}

impl MembershipPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MEMBERSHIP);
        writer.write_u8((self.remaining_seconds > 0) as u8);
        writer.write_i32(self.remaining_seconds.min(i32::MAX as u32) as i32);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashShopCoinsPacket {
    pub premium: i32,
    pub silk: i32,
}

impl CashShopCoinsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::CASH_SHOP_COINS);
        writer.write_i32(0);
        writer.write_i32(self.silk);
        writer.write_i32(self.premium);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashShopBuyResponsePacket {
    pub result: u16,
    pub real_cash: i32,
    pub bonus_cash: i32,
    pub success_product_ids: Vec<i32>,
    pub failed_product_ids: Vec<i32>,
}

impl CashShopBuyResponsePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::CASHSHOP_BUY);
        writer.write_u16(self.result);
        writer.write_i32(self.real_cash);
        writer.write_i32(self.bonus_cash);
        writer.write_u8(self.success_product_ids.len() as u8);
        for &id in &self.success_product_ids {
            writer.write_i32(id);
        }
        writer.write_u8(self.failed_product_ids.len() as u8);
        for &id in &self.failed_product_ids {
            writer.write_i32(id);
        }
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashShopCropResponsePacket {
    pub result: i32,
    pub rtime: i32,
    pub cash_warehouse_slot: u16,
    pub slot_type: u8,
    pub slot_no: u8,
    pub item_type: u32,
    pub item_count: u32,
    pub item_rate: u8,
    pub item_end_time: u32,
    pub remain_trade_limit_time: u32,
}

impl CashShopCropResponsePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::RETRIEVE_ACCOUNT_WAREHOUSE);
        writer.write_i32(self.result);
        writer.write_i32(self.rtime);
        writer.write_u16(self.cash_warehouse_slot);
        writer.write_u8(self.slot_type);
        writer.write_u8(self.slot_no);
        writer.write_u32(self.item_type);
        writer.write_u32(self.item_count);
        writer.write_u8(self.item_rate);
        writer.write_u32(self.item_end_time);
        writer.write_u32(self.remain_trade_limit_time);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CashShopWarehousePacket {
    pub items: Vec<ItemRecord>,
}

impl CashShopWarehousePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LOAD_ACCOUNT_WAREHOUSE);
        writer.write_i16(clamp_i16_len(self.items.len()));
        for item in &self.items {
            let record = normalize_item_record(item);
            writer.write_bytes(&record);
        }
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigiSummonSyncResponsePacket {
    pub result: u8,
    pub products: Vec<DigiSummonProduct>,
}

impl DigiSummonSyncResponsePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGI_SUMMON_SYNC_RESPONSE);
        writer.write_u8(self.result);
        writer.write_u16(clamp_u16_len(self.products.len()));
        for product in &self.products {
            writer.write_i32(product.product_id);
            writer.write_i32(product.rank);
            writer.write_u16(product.draw_count.clamp(0, u16::MAX as i32) as u16);
            writer.write_i32(product.remaining_daily_limit);
        }
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigiSummonPurchaseResponsePacket {
    pub result: u8,
    pub product_id: i32,
    pub rewards: Vec<DigiSummonReward>,
    pub products: Vec<DigiSummonProduct>,
}

impl DigiSummonPurchaseResponsePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGI_SUMMON_PURCHASE_RESPONSE);
        writer.write_u8(self.result);
        writer.write_i32(self.product_id);
        writer.write_u16(clamp_u16_len(self.rewards.len()));
        for reward in &self.rewards {
            writer.write_i32(reward.item_id);
            writer.write_u16(reward.amount.clamp(1, u16::MAX as i32) as u16);
            writer.write_u16(reward.grade.clamp(0, u16::MAX as i32) as u16);
        }

        writer.write_u16(clamp_u16_len(self.products.len()));
        for product in &self.products {
            writer.write_i32(product.product_id);
            writer.write_i32(product.rank);
            writer.write_u16(product.draw_count.clamp(0, u16::MAX as i32) as u16);
            writer.write_i32(product.remaining_daily_limit);
        }

        // The detail list carries per-reward item descriptors; empty for the
        // common purchase result, so emit a zero count followed by the trailer.
        writer.write_u16(0); // detail_count
        writer.write_i64(0); // trailer
        writer.finalize()
    }
}

/// Combine sync response: a leading result byte and the ceiling map only.
///
/// Digivice combine (3661) and union combine (4301) share this layout; the
/// target opcode selects which flow the response belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombineSyncResponsePacket {
    pub opcode: i16,
    pub result: u8,
    pub ceiling: Vec<CombineCeilingEntry>,
}

impl CombineSyncResponsePacket {
    /// Build a Digivice combine sync response (opcode 3661).
    pub fn digi(result: u8, ceiling: Vec<CombineCeilingEntry>) -> Self {
        Self {
            opcode: game::DIGI_COMBINE_SYNC,
            result,
            ceiling,
        }
    }

    /// Build a union combine sync response (opcode 4301).
    pub fn union(result: u8, ceiling: Vec<CombineCeilingEntry>) -> Self {
        Self {
            opcode: game::UNION_COMBINE_SYNC,
            result,
            ceiling,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_u8(self.result);
        write_combine_ceiling(&mut writer, &self.ceiling);
        writer.finalize()
    }
}

/// Combine result/reward response: result byte, ceiling map, the submitted
/// material echo list, and the reward detail list.
///
/// Digivice combine/reward (3662/3663) and union combine/reward (4302/4303)
/// share this layout; the target opcode selects which flow the response
/// belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombineResultResponsePacket {
    pub opcode: i16,
    pub result: u8,
    pub ceiling: Vec<CombineCeilingEntry>,
    pub materials: Vec<CombineItemRef>,
    pub rewards: Vec<DigiCombineReward>,
}

impl CombineResultResponsePacket {
    /// Build a Digivice combine result response (opcode 3662).
    pub fn digi_result(
        result: u8,
        ceiling: Vec<CombineCeilingEntry>,
        materials: Vec<CombineItemRef>,
        rewards: Vec<DigiCombineReward>,
    ) -> Self {
        Self {
            opcode: game::DIGI_COMBINE,
            result,
            ceiling,
            materials,
            rewards,
        }
    }

    /// Build a Digivice combine reward response (opcode 3663).
    pub fn digi_reward(
        result: u8,
        ceiling: Vec<CombineCeilingEntry>,
        materials: Vec<CombineItemRef>,
        rewards: Vec<DigiCombineReward>,
    ) -> Self {
        Self {
            opcode: game::DIGI_COMBINE_REWARD,
            result,
            ceiling,
            materials,
            rewards,
        }
    }

    /// Build a union combine result response (opcode 4302).
    pub fn union_result(
        result: u8,
        ceiling: Vec<CombineCeilingEntry>,
        materials: Vec<CombineItemRef>,
        rewards: Vec<DigiCombineReward>,
    ) -> Self {
        Self {
            opcode: game::UNION_COMBINE,
            result,
            ceiling,
            materials,
            rewards,
        }
    }

    /// Build a union combine reward response (opcode 4303).
    pub fn union_reward(
        result: u8,
        ceiling: Vec<CombineCeilingEntry>,
        materials: Vec<CombineItemRef>,
        rewards: Vec<DigiCombineReward>,
    ) -> Self {
        Self {
            opcode: game::UNION_COMBINE_REWARD,
            result,
            ceiling,
            materials,
            rewards,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_u8(self.result);
        write_combine_ceiling(&mut writer, &self.ceiling);
        write_combine_item_list(&mut writer, &self.materials);
        write_combine_reward_list(&mut writer, &self.rewards);
        writer.finalize()
    }
}

/// Random box list/sync response (opcode 16067).
///
/// Layout: a leading field, then a `u1`-counted list of fixed entries. Field
/// semantics are not yet decoded; the wire widths and order are fixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomBoxListResponsePacket {
    pub field0: i32,
    pub entries: Vec<RandomBoxListEntry>,
}

/// One entry in the random box list response: three 32-bit fields and a
/// trailing 16-bit field. Semantics are undecoded; widths are fixed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RandomBoxListEntry {
    pub a: i32,
    pub b: i32,
    pub c: i32,
    pub d: u16,
}

impl RandomBoxListResponsePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::RANDOM_BOX_LIST);
        writer.write_i32(self.field0);
        writer.write_u8(clamp_u8_len(self.entries.len()));
        for entry in &self.entries {
            writer.write_i32(entry.a);
            writer.write_i32(entry.b);
            writer.write_i32(entry.c);
            writer.write_u16(entry.d);
        }
        writer.finalize()
    }
}

/// Random box purchase result response (opcode 16068).
///
/// Layout: three leading fields, a `u1`-counted list of 32-bit pairs, a second
/// `u1`-counted list of `[u64, u16]` pairs, then a trailing `[u64, u16]` summary
/// block. Field semantics are not yet decoded; the wire widths and order are
/// fixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RandomBoxPurchaseResponsePacket {
    pub field0: i32,
    pub field1: i32,
    pub field2: u16,
    pub list_a: Vec<(i32, i32)>,
    pub list_b: Vec<(u64, u16)>,
    pub summary: (u64, u16),
}

impl RandomBoxPurchaseResponsePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::RANDOM_BOX_PURCHASE);
        writer.write_i32(self.field0);
        writer.write_i32(self.field1);
        writer.write_u16(self.field2);
        writer.write_u8(clamp_u8_len(self.list_a.len()));
        for (a, b) in &self.list_a {
            writer.write_i32(*a);
            writer.write_i32(*b);
        }
        writer.write_u8(clamp_u8_len(self.list_b.len()));
        for (a, b) in &self.list_b {
            writer.write_u64(*a);
            writer.write_u16(*b);
        }
        let (summary_a, summary_b) = self.summary;
        writer.write_u64(summary_a);
        writer.write_u16(summary_b);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpiritToDigimonResultPacket {
    pub digimon_id: u32,
    pub remaining_bits: i64,
    pub consumed_items: Vec<(u8, u32)>,
}

impl SpiritToDigimonResultPacket {
    /// Encode the spirit-item-to-digimon result:
    /// `[u32 digimon_id][i64 remaining_bits]`
    /// followed by consumed-item blocks (`[u8 count][u32 item_id]`) terminated by
    /// a zero count byte.
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SPIRIT_TO_DIGIMON);
        writer.write_u32(self.digimon_id);
        writer.write_i64(self.remaining_bits);
        for (amount, item_id) in &self.consumed_items {
            writer.write_u8(*amount);
            writer.write_u32(*item_id);
        }
        writer.write_u8(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigimonToSpiritResultPacket {
    pub slot: u8,
    pub remaining_bits: i64,
    pub consumed_items: Vec<(u8, u32)>,
    pub gained_items: Vec<(u8, u32)>,
}

impl DigimonToSpiritResultPacket {
    /// Encode the digimon-to-spirit result: `[u8 deleted_slot][i64 remaining_bits]`
    /// followed by a zero-terminated consumed-item block list and then a
    /// zero-terminated gained-item block list (each block `[u8 count][u32 item_id]`).
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGIMON_TO_SPIRIT);
        writer.write_u8(self.slot);
        writer.write_i64(self.remaining_bits);
        for (amount, item_id) in &self.consumed_items {
            writer.write_u8(*amount);
            writer.write_u32(*item_id);
        }
        writer.write_u8(0);
        for (amount, item_id) in &self.gained_items {
            writer.write_u8(*amount);
            writer.write_u32(*item_id);
        }
        writer.write_u8(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AvailableChannelsPacket {
    pub channels: Vec<ChannelAvailability>,
}

impl AvailableChannelsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::AVAILABLE_CHANNELS);
        for channel in &self.channels {
            writer.write_u8(channel.channel);
            writer.write_u8(channel.load);
        }
        writer.write_u8(u8::MAX);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealsPacket {
    pub seal_list: SealListSnapshot,
}

impl SealsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SEALS);
        writer.write_i16(self.seal_list.seal_leader_id);
        writer.write_i16(clamp_i16_len(self.seal_list.seals.len()));
        for seal in &self.seal_list.seals {
            writer.write_i16(0);
            writer.write_i32(seal.seal_id);
            writer.write_i16(seal.amount);
        }

        writer.write_i16(clamp_i16_len(self.seal_list.seals.len()));
        for seal in &self.seal_list.seals {
            writer.write_i16(seal.sequential_id);
            writer.write_u8(seal.favorite as u8);
        }

        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeRewardPacket {
    pub reward: DailyRewardStatus,
}

impl TimeRewardPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TIME_REWARD);
        writer.write_i32(self.reward.event_no);
        writer.write_i32(self.reward.remaining_seconds);
        writer.write_i32(self.reward.total_seconds);
        writer.write_u8(self.reward.week);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TamerRelationsPacket {
    pub friends: Vec<RelationEntry>,
    pub foes: Vec<RelationEntry>,
}

impl TamerRelationsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::AVAILABLE_RELATIONS);
        // Modern wire: receive-side parser reads {u2 friends; friends x {u1
        // presence-flag; str2; str2}-{u2 foes; foes x {str2; str2}}}; all
        // strings are {u2 len; bytes} with the empty form still consuming the
        // 2-byte prefix.
        writer.write_i16(clamp_i16_len(self.friends.len()));
        for friend in &self.friends {
            writer.write_u8(friend.connected as u8);
            write_optional_modern_string(&mut writer, non_empty_relation_name(friend));
            write_optional_modern_string(&mut writer, &friend.annotation);
        }

        writer.write_i16(clamp_i16_len(self.foes.len()));
        for foe in &self.foes {
            write_optional_modern_string(&mut writer, non_empty_relation_name(foe));
            write_optional_modern_string(&mut writer, &foe.annotation);
        }

        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FriendConnectPacket {
    pub name: String,
}

impl FriendConnectPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::FRIEND_CONNECT);
        // Modern wire: {u2 byte-length; bytes} parsed by the client's buddy
        // reader then re-dispatched with an on-flag into the friends UI.
        // Legacy writers sent a NUL-terminated u8-length string here.
        writer.write_modern_utf8_string(&self.name);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyInvitePacket {
    pub inviter_name: String,
}

impl PartyInvitePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_INVITE);
        writer.write_modern_utf8_string(&self.inviter_name);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyInviteResultPacket {
    pub result_type: i32,
    pub target_name: String,
}

impl PartyInviteResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_INVITE_RESPONSE);
        writer.write_i32(self.result_type);
        writer.write_modern_utf8_string(&self.target_name);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyCreatedPacket {
    pub party_id: u32,
    pub loot_type: u32,
}

impl PartyCreatedPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_CREATED);
        writer.write_u32(self.party_id);
        writer.write_u32(self.loot_type);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartyMemberListEntry {
    pub party_slot: i32,
    pub character: CharacterSummary,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartyJoinPacket {
    pub member: PartyMemberListEntry,
}

impl PartyJoinPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_JOIN);
        write_party_member(&mut writer, &self.member);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartyMemberListPacket {
    pub party_id: u32,
    pub my_slot: i32,
    pub leader_slot: i32,
    pub loot_type: u32,
    pub rare_rate: u8,
    pub disp_rare_grade: u8,
    pub members: Vec<PartyMemberListEntry>,
}

impl PartyMemberListPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_MEMBER_LIST);
        writer.write_u32(self.party_id);
        writer.write_i32(self.my_slot);
        writer.write_i32(self.leader_slot);
        writer.write_u32(self.loot_type);
        writer.write_u8(self.rare_rate);
        writer.write_u8(self.disp_rare_grade);
        for member in &self.members {
            write_party_member(&mut writer, member);
        }
        writer.write_i32(-1);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyLeavePacket {
    pub member_slot: u8,
}

impl PartyLeavePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_LEAVE);
        writer.write_u8(self.member_slot);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyKickPacket {
    pub member_slot: u8,
}

impl PartyKickPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_KICK);
        writer.write_u8(self.member_slot);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyLeaderChangedPacket {
    pub new_leader_slot: i32,
}

impl PartyLeaderChangedPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_CHANGE_MASTER);
        writer.write_i32(self.new_leader_slot);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyChangeLootTypePacket {
    pub loot_type: i32,
    pub rare_type: u8,
    pub disp_rare_grade: u8,
}

impl PartyChangeLootTypePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_CHANGE_LOOT);
        writer.write_i32(self.loot_type);
        writer.write_u8(self.rare_type);
        writer.write_u8(self.disp_rare_grade);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberInfoPacket {
    pub member_slot: u8,
    pub digimon_type: i32,
    pub tamer_hp: i32,
    pub tamer_max_hp: i32,
    pub tamer_ds: i32,
    pub tamer_max_ds: i32,
    pub digimon_hp: i32,
    pub digimon_max_hp: i32,
    pub digimon_ds: i32,
    pub digimon_max_ds: i32,
    pub tamer_level: u16,
    pub digimon_level: u16,
}

impl PartyMemberInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_MEMBER_INFO);
        writer.write_u8(self.member_slot);
        writer.write_i32(self.digimon_type);
        writer.write_i32(self.tamer_hp);
        writer.write_i32(self.tamer_max_hp);
        writer.write_i32(self.tamer_ds);
        writer.write_i32(self.tamer_max_ds);
        writer.write_i32(self.digimon_hp);
        writer.write_i32(self.digimon_max_hp);
        writer.write_i32(self.digimon_ds);
        writer.write_i32(self.digimon_max_ds);
        writer.write_u16(self.tamer_level);
        writer.write_u16(self.digimon_level);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberPositionPacket {
    pub member_slot: u8,
    pub tamer_x: i32,
    pub tamer_y: i32,
    pub digimon_x: i32,
    pub digimon_y: i32,
}

impl PartyMemberPositionPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_MEMBER_POSITION);
        writer.write_u8(self.member_slot);
        writer.write_i32(self.tamer_x);
        writer.write_i32(self.tamer_y);
        writer.write_i32(self.digimon_x);
        writer.write_i32(self.digimon_y);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberMapChangePacket {
    pub member_slot: u8,
    pub map_id: i32,
    pub channel: i32,
    pub tamer_handler: u32,
    pub digimon_handler: u32,
}

impl PartyMemberMapChangePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_MEMBER_MAP_CHANGE);
        writer.write_u8(self.member_slot);
        writer.write_i32(self.map_id);
        writer.write_i32(self.channel);
        writer.write_u32(self.tamer_handler);
        writer.write_u32(self.digimon_handler);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberDigimonChangePacket {
    pub member_slot: u8,
    pub digimon_type: i32,
    pub digimon_name: String,
    pub digimon_hp: u16,
    pub digimon_max_hp: u16,
    pub digimon_ds: u16,
    pub digimon_max_ds: u16,
}

impl PartyMemberDigimonChangePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_MEMBER_DIGIMON_CHANGE);
        writer.write_u8(self.member_slot);
        writer.write_i32(self.digimon_type);
        writer.write_modern_utf8_string(&self.digimon_name);
        writer.write_u16(self.digimon_hp);
        writer.write_u16(self.digimon_max_hp);
        writer.write_u16(self.digimon_ds);
        writer.write_u16(self.digimon_max_ds);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberBuffEntry {
    pub status: u8,
    pub buff_code: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberBuffChangePacket {
    pub member_slot: u8,
    pub buffs: Vec<PartyMemberBuffEntry>,
}

impl PartyMemberBuffChangePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_MEMBER_BUFF_CHANGE);
        writer.write_u8(self.member_slot);
        writer.write_u16(self.buffs.len() as u16);
        for buff in &self.buffs {
            writer.write_u8(buff.status);
            writer.write_u16(buff.buff_code);
        }
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartnerSwitchFailurePacket;

impl PartnerSwitchFailurePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTNER_SWITCH_RESPONSE);
        writer.write_u32(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DigimonEvolutionFailPacket;

impl DigimonEvolutionFailPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::EVOLUTION_FAILURE);
        writer.write_i32(0);
        writer.finalize()
    }
}

/// Hit type for combat damage packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HitType {
    Normal = 0,
    Critical = 1,
    Block = 2,
}

fn write_modern_damage_block(writer: &mut PacketWriter, damage: i32) {
    // Modern client damage block: 10 i32 values; first carries the (negative) damage,
    // remaining nine are reserved/zero on the legacy server contract.
    writer.write_i32(damage);
    for _ in 1..10 {
        writer.write_i32(0);
    }
}

/// `PartnerAttack` success response — opcode 1013.
/// Mirrors `HitPacket(int attackerHandler, int targetHandler, int finalDamage,
/// long hpBeforeHit, long hpAfterHit, int hitType)` from the legacy server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitPacket {
    pub attacker_handler: u32,
    pub target_handler: u32,
    pub final_damage: i32,
    pub hp_before_hit: i64,
    pub hp_after_hit: i64,
    pub hit_type: HitType,
}

impl HitPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTNER_ATTACK_RESPONSE);
        writer.write_u32(self.attacker_handler);
        writer.write_u32(self.target_handler);
        write_modern_damage_block(&mut writer, -self.final_damage);
        writer.write_i32(self.hit_type as i32);
        writer.write_i64(self.hp_after_hit);
        writer.write_i64(self.hp_before_hit);
        writer.finalize()
    }
}

/// `PartnerAttack` miss response — opcode 1014.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissHitPacket {
    pub attacker_handler: u32,
    pub target_handler: u32,
}

impl MissHitPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ATTACK_MISS);
        writer.write_u32(self.attacker_handler);
        writer.write_u32(self.target_handler);
        writer.finalize()
    }
}

/// Lethal partner attack response — opcode 1020.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillOnHitPacket {
    pub attacker_handler: u32,
    pub target_handler: u32,
    pub final_damage: i32,
    pub hit_type: HitType,
}

impl KillOnHitPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::KILL_ON_HIT);
        writer.write_u32(self.attacker_handler);
        writer.write_u32(self.target_handler);
        write_modern_damage_block(&mut writer, -self.final_damage);
        writer.write_i32(self.hit_type as i32);
        writer.finalize()
    }
}

/// Partner skill cast announcement — opcode 1015.
/// Mirrors `CastSkillPacket(byte skillSlot, int attackerHandler, int targetHandler)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CastSkillPacket {
    pub skill_slot: u8,
    pub attacker_handler: u32,
    pub target_handler: u32,
}

impl CastSkillPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTNER_SKILL_RESPONSE);
        writer.write_u8(self.skill_slot);
        writer.write_u32(self.attacker_handler);
        writer.write_u32(self.target_handler);
        writer.finalize()
    }
}

/// Lethal partner skill response — opcode 1021.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KillOnSkillPacket {
    pub attacker_handler: u32,
    pub target_handler: u32,
    pub skill_slot: u32,
    pub final_damage: i32,
}

impl KillOnSkillPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::KILL_ON_SKILL);
        writer.write_u32(self.attacker_handler);
        writer.write_u32(self.target_handler);
        writer.write_u32(self.skill_slot);
        write_modern_damage_block(&mut writer, -self.final_damage);
        writer.finalize()
    }
}

/// Skill request rejection — opcode 1105.
/// Mirrors `PartnerSkillErrorPacket(int attackerHandler, byte parameter, byte value, byte value2, int context)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartnerSkillErrorPacket {
    pub attacker_handler: u32,
    pub parameter: u8,
    pub value: u8,
    pub value2: u8,
    pub context: i32,
}

impl PartnerSkillErrorPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTNER_SKILL_ERROR);
        writer.write_u32(self.attacker_handler);
        writer.write_u8(self.parameter);
        writer.write_u8(self.value);
        writer.write_u8(self.value2);
        writer.write_i32(self.context);
        writer.finalize()
    }
}

// ===========================================================================
// Quest packets
// ===========================================================================

/// `QuestAvailableList` response — opcode 11009.
/// Mirrors `QuestAvailableListPacket(int npcId, IEnumerable<int> questIds)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestAvailableListPacket {
    pub npc_id: i32,
    pub quest_ids: Vec<i16>,
}

impl QuestAvailableListPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::QUEST_AVAILABLE_LIST);
        writer.write_i32(self.npc_id);
        let len = self.quest_ids.len().min(u16::MAX as usize) as u16;
        writer.write_u16(len);
        for id in self.quest_ids.iter().take(u16::MAX as usize) {
            writer.write_i16(*id);
        }
        writer.finalize()
    }
}

/// `QuestGoalUpdate` packet — opcode 11001.
/// Mirrors `QuestGoalUpdatePacket(short questId, byte goalIndex, short currentGoalValue)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestGoalUpdatePacket {
    pub quest_id: i16,
    pub goal_index: u8,
    pub current_goal_value: i16,
}

impl QuestGoalUpdatePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::QUEST_GOAL_UPDATE);
        writer.write_i16(self.quest_id);
        writer.write_u8(self.goal_index);
        writer.write_i16(self.current_goal_value);
        writer.finalize()
    }
}

/// `QuestDailyUpdate` packet — opcode 11006. Empty payload; signals daily reset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuestDailyUpdatePacket;

impl QuestDailyUpdatePacket {
    pub fn encode(&self) -> Vec<u8> {
        PacketWriter::new(game::QUEST_DAILY_UPDATE).finalize()
    }
}

// ===========================================================================
// Encyclopedia packets
// ===========================================================================

/// `EncyclopediaLoad` response — opcode 3234.
/// Each entry encodes the unlocked-evolution bitmask + enchant stats + size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncyclopediaLoadPacket {
    pub entries: Vec<odmo_types::EncyclopediaEntry>,
}

impl EncyclopediaLoadPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ENCYCLOPEDIA_LOAD_RESPONSE);
        writer.write_i32(self.entries.len() as i32);
        for entry in &self.entries {
            // Build unlocked-slot bitmask (bit n => slot_level (n+1) unlocked).
            let mut slot_opened: u64 = 0;
            for ev in &entry.evolutions {
                if ev.unlocked && ev.slot_level >= 1 && ev.slot_level <= 63 {
                    slot_opened |= 1u64 << (ev.slot_level - 1);
                }
            }
            // The stored "type" is the encyclopedia entry's first evolution type
            // (matches the legacy `EvolutionAsset.Type` field). For the Rust port
            // we use the digimon_evolution_id directly.
            writer.write_i32(entry.digimon_evolution_id as i32);
            writer.write_u16(u16::from(entry.level));
            writer.write_u64(slot_opened);
            writer.write_i16(entry.enchant_at);
            writer.write_i16(entry.enchant_bl);
            writer.write_i16(entry.enchant_ct);
            writer.write_i16(entry.enchant_ev);
            writer.write_i16(entry.enchant_hp);
            writer.write_i16(entry.size);
            // Reward "not allowed" flag = 1 when reward already received.
            writer.write_u8(if entry.reward_received { 1 } else { 0 });
        }
        writer.write_u8(0);
        writer.finalize()
    }
}

/// `EncyclopediaReceiveRewardItem` packet — opcode 3236.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncyclopediaReceiveRewardItemPacket {
    pub item_id: i32,
    pub amount: i16,
}

impl EncyclopediaReceiveRewardItemPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ENCYCLOPEDIA_GET_REWARD);
        writer.write_u32(self.item_id as u32);
        writer.write_u16(self.amount as u16);
        writer.finalize()
    }
}

/// `EncyclopediaDeckBuffUse` packet — opcode 3237. Modern parser
/// `EncyBook_DeckEff_RecvParser` reads `{u2 option_index; u4 value}` and emits event
/// 161. Consumer validates `option_index == 100` as the clear-buffs sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EncyclopediaDeckBuffUsePacket {
    pub option_index: u16,
    pub value: u32,
}

impl EncyclopediaDeckBuffUsePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ENCYCLOPEDIA_DECK_BUFF);
        writer.write_u16(self.option_index);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `CareSlotSize` notice — opcode 3205, {u2 slot_count}, consumed by event 407.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CareSlotSizePacket {
    pub slot_count: u16,
}

impl CareSlotSizePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGIMON_CARE_SLOT_SIZE);
        writer.write_u16(self.slot_count);
        writer.finalize()
    }
}

/// `BattleTagSuccess` notice — opcode 3209, empty payload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BattleTagSuccessPacket;

impl BattleTagSuccessPacket {
    pub fn encode(&self) -> Vec<u8> {
        PacketWriter::new(game::DIGIMON_BATTLE_TAG_SUCCESS).finalize()
    }
}

/// `BattleTagFailure` notice — opcode 3210, empty payload.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BattleTagFailurePacket;

impl BattleTagFailurePacket {
    pub fn encode(&self) -> Vec<u8> {
        PacketWriter::new(game::DIGIMON_BATTLE_TAG_FAILURE).finalize()
    }
}

/// `SkillPointInit` notice — opcode 3213.
/// Modern parser reads {u1 count; count × 28-byte record blobs} and emits
/// event 477. Record contents are not yet attributed; the wire shape is exact.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DigimonSkillPointInitPacket {
    pub records: Vec<[u8; 28]>,
}

impl DigimonSkillPointInitPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGIMON_SKILL_POINT_INIT);
        writer.write_u8(self.records.len().min(255) as u8);
        for record in self.records.iter().take(255) {
            writer.write_bytes(&record[..]);
        }
        writer.finalize()
    }
}

/// `Facilitation` reply — opcode 3227, {u1 kind; u4 arg}, event 412.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigimonFacilitationPacket {
    pub kind: u8,
    pub arg: u32,
}

impl DigimonFacilitationPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGIMON_FACILITATION);
        writer.write_u8(self.kind);
        writer.write_u32(self.arg);
        writer.finalize()
    }
}

/// `IncubaterError` notice — opcode 3228, {u4}; the modern parser consumes
/// the dword and drops it, so the wire shape is exact but the field stays
/// unattributed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DigimonIncubaterErrorPacket {
    pub code: u32,
}

impl DigimonIncubaterErrorPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGIMON_INCUBATER_ERROR);
        writer.write_u32(self.code);
        writer.finalize()
    }
}

/// `IncubaterList` notice — opcode 3226. Modern parser always reads three {u4; u4; u4}
/// rows, hands each to the slot builder and dispatches an empty event 408. Row field
/// meanings stay unattributed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DigimonIncubaterListPacket {
    pub rows: Vec<(u32, u32, u32)>, // always 3 on the modern read side
}

impl DigimonIncubaterListPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert_eq!(self.rows.len(), 3, "incubator list is fixed to 3 rows");
        let mut writer = PacketWriter::new(game::DIGIMON_INCUBATER_LIST);
        for (first, second, third) in &self.rows {
            writer.write_u32(*first);
            writer.write_u32(*second);
            writer.write_u32(*third);
        }
        writer.finalize()
    }
}

/// `Extraction` notice — opcode 3229: one u8 handed to event 413.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DigimonExtractionPacket {
    pub result: u8, // unresolved
}

impl DigimonExtractionPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGIMON_EXTRACTION);
        writer.write_u8(self.result);
        writer.finalize()
    }
}

/// `IncubaterSlotChange` notice — opcode 3231: {u4 slot; u4 value} handed to
/// event 411; slot -1 makes the client close the incubator view instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DigimonIncubaterSlotChangePacket {
    pub slot: u32,  // -1 closes the view
    pub value: u32, // unresolved
}

impl DigimonIncubaterSlotChangePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DIGIMON_INCUBATER_SLOT_CHANGE);
        writer.write_u32(self.slot);
        writer.write_u32(self.value);
        writer.finalize()
    }
}

/// `InchantInfo` notice — opcode 3214. Modern parser reads one u4 digimon handler, one
/// u2 anchor value and five u2s that the client inserts into fixed positions 0, 1, 2, 4
/// and 6 of that digimon's enchant-info list; individual slot meanings stay
/// unattributed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigimonInchantInfoPacket {
    pub handler: u32,     // resolved by the client digimon lookup
    pub anchor: u16,      // read-side writes (value - 1) into the list cursor
    pub entries: Vec<u16>, // five values at list positions 0, 1, 2, 4, 6
}

impl DigimonInchantInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert_eq!(self.entries.len(), 5, "enchant info carries five fixed slots");
        let mut writer = PacketWriter::new(game::DIGIMON_INCHANT_INFO);
        writer.write_u32(self.handler);
        writer.write_u16(self.anchor);
        for entry in &self.entries {
            writer.write_u16(*entry);
        }
        writer.finalize()
    }
}

/// `DecreaseDigimonInchant` notice — opcode 1083. Modern parser applies everything to
/// the local digimon: it reads one u2 anchor (stored client-side as value - 1) and
/// fourteen more u2s in two sequential runs of seven; the client then pairs run one
/// with run two into seven info-list entries, decrements one 69-byte skill-table row
/// (zeroing it on underflow) and pings event 718 with the digimon plus an empty
/// event-476 alongside the tactics-fail feedback. Element meanings stay unattributed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigimonDecreaseInchantPacket {
    pub anchor: u16,       // stored as (value - 1)
    pub first_run: Vec<u16>,  // seven u2 values
    pub second_run: Vec<u16>, // seven u2 values paired with first_run
}

impl DigimonDecreaseInchantPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert_eq!(self.first_run.len(), 7, "first run carries seven u2 slots");
        assert_eq!(self.second_run.len(), 7, "second run carries seven u2 slots");
        let mut writer = PacketWriter::new(game::DIGIMON_DECREASE_INCHANT);
        writer.write_u16(self.anchor);
        for value in self.first_run.iter().chain(&self.second_run) {
            writer.write_u16(*value);
        }
        writer.finalize()
    }
}

/// `DigimonInchant` notice — opcode 1075. Modern parser reads one u4 digimon handler,
/// one u2 anchor (stored as value - 1) and fourteen u2s in two runs of seven, mirroring
/// the decrease variant; the pairing loop fills seven info-list entries and the client
/// recomputes the digimon stats through the base-stats, evolution and digimon tables,
/// dispatching events 41, 167 and 476.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigimonInchantPacket {
    pub handler: u32,         // resolved by the client digimon lookup
    pub anchor: u16,          // stored as (value - 1)
    pub first_run: Vec<u16>,  // seven u2 values
    pub second_run: Vec<u16>, // seven u2 values paired with first_run
}

impl DigimonInchantPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert_eq!(self.first_run.len(), 7, "first run carries seven u2 slots");
        assert_eq!(self.second_run.len(), 7, "second run carries seven u2 slots");
        let mut writer = PacketWriter::new(game::DIGIMON_INCHANT);
        writer.write_u32(self.handler);
        writer.write_u16(self.anchor);
        for value in self.first_run.iter().chain(&self.second_run) {
            writer.write_u16(*value);
        }
        writer.finalize()
    }
}

/// Coat notices — opcodes 2201, 2202 and 2203.
/// Modern parsers: 2201 reads one u4 handler (entity type 14 triggers its
/// vanity-slot setter), 2202 reads two u4s where only the first feeds the
/// vanity table and 2203 reads {u4 value; u4 handler}, routing type 13
/// through the tamer vanity table and type 14 through the digimon one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemCoatNoticePacket {
    pub value: u32,   // routed into the vanity table (2202, 2203)
    pub handler: u32, // unresolved (2201, 2203; 2202 consumes it silently)
}

impl ItemCoatNoticePacket {
    pub fn encode_on(&self, opcode: i16) -> Vec<u8> {
        let mut writer = PacketWriter::new(opcode);
        if opcode == game::ITEM_COAT_ALL {
            // 2201 carries only the handler; the other variants add the value.
            writer.write_u32(self.handler);
        } else {
            writer.write_u32(self.value);
            writer.write_u32(self.handler);
        }
        writer.finalize()
    }
}

/// Slot-size update — opcode 3924 (inventory) and 3925 (warehouse): one u2
/// that refreshes the client's bag or warehouse size and pings generic
/// events carrying node flavor 103 (inventory) or 104 (warehouse).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSlotSizePacket {
    pub size: u16,
}

impl ItemSlotSizePacket {
    pub fn encode_on(&self, opcode: i16) -> Vec<u8> {
        let mut writer = PacketWriter::new(opcode);
        writer.write_u16(self.size);
        writer.finalize()
    }
}

/// `UseFailure` notice — opcode 3902. Modern parser reads {u2 code; u4 item serial; u4
/// failure kind}; kind 1 resolves the serial against the CItemTable row and opens a
/// failure message box carrying the code, kind 13 pings event 154, kind 19 beeps and
/// remaining kinds only reset the client.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemUseFailurePacket {
    pub code: u16,        // message-box code for kind 1
    pub item_serial: u32, // keyed into CItemTable for kind 1
    pub failure_kind: u32, // switch selector (1 / 13 / 19 handled)
}

impl ItemUseFailurePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_USE_FAILURE);
        writer.write_u16(self.code);
        writer.write_u32(self.item_serial);
        writer.write_u32(self.failure_kind);
        writer.finalize()
    }
}

/// `PortableItemFail` notice — opcode 3943: one u4 code; 7701 pings ev212 and
/// 7702/7703 gate on prior portable-item usage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PortableItemFailPacket {
    pub code: u32, // switch selector (7701..7703)
}

impl PortableItemFailPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_PORTABLE_FAIL);
        writer.write_u32(self.code);
        writer.finalize()
    }
}

/// `CropMoney` notice — opcode 3911. Modern parser reads four u4s: the first two form
/// one i64 and the trailing pair feeds the coin-delta sums; the client closes three
/// fixed UI handlers, plays the coin sound, builds the money display rings and pings
/// event 671 with the i64 sum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemCropMoneyPacket {
    pub first_amount_lo: u32,  // low half of the first i64 amount
    pub first_amount_hi: u32,  // high half of the first i64 amount
    pub third: u32,            // unresolved coin-delta field
    pub fourth: u32,           // unresolved coin-delta field
}

impl ItemCropMoneyPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_CROP_MONEY);
        writer.write_u32(self.first_amount_lo);
        writer.write_u32(self.first_amount_hi);
        writer.write_u32(self.third);
        writer.write_u32(self.fourth);
        writer.finalize()
    }
}

/// `ChangeLimitedAttribute` notice — opcode 3934: {u2; u1} handed to event 585.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChangeLimitedAttributePacket {
    pub attribute: u16, // unresolved
    pub flag: u8,       // unresolved (conditionally consumed)
}

impl ChangeLimitedAttributePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_CHANGE_LIMITED_ATTRIBUTE);
        writer.write_u16(self.attribute);
        writer.write_u8(self.flag);
        writer.finalize()
    }
}

/// A `{key, value}` pair as carried by the detail-info skill and enchant grids.
/// Skills use `{skill_id, level}`; enchants use `{attribute_kind, value}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DetailStatPair {
    pub key: u32,
    pub value: u32,
}

/// One equipped-item row in a tamer-detail equipment list (71 wire bytes).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DetailItemRecord {
    pub item_id: u32,
    pub slot: u16,
    pub stack: u16,
    pub flags: [u8; 4],
    pub serial: u32,
    pub bound: u16,
    pub durability: u16,
    pub grade: u8,
    pub sockets: [u8; 16],
    pub enchant: [u8; 16],
    pub expire_low: u32,
    pub expire_high: u32,
    pub extra_a: u32,
    pub extra_b: u32,
    pub tag_a: u8,
    pub tag_b: u8,
}

impl DetailItemRecord {
    fn write(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.item_id);
        writer.write_u16(self.slot);
        writer.write_u16(self.stack);
        writer.write_bytes(&self.flags);
        writer.write_u32(self.serial);
        writer.write_u16(self.bound);
        writer.write_u16(self.durability);
        writer.write_u8(self.grade);
        writer.write_bytes(&self.sockets);
        writer.write_bytes(&self.enchant);
        writer.write_u32(self.expire_low);
        writer.write_u32(self.expire_high);
        writer.write_u32(self.extra_a);
        writer.write_u32(self.extra_b);
        writer.write_u8(self.tag_a);
        writer.write_u8(self.tag_b);
    }
}

/// Partner block embedded inside the other-tamer detail response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherTamerDetailDigimon {
    pub model: u32,
    pub name: String,
    pub appearance: [u8; 72],
    pub field_104: u16,
    pub field_106: u16,
    pub hatch_level: u8,
    pub field_109: u16,
    pub field_111: u32,
    pub field_115: u32,
    pub group_119: (u32, u16, u8),
    pub group_126: (u32, u16, u8),
    pub field_133: u16,
    pub attribute_records: [(u64, u32, u16); 3],
    pub group_177: (u32, u16),
    pub block_183: [u8; 16],
    pub field_199: u32,
    pub group_203: (u32, u32),
    pub skills: Vec<DetailStatPair>,
    pub enchants: Vec<DetailStatPair>,
}

impl OtherTamerDetailDigimon {
    pub fn empty() -> Self {
        Self {
            model: 0,
            name: String::new(),
            appearance: [0u8; 72],
            field_104: 0,
            field_106: 0,
            hatch_level: 0,
            field_109: 0,
            field_111: 0,
            field_115: 0,
            group_119: (0, 0, 0),
            group_126: (0, 0, 0),
            field_133: 0,
            attribute_records: [(0, 0, 0); 3],
            group_177: (0, 0),
            block_183: [0u8; 16],
            field_199: 0,
            group_203: (0, 0),
            skills: Vec::new(),
            enchants: Vec::new(),
        }
    }

    fn write(&self, writer: &mut PacketWriter) {
        writer.write_u32(self.model);
        writer.write_modern_utf8_string(&self.name);
        writer.write_bytes(&self.appearance);
        writer.write_u16(self.field_104);
        writer.write_u16(self.field_106);
        writer.write_u8(self.hatch_level);
        writer.write_u16(self.field_109);
        writer.write_u32(self.field_111);
        writer.write_u32(self.field_115);
        writer.write_u32(self.group_119.0);
        writer.write_u16(self.group_119.1);
        writer.write_u8(self.group_119.2);
        writer.write_u32(self.group_126.0);
        writer.write_u16(self.group_126.1);
        writer.write_u8(self.group_126.2);
        writer.write_u16(self.field_133);
        for (a, b, c) in self.attribute_records {
            writer.write_u64(a);
            writer.write_u32(b);
            writer.write_u16(c);
        }
        writer.write_u32(self.group_177.0);
        writer.write_u16(self.group_177.1);
        writer.write_bytes(&self.block_183);
        writer.write_u32(self.field_199);
        writer.write_u32(self.group_203.0);
        writer.write_u32(self.group_203.1);
        writer.write_u16(self.skills.len() as u16);
        for pair in &self.skills {
            writer.write_u32(pair.key);
            writer.write_u32(pair.value);
        }
        writer.write_u16(self.enchants.len() as u16);
        for pair in &self.enchants {
            writer.write_u32(pair.key);
            writer.write_u32(pair.value);
        }
    }
}

/// `OtherTamerDetailInfo` response (opcode 16071) — the modern DetailInfo wire shape.
///
/// Layout: tamer name/guild fixed buffers, the 13-slot stat block, three equipment
/// lists, the variable tamer name, the embedded partner block, and the trailing
/// attribute block. `target_handler` lets the client match it to its request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtherTamerDetailInfoPacket {
    pub target_handler: u32,
    pub tamer_name: String,
    pub guild_name: String,
    pub stats: [i32; 13],
    pub field_200: u16,
    pub equipment: [Vec<DetailItemRecord>; 3],
    pub field_238: i32,
    pub digimon: OtherTamerDetailDigimon,
    pub field_493: u16,
    pub attributes: [i32; 19],
}

impl OtherTamerDetailInfoPacket {
    pub fn empty(target_handler: u32) -> Self {
        Self {
            target_handler,
            tamer_name: String::new(),
            guild_name: String::new(),
            stats: [0; 13],
            field_200: 0,
            equipment: [Vec::new(), Vec::new(), Vec::new()],
            field_238: 0,
            digimon: OtherTamerDetailDigimon::empty(),
            field_493: 0,
            attributes: [0; 19],
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::OTHER_TAMER_DETAIL_INFO_RESPONSE);
        writer.write_u32(self.target_handler);
        // Fixed wide buffers retained alongside the variable name below.
        writer.write_fixed_wide_string(&self.tamer_name, 36);
        writer.write_fixed_wide_string(&self.guild_name, 36);
        for value in self.stats {
            writer.write_i32(value);
        }
        writer.write_u16(self.field_200);
        for list in &self.equipment {
            writer.write_u16(list.len() as u16);
            for record in list {
                record.write(&mut writer);
            }
        }
        writer.write_i32(self.field_238);
        writer.write_modern_utf8_string(&self.tamer_name);
        self.digimon.write(&mut writer);
        writer.write_u16(self.field_493);
        for value in self.attributes {
            writer.write_i32(value);
        }
        writer.finalize()
    }
}

// ===========================================================================
// D-Unit / Union hacking & init packets (modern flow, opcode 4311/4312/4313)
// ===========================================================================

/// One slot row in the D-Unit / Union hacking grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionHackSlot {
    /// Slot index inside the hacking grid (0..).
    pub slot: u8,
    /// Equipped part model id (0 when empty).
    pub part_id: i32,
    /// Grade / level of the part in that slot.
    pub grade: i16,
    /// Locked flag (cannot be replaced until unlocked with an item).
    pub locked: bool,
}

/// Response payload for `UNION_HACK_OPEN` (opcode 4311).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionHackOpenResponsePacket {
    pub result: u8,
    pub unlocked_slots: u8,
    pub slots: Vec<UnionHackSlot>,
}

impl UnionHackOpenResponsePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::UNION_HACK_OPEN_RESPONSE);
        writer.write_u8(self.result);
        writer.write_u8(self.unlocked_slots);
        writer.write_u8(self.slots.len() as u8);
        for slot in &self.slots {
            writer.write_u8(slot.slot);
            writer.write_i32(slot.part_id);
            writer.write_i16(slot.grade);
            writer.write_u8(if slot.locked { 1 } else { 0 });
        }
        writer.finalize()
    }
}

/// Response payload for `UNION_HACK_MODIFY` (opcode 4312).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionHackModifyResponsePacket {
    pub result: u8,
    pub slot: u8,
    pub new_part_id: i32,
    pub new_grade: i16,
    pub total_rating: i32,
}

impl UnionHackModifyResponsePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::UNION_HACK_MODIFY_RESPONSE);
        writer.write_u8(self.result);
        writer.write_u8(self.slot);
        writer.write_i32(self.new_part_id);
        writer.write_i16(self.new_grade);
        writer.write_i32(self.total_rating);
        writer.finalize()
    }
}

/// `UNION_INIT_DATA` push (opcode 4313) — sends the full D-Unit / Union state
/// to the client on login so the modern `cUnionContents` can hydrate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnionInitDataPacket {
    pub slots: Vec<UnionHackSlot>,
    pub total_rating: i32,
    pub synergy_bonus: i32,
}

impl UnionInitDataPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::UNION_INIT_DATA);
        writer.write_u8(self.slots.len() as u8);
        for slot in &self.slots {
            writer.write_u8(slot.slot);
            writer.write_i32(slot.part_id);
            writer.write_i16(slot.grade);
            writer.write_u8(if slot.locked { 1 } else { 0 });
        }
        writer.write_i32(self.total_rating);
        writer.write_i32(self.synergy_bonus);
        writer.finalize()
    }
}

// ===========================================================================
// Trade packets
// ===========================================================================

/// `TradeRequest` success — opcode 1501.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeRequestSuccessPacket {
    pub target_handler: u32,
}

impl TradeRequestSuccessPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_REQUEST_SUCCESS);
        writer.write_u32(self.target_handler);
        writer.finalize()
    }
}

/// `TradeRequest` error — opcode 1507.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeRequestErrorPacket {
    pub result: i32,
}

impl TradeRequestErrorPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_REQUEST_ERROR);
        writer.write_i32(self.result);
        writer.finalize()
    }
}

/// `TradeAccept` — opcode 1502.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeAcceptPacket {
    pub target_handler: u32,
}

impl TradeAcceptPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_ACCEPT);
        writer.write_u32(self.target_handler);
        writer.finalize()
    }
}

/// `TradeCancel` — opcode 1506.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeCancelPacket {
    pub target_handler: u32,
}

impl TradeCancelPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_CANCEL);
        writer.write_u32(self.target_handler);
        writer.finalize()
    }
}

/// `TradeConfirmation` — opcode 1503.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeConfirmationPacket {
    pub target_handler: u32,
}

impl TradeConfirmationPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_CONFIRM);
        writer.write_u32(self.target_handler);
        writer.finalize()
    }
}

/// `TradeFinalConfirmation` — opcode 1504.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeFinalConfirmationPacket {
    pub target_handler: u32,
}

impl TradeFinalConfirmationPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_FINAL_CONFIRMATION);
        writer.write_u32(self.target_handler);
        writer.finalize()
    }
}

/// `TradeAddItem` — opcode 1508.
/// `[u32 target][item bytes (~80)][u8 trade_slot][i32 inventory_slot]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TradeAddItemPacket {
    pub target_handler: u32,
    pub item_bytes: Vec<u8>,
    pub trade_slot: u8,
    pub inventory_slot: i32,
}

impl TradeAddItemPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_ADD_ITEM);
        writer.write_u32(self.target_handler);
        writer.write_bytes(&self.item_bytes);
        writer.write_u8(self.trade_slot);
        writer.write_i32(self.inventory_slot);
        writer.finalize()
    }
}

/// `TradeRemoveItem` — opcode 1519.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeRemoveItemPacket {
    pub target_handler: u32,
    pub trade_slot: u8,
}

impl TradeRemoveItemPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_REMOVE_ITEM_RESPONSE);
        writer.write_u32(self.target_handler);
        writer.write_u8(self.trade_slot);
        writer.finalize()
    }
}

/// `TradeAddMoney` — opcode 1509.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeAddMoneyPacket {
    pub target_handler: u32,
    pub money: i32,
}

impl TradeAddMoneyPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_ADD_MONEY);
        writer.write_u32(self.target_handler);
        writer.write_i32(self.money);
        writer.finalize()
    }
}

/// `TradeInventoryLock` — opcode 1532.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeInventoryLockPacket {
    pub target_handler: u32,
}

impl TradeInventoryLockPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_LOCK);
        writer.write_u32(self.target_handler);
        writer.write_u8(1);
        writer.finalize()
    }
}

/// `TradeInventoryUnlock` — opcode 1505.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeInventoryUnlockPacket {
    pub target_handler: u32,
}

impl TradeInventoryUnlockPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TRADE_UNLOCK);
        writer.write_u32(self.target_handler);
        writer.write_u8(0);
        writer.finalize()
    }
}

// ===========================================================================
// Arena packets
// ===========================================================================

/// `ArenaRankingDailyLoad` — opcode 4130.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaRankingDailyLoadPacket {
    pub remaining_minutes: i64,
    pub points: i32,
}

impl ArenaRankingDailyLoadPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ARENA_DAILY_RANKING);
        writer.write_u8(1);
        writer.write_i64(self.remaining_minutes);
        writer.write_i32(self.points);
        writer.finalize()
    }
}

/// `ArenaRankingDailyUpdatePoints` — opcode 4131.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArenaRankingDailyUpdatePointsPacket {
    pub points: i32,
}

impl ArenaRankingDailyUpdatePointsPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ARENA_DAILY_POINTS);
        writer.write_u8(100);
        writer.write_i32(self.points);
        writer.finalize()
    }
}

/// One row of the live weekly/monthly/season board:
/// `{u1 rank; string name; string guild; u4 point; u1 is_new; i8 position_delta}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaRankRow {
    pub rank: u8,
    pub name: String,
    pub guild: String,
    pub points: u32,
    pub is_new: bool,
    pub position_delta: i8,
}

/// One row of an archived board (a previous period or the hall of fame):
/// `{u1 rank; string name; string guild; u4 point}`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaArchiveRankRow {
    pub rank: u8,
    pub name: String,
    pub guild: String,
    pub points: u32,
}

/// Upper bound the client reserves for the live board row list.
const ARENA_RANK_ROWS_MAX: usize = 100;
/// Upper bound the client reserves for the hall-of-fame row list.
const ARENA_FAME_ROWS_MAX: usize = 50;

/// `ArenaRequestRank` — opcode 16023: live board for one rank type.
///
/// Header: `{u1 rank_type; u1 result; u1 status; u4 point; u4 rank;
/// u4 position_delta; u1 rank_position_kind; u1 rank_percent; u4 renew_sec;
/// u4 end_sec; u1 count}`, then `count` rows. The client drops the packet when
/// `result` is non-zero; a zero-length list is a valid empty board.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaRankingInfoPacket {
    pub ranking_type: u8,
    pub result: u8,
    pub status: u8,
    pub point: u32,
    pub rank: u32,
    pub position_delta: u32,
    /// 0 = `rank_percent` is an absolute position, 1 = it is a percentage.
    pub rank_position_kind: u8,
    pub rank_percent: u8,
    pub renew_sec: u32,
    pub end_sec: u32,
    pub entries: Vec<ArenaRankRow>,
}

impl ArenaRankingInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ARENA_REQUEST_RANK);
        writer.write_u8(self.ranking_type);
        writer.write_u8(self.result);
        writer.write_u8(self.status);
        writer.write_u32(self.point);
        writer.write_u32(self.rank);
        writer.write_u32(self.position_delta);
        writer.write_u8(self.rank_position_kind);
        writer.write_u8(self.rank_percent);
        writer.write_u32(self.renew_sec);
        writer.write_u32(self.end_sec);
        writer.write_u8(self.entries.len().min(ARENA_RANK_ROWS_MAX) as u8);
        for entry in self.entries.iter().take(ARENA_RANK_ROWS_MAX) {
            writer.write_u8(entry.rank);
            writer.write_modern_utf8_string(&entry.name);
            writer.write_modern_utf8_string(&entry.guild);
            writer.write_u32(entry.points);
            writer.write_u8(u8::from(entry.is_new));
            writer.write_i8(entry.position_delta);
        }
        writer.finalize()
    }
}

/// `ArenaRequestOldRank` — opcode 16024: the previous period's board.
///
/// `{u1 rank_type; u1 result; u4 rank; u1 count}` then archived rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaOldRankingInfoPacket {
    pub ranking_type: u8,
    pub result: u8,
    /// The tamer's own position in the archived board, 0 when absent.
    pub rank: u32,
    pub entries: Vec<ArenaArchiveRankRow>,
}

impl ArenaOldRankingInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ARENA_REQUEST_OLD_RANK);
        writer.write_u8(self.ranking_type);
        writer.write_u8(self.result);
        writer.write_u32(self.rank);
        writer.write_u8(self.entries.len().min(ARENA_RANK_ROWS_MAX) as u8);
        for entry in self.entries.iter().take(ARENA_RANK_ROWS_MAX) {
            writer.write_u8(entry.rank);
            writer.write_modern_utf8_string(&entry.name);
            writer.write_modern_utf8_string(&entry.guild);
            writer.write_u32(entry.points);
        }
        writer.finalize()
    }
}

/// `ArenaRequestFame` — opcode 16025: the hall of fame for one season.
///
/// `{u2 season; u1 result; u1 count}` then archived rows. The season is
/// echoed back so the client can key its per-season cache.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArenaFameInfoPacket {
    pub season: u16,
    pub result: u8,
    pub entries: Vec<ArenaArchiveRankRow>,
}

impl ArenaFameInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ARENA_REQUEST_FAME);
        writer.write_u16(self.season);
        writer.write_u8(self.result);
        writer.write_u8(self.entries.len().min(ARENA_FAME_ROWS_MAX) as u8);
        for entry in self.entries.iter().take(ARENA_FAME_ROWS_MAX) {
            writer.write_u8(entry.rank);
            writer.write_modern_utf8_string(&entry.name);
            writer.write_modern_utf8_string(&entry.guild);
            writer.write_u32(entry.points);
        }
        writer.finalize()
    }
}

/// `ArenaRequestFameList` — opcode 16026: the seasons that have a hall of fame.
///
/// `{u2 count}` then `count` season numbers in ascending order.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ArenaFameSeasonListPacket {
    pub seasons: Vec<u16>,
}

impl ArenaFameSeasonListPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ARENA_REQUEST_FAME_LIST);
        writer.write_u16(self.seasons.len() as u16);
        for season in &self.seasons {
            writer.write_u16(*season);
        }
        writer.finalize()
    }
}

/// `DungeonArenaNextStage` — opcode 4126.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DungeonArenaNextStagePacket {
    pub current_stage: u8,
    pub npc_id: i32,
    pub remain_time: i32,
}

impl DungeonArenaNextStagePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DUNGEON_NEXT_STAGE);
        writer.write_u8(self.current_stage);
        writer.write_i32(self.npc_id);
        writer.write_i32(self.remain_time);
        writer.finalize()
    }
}

// ===========================================================================
// Event packets
// ===========================================================================

/// `DailyCheckEventInfo` - opcode 3136.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyCheckEventInfoRow {
    pub group_id: i32,
    pub current_day: i32,
    pub next_left_seconds: i32,
    pub claimed_days: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyCheckEventInfoPacket {
    pub rows: Vec<DailyCheckEventInfoRow>,
}

impl DailyCheckEventInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DAILY_CHECK_EVENT);
        let len = self.rows.len().min(u16::MAX as usize) as u16;
        writer.write_u16(len);
        for row in self.rows.iter().take(u16::MAX as usize) {
            writer.write_i32(row.group_id);
            writer.write_i32(row.current_day);
            writer.write_i32(row.next_left_seconds);
            writer.write_bytes(&row.claimed_days);
        }
        writer.finalize()
    }
}

/// `DailyCheckEventItemResult` — opcode 3137.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DailyCheckEventItemResultPacket {
    pub result: i32,
    pub group_id: i32,
    pub current_day: i32,
    pub next_left_seconds: i32,
    /// Each tuple: (slot, item bytes — modern network array).
    pub items: Vec<(u16, Vec<u8>)>,
}

impl DailyCheckEventItemResultPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::DAILY_CHECK_EVENT_REQUEST);
        writer.write_i32(self.result);
        writer.write_i32(self.group_id);
        writer.write_i32(self.current_day);
        writer.write_i32(self.next_left_seconds);
        writer.write_u8(self.items.len().min(u8::MAX as usize) as u8);
        for (slot, item_bytes) in self.items.iter().take(u8::MAX as usize) {
            writer.write_u16(*slot);
            writer.write_bytes(item_bytes);
        }
        writer.finalize()
    }
}

// ===========================================================================
// Misc packets
// ===========================================================================

/// `UpdateCurrentTitle` — opcode 15.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateCurrentTitlePacket {
    pub handler: u32,
    pub title_id: i16,
}

impl UpdateCurrentTitlePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SET_TITLE);
        writer.write_u32(self.handler);
        writer.write_i16(self.title_id);
        writer.finalize()
    }
}

/// `ChangeTamerModel` — opcode 1314.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeTamerModelPacket {
    pub new_model: i32,
    pub item_slot: i16,
}

impl ChangeTamerModelPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::CHANGE_TAMER_MODEL);
        writer.write_i32(self.new_model);
        writer.write_i16(self.item_slot);
        writer.finalize()
    }
}

/// `TamerChangeName` — opcode 1311.
///
/// The leading result code selects the body, and the reader stops at the code for
/// the two rejection forms, so an unrecognised code would be parsed as an applied
/// rename. Rejected is `[i32 result]`, Nearby is
/// `[i32 result = 3][string previous][string next]`, and Applied is
/// `[i32 result = 0][i32 item_slot][string name]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TamerChangeNamePacket {
    /// The name was not applied; the client only clears the pending cash request.
    Rejected {
        result: i32,
    },
    /// A tamer nearby changed name, carried to every other viewer.
    Nearby {
        previous: String,
        next: String,
    },
    /// The requester's own rename completed.
    Applied {
        item_slot: i32,
        name: String,
    },
}

impl TamerChangeNamePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TAMER_NAME_CHANGE);
        match self {
            Self::Rejected { result } => writer.write_i32(*result),
            Self::Nearby { previous, next } => {
                writer.write_i32(3);
                writer.write_modern_utf8_string(previous);
                writer.write_modern_utf8_string(next);
            }
            Self::Applied { item_slot, name } => {
                writer.write_i32(0);
                writer.write_i32(*item_slot);
                writer.write_modern_utf8_string(name);
            }
        }
        writer.finalize()
    }
}

/// `RemoveBuff` — opcode 4002.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemoveBuffPacket {
    pub handler: u32,
    pub buff_id: u16,
    pub amount: i16,
}

impl RemoveBuffPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(4002);
        writer.write_u32(self.handler);
        writer.write_i16(self.amount);
        writer.write_u16(self.buff_id);
        writer.finalize()
    }
}

/// `MonsterRespawnTimer` — opcode 16064.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonsterRespawnTimerRow {
    pub state: u8,
    pub target_mob_type: i32,
    pub remaining_value: i32,
    pub source_mob_type: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonsterRespawnTimerPacket {
    pub rows: Vec<MonsterRespawnTimerRow>,
}

impl MonsterRespawnTimerPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MONSTER_RESPAWN_TIMER);
        writer.write_u8(1);
        let len = self.rows.len().min(u8::MAX as usize) as u8;
        writer.write_u8(len);
        for row in self.rows.iter().take(u8::MAX as usize) {
            writer.write_u8(row.state);
            writer.write_i32(row.target_mob_type);
            writer.write_i32(row.remaining_value);
            writer.write_i32(row.source_mob_type);
        }
        writer.finalize()
    }
}

/// `InventorySort` — opcode 3980.
/// `[u8 inventory_type][i32 0][i16 size][bytes inventory_data]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventorySortPacket {
    pub inventory_type: u8,
    pub size: i16,
    pub inventory_data: Vec<u8>,
}

impl InventorySortPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::INVENTORY_SORT);
        writer.write_u8(self.inventory_type);
        writer.write_i32(0);
        writer.write_i16(self.size);
        writer.write_bytes(&self.inventory_data);
        writer.finalize()
    }
}

/// `ItemReturn` notice — opcode 3923.
/// Modern read side consumes a fixed 12-byte body `{u4; i64}` and hands both
/// fields to event 151; the legacy trailing i32 is off the wire. Field
/// semantics stay the inventory-bit refund pair computed by the application
/// flow; the transferred modern label for this op is a lead only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemReturnPacket {
    pub received_bits: i32,
    pub previous_bits: i64,
}

impl ItemReturnPacket {
    pub fn encode(&self) -> Vec<u8> {
        // Modern read side stops after {u4; i64}; trailing bytes would leave
        // stale data in the client's packet ring buffer.
        let mut writer = PacketWriter::new(game::ITEM_RETURN);
        writer.write_i32(self.received_bits);
        writer.write_i64(self.previous_bits);
        writer.finalize()
    }
}

/// `ItemSocketIn` — opcode 3926.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSocketInPacket {
    pub money: i32,
}

impl ItemSocketInPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_SOCKET_IN);
        writer.write_i32(100);
        writer.write_i32(self.money);
        writer.write_i32(0);
        writer.finalize()
    }
}

/// `ItemSocketOut` — opcode 3927.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSocketOutPacket {
    pub money: i32,
}

impl ItemSocketOutPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_SOCKET_OUT);
        writer.write_i32(100);
        writer.write_i32(self.money);
        writer.write_i32(0);
        writer.finalize()
    }
}

/// `ItemSocketIdentify` — opcode 3929.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemSocketIdentifyPacket {
    pub power: u8,
    pub money: i32,
}

impl ItemSocketIdentifyPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_SOCKET_IDENTIFY);
        writer.write_u8(self.power);
        writer.write_i32(self.money);
        writer.write_i32(0);
        writer.finalize()
    }
}

/// `ItemIdentify` — opcode 3968.
/// `[i16 slot][u8 power][u8 reroll_left][i16*4 types][i16*4 values]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemIdentifyPacket {
    pub slot: i16,
    pub power: u8,
    pub reroll_left: u8,
    pub types: [i16; 4],
    pub values: [i16; 4],
}

impl ItemIdentifyPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_IDENTIFY);
        writer.write_i16(self.slot);
        writer.write_u8(self.power);
        writer.write_u8(self.reroll_left);
        for ty in self.types {
            writer.write_i16(ty);
        }
        for val in self.values {
            writer.write_i16(val);
        }
        writer.finalize()
    }
}

/// `ItemReroll` — opcode 3969.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRerollPacket {
    pub result: u8,
    pub accessory_slot: i16,
    pub power: u8,
    pub reroll_left: u8,
    pub types: [i16; 4],
    pub values: [i16; 4],
}

impl ItemRerollPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_REROLL);
        writer.write_u8(self.result);
        writer.write_i16(self.accessory_slot);
        writer.write_u8(self.power);
        writer.write_u8(self.reroll_left);
        for ty in self.types {
            writer.write_i16(ty);
        }
        for val in self.values {
            writer.write_i16(val);
        }
        writer.finalize()
    }
}

/// `LoadGiftStorage` and `LoadRewardStorage` use the same envelope (opcodes 3935 / 16001):
/// `[i32 count][item_record bytes per entry]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemStoragePacket {
    pub opcode: i16,
    pub items: Vec<odmo_types::ItemRecord>,
}

impl ItemStoragePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_i32(self.items.len() as i32);
        for item in &self.items {
            writer.write_bytes(&item.record);
        }
        writer.finalize()
    }
}

/// `RecompenseGain` (`16002`) result payload — `[i32 result]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecompenseGainPacket {
    pub result: i32,
}

impl RecompenseGainPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::RECOMPENSE_GAIN);
        writer.write_i32(self.result);
        writer.finalize()
    }
}

/// `GiftStorageRetrieve` (`3936`) — same shape as recompense.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GiftStorageRetrievePacket {
    pub result: i32,
}

impl GiftStorageRetrievePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GIFT_STORAGE_RETRIEVE);
        writer.write_i32(self.result);
        writer.finalize()
    }
}

/// `LevelUp` — opcode 1019. `[u32 handler][u8 level]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LevelUpPacket {
    pub handler: u32,
    pub level: u8,
}

impl LevelUpPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(1019);
        writer.write_u32(self.handler);
        writer.write_u8(self.level);
        writer.finalize()
    }
}

/// `ReceiveExp` — opcode 1018.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiveExpPacket {
    pub tamer_exp: i64,
    pub tamer_bonus: i64,
    pub tamer_total: i64,
    pub partner_handler: u32,
    pub partner_exp: i64,
    pub partner_bonus: i64,
    pub partner_total: i64,
    pub skill_exp: i64,
}

impl ReceiveExpPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(1018);
        writer.write_i64(self.tamer_exp);
        writer.write_i64(self.tamer_bonus);
        writer.write_i64(self.tamer_total);
        writer.write_u32(self.partner_handler);
        writer.write_i64(self.partner_exp);
        writer.write_i64(self.partner_bonus);
        writer.write_i64(self.partner_total);
        writer.write_i64(self.skill_exp);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigimonEvolutionSuccessPacket {
    pub digimon_handler: u32,
    pub tamer_handler: u32,
    pub new_type: i32,
    pub evolution_slot: u8,
    pub hp_rate: u8,
    pub parts_type: i32,
}

impl DigimonEvolutionSuccessPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::EVOLUTION);
        writer.write_u32(self.digimon_handler);
        writer.write_u32(self.tamer_handler);
        writer.write_i32(self.new_type);
        writer.write_u8(self.evolution_slot);
        writer.write_u8(self.hp_rate);
        writer.write_i32(self.parts_type);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PartnerSwitchPacket {
    pub handler: u32,
    pub old_partner_current_type: i32,
    pub slot: u8,
    pub partner: odmo_types::PartnerSlotSnapshot,
}

impl PartnerSwitchPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTNER_SWITCH_RESPONSE);
        writer.write_u32(self.handler);
        writer.write_i32(self.old_partner_current_type);
        writer.write_u8(self.slot);
        writer.write_i32(self.partner.digimon_type);
        writer.write_u8(self.partner.level);
        writer.write_fixed_wide_string(&self.partner.name, 32);
        writer.write_i16(self.partner.size);
        writer.write_i32(0);
        writer.write_u16(self.partner.clone_level);
        writer.write_u16(self.partner.clone_at_value);
        writer.write_u16(self.partner.clone_bl_value);
        writer.write_u16(self.partner.clone_ct_value);
        writer.write_u16(0);
        writer.write_u16(self.partner.clone_ev_value);
        writer.write_u16(0);
        writer.write_u16(self.partner.clone_hp_value);
        writer.write_u16(self.partner.clone_at_level);
        writer.write_u16(self.partner.clone_bl_level);
        writer.write_u16(self.partner.clone_ct_level);
        writer.write_u16(0);
        writer.write_u16(self.partner.clone_ev_level);
        writer.write_u16(0);
        writer.write_u16(self.partner.clone_hp_level);
        writer.write_u16(self.partner.active_buffs.len() as u16);
        for buff in &self.partner.active_buffs {
            writer.write_u16(buff.buff_id);
            writer.write_u16(buff.buff_class);
            writer.write_u32(buff.remaining_seconds.max(0) as u32);
            writer.write_i32(buff.skill_id);
        }
        writer.write_i32(self.partner.hp);
        writer.write_i32(self.partner.ds);
        writer.write_i32(self.partner.de);
        writer.write_i32(self.partner.at);
        writer.write_i32(self.partner.current_hp);
        writer.write_i32(self.partner.current_ds);
        writer.write_i32(self.partner.fs);
        writer.write_i32(0);
        writer.write_i32(self.partner.ev);
        writer.write_i32(self.partner.cc);
        writer.write_i32(self.partner.ms);
        writer.write_i32(self.partner.as_value);
        writer.write_i32(self.partner.ar);
        writer.write_i32(self.partner.ht);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.write_i32(0);
        writer.write_i32(self.partner.bl);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyMemberDisconnectedPacket {
    pub member_slot: i32,
}

impl PartyMemberDisconnectedPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::PARTY_MEMBER_DISCONNECTED);
        writer.write_i32(self.member_slot);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TamerAttendancePacket {
    pub attendance: AttendanceStatus,
}

impl TamerAttendancePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TAMER_ATTENDANCE);
        writer.write_u8(self.attendance.event_no);
        writer.write_u8(self.attendance.attendance_count);
        writer.write_u8(self.attendance.notify as u8);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildInformationPacket {
    pub guild: GuildSnapshot,
}

impl GuildInformationPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_INFORMATION);
        writer.write_modern_utf8_string(&self.guild.name);
        writer.write_i32(self.guild.id.min(i32::MAX as u32) as i32);
        writer.write_u8(self.guild.level);
        writer.write_i32(self.guild.current_experience);
        writer.write_modern_utf8_string(&self.guild.notice);
        writer.write_i32(self.guild.extra_slots);

        for authority in normalized_guild_authorities(&self.guild) {
            writer.write_modern_utf8_string(&authority.title);
            writer.write_modern_utf8_string(&authority.duty);
        }

        let mut members = self.guild.members.clone();
        members.sort_by_key(|member| (std::cmp::Reverse(member.map_id), member.authority));
        for member in members {
            writer.write_u8(member.authority);
            writer.write_u8(guild_member_model(&member));
            writer.write_modern_utf8_string(&member.character_name);
            writer.write_i32(member.contribution);
            writer.write_u8(member.character_level);

            if is_online_state(member.state) {
                writer.write_i16(member.map_id);
                writer.write_u8(member.channel);
            } else {
                writer.write_i16(0);
            }
        }

        writer.write_u8(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildHistoricPacket {
    pub entries: Vec<GuildHistoricEntry>,
}

impl GuildHistoricPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_HISTORIC);
        for entry in &self.entries {
            writer.write_u8(entry.historic_type);
            writer.write_i32(entry.date_utc_seconds.min(i32::MAX as u32) as i32);
            writer.write_u8(entry.master_class);
            writer.write_modern_utf8_string(&entry.master_name);
            writer.write_u8(entry.member_class);
            writer.write_modern_utf8_string(&entry.member_name);
        }
        writer.write_u8(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildRankPacket {
    pub position: i16,
}

impl GuildRankPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_RANK);
        writer.write_i16(self.position);
        writer.finalize()
    }
}

/// `GuildCreate` success response — opcode 2101.
/// Mirrors `GuildCreateSuccessPacket(string leaderName, int itemSlot, string guildName)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildCreateSuccessPacket {
    pub leader_name: String,
    pub item_slot: i32,
    pub guild_name: String,
}

impl GuildCreateSuccessPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_CREATE);
        writer.write_modern_utf8_string(&self.leader_name);
        writer.write_i32(self.item_slot);
        writer.write_modern_utf8_string(&self.guild_name);
        writer.finalize()
    }
}

/// `GuildCreate` failure response — opcode 2101 with `itemSlot = -1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildCreateFailPacket {
    pub leader_name: String,
    pub guild_name: String,
}

impl GuildCreateFailPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_CREATE);
        writer.write_modern_utf8_string(&self.leader_name);
        writer.write_i32(-1);
        writer.write_modern_utf8_string(&self.guild_name);
        writer.finalize()
    }
}

/// `GuildDelete` notification — opcode 2102.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildDeletePacket {
    pub guild_name: String,
}

impl GuildDeletePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_DELETE);
        writer.write_modern_utf8_string(&self.guild_name);
        writer.finalize()
    }
}

/// `GuildInvite` success response — opcode 2109.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildInviteSuccessPacket {
    pub target_name: String,
    pub guild_id: u32,
    pub guild_name: String,
}

impl GuildInviteSuccessPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_INVITE);
        writer.write_modern_utf8_string(&self.target_name);
        writer.write_u32(self.guild_id);
        writer.write_modern_utf8_string(&self.guild_name);
        writer.finalize()
    }
}

/// `GuildInvite` failure — opcode 2110.
/// Reason codes mirror `GuildInviteFailEnum` from the legacy server (1=already in guild,
/// 2=offline, 3=on cooldown, 4=invalid target).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildInviteFailPacket {
    pub reason: i32,
    pub target_name: String,
}

impl GuildInviteFailPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_INVITE_FAIL);
        writer.write_i32(self.reason);
        writer.write_modern_utf8_string(&self.target_name);
        writer.finalize()
    }
}

/// `GuildInviteAccept` projection of the new member — opcode 2108.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildInviteAcceptPacket {
    pub authority: u8,
    pub member_model: u8,
    pub character_name: String,
    pub level: u8,
    pub map_id: i16,
    pub channel: u8,
    pub guild_name: String,
}

impl GuildInviteAcceptPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_INVITE_RESPONSE_ACCEPT);
        writer.write_u8(self.authority);
        writer.write_u8(self.member_model);
        writer.write_modern_utf8_string(&self.character_name);
        writer.write_u8(self.level);
        writer.write_i16(self.map_id);
        writer.write_u8(self.channel);
        writer.write_modern_utf8_string(&self.guild_name);
        writer.finalize()
    }
}

/// `GuildInviteDeny` notification — opcode 2105.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildInviteDenyPacket {
    pub target_name: String,
}

impl GuildInviteDenyPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_INVITE_DENY);
        writer.write_modern_utf8_string(&self.target_name);
        writer.finalize()
    }
}

/// `GuildMemberKick` notification — opcode 2106.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildMemberKickPacket {
    pub target_name: String,
}

impl GuildMemberKickPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_KICK);
        writer.write_modern_utf8_string(&self.target_name);
        writer.finalize()
    }
}

/// `GuildMemberQuit` notification — opcode 2107.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildMemberQuitPacket {
    pub target_name: String,
}

impl GuildMemberQuitPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_LEAVE);
        writer.write_modern_utf8_string(&self.target_name);
        writer.finalize()
    }
}

/// `GuildNoticeUpdate` — opcode 2126.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildNoticeUpdatePacket {
    pub notice: String,
}

impl GuildNoticeUpdatePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GUILD_NOTICE);
        writer.write_modern_utf8_string(&self.notice);
        writer.finalize()
    }
}

/// `GuildAuthorityUpdate` — opcode 2129.
/// Carries the new authority class and labels for a single rank.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildAuthorityUpdatePacket {
    pub authority_class: u8,
    pub title: String,
    pub duty: String,
}

impl GuildAuthorityUpdatePacket {
    pub fn encode(&self) -> Vec<u8> {
        // Modern wire: {u1 class; str title; str duty} - no trailing byte, the
        // final legacy u8 is absent from the read side.
        let mut writer = PacketWriter::new(game::GUILD_SET_TITLE);
        writer.write_u8(self.authority_class);
        writer.write_modern_utf8_string(&self.title);
        writer.write_modern_utf8_string(&self.duty);
        writer.finalize()
    }
}

/// `ItemScanNotice` notice — opcode 3988 (receive table). Modern parser reads `{u4;
/// i64; u2 count; count x u2}` and assembles one 12-byte list node per row before event
/// 96. Field meanings stay unattributed; the row count drives the list length.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemScanNoticePacket {
    pub first: u32,   // unresolved
    pub second: i64,  // unresolved
    pub entries: Vec<u16>,
}

impl ItemScanNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ITEM_SCAN_NOTICE);
        writer.write_u32(self.first);
        writer.write_i64(self.second);
        writer.write_u16(self.entries.len() as u16);
        for entry in &self.entries {
            writer.write_u16(*entry);
        }
        writer.finalize()
    }
}

/// `AdvertBossCount` notice — opcode 1059. Modern parser reads three u4 fields, then
/// either resolves the existing advert-boss entry or creates one; all three field
/// meanings stay unattributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvertBossCountNoticePacket {
    pub first: u32,   // unresolved
    pub second: u32,  // unresolved
    pub third: u32,   // unresolved
}

impl AdvertBossCountNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ADVERT_BOSS_COUNT);
        writer.write_u32(self.first);
        writer.write_u32(self.second);
        writer.write_u32(self.third);
        writer.finalize()
    }
}

/// `AdvertBossDie` notice — opcode 1061. Modern parser reads two u4 fields and hands
/// the first to the removal dispatch; the second gates whether the dispatch runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvertBossDieNoticePacket {
    pub first: u32,   // unresolved
    pub second: u32,  // unresolved
}

impl AdvertBossDieNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ADVERT_BOSS_DIE);
        writer.write_u32(self.first);
        writer.write_u32(self.second);
        writer.finalize()
    }
}

/// `AdvertBossCreate` notice — opcode 1060: {u4 map; u4 channel; u4 monster id;
/// u4 created count}.
///
/// The announcement is broadcast to every tamer in the map. The map and monster
/// keys resolve the place and the spawned boss name, the channel lets a tamer on
/// another channel ignore the spawn, and the count is carried for display even
/// though no consumer uses it yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdvertBossCreateNoticePacket {
    pub map_id: u32,        // keyed through CMapTable (table id 33)
    pub channel_id: u32,    // selects the channel that owns the spawn
    pub monster_id: u32,    // keyed through CMonsterTable (table id 39)
    pub created_count: u32, // spawned count, carried but not consumed
}

impl AdvertBossCreateNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::ADVERT_BOSS_CREATE);
        writer.write_u32(self.map_id);
        writer.write_u32(self.channel_id);
        writer.write_u32(self.monster_id);
        writer.write_u32(self.created_count);
        writer.finalize()
    }
}

/// `DigimonSkillLimitOpen` notice — opcode 3245. Modern parser: a nonzero first u4
/// short-circuits into a plain disable call; otherwise the client updates one digimon
/// row (bounded index 19) with a 28-byte inline skill blob, validates the u4 skill
/// index against a 200-entry skill table using the final u4, and pings event 477. Blob
/// and flag meanings stay unattributed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigimonSkillLimitOpenPacket {
    pub state: u32,             // unresolved (0 = full update path)
    pub digimon_index: u32,     // bounded to 19 by the read side
    pub skill_blob: Vec<u8>,    // 28 inline bytes, row-shaped
    pub skill_index: u32,       // bounded to 199 by the read side
    pub expected_value: u32,    // compared against the table-row u16
}

impl DigimonSkillLimitOpenPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert_eq!(self.skill_blob.len(), 28, "skill blob is fixed-size on the wire");
        let mut writer = PacketWriter::new(game::SKILL_LIMIT_OPEN);
        writer.write_u32(self.state);
        writer.write_u32(self.digimon_index);
        writer.write_bytes(&self.skill_blob);
        writer.write_u32(self.skill_index);
        writer.write_u32(self.expected_value);
        writer.finalize()
    }
}

/// `MacroFailure` notice — opcode 1066: one u4 handed to event 359.
/// `MacroResult` notice — opcode 1068: one u4 handed to event 360.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MacroResultNoticePacket {
    pub result: u32, // unresolved
}

impl MacroResultNoticePacket {
    /// Encode the failure variant (1066) or the result variant (1068).
    pub fn encode_on(&self, opcode: i16) -> Vec<u8> {
        let mut writer = PacketWriter::new(opcode);
        writer.write_u32(self.result);
        writer.finalize()
    }
}

/// `MacroSuccess` notice — opcode 1067: empty payload; the client dispatches
/// event 359 with a locally zeroed struct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MacroSuccessNoticePacket;

impl MacroSuccessNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        PacketWriter::new(game::MACRO_SUCCESS).finalize()
    }
}

/// Dropped-slot notice — opcodes 1071 and 3228 (shared parser): the client consumes one
/// u4 and discards it without dispatching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DroppedSlotNoticePacket {
    pub reserved: u32, // unresolved (read and dropped)
}

impl DroppedSlotNoticePacket {
    pub fn encode_on(&self, opcode: i16) -> Vec<u8> {
        let mut writer = PacketWriter::new(opcode);
        writer.write_u32(self.reserved);
        writer.finalize()
    }
}

/// `GetExp` notice — opcode 1018. Modern parser reads five i64 quantity pairs
/// interleaved with two u4 fields: `{i64; i64; i64; u4; i64; i64; u4}` (48 bytes). All
/// five pairs go through the same x100 fixed-point conversion; the first trailing u4 is
/// consumed as u16 and the last feeds a float exp path. Individual pair meanings stay
/// unattributed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameGetExpNoticePacket {
    pub first: i64,    // unresolved
    pub second: i64,   // unresolved
    pub third: i64,    // unresolved
    pub flag: u32,     // unresolved (read side uses u16)
    pub fourth: i64,   // unresolved
    pub fifth: i64,    // unresolved
    pub exp_value: u32, // unresolved (feeds the float exp path)
}

impl GameGetExpNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_GET_EXP);
        writer.write_i64(self.first);
        writer.write_i64(self.second);
        writer.write_i64(self.third);
        writer.write_u32(self.flag);
        writer.write_i64(self.fourth);
        writer.write_i64(self.fifth);
        writer.write_u32(self.exp_value);
        writer.finalize()
    }
}

/// `TargetInfo` notice — opcode 1017. Modern parser reads `{u4 target handler; u1 flag;
/// u4; u4}`; the handler resolves through the entity lookup and the flag feeds a slot
/// setter, while the last two u4 fields are stored verbatim on the tracked entity
/// (their delta semantics stay unattributed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameTargetInfoPacket {
    pub target_handler: u32, // resolved by the client entity lookup
    pub flag: u8,            // feeds a slot setter
    pub first: u32,          // stored verbatim on the entity
    pub second: u32,         // stored as (field - value) on the entity
}

impl GameTargetInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_TARGET_INFO);
        writer.write_u32(self.target_handler);
        writer.write_u8(self.flag);
        writer.write_u32(self.first);
        writer.write_u32(self.second);
        writer.finalize()
    }
}

/// `ChangeStat` notice — opcode 1084. Modern parser reads `{u4 handler; u2 count; count
/// x {u2 stat; u4 value}}`; stat selectors 1.10 and 12 dispatch into distinct setters
/// (9 renders a float), other selectors are skipped. After the loop the client pings an
/// empty event 590 (type 13) or 476 (type 11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameChangeStatPacket {
    pub handler: u32, // resolved by the client entity lookup
    pub stats: Vec<(u16, u32)>,
}

impl GameChangeStatPacket {
    pub fn encode(&self) -> Vec<u8> {
        assert!(self.stats.len() <= u16::MAX as usize, "stat count overflows u2");
        let mut writer = PacketWriter::new(game::GAME_CHANGE_STAT);
        writer.write_u32(self.handler);
        writer.write_u16(self.stats.len() as u16);
        for (stat, value) in &self.stats {
            writer.write_u16(*stat);
            writer.write_u32(*value);
        }
        writer.finalize()
    }
}

/// `GameEvent` notice — opcode 1062: the client consumes `{u2; u4}` with no
/// dispatch; both slots stay reserved to keep the cursor aligned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GameEventNoticePacket {
    pub first: u16,  // unresolved (consumed silently)
    pub second: u32, // unresolved (consumed silently)
}

impl GameEventNoticePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_EVENT);
        writer.write_u16(self.first);
        writer.write_u32(self.second);
        writer.finalize()
    }
}

/// `GuildPromotionDemotion` — opcodes 2115/2116/2117/2118/2119.
/// The per-rank authority change messages share one payload: the affected
/// member's name. The rank label is chosen by the client from the opcode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildPromotionDemotionPacket {
    pub opcode: i16,
    pub member_name: String,
}

impl GuildPromotionDemotionPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(self.opcode);
        writer.write_modern_utf8_string(&self.member_name);
        writer.finalize()
    }
}

/// `GuildMessage` (chat) — opcode 2114.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildMessagePacket {
    pub sender_name: String,
    pub message: String,
}

impl GuildMessagePacket {
    pub fn encode(&self) -> Vec<u8> {
        // Modern parser reads two strings only; the legacy leading handler
        // u4 is no longer on the wire.
        let mut writer = PacketWriter::new(game::GUILD_MESSAGE);
        writer.write_modern_utf8_string(&self.sender_name);
        writer.write_modern_utf8_string(&self.message);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XaiInfoPacket {
    pub xai: Option<XaiSnapshot>,
}

impl XaiInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::XAI_INFO);
        writer.write_i32(self.xai.as_ref().map_or(0, |xai| xai.max_xgauge));
        writer.write_i16(self.xai.as_ref().map_or(0, |xai| xai.max_xcrystals));
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TamerXaiResourcesPacket {
    pub current_xgauge: i32,
    pub current_xcrystals: i16,
}

impl TamerXaiResourcesPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::TAMER_XAI_RESOURCES);
        writer.write_i32(self.current_xgauge);
        writer.write_i16(self.current_xcrystals);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateStatusPacket {
    pub character: CharacterSummary,
}

impl UpdateStatusPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::UPDATE_STATUS);
        writer.write_i32(self.character.hp);
        writer.write_i32(self.character.ds);
        writer.write_i32(self.character.current_hp);
        writer.write_i32(self.character.current_ds);
        writer.write_u16(clamp_u16(self.character.at));
        writer.write_u16(clamp_u16(self.character.de));
        writer.write_u16(clamp_u16(self.character.ms));
        writer.write_i32(self.character.partner_hp);
        writer.write_i32(self.character.partner_ds);
        writer.write_i32(self.character.partner_current_hp);
        writer.write_i32(self.character.partner_current_ds);
        writer.write_u16(clamp_u16(self.character.partner_fs));
        writer.write_u16(clamp_u16(self.character.partner_at));
        writer.write_u16(clamp_u16(self.character.partner_de));
        writer.write_u16(clamp_u16(self.character.partner_cc));
        writer.write_u16(clamp_u16(self.character.partner_as));
        writer.write_u16(clamp_u16(self.character.partner_ev));
        writer.write_u16(clamp_u16(self.character.partner_ht));
        writer.write_u16(clamp_u16(self.character.partner_ar));
        writer.write_u16(clamp_u16(self.character.partner_bl));
        writer.write_u16(self.character.partner_clone_level);
        writer.write_u16(self.character.partner_clone_at_value);
        writer.write_u16(self.character.partner_clone_bl_value);
        writer.write_u16(self.character.partner_clone_ct_value);
        writer.write_u16(0);
        writer.write_u16(self.character.partner_clone_ev_value);
        writer.write_u16(0);
        writer.write_u16(self.character.partner_clone_hp_value);
        writer.write_u16(self.character.partner_clone_at_level);
        writer.write_u16(self.character.partner_clone_bl_level);
        writer.write_u16(self.character.partner_clone_ct_level);
        writer.write_u16(0);
        writer.write_u16(self.character.partner_clone_ev_level);
        writer.write_u16(0);
        writer.write_u16(self.character.partner_clone_hp_level);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateMovementSpeedPacket {
    pub character: CharacterSummary,
}

impl UpdateMovementSpeedPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::UPDATE_MOVEMENT_SPEED);
        writer
            .write_u32(non_zero_handler(self.character.general_handler, self.character.id) as u32);
        writer.write_u32(non_zero_handler(
            self.character.partner_handler,
            self.character.id.saturating_add(10_000),
        ) as u32);

        let effective_speed = if self.character.current_condition == 1 {
            self.character.proper_ms.saturating_mul(2)
        } else {
            self.character.proper_ms
        };

        writer.write_i16(effective_speed);
        writer.write_i16(effective_speed);
        writer.write_i32(self.character.current_condition);
        writer.write_i32(self.character.partner_condition);
        writer.finalize()
    }
}

fn write_empty_item_records(writer: &mut PacketWriter, slots: usize) {
    for _ in 0..slots {
        writer.write_zeroes(60);
    }
}

fn write_zero_i16s(writer: &mut PacketWriter, count: usize) {
    for _ in 0..count {
        writer.write_i16(0);
    }
}

fn normalize_item_record(item: &ItemRecord) -> Vec<u8> {
    let mut record = item.record.clone();
    if record.len() > 69 {
        record.truncate(69);
    } else if record.len() < 69 {
        record.resize(69, 0);
    }
    record
}

fn write_map_region(writer: &mut PacketWriter, region: &[u8]) {
    let capped_len = region.len().min(255);
    writer.write_bytes(&region[..capped_len]);
    if capped_len < 255 {
        writer.write_zeroes(255 - capped_len);
    }
}

fn non_zero_handler(raw: u32, fallback_id: u64) -> i32 {
    if raw == 0 {
        fallback_id.min(i32::MAX as u64) as i32
    } else {
        raw.min(i32::MAX as u32) as i32
    }
}

fn clamp_i16_len(len: usize) -> i16 {
    len.min(i16::MAX as usize) as i16
}

fn clamp_u16_len(len: usize) -> u16 {
    len.min(u16::MAX as usize) as u16
}

/// Wire size of a reward detail node. The leading item fields are decoded; the
/// remaining bytes are a reserved fixed-size block (stat/option blobs) carried
/// verbatim and zero-filled when the server has nothing to populate.
const COMBINE_REWARD_NODE_LEN: usize = 71;

/// Read a `u2 count` followed by `count` material nodes `{u4 item_uid, u2 item_type, u2 count}`.
fn read_combine_materials(reader: &mut PacketReader) -> Result<Vec<CombineItemRef>, ProtocolError> {
    let count = reader.read_u16()? as usize;
    let mut materials = Vec::with_capacity(count);
    for _ in 0..count {
        let item_uid = reader.read_u32()?;
        let item_type = reader.read_u16()?;
        let node_count = reader.read_u16()?;
        materials.push(CombineItemRef {
            item_uid,
            item_type,
            count: node_count,
        });
    }
    Ok(materials)
}

/// Write a `u2 count` followed by the material/echo nodes `{u4 item_uid, u2 item_type, u2 count}`.
fn write_combine_item_list(writer: &mut PacketWriter, items: &[CombineItemRef]) {
    writer.write_u16(clamp_u16_len(items.len()));
    for item in items {
        writer.write_u32(item.item_uid);
        writer.write_u16(item.item_type);
        writer.write_u16(item.count);
    }
}

/// Write the ceiling map block: `u2 count` followed by `{u1 tier, u1 value_a, u2 value_b}` entries.
fn write_combine_ceiling(writer: &mut PacketWriter, entries: &[CombineCeilingEntry]) {
    writer.write_u16(clamp_u16_len(entries.len()));
    for entry in entries {
        writer.write_u8(entry.tier);
        writer.write_u8(entry.value_a);
        writer.write_u16(entry.value_b);
    }
}

/// Write the reward detail list: `u2 count` followed by fixed-size reward nodes.
///
/// Each reward detail node carries the decoded leading item fields
/// `{n4 item_id, u2 amount, u1 grade}`; the remaining bytes of the fixed-size
/// node are reserved and emitted as zeros so the wire length matches the node
/// width the client expects.
fn write_combine_reward_list(writer: &mut PacketWriter, rewards: &[DigiCombineReward]) {
    writer.write_u16(clamp_u16_len(rewards.len()));
    for reward in rewards {
        writer.write_i32(reward.item_id);
        writer.write_u16(reward.amount);
        writer.write_u8(reward.grade);
        writer.write_zeroes(COMBINE_REWARD_NODE_LEN - 7);
    }
}

fn clamp_u8_len(len: usize) -> u8 {
    len.min(u8::MAX as usize) as u8
}

fn clamp_u16(value: i32) -> u16 {
    value.clamp(u16::MIN as i32, u16::MAX as i32) as u16
}

fn write_active_buff(writer: &mut PacketWriter, buff: &ActiveBuffSnapshot) {
    writer.write_u16(buff.buff_id);
    writer.write_i16(1);
    writer.write_i32(buff.remaining_seconds.max(0));
    writer.write_i32(buff.skill_id);
}

fn normalized_guild_authorities(guild: &GuildSnapshot) -> Vec<odmo_types::GuildAuthoritySnapshot> {
    let mut authorities = guild.authorities.clone();
    if authorities.len() < 5 {
        let defaults = odmo_types::GuildSnapshot::default().authorities;
        for authority in defaults.into_iter().skip(authorities.len()) {
            authorities.push(authority);
        }
    }
    authorities.truncate(5);
    authorities
}

fn guild_member_model(member: &odmo_types::GuildMemberSnapshot) -> u8 {
    member
        .character_model
        .saturating_sub(80_000)
        .clamp(u8::MIN as i32, u8::MAX as i32) as u8
}

fn write_party_member(writer: &mut PacketWriter, member: &PartyMemberListEntry) {
    writer.write_i32(member.party_slot);
    writer.write_i32(member.character.model);
    writer.write_i16(i16::from(member.character.level));
    writer.write_string(&member.character.name);
    writer.write_i32(member.character.partner_current_type);
    writer.write_i16(i16::from(member.character.partner_level));
    writer.write_string(&member.character.partner_name);
    writer.write_i32(i32::from(member.character.map_id));
    writer.write_i32(i32::from(member.character.channel));
    writer.write_u32(member.character.general_handler);
    writer.write_u32(member.character.partner_handler);
}

fn is_online_state(state: odmo_types::CharacterConnectionState) -> bool {
    matches!(
        state,
        odmo_types::CharacterConnectionState::Connected
            | odmo_types::CharacterConnectionState::Ready
    )
}

fn write_optional_modern_string(writer: &mut PacketWriter, value: &str) {
    if value.is_empty() {
        writer.write_u16(0);
    } else {
        writer.write_modern_utf8_string(value);
    }
}

fn non_empty_relation_name(entry: &RelationEntry) -> &str {
    if entry.name.is_empty() {
        "Friend"
    } else {
        &entry.name
    }
}

// --- Movement packets ---

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TamerWalkPacket {
    pub handler: u32,
    pub x: i32,
    pub y: i32,
}

impl TamerWalkPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MAP_ENTITY);
        writer.write_u8(5);
        writer.write_i16(1);
        writer.write_u32(self.handler);
        writer.write_i32(self.x);
        writer.write_i32(self.y);
        writer.write_i32(0);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigimonWalkPacket {
    pub handler: u32,
    pub x: i32,
    pub y: i32,
}

impl DigimonWalkPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::MAP_ENTITY);
        writer.write_u8(6);
        writer.write_i16(1);
        writer.write_u32(self.handler);
        writer.write_i32(self.x);
        writer.write_i32(self.y);
        writer.write_i32(0);
        writer.finalize()
    }
}

/// `SyncCondition` notice — opcode 1070: `{u4 handler; u4 condition}`. The condition is a bit
/// set, and when the personal-shop bit (`4`) is up the frame carries one more modern text with
/// that tamer's shop title. An empty title is legal: the client substitutes its own placeholder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncConditionPacket {
    pub handler: i32,
    pub condition: i32,
    pub shop_title: Option<String>, // modern text, present only with the shop bit
}

impl SyncConditionPacket {
    pub fn encode(&self) -> Vec<u8> {
        const CONDITION_SHOP: i32 = 4;
        let mut writer = PacketWriter::new(game::SYNC_CONDITION);
        writer.write_i32(self.handler);
        writer.write_i32(self.condition);
        if self.condition & CONDITION_SHOP != 0 {
            writer.write_modern_utf8_string(self.shop_title.as_deref().unwrap_or_default());
        }
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapSwapPacket {
    pub address: String,
    pub port: i32,
    pub map_id: i32, // widened from i16 on the modern read side
    pub x: i32,
    pub y: i32,
}

impl MapSwapPacket {
    pub fn encode(&self) -> Vec<u8> {
        // Modern wire: {str; u4 port; u4 map; u4 x; u4 y} - the legacy i16
        // map field reads as a full u4 now.
        let mut writer = PacketWriter::new(game::WARP_GATE);
        writer.write_modern_utf8_string(&self.address);
        writer.write_i32(self.port);
        writer.write_i32(self.map_id);
        writer.write_i32(self.x);
        writer.write_i32(self.y);
        writer.finalize()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalMapSwapPacket {
    pub tamer_handler: i32,
    pub partner_handler: i32,
    pub x: i32,
    pub y: i32,
}

impl LocalMapSwapPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::LOCAL_MAP_SWAP);
        writer.write_i32(self.tamer_handler);
        writer.write_i32(self.partner_handler);
        writer.write_i32(self.x);
        writer.write_i32(self.y);
        writer.finalize()
    }
}

/// Packet sent to update skill cooldowns for a digimon (opcode 3246).
#[derive(Debug, Clone)]
pub struct SkillUpdateCooldownPacket {
    pub handler: i32,
    pub current_type: i32,
    pub cooldowns: Vec<(i32, i32)>, // (skill_id, end_timestamp)
}

impl SkillUpdateCooldownPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SKILL_UPDATE_COOLDOWN);
        writer.write_i32(self.handler);
        writer.write_i32(self.current_type);
        writer.write_i32(self.cooldowns.len() as i32);
        for (skill_id, end_time) in &self.cooldowns {
            writer.write_i32(*skill_id);
            writer.write_i32(*end_time);
        }
        writer.finalize()
    }
}

/// `GameHpRate` notice — opcode 1007. Modern parser reads `{u4 handler_id; u1 rate}`; a
/// non-zero id resolves the entity and invokes its HP-rate setter (vm+108) with the
/// rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameHpRatePacket {
    pub handler_id: u32,
    pub rate: u8,
}

impl GameHpRatePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_HP_RATE);
        writer.write_u32(self.handler_id);
        writer.write_u8(self.rate);
        writer.finalize()
    }
}

/// `GameLevelUp` notice — opcode 1019: `{u4 handler_id; u1 level}`. A non-zero
/// id resolves through the entity table before the level event is raised.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameLevelUpPacket {
    pub handler_id: u32,
    pub level: u8,
}

impl GameLevelUpPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_LEVEL_UP);
        writer.write_u32(self.handler_id);
        writer.write_u8(self.level);
        writer.finalize()
    }
}

/// `GameBattleOn` notice — opcode 1034. Modern parser reads `{u4 handler_id}` and flags
/// the resolved entity as in-battle (vm+192 call); zero ids are ignored client-side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameBattleOnPacket {
    pub handler_id: u32,
}

impl GameBattleOnPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_BATTLE_ON);
        writer.write_u32(self.handler_id);
        writer.finalize()
    }
}

/// `GameBattleOff` notice — opcode 1035. Modern parser reads `{u4 handler_id}`, clears
/// the entity battle state and resets a monster-only field (+0x1332) for CMonster
/// objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameBattleOffPacket {
    pub handler_id: u32,
}

impl GameBattleOffPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_BATTLE_OFF);
        writer.write_u32(self.handler_id);
        writer.finalize()
    }
}

/// `GameHealUp` notice — opcode 1023. Modern parser reads `{u4 handler_id; u4 a; u4 b;
/// u4 c}`, resolves the entity (class checks 11/13 = partner digimon family) and
/// invokes three sequential status setters (status-object vtable +27/+29/+30). Setter
/// semantics are unattributed; wire field order is preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameHealUpPacket {
    pub handler_id: u32,
    pub value_a: u32, // unresolved (status setter 1)
    pub value_b: u32, // unresolved (status setter 2)
    pub value_c: u32, // unresolved (status setter 3)
}

impl GameHealUpPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_HEAL_UP);
        writer.write_u32(self.handler_id);
        writer.write_u32(self.value_a);
        writer.write_u32(self.value_b);
        writer.write_u32(self.value_c);
        writer.finalize()
    }
}

/// `GameTing` notice — opcode 1053. Modern parser reads `{u4 handler_id}`, resolves the
/// entity and plays the surprise-mark effect plus the "SystemEnemyWar" entity chat
/// line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameTingPacket {
    pub handler_id: u32,
}

impl GameTingPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_TING);
        writer.write_u32(self.handler_id);
        writer.finalize()
    }
}

/// `GameEmoticon` notice — opcode 1058. Modern parser reads `{u4 handler_id; u4
/// emotion_id; u4 flag}`, resolves `emotion_id` through table 17 (CEmotionTable) and
/// applies it to the entity; flag == 1 also plays the aura variant. Entities of type 13
/// raise event 669 afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameEmoticonPacket {
    pub handler_id: u32,
    pub emotion_id: u32,
    pub flag: u32,
}

impl GameEmoticonPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_EMOTICON);
        writer.write_u32(self.handler_id);
        writer.write_u32(self.emotion_id);
        writer.write_u32(self.flag);
        writer.finalize()
    }
}

/// `GameStartMessage` notice — opcode 1047. Modern parser reads `{u1 count; count x {u2
/// len; len x bytes}}` (strings through the standard reader, capped at 512 bytes) and
/// delivers each to the message UI via event 42.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameStartMessagePacket {
    pub messages: Vec<String>,
}

impl GameStartMessagePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_START_MESSAGE);
        writer.write_u8(self.messages.len() as u8);
        for message in &self.messages {
            writer.write_modern_utf8_string(message);
        }
        writer.finalize()
    }
}

/// `GameDiedAlready` notice — opcode 1052. Modern parser advances past a single `{u2}`
/// and clears the corpse-requeue party entry plus the digi-death fields; the u2 value
/// itself is unattributed (skipped client-side).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameDiedAlreadyPacket {
    pub reserved: u16, // unresolved (skipped client-side)
}

impl GameDiedAlreadyPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::GAME_DIED_ALREADY);
        writer.write_u16(self.reserved);
        writer.finalize()
    }
}

/// `SvrChannelInfo` notice — opcode 1713. Modern parser loops `n x {u1 offset; u1
/// byte}` pairs until an 0xFF offset terminator and assembles the 32-byte status buffer
/// delivered with event 118. Slot byte meanings are unattributed; offsets beyond 31 are
/// out of the client's slot range and rejected here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvrChannelInfoPacket {
    pub slots: BTreeMap<u8, u8>,
}

impl SvrChannelInfoPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::SVR_CHANNEL_INFO);
        for (offset, value) in &self.slots {
            if *offset < 32 {
                writer.write_u8(*offset);
                writer.write_u8(*value);
            }
        }
        writer.write_u8(0xFF);
        writer.finalize()
    }
}

fn write_buddy_string(writer: &mut PacketWriter, value: &str) {
    writer.write_modern_utf8_string(value);
}

/// `BuddyAdd` notice — opcode 2401. Modern parser reads `{u1 flag; {u2 len; len x
/// bytes}}` and dispatches into the buddy UI with an add classification byte. The
/// flag's exact role is unattributed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuddyAddPacket {
    pub flag: u8, // unresolved
    pub name: String,
}

impl BuddyAddPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUDDY_ADD);
        writer.write_u8(self.flag);
        write_buddy_string(&mut writer, &self.name);
        writer.finalize()
    }
}

/// `BuddyBan` notice — opcode 2403. Modern parser reads one `{u2 len; len x bytes}`
/// string and dispatches a ban-state message to the friends UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuddyBanPacket {
    pub name: String,
}

impl BuddyBanPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUDDY_BAN);
        write_buddy_string(&mut writer, &self.name);
        writer.finalize()
    }
}

/// `BuddyNotFound` notice — opcode 2407. Modern parser reads one `{u2 len; len x
/// bytes}` string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuddyNotFoundPacket {
    pub name: String,
}

impl BuddyNotFoundPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUDDY_NOT_FOUND);
        write_buddy_string(&mut writer, &self.name);
        writer.finalize()
    }
}

/// `BuddyOff` notice — opcode 2409. Modern parser reads one `{u2 len; len x bytes}`
/// string and dispatches an offline-state update (same event as the on-notice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuddyOffPacket {
    pub name: String,
}

impl BuddyOffPacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUDDY_OFF);
        write_buddy_string(&mut writer, &self.name);
        writer.finalize()
    }
}

/// `BuddyChangeName` notice — opcode 2410. Modern parser reads `{u1 flag; {u2 len;.};
/// {u2 len;.}}`. The flag's exact role is unattributed; the string order is wire order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuddyChangeNamePacket {
    pub flag: u8, // unresolved
    pub original_name: String,
    pub updated_name: String,
}

impl BuddyChangeNamePacket {
    pub fn encode(&self) -> Vec<u8> {
        let mut writer = PacketWriter::new(game::BUDDY_CHANGE_NAME);
        writer.write_u8(self.flag);
        write_buddy_string(&mut writer, &self.original_name);
        write_buddy_string(&mut writer, &self.updated_name);
        writer.finalize()
    }
}

// ---

impl TryFrom<RawPacket> for GameRequest {
    type Error = ProtocolError;

    fn try_from(packet: RawPacket) -> Result<Self, Self::Error> {
        let mut reader = PacketReader::new(packet.payload);
        match packet.packet_type {
            game::CONNECTION => Ok(Self::Connection {
                kind: reader.read_u8()?,
            }),
            game::KEEP_CONNECTION => Ok(Self::KeepConnection),
            game::INITIAL_INFORMATION => {
                // Client AccessCode packet payload: [XOR(4)][accountIdx(4)][accessCode(4)]
                // Skip XOR value; read accountIdx at offset 4 in payload
                reader.seek(4)?;
                Ok(Self::InitialInformation {
                    account_id: reader.read_u32()? as u64,
                    access_code: reader.read_u32()?,
                })
            }
            game::COMPLEMENTAR_INFORMATION => Ok(Self::ComplementarInformation),
            game::TAMER_MOVIMENTATION => {
                let ticks = reader.read_u32()?;
                let handler = reader.read_u32()?;
                let x = reader.read_i32()?;
                let y = reader.read_i32()?;
                let z = reader.read_f32()?;
                Ok(Self::TamerMovimentation {
                    ticks,
                    handler,
                    x,
                    y,
                    z,
                })
            }
            game::WARP_GATE => {
                let portal_id = reader.read_i32()?;
                Ok(Self::WarpGate { portal_id })
            }
            game::CONSUME_ITEM => {
                let target_handler = reader.read_i32()?;
                let slot = reader.read_u16()?;
                Ok(Self::ConsumeItem {
                    target_handler,
                    slot,
                })
            }
            game::MOVE_ITEM => {
                let origin_slot = reader.read_u16()?;
                let destination_slot = reader.read_u16()?;
                Ok(Self::MoveItem {
                    origin_slot,
                    destination_slot,
                })
            }
            game::SPLIT_ITEM => {
                let origin_slot = reader.read_u16()?;
                let destination_slot = reader.read_u16()?;
                let amount = reader.read_u16()?;
                Ok(Self::SplitItem {
                    origin_slot,
                    destination_slot,
                    amount,
                })
            }
            game::ITEM_REMOVE => {
                let slot = reader.read_u16()?;
                let x = reader.read_i32()?;
                let y = reader.read_i32()?;
                let amount = reader.read_u16()?;
                Ok(Self::RemoveItem { slot, x, y, amount })
            }
            game::NPC_PURCHASE => {
                // On-wire layout:
                //   [u1 vip][u4 npc_id][u1 marker=0x38][n4 shop_slot][u2 count][u1 marker]
                // The leading vip byte is decoded first; the trailing marker is optional.
                let vip = reader.read_u8()?;
                let npc_id = reader.read_i32()?;
                let marker = reader.read_u8()?;
                let shop_slot = reader.read_i32()?;
                let purchase_count = reader.read_u16()?;
                if reader.remaining_len() >= 1 {
                    // Trailing marker byte.
                    let _trailing_marker = reader.read_u8()?;
                }
                Ok(Self::NpcPurchase {
                    vip,
                    npc_id,
                    marker,
                    shop_slot,
                    purchase_count,
                })
            }
            game::NPC_SELL => {
                // On-wire layout:
                //   [u1 vip][u4 npc_id][u1 marker=0x38][u1 inven_slot][u2 count][u1 marker]
                let vip = reader.read_u8()?;
                let npc_id = reader.read_i32()?;
                let marker = reader.read_u8()?;
                let item_slot = reader.read_u8()?;
                let sell_amount = reader.read_u16()?;
                if reader.remaining_len() >= 1 {
                    // Trailing marker byte.
                    let _trailing_marker = reader.read_u8()?;
                }
                Ok(Self::NpcSell {
                    vip,
                    npc_id,
                    marker,
                    item_slot,
                    sell_amount,
                })
            }
            game::LOOT_ITEM => Ok(Self::LootItem {
                drop_handler: reader.read_u32()?,
            }),
            game::DIGI_SUMMON_SYNC_REQUEST => Ok(Self::DigiSummonSyncRequest),
            game::AVAILABLE_CHANNELS => Ok(Self::ChannelInfo),
            game::MEMBERSHIP => Ok(Self::Membership),
            game::EMOTICON => {
                let emoticon_type = reader.read_i32()?;
                let value = if reader.remaining_len() >= 4 {
                    reader.read_i32()?
                } else {
                    -1
                };
                Ok(Self::Emoticon {
                    emoticon_type,
                    value,
                })
            }
            game::FRIENDLY_INFO => {
                let target_handler = if reader.remaining_len() >= 4 {
                    reader.read_u32()?
                } else {
                    0
                };
                Ok(Self::FriendlyInfo { target_handler })
            }
            game::AVAILABLE_RELATIONS => Ok(Self::FriendList),
            game::EXITEM_USE => {
                let category = reader.read_u8()?;
                let extra_slot = reader.read_u16()?;
                Ok(Self::ExtraInventoryUse {
                    category,
                    extra_slot,
                })
            }
            game::CHAT_MESSAGE => {
                let message = reader.read_modern_utf8_string()?;
                Ok(Self::ChatMessage { message })
            }
            game::WHISPER_MESSAGE => {
                let target_name = reader.read_modern_utf8_string()?;
                let message = reader.read_modern_utf8_string()?;
                Ok(Self::WhisperMessage {
                    target_name,
                    message,
                })
            }
            game::MEGAPHONE_MESSAGE => {
                let mode = reader.read_u8()?;
                let item_code = reader.read_u32()?;
                let message = reader.read_modern_utf8_string()?;
                let item_slot = reader.read_u16()?;
                Ok(Self::MegaphoneMessage {
                    mode,
                    item_code,
                    message,
                    item_slot,
                })
            }
            game::TAMER_REACTION => {
                let reaction_type = reader.read_i32()?;
                Ok(Self::TamerReaction { reaction_type })
            }
            game::PARTNER_STOP => {
                let uid = reader.read_u32()?;
                Ok(Self::PartnerStop { uid })
            }
            game::EVOLUTION => {
                let digimon_handler = reader.read_u32()?;
                let evolution_slot = reader.read_u8()?;
                Ok(Self::PartnerEvolution {
                    digimon_handler,
                    evolution_slot,
                })
            }
            game::PARTNER_ATTACK => {
                let attacker_handler = reader.read_u32()?;
                let target_handler = reader.read_u32()?;
                Ok(Self::PartnerAttack {
                    attacker_handler,
                    target_handler,
                })
            }
            game::PARTNER_SKILL => {
                let skill_slot = reader.read_u8()?;
                let attacker_handler = reader.read_u32()?;
                let target_handler = reader.read_u32()?;
                Ok(Self::PartnerSkill {
                    skill_slot,
                    attacker_handler,
                    target_handler,
                })
            }
            game::PARTNER_SWITCH => {
                let slot = reader.read_u8()?;
                Ok(Self::PartnerSwitch { slot })
            }
            game::PARTNER_DELETE => {
                let slot = reader.read_u8()?;
                let validation = reader.read_modern_utf8_string()?;
                Ok(Self::PartnerDelete { slot, validation })
            }
            game::EVOLUTION_UNLOCK => {
                let evo_slot = reader.read_i32()?;
                let mut items = Vec::new();
                let selected_inven_idx = if reader.remaining_len() == 2 {
                    reader.read_i16()?
                } else if reader.remaining_len() >= 4 {
                    let item_count = reader.read_u16()? as usize;
                    for _ in 0..item_count {
                        let item_id = reader.read_u32()?;
                        let item_count = reader.read_u32()?;
                        items.push(EvolutionUnlockItem {
                            item_id,
                            item_count,
                        });
                    }
                    if reader.remaining_len() >= 2 {
                        reader.read_i16()?
                    } else {
                        -1
                    }
                } else {
                    -1
                };
                Ok(Self::EvolutionUnlock {
                    evo_slot,
                    items,
                    selected_inven_idx,
                })
            }
            game::RIDE_MODE_START => Ok(Self::RideModeStart),
            game::RIDE_MODE_STOP => Ok(Self::RideModeStop),
            game::OPEN_RIDE_MODE => {
                let evo_unit_idx = reader.read_u32()?;
                let item_type = reader.read_i32()?;
                Ok(Self::OpenRideMode {
                    evo_unit_idx,
                    item_type,
                })
            }
            game::SET_TARGET => {
                let attacker_handler = reader.read_u32()?;
                let target_handler = reader.read_u32()?;
                Ok(Self::SetTarget {
                    attacker_handler,
                    target_handler,
                })
            }
            game::STAT_UP => {
                let uid = reader.read_u32()?;
                let stat = reader.read_u8()?;
                Ok(Self::StatUp { uid, stat })
            }
            game::REFRESH_SCREEN => Ok(Self::RefreshScreen),
            game::AWAY_TIME => Ok(Self::AwayTime),
            game::DIGIMON_CHANGE_NAME => {
                let inven_slot = reader.read_i32()?;
                let new_name = reader.read_modern_utf8_string()?;
                Ok(Self::DigimonChangeName {
                    inven_slot,
                    new_name,
                })
            }
            game::HATCH_INSERT_EGG => {
                let vip = reader.read_u8()?;
                let inven_slot = reader.read_u32()?;
                let npc_idx = reader.read_i32()?;
                Ok(Self::HatchInsertEgg {
                    vip,
                    inven_slot,
                    npc_idx,
                })
            }
            game::HATCH_INCREASE => {
                let vip = reader.read_u8()?;
                let npc_idx = reader.read_i32()?;
                let data_level = reader.read_u8()? as i8;
                Ok(Self::HatchIncrease {
                    vip,
                    npc_idx,
                    data_level,
                })
            }
            game::HATCH_FINISH => {
                let flag = reader.read_u8()?;
                let portable_pos = reader.read_u32()?;
                let name = reader.read_modern_utf8_string()?;
                let npc_idx = reader.read_i32()?;
                Ok(Self::HatchFinish {
                    flag,
                    portable_pos,
                    name,
                    npc_idx,
                })
            }
            game::HATCH_REMOVE_EGG => {
                let vip = reader.read_u8()?;
                let npc_idx = reader.read_i32()?;
                Ok(Self::HatchRemoveEgg { vip, npc_idx })
            }
            game::HATCH_BACKUP_INSERT => {
                let vip = reader.read_u8()?;
                let inven_slot = reader.read_u32()?;
                let npc_idx = reader.read_i32()?;
                Ok(Self::HatchBackupInsert {
                    vip,
                    inven_slot,
                    npc_idx,
                })
            }
            game::HATCH_BACKUP_CANCEL => {
                let vip = reader.read_u8()?;
                let npc_idx = reader.read_i32()?;
                Ok(Self::HatchBackupCancel { vip, npc_idx })
            }
            game::INCUBATOR_CLOSE => Ok(Self::IncubatorClose),
            game::DIGIMON_ARCHIVE_MOVE => {
                let vip = reader.read_u8()?;
                let slot1 = reader.read_i32()?;
                let slot2 = reader.read_i32()?;
                let npc_type = reader.read_u32()?;
                Ok(Self::DigimonArchiveMove {
                    vip,
                    slot1,
                    slot2,
                    npc_type,
                })
            }
            game::DIGIMON_ARCHIVE_LIST => {
                let vip = reader.read_u8()?;
                let inven_idx = reader.read_u32()?;
                let npc_type = reader.read_u32()?;
                Ok(Self::DigimonArchiveList {
                    vip,
                    inven_idx,
                    npc_type,
                })
            }
            game::DIGIMON_ARCHIVE_SWAP => {
                let npc_idx = reader.read_u32()?;
                let archive_type = reader.read_i32()?;
                let src_arr = reader.read_u8()?;
                let dst_arr = reader.read_u8()?;
                Ok(Self::DigimonArchiveSwap {
                    npc_idx,
                    archive_type,
                    src_arr,
                    dst_arr,
                })
            }
            game::INVENTORY_SORT => {
                let sort_type = reader.read_u8()?;
                Ok(Self::InventorySort { sort_type })
            }
            game::ITEM_IDENTIFY => {
                let item_slot = reader.read_i16()?;
                Ok(Self::ItemIdentify { item_slot })
            }
            game::ITEM_CRAFT => {
                let recipe_slot = reader.read_i16()?;
                Ok(Self::ItemCraft { recipe_slot })
            }
            game::ITEM_REROLL => {
                let item_slot = reader.read_i16()?;
                Ok(Self::ItemReroll { item_slot })
            }
            game::ITEM_SOCKET_IN => {
                let vip = reader.read_u8()?;
                let inven_portable_pos = reader.read_u32()?;
                let npc_idx = reader.read_i32()?;
                let src_inven_pos = reader.read_u16()?;
                let dst_inven_pos = reader.read_u16()?;
                let socket_order = reader.read_u8()?;
                Ok(Self::ItemSocketIn {
                    vip,
                    inven_portable_pos,
                    npc_idx,
                    src_inven_pos,
                    dst_inven_pos,
                    socket_order,
                })
            }
            game::ITEM_SOCKET_OUT => {
                let vip = reader.read_u8()?;
                let inven_portable_pos = reader.read_u32()?;
                let npc_idx = reader.read_i32()?;
                let src_inven_pos = reader.read_u16()?;
                let dst_inven_pos = reader.read_u16()?;
                let socket_order = reader.read_u8()?;
                Ok(Self::ItemSocketOut {
                    vip,
                    inven_portable_pos,
                    npc_idx,
                    src_inven_pos,
                    dst_inven_pos,
                    socket_order,
                })
            }
            game::ITEM_SOCKET_IDENTIFY => {
                let vip = reader.read_u8()?;
                let npc_idx = reader.read_i32()?;
                let inven_portable_pos = reader.read_u32()?;
                let inven_pos = reader.read_u16()?;
                Ok(Self::ItemSocketIdentify {
                    vip,
                    npc_idx,
                    inven_portable_pos,
                    inven_pos,
                })
            }
            game::ITEM_RETURN => {
                let item_slot = reader.read_i16()?;
                Ok(Self::ItemReturn { item_slot })
            }
            game::ITEM_SCAN => {
                let item_slot = reader.read_i16()?;
                Ok(Self::ItemScan { item_slot })
            }
            game::LOAD_GIFT_STORAGE => Ok(Self::LoadGiftStorage),
            game::GIFT_STORAGE_RETRIEVE => {
                let item_slot = reader.read_i16()?;
                Ok(Self::GiftStorageRetrieve { item_slot })
            }
            game::LOAD_REWARD_STORAGE => Ok(Self::LoadRewardStorage),
            game::RECOMPENSE_GAIN => {
                let reward_id = reader.read_i32()?;
                Ok(Self::RecompenseGain { reward_id })
            }
            game::TAMER_SHOP_OPEN => Ok(Self::TamerShopOpen),
            game::TAMER_SHOP_CLOSE => Ok(Self::TamerShopClose),
            game::TAMER_SHOP_BUY => {
                let item_id = reader.read_i32()?;
                let amount = reader.read_i16()?;
                Ok(Self::TamerShopBuy { item_id, amount })
            }
            game::CONSIGNSHOP_OPEN => Ok(Self::ConsignedShopOpen),
            game::CONSIGNSHOP_VIEW => {
                let shop_id = reader.read_i32()?;
                Ok(Self::ConsignedShopView { shop_id })
            }
            game::CONSIGNSHOP_PURCHASE => {
                let item_id = reader.read_i32()?;
                let amount = reader.read_i16()?;
                Ok(Self::ConsignedShopPurchase { item_id, amount })
            }
            game::CONSIGNSHOP_RETRIEVE => {
                let item_slot = reader.read_i16()?;
                Ok(Self::ConsignedShopRetrieve { item_slot })
            }
            game::CASHSHOP_BUY => {
                // On-wire layout:
                //   [u1 item_count][n4 total_price][u2 or u8 order_id][n4 product_ids[count]]
                // Order id width varies by server generation: u2 in some
                // payloads, u8 in newer ones.
                let amount = reader.read_u8()?;
                let total_price = reader.read_i32()?;
                let order_id = if reader.remaining_len() >= 8 + (amount as usize) * 4 {
                    reader.read_u64()?
                } else {
                    u64::from(reader.read_u16()?)
                };
                let mut product_ids = Vec::with_capacity(amount as usize);
                for _ in 0..amount {
                    product_ids.push(reader.read_i32()?);
                }
                Ok(Self::CashShopBuy {
                    amount,
                    total_price,
                    order_id,
                    product_ids,
                })
            }
            game::CASHSHOP_RELOAD => Ok(Self::CashShopReload),
            game::QUEST_AVAILABLE_LIST => {
                let npc_id = reader.read_i32()?;
                Ok(Self::QuestAvailableList { npc_id })
            }
            game::QUEST_ACCEPT => {
                let quest_id = reader.read_i16()?;
                Ok(Self::QuestAccept { quest_id })
            }
            game::QUEST_DELIVER => {
                let quest_id = reader.read_i16()?;
                Ok(Self::QuestDeliver { quest_id })
            }
            game::QUEST_GIVE_UP => {
                let quest_id = reader.read_i16()?;
                Ok(Self::QuestGiveUp { quest_id })
            }
            game::QUEST_UPDATE => {
                let quest_id = reader.read_i16()?;
                let cond_index = reader.read_u8()?;
                let value = reader.read_u8()?;
                Ok(Self::QuestUpdate {
                    quest_id,
                    cond_index,
                    value,
                })
            }
            game::DIE_CONFIRM => Ok(Self::DieConfirm),
            game::REMOVE_BUFF => {
                let buff_id = reader.read_i32()?;
                Ok(Self::RemoveBuff { buff_id })
            }
            game::DAMAGE_SKIN_CHANGE => {
                let skin_id = reader.read_i32()?;
                Ok(Self::DamageSkinChange { skin_id })
            }
            game::SEAL_OPEN => {
                let seal_idx = reader.read_i16()?;
                Ok(Self::SealOpen { seal_idx })
            }
            game::SEAL_CLOSE => {
                let seal_idx = reader.read_i16()?;
                let value = reader.read_u32()?;
                Ok(Self::SealClose { seal_idx, value })
            }
            game::SEAL_SET_LEADER => {
                let card_code = reader.read_u16()?;
                Ok(Self::SealSetLeader { card_code })
            }
            game::SEAL_REMOVE_LEADER => {
                if reader.remaining_len() == 0 {
                    // Empty payload selects the encyclopedia open request;
                    // otherwise this is the seal leader unset with a u16 card code.
                    Ok(Self::EncyclopediaLoad)
                } else {
                    let _card_code = reader.read_u16()?;
                    Ok(Self::SealRemoveLeader)
                }
            }
            game::SEAL_SET_FAVORITE => {
                let card_code = reader.read_u16()?;
                let bookmark = reader.read_u8()?;
                Ok(Self::SealSetFavorite {
                    card_code,
                    bookmark,
                })
            }
            game::ENCYCLOPEDIA_GET_REWARD => {
                let digimon_id = reader.read_u32()?;
                Ok(Self::EncyclopediaGetReward { digimon_id })
            }
            game::ENCYCLOPEDIA_DECK_BUFF => {
                // Modern contract: {u2 option_index} (ENCY_MAX_OPTION == 4).
                let deck_idx = reader.read_u16()?;
                Ok(Self::EncyclopediaDeckBuff { deck_idx })
            }
            game::OTHER_TAMER_DETAIL_INFO_REQUEST => {
                let target_handler = reader.read_u32()?;
                Ok(Self::OtherTamerDetailInfo { target_handler })
            }
            game::ARENA_DAILY_POINTS => {
                let _skip = reader.read_u16()?; // skip 2 bytes
                let item_slot = reader.read_i16()?;
                let points = reader.read_i16()?;
                let item_id = reader.read_i16()?;
                Ok(Self::ArenaDailyPoints {
                    item_slot,
                    points,
                    item_id,
                })
            }
            game::ARENA_DAILY_RANKING => Ok(Self::ArenaDailyRanking),
            game::ARENA_REQUEST_RANK => {
                let ranking_type = reader.read_u8()?;
                Ok(Self::ArenaRequestRank { ranking_type })
            }
            game::ARENA_REQUEST_OLD_RANK => {
                let ranking_type = reader.read_u8()?;
                Ok(Self::ArenaRequestOldRank { ranking_type })
            }
            game::ARENA_REQUEST_FAME => {
                let season = reader.read_u16()?;
                Ok(Self::ArenaRequestFame { season })
            }
            game::ARENA_REQUEST_FAME_LIST => Ok(Self::ArenaRequestFameList),
            game::DUNGEON_NEXT_STAGE => Ok(Self::DungeonNextStage),
            game::DUNGEON_SURRENDER => Ok(Self::DungeonSurrender),
            game::BURNING_EVENT => Ok(Self::BurningEvent),
            game::DAILY_CHECK_EVENT => Ok(Self::DailyCheckEvent),
            game::DAILY_CHECK_EVENT_REQUEST => {
                let event_no = reader.read_i32()?;
                Ok(Self::DailyCheckEventRequest { event_no })
            }
            game::JOIN_EVENT_QUEUE => {
                let event_id = reader.read_i32()?;
                Ok(Self::JoinEventQueue { event_id })
            }
            game::REGION_UNLOCK => {
                // Modern client `SendOpenRegion` pushes a single signed byte.
                let region_idx = reader.read_u8()? as i16;
                Ok(Self::RegionUnlock { region_idx })
            }
            game::SET_TITLE => {
                let title_id = reader.read_i16()?;
                Ok(Self::SetTitle { title_id })
            }
            game::CHANGE_TAMER_MODEL => {
                let model_id = reader.read_i32()?;
                let inven_slot = reader.read_i32()?;
                Ok(Self::ChangeTamerModel {
                    model_id,
                    inven_slot,
                })
            }
            game::TAMER_NAME_CHANGE => {
                let inven_slot = reader.read_i32()?;
                let new_name = reader.read_modern_utf8_string()?;
                Ok(Self::TamerNameChange {
                    inven_slot,
                    new_name,
                })
            }
            game::RARE_MACHINE_OPEN => {
                let npc_idx = reader.read_u32()?;
                Ok(Self::RareMachineOpen { npc_idx })
            }
            game::RARE_MACHINE_RUN => {
                let npc_idx = reader.read_u32()?;
                let inven_idx = reader.read_u32()?;
                let reset_count = reader.read_u32()?;
                Ok(Self::RareMachineRun {
                    npc_idx,
                    inven_idx,
                    reset_count,
                })
            }
            game::PARTY_INVITE => {
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::PartyInvite { target_name })
            }
            game::PARTY_INVITE_RESPONSE => {
                let result_type = reader.read_i32()?;
                let inviter_name = reader.read_modern_utf8_string()?;
                Ok(Self::PartyInviteResponse {
                    result_type,
                    inviter_name,
                })
            }
            game::PARTY_CHAT => {
                let message = reader.read_modern_utf8_string()?;
                Ok(Self::PartyChat { message })
            }
            game::PARTY_KICK => {
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::PartyKick { target_name })
            }
            game::PARTY_LEAVE => Ok(Self::PartyLeave),
            game::PARTY_CHANGE_MASTER => {
                let new_leader_slot = reader.read_i32()?;
                Ok(Self::PartyChangeMaster { new_leader_slot })
            }
            game::PARTY_CHANGE_LOOT => {
                let loot_type = reader.read_i32()?;
                let rare_type = reader.read_u8()?;
                let disp_rare_grade = reader.read_u8()?;
                Ok(Self::PartyChangeLoot {
                    loot_type,
                    rare_type,
                    disp_rare_grade,
                })
            }
            game::PARTY_DISMISS => Ok(Self::PartyDismiss),
            game::GUILD_CREATE => {
                let guild_name = reader.read_modern_utf8_string()?;
                let inven_slot = reader.read_u16()?;
                let npc_id = reader.read_u32()?;
                Ok(Self::GuildCreate {
                    guild_name,
                    inven_slot,
                    npc_id,
                })
            }
            game::GUILD_DELETE => Ok(Self::GuildDelete),
            game::GUILD_INVITE => {
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildInvite { target_name })
            }
            game::GUILD_INVITE_ACCEPT => {
                let certified_code = reader.read_u32()?;
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildInviteAccept {
                    certified_code,
                    target_name,
                })
            }
            game::GUILD_INVITE_DENY => {
                let certified_code = reader.read_u32()?;
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildInviteDeny {
                    certified_code,
                    target_name,
                })
            }
            game::GUILD_KICK => {
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildKick { target_name })
            }
            game::GUILD_LEAVE => Ok(Self::GuildLeave),
            game::GUILD_MESSAGE => {
                let message = reader.read_modern_utf8_string()?;
                Ok(Self::GuildMessage { message })
            }
            game::GUILD_NOTICE => {
                let notice = reader.read_modern_utf8_string()?;
                Ok(Self::GuildNotice { notice })
            }
            game::GUILD_HISTORY => Ok(Self::GuildHistory),
            game::GUILD_SET_TITLE => {
                let authority_class = reader.read_i32()?;
                let title = reader.read_modern_utf8_string()?;
                let duty = reader.read_modern_utf8_string()?;
                Ok(Self::GuildSetTitle {
                    authority_class,
                    title,
                    duty,
                })
            }
            game::TRADE_REQUEST => {
                let target_handler = reader.read_u32()?;
                Ok(Self::TradeRequest { target_handler })
            }
            game::TRADE_ACCEPT => {
                let accepter_handler = reader.read_u32()?;
                Ok(Self::TradeAccept { accepter_handler })
            }
            game::TRADE_CANCEL => Ok(Self::TradeCancel),
            game::TRADE_ADD_ITEM => {
                let inven_pos = reader.read_u16()?;
                let amount = reader.read_u16()?;
                Ok(Self::TradeAddItem { inven_pos, amount })
            }
            game::TRADE_REMOVE_ITEM => {
                let trade_slot = reader.read_u8()? as i8;
                Ok(Self::TradeRemoveItem { trade_slot })
            }
            game::TRADE_ADD_MONEY => {
                let amount = reader.read_u32()?;
                Ok(Self::TradeAddMoney { amount })
            }
            game::TRADE_CONFIRM => Ok(Self::TradeConfirm),
            game::TRADE_LOCK => Ok(Self::TradeLock),
            game::TRADE_UNLOCK => Ok(Self::TradeUnlock),
            game::SEASON_PASS_DETAILS => Ok(Self::SeasonPassDetails),
            game::SEASON_PASS_PURCHASE_EXP => {
                let purchase_count = reader.read_i32()?;
                Ok(Self::SeasonPassPurchaseExp { purchase_count })
            }
            game::SEASON_PASS_MISSION_REWARD => {
                let mission_id = reader.read_i32()?;
                Ok(Self::SeasonPassMissionReward { mission_id })
            }
            game::SEASON_PASS_SEASON_REWARD => {
                let level = reader.read_i32()?;
                Ok(Self::SeasonPassSeasonReward { level })
            }
            game::CHANGE_CHANNEL => {
                let channel = reader.read_u32()?;
                Ok(Self::ChangeChannel { channel })
            }
            game::CHANNEL_SWITCH_CONFIRM => Ok(Self::ChannelSwitchConfirm),
            game::TAMER_SHOP_LIST => Ok(Self::TamerShopList),
            game::CONSIGNSHOP_WAREHOUSE => Ok(Self::ConsignedWarehouse),
            game::CONSIGNSHOP_WAREHOUSE_RETRIEVE => {
                let item_slot = reader.read_i16()?;
                Ok(Self::ConsignedWarehouseRetrieve { item_slot })
            }
            game::CASHSHOP_BUY_HISTORY => Ok(Self::CashShopBuyHistory),
            game::ADD_FRIEND => {
                let friend_name = reader.read_modern_utf8_string()?;
                Ok(Self::AddFriend { friend_name })
            }
            game::BUDDY_DELETE => {
                let friend_name = reader.read_modern_utf8_string()?;
                Ok(Self::DeleteFriend { friend_name })
            }
            game::BUDDY_MEMO => {
                let tamer_name = reader.read_modern_utf8_string()?;
                let memo = reader.read_modern_utf8_string()?;
                Ok(Self::FriendMemo { tamer_name, memo })
            }
            game::GUILD_INVITE_FAIL => {
                let inviter_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildInviteFailure { inviter_name })
            }
            game::GUILD_AUTHORITY_MASTER => {
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildAuthorityMaster { target_name })
            }
            game::GUILD_AUTHORITY_SUBMASTER => {
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildAuthoritySubMaster { target_name })
            }
            game::GUILD_AUTHORITY_MEMBER => {
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildAuthorityMember { target_name })
            }
            game::GUILD_AUTHORITY_NEW_MEMBER => {
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildAuthorityNewMember { target_name })
            }
            game::GUILD_AUTHORITY_DATS => {
                let target_name = reader.read_modern_utf8_string()?;
                Ok(Self::GuildAuthorityDats { target_name })
            }
            game::SPIRIT_TO_DIGIMON => {
                let model_id = reader.read_i32()?;
                let name = reader.read_modern_utf8_string()?;
                let npc_id = reader.read_i32()?;
                Ok(Self::SpiritToDigimon {
                    model_id,
                    name,
                    npc_id,
                })
            }
            game::DIGI_SUMMON_PURCHASE => {
                let product_id = reader.read_i32()?;
                let ticket_slot = reader.read_i32()?;
                Ok(Self::DigiSummonPurchase {
                    product_id,
                    ticket_slot,
                })
            }
            game::DIGI_COMBINE_SYNC => Ok(Self::DigiCombineSyncRequest),
            game::DIGI_COMBINE => {
                let ceiling_type = reader.read_u8()?;
                let materials = read_combine_materials(&mut reader)?;
                Ok(Self::DigiCombine {
                    ceiling_type,
                    materials,
                })
            }
            game::DIGI_COMBINE_REWARD => {
                let ceiling_type = reader.read_u8()?;
                Ok(Self::DigiCombineRewardClaim { ceiling_type })
            }
            game::UNION_COMBINE_SYNC => Ok(Self::UnionCombineSyncRequest),
            game::UNION_COMBINE => {
                let ceiling_type = reader.read_u8()?;
                let materials = read_combine_materials(&mut reader)?;
                Ok(Self::UnionCombine {
                    ceiling_type,
                    materials,
                })
            }
            game::UNION_COMBINE_REWARD => {
                let ceiling_type = reader.read_u8()?;
                Ok(Self::UnionCombineRewardClaim { ceiling_type })
            }
            game::UNION_HACK_OPEN_REQUEST => Ok(Self::UnionHackOpenRequest),
            game::UNION_HACK_MODIFY_REQUEST => {
                let slot = reader.read_u8()?;
                let part_id = reader.read_i32()?;
                let grade = reader.read_i16()?;
                Ok(Self::UnionHackModify {
                    slot,
                    part_id,
                    grade,
                })
            }
            game::RANDOM_BOX_LIST => {
                let flag = reader.read_u8()?;
                let index = reader.read_i32()?;
                Ok(Self::RandomBoxList { flag, index })
            }
            game::RANDOM_BOX_PURCHASE => {
                let flag = reader.read_u8()?;
                let product_id = reader.read_i32()?;
                let item_uid = reader.read_i32()?;
                let count = reader.read_u16()?;
                let state = reader.read_i32()?;
                Ok(Self::RandomBoxPurchase {
                    flag,
                    product_id,
                    item_uid,
                    count,
                    state,
                })
            }
            game::LOAD_ACCOUNT_WAREHOUSE => Ok(Self::LoadAccountWarehouse),
            game::RETRIEVE_ACCOUNT_WAREHOUSE => {
                let item_slot = reader.read_i16()?;
                Ok(Self::RetrieveAccountWarehouse { item_slot })
            }
            game::EXITEM_BATCH_MOVE => {
                let _unknown = reader.read_u8()?;
                let category = reader.read_u8()?;
                Ok(Self::ExtraInventoryBatchMove { category })
            }
            game::EXITEM_MOVE => {
                let category = reader.read_u16()?;
                let extra_slot = reader.read_u16()?;
                let inventory_slot = reader.read_u16()?;
                Ok(Self::ExtraInventoryMove {
                    category,
                    extra_slot,
                    inventory_slot,
                })
            }
            game::EXITEM_SORT => {
                let category = reader.read_u8()?;
                Ok(Self::ExtraInventorySort { category })
            }
            game::PARTY_MEMBER_DISCONNECT => Ok(Self::PartyMemberDisconnect),
            game::MONSTER_RESPAWN_TIMER => Ok(Self::MonsterRespawnTimer),
            game::JUMP_BOOSTER => Ok(Self::JumpBooster),
            game::SKILL_LEVEL_UP => {
                let uid = reader.read_u32()?;
                let evo_unit_idx = reader.read_u8()?;
                let skill_idx = reader.read_u8()?;
                Ok(Self::SkillLevelUp {
                    uid,
                    evo_unit_idx,
                    skill_idx,
                })
            }
            game::TAMER_CHARGE_XCRYSTAL => Ok(Self::TamerChargeXCrystal),
            game::TAMER_CONSUME_XCRYSTAL => {
                let amount = reader.read_i32()?;
                Ok(Self::TamerConsumeXCrystal { amount })
            }
            game::TAMER_SUMMON => {
                let case = reader.read_u8()?;
                let summon = match case {
                    1 => TamerSummonRequest::ByName {
                        target_name: reader.read_modern_utf8_string()?,
                        skill_index: reader.read_u32()?,
                    },
                    4 => {
                        let submode = reader.read_u8()?;
                        let value = match submode {
                            1 => u32::from(reader.read_u8()?),
                            2 => reader.read_u32()?,
                            _ => {
                                return Err(ProtocolError::UnsupportedSummonCase(submode));
                            }
                        };
                        TamerSummonRequest::StateCheck { submode, value }
                    }
                    _ => TamerSummonRequest::Accept {
                        case,
                        value: reader.read_u8()?,
                    },
                };
                Ok(Self::TamerSummon(summon))
            }
            game::TAMER_SKILL_REQUEST => {
                let skill_idx = reader.read_u32()?;
                let target_uid = reader.read_u32()?;
                Ok(Self::TamerSkillRequest {
                    skill_idx,
                    target_uid,
                })
            }
            game::TRANSCENDENCE_RECEIVE_EXP => Ok(Self::TranscendenceReceiveExp),
            game::TRANSCENDENCE_SUCCESS => Ok(Self::TranscendenceSuccess),
            game::TIME_CHARGE_RESULT => {
                let charge_type = reader.read_u8()?;
                Ok(Self::TimeChargeResult { charge_type })
            }
            game::WARP_GATE_DUNGEON => Ok(Self::WarpGateDungeon),
            game::DIGIMON_TO_SPIRIT => {
                let slot = reader.read_u8()?;
                let validation = reader.read_modern_utf8_string()?;
                let npc_id = reader.read_i32()?;
                Ok(Self::DigimonToSpirit {
                    slot,
                    validation,
                    npc_id,
                })
            }
            other => Err(ProtocolError::InvalidGamePacketType(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::PacketReader;
    use odmo_types::{
        CombineCeilingEntry, CombineItemRef, DEFAULT_PARTNER_MODEL_ID, DEFAULT_START_MAP_ID,
        DEFAULT_TAMER_MODEL_ID, DigiCombineReward, DigiSummonProduct, DigiSummonReward,
    };

    #[test]
    fn parse_initial_information_reads_account_id_from_offset_4() {
        // Client sends: newp(1706); push(XOR); push(accountIdx); push(accessCode); endp();
        // Payload = [XOR(4)][accountIdx(4)][accessCode(4)] = 12 bytes
        // Skip XOR value; read accountIdx at offset 4 in payload
        let mut payload = Vec::new();
        payload.extend_from_slice(&(0xDEAD_u32).to_le_bytes()); // XOR value (skipped)
        payload.extend_from_slice(&(1_u32).to_le_bytes()); // account_idx
        payload.extend_from_slice(&(0xBEEF_u32).to_le_bytes()); // access_code

        let request = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::INITIAL_INFORMATION,
            payload,
        })
        .expect("request should parse");

        assert_eq!(
            request,
            GameRequest::InitialInformation {
                account_id: 1,
                access_code: 0xBEEF,
            }
        );
    }

    #[test]
    fn parse_loot_item_reads_drop_handler() {
        let request = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::LOOT_ITEM,
            payload: (49_200_u32).to_le_bytes().to_vec(),
        })
        .expect("loot request should parse");

        assert_eq!(
            request,
            GameRequest::LootItem {
                drop_handler: 49_200,
            }
        );
    }

    #[test]
    fn game_hp_rate_notice_writes_u32_then_u8() {
        let packet = GameHpRatePacket {
            handler_id: 0x1234_5678,
            rate: 85,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAME_HP_RATE);
        assert_eq!(raw.payload, [0x78, 0x56, 0x34, 0x12, 85]);
    }

    #[test]
    fn game_level_up_notice_writes_u32_then_u8() {
        let packet = GameLevelUpPacket {
            handler_id: 7,
            level: 42,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAME_LEVEL_UP);
        assert_eq!(raw.payload, [7, 0, 0, 0, 42]);
    }

    #[test]
    fn game_battle_notices_write_single_u32_handler() {
        let on = GameBattleOnPacket { handler_id: 0xAB }.encode();
        let off = GameBattleOffPacket { handler_id: 0xAB }.encode();

        let on_raw = PacketReader::from_frame(&on).expect("frame should decode");
        assert_eq!(on_raw.packet_type, game::GAME_BATTLE_ON);
        assert_eq!(on_raw.payload, [0xAB, 0, 0, 0]);

        let off_raw = PacketReader::from_frame(&off).expect("frame should decode");
        assert_eq!(off_raw.packet_type, game::GAME_BATTLE_OFF);
        assert_eq!(off_raw.payload, [0xAB, 0, 0, 0]);
    }

    #[test]
    fn game_heal_up_notice_writes_four_u32_values() {
        let packet = GameHealUpPacket {
            handler_id: 0x10,
            value_a: 1,
            value_b: 2,
            value_c: 3,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAME_HEAL_UP);
        assert_eq!(
            raw.payload,
            [0x10, 0, 0, 0, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]
        );
    }

    #[test]
    fn game_ting_notice_writes_single_u32_handler() {
        let packet = GameTingPacket { handler_id: 0x77 }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAME_TING);
        assert_eq!(raw.payload, [0x77, 0, 0, 0]);
    }

    #[test]
    fn game_emoticon_notice_writes_u32_triple() {
        let packet = GameEmoticonPacket {
            handler_id: 5,
            emotion_id: 8,
            flag: 1,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAME_EMOTICON);
        assert_eq!(raw.payload, [5, 0, 0, 0, 8, 0, 0, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn game_start_message_notice_writes_count_then_strings() {
        let packet = GameStartMessagePacket {
            messages: vec!["hi".into(), "ok".into()],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAME_START_MESSAGE);
        assert_eq!(
            raw.payload,
            vec![
                2,
                2, 0, b'h', b'i',
                2, 0, b'o', b'k',
            ]
        );
    }

    #[test]
    fn game_died_already_notice_writes_single_u16() {
        let packet = GameDiedAlreadyPacket { reserved: 1 }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAME_DIED_ALREADY);
        assert_eq!(raw.payload, [1, 0]);
    }

    #[test]
    fn sync_condition_without_the_shop_bit_stops_after_the_condition() {
        let packet = SyncConditionPacket {
            handler: 42,
            condition: 1,
            shop_title: Some("ignored".to_string()),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SYNC_CONDITION);
        assert_eq!(raw.payload, [42, 0, 0, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn sync_condition_appends_modern_shop_title_when_the_bit_is_set() {
        let packet = SyncConditionPacket {
            handler: 42,
            condition: 5,
            shop_title: Some("Shop".to_string()),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.payload.len(), 8 + 2 + 4);
        assert_eq!(&raw.payload[8..10], &4u16.to_le_bytes());
        assert_eq!(&raw.payload[10..], b"Shop");
    }

    #[test]
    fn item_scan_notice_writes_header_pairs_then_u16_rows() {
        let packet = ItemScanNoticePacket {
            first: 7,
            second: -2,
            entries: vec![11, 12],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ITEM_SCAN_NOTICE);
        assert_eq!(
            raw.payload,
            [
                7, 0, 0, 0, // u4
                0xFE, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, // i64 -2
                2, 0, // count
                11, 0, 12, 0, // rows
            ]
        );
    }

    #[test]
    fn advert_boss_notices_write_fixed_u32_fields() {
        let count = AdvertBossCountNoticePacket {
            first: 1,
            second: 2,
            third: 3,
        }
        .encode();

        let count_raw = PacketReader::from_frame(&count).expect("frame should decode");
        assert_eq!(count_raw.packet_type, game::ADVERT_BOSS_COUNT);
        assert_eq!(count_raw.payload, [1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);

        let die = AdvertBossDieNoticePacket {
            first: 0x55,
            second: 0,
        }
        .encode();
        let die_raw = PacketReader::from_frame(&die).expect("frame should decode");
        assert_eq!(die_raw.packet_type, game::ADVERT_BOSS_DIE);
        assert_eq!(
            die_raw.payload,
            [0x55, 0, 0, 0, 0, 0, 0, 0]
        );

        let create = AdvertBossCreateNoticePacket {
            map_id: 500,
            channel_id: 2,
            monster_id: 9001,
            created_count: 3,
        }
        .encode();
        let create_raw = PacketReader::from_frame(&create).expect("frame should decode");
        assert_eq!(create_raw.packet_type, game::ADVERT_BOSS_CREATE);
        assert_eq!(
            create_raw.payload,
            [
                244, 1, 0, 0, // map id
                2, 0, 0, 0, // channel id
                41, 35, 0, 0, // monster id (9001)
                3, 0, 0, 0, // created count
            ]
        );
    }

    #[test]
    fn skill_apply_around_bodies_follow_their_tag() {
        let melee = SkillApplyAroundPacket {
            body: SkillApplyAroundBody::Melee {
                caster: 0,
                skill_id: 2,
                target: 3,
                flag: 4,
                caster_block: None,
                caster_stats: vec![SkillStatRow {
                    value: 5,
                    extra: 6,
                    stat: 7,
                }],
                target_stats: vec![SkillStatRow {
                    value: 8,
                    extra: 9,
                    stat: 10,
                }],
                tail: 11,
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&melee).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_APPLY_AROUND);
        assert_eq!(
            raw.payload,
            [
                0, // tag
                0, 0, 0, 0, // caster
                2, 0, 0, 0, // skill
                3, 0, 0, 0, // target
                4, // flag
                1, // block marker byte (no 40-byte block: caster is zero)
                5, 0, 0, 0, 6, 0, 0, 0, 7, // caster stat row
                18, // caster terminator
                8, 0, 0, 0, 9, 0, 0, 0, 10, // target stat row
                18, // target terminator
                11, 0, 0, 0, // tail
            ]
        );

        let dot = SkillApplyAroundPacket {
            body: SkillApplyAroundBody::Dot {
                source: 1,
                flag: 2,
                rows: vec![SkillDotRow {
                    handler: 3,
                    buff: 4,
                    stat: 5,
                    value: 6,
                    second_value: 7,
                }],
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&dot).expect("frame should decode");
        assert_eq!(
            raw.payload,
            [
                2, // tag
                1, 0, 0, 0, // source
                2, // flag
                1, // row count
                3, 0, 0, 0, 4, 0, 5, 6, 0, 0, 0, 7, 0, 0, 0, // 15-byte row
            ]
        );

        let scene = SkillApplyAroundPacket {
            body: SkillApplyAroundBody::Scene {
                source: 9,
                index: 10,
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&scene).expect("frame should decode");
        assert_eq!(raw.payload, [3, 9, 0, 0, 0, 10, 0, 0, 0]);
    }

    #[test]
    fn incubator_and_slot_notices_carry_their_fields() {
        let frame = LevelUpInIncubatorPacket {
            digimon: 1,
            level: 2,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LEVELUP_DIGIMON_IN_INCUBATER);
        assert_eq!(raw.payload, [1, 0, 0, 0, 2, 0, 0, 0]);

        let frame = ExtraInvenSlotExpansionPacket { flag: 3, slots: 4 }.encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NTF_EXTRAINVEN_SLOTEXPANSION);
        assert_eq!(raw.payload, [3, 4, 0]);
    }

    #[test]
    fn short_word_and_wide_field_notices_keep_their_layout() {
        let frame = ShortWordNoticePacket {
            value: 1,
            first_short: 2,
            second_value: 3,
            third_value: 4,
            fourth_value: 5,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NOTICE_3601);
        assert_eq!(
            raw.payload,
            [
                1, 0, 0, 0, // u4
                2, 0, // u2
                3, 0, 0, 0, // u4
                4, 0, 0, 0, // u4
                5, 0, 0, 0, // u4
            ]
        );
        assert_eq!(raw.payload.len(), 18);

        let frame = WideFieldNoticePacket {
            value: 6,
            wide_value: 0x0000_0008_0000_0007,
            short: 9,
            second_wide_value: 0x0000_000B_0000_000A,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NOTICE_3512);
        assert_eq!(
            raw.payload,
            [
                6, 0, 0, 0, // u4
                7, 0, 0, 0, 8, 0, 0, 0, // u8
                9, 0, // u2
                10, 0, 0, 0, 11, 0, 0, 0, // u8
            ]
        );
        assert_eq!(raw.payload.len(), 22);
    }

    #[test]
    fn mixed_width_notices_interleave_byte_word_and_short() {
        for opcode in [game::NOTICE_4125, game::NOTICE_4136] {
            let frame = MixedWidthNoticePacket {
                opcode,
                value: 1,
                flag: 2,
                second_value: 3,
                first_short: 4,
                second_short: 5,
                third_value: 6,
            }
            .encode();
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert_eq!(
                raw.payload,
                [
                    1, 0, 0, 0, // u4
                    2, // u1
                    3, 0, 0, 0, // u4
                    4, 0, // u2
                    5, 0, // u2
                    6, 0, 0, 0, // u4
                ]
            );
            assert_eq!(raw.payload.len(), 17);
        }

        let frame = FlagDoubleWordNoticePacket {
            flag: 7,
            value: 0x0000_0009_0000_0008,
            tail: 10,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NOTICE_4132);
        assert_eq!(raw.payload, [7, 8, 0, 0, 0, 9, 0, 0, 0, 10, 0, 0, 0]);

        let frame = ExItemMoveResultPacket { result: 11 }.encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RES_EXITEM_MOVE);
        assert_eq!(raw.payload, [11, 0, 0, 0]);
    }

    #[test]
    fn item_flag_notices_keep_their_widths() {
        let frame = FlagWordNoticePacket { flag: 1, value: 2 }.encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NOTICE_4138);
        assert_eq!(raw.payload, [1, 2, 0, 0, 0]);

        let frame = FlagTwoWordNoticePacket {
            flag: 3,
            value: 4,
            second_value: 5,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NOTICE_4137);
        assert_eq!(raw.payload, [3, 4, 0, 0, 0, 5, 0, 0, 0]);

        for opcode in [game::RESPONSE_3501, game::RESPONSE_3514] {
            let frame = TwoWordPairPacket {
                opcode,
                value: 6,
                second_value: 7,
            }
            .encode();
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert_eq!(raw.payload, [6, 0, 0, 0, 7, 0, 0, 0]);
        }

        let frame = FixedBlockNoticePacket {
            block: [9; 1024],
            value: 10,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NOTICE_16035);
        assert_eq!(raw.payload.len(), 1028);
        assert_eq!(&raw.payload[..1024], &[9u8; 1024]);
        assert_eq!(&raw.payload[1024..], &[10, 0, 0, 0]);
    }

    #[test]
    fn skin_notices_carry_their_fields() {
        let frame = ChangeSkinResultPacket { result: 4 }.encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RES_CHANGE_SKIN_RESULT);
        assert_eq!(raw.payload, [4]);

        let frame = AcquireSkinPacket { flag: 1, skin: 2 }.encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NTF_ACQUIRE_SKIN);
        assert_eq!(raw.payload, [1, 2, 0, 0, 0]);

        let frame = TwoWordReplyPacket {
            value: 5,
            second_value: 6,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RESPONSE_3603);
        assert_eq!(raw.payload, [5, 0, 0, 0, 6, 0, 0, 0]);
    }

    #[test]
    fn season_notices_count_their_rows_after_a_wide_field() {
        for opcode in [game::NTF_SEASON_3507, game::NTF_SEASON_3508] {
            let frame = SeasonNoticePacket {
                opcode,
                value: 1,
                wide_value: 0x0000_0003_0000_0002,
                second_value: 4,
                short_value: 5,
                second_wide_value: 0x0000_0007_0000_0006,
                rows: vec![SeasonRow {
                    value: 0x0000_0009_0000_0008,
                    tail: 10,
                }],
            }
            .encode();
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert_eq!(
                raw.payload,
                [
                    1u8, 0, 0, 0, // first word
                    2, 0, 0, 0, 3, 0, 0, 0, // first wide value
                    4, 0, 0, 0, // second word
                    5, 0, // short
                    6, 0, 0, 0, 7, 0, 0, 0, // second wide value
                    1, 0, // row count
                    8, 0, 0, 0, 9, 0, 0, 0, 10, 0, // one row
                ]
            );
        }
    }

    #[test]
    fn circuit_module_notices_share_a_prefix_and_differ_by_tail() {
        for opcode in [
            game::RES_DS_CIRCUIT_MODULE_EQUIP,
            game::RES_DS_CIRCUIT_MODULE_MOVE,
        ] {
            let frame = CircuitModuleNoticePacket {
                opcode,
                flag: 1,
                slot: 2,
                second_flag: 3,
                wide_value: None,
                payload: vec![4, 5],
            }
            .encode();
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert_eq!(raw.payload, [1u8, 2, 0, 0, 0, 3, 2, 0, 4, 5]);
        }

        let frame = CircuitModuleNoticePacket {
            opcode: game::RES_DS_CIRCUIT_MODULE_UNEQUIP,
            flag: 6,
            slot: 7,
            second_flag: 8,
            wide_value: Some(0x0000_000A_0000_0009),
            payload: vec![11],
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RES_DS_CIRCUIT_MODULE_UNEQUIP);
        assert_eq!(
            raw.payload,
            [6u8, 7, 0, 0, 0, 8, 9, 0, 0, 0, 10, 0, 0, 0, 1, 0, 11]
        );
    }

    #[test]
    fn breakthrough_data_carries_a_counted_option_run() {
        let frame = CreateBreakthroughDataPacket {
            item: 1,
            flag: 2,
            options: vec![3, 4],
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RES_CREATE_BREAKTHROUGH_DATA);
        assert_eq!(
            raw.payload,
            [
                1, 0, 0, 0, // item
                2, 0, 0, 0, // flag
                2, 0, // option count
                3, 0, 0, 0, 4, 0, 0, 0, // options
            ]
        );
    }

    #[test]
    fn sync_verification_counts_its_words_with_a_short() {
        let frame = SyncVerificationPacket {
            values: vec![1, 2],
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RES_SYNC_VERIFICATION);
        assert_eq!(raw.payload, [2, 0, 1, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn mail_delete_carries_two_independent_runs() {
        let frame = MailDeleteResultPacket {
            first: MailDeleteRun {
                entries: vec![(1, 2)],
            },
            second: MailDeleteRun {
                entries: vec![(3, 4), (5, 6)],
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RES_MAIL_DELETE);
        assert_eq!(
            raw.payload,
            [
                1, 0, // first run count
                1, 0, 0, 0, 2, 0, 0, 0, // first run entry
                2, 0, // second run count
                3, 0, 0, 0, 4, 0, 0, 0, // first second-run entry
                5, 0, 0, 0, 6, 0, 0, 0, // second second-run entry
            ]
        );
    }

    #[test]
    fn ranking_stage_clear_mixes_short_byte_and_words() {
        let frame = RankingStageClearPacket {
            stage: 1,
            flag: 2,
            value: 3,
            second_value: 4,
            tail: 5,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NTF_RANKING_STAGE_CLEAR);
        assert_eq!(
            raw.payload,
            [
                1u8, 0, // stage
                2, // flag
                3, 0, 0, 0, // first word
                4, 0, 0, 0, // second word
                5, 0, // tail
            ]
        );
        assert_eq!(raw.payload.len(), 13);
    }

    #[test]
    fn skin_snapshot_counts_groups_and_items_separately() {
        let frame = InitSkinDataPacket {
            groups: vec![
                SkinGroup {
                    flag: 1,
                    items: vec![2, 3],
                },
                SkinGroup {
                    flag: 4,
                    items: vec![],
                },
            ],
            tail: 5,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NTF_INIT_SKIN_DATA);
        assert_eq!(
            raw.payload,
            [
                2, 0, // group count
                1, // group: flag
                2, 0, // group: item count
                2, 0, 0, 0, 3, 0, 0, 0, // group: items
                4, // group: flag
                0, 0, // group: empty item count
                5, 0, 0, 0, // tail
            ]
        );
    }

    #[test]
    fn recruiter_query_list_counts_its_rows_with_a_word() {
        let frame = PartyRecruitQueryListPacket {
            value: 1,
            second_value: 2,
            views: vec![PartyRecruitView {
                first_value: 3,
                second_value: 4,
                tamer: 5,
                digimon: 6,
                name: "a".to_string(),
                flag: 7,
                note: "b".to_string(),
                tail: 8,
            }],
            third_value: 9,
            fourth_value: 10,
            fifth_value: 11,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_RECRUIT_QUERY_LIST);
        let expected: Vec<u8> = vec![
            1, 0, 0, 0, 2, 0, 0, 0, 1, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0, 5, 0, 0, 0, 6, 0, 0, 0,
            1, 0, b'a', 7, 1, 0, b'b', 8, 0, 0, 0, 9, 0, 0, 0, 10, 0, 0, 0, 11, 0, 0, 0,
        ];
        assert_eq!(raw.payload, expected.as_slice());
    }

    #[test]
    fn recruiter_member_notices_read_exactly_one_member() {
        for opcode in [
            game::PARTY_RECRUIT_NEW_MEMBER,
            game::PARTY_RECRUIT_SYNC_MEMBER,
        ] {
            let frame = PartyRecruitMemberNoticePacket {
                opcode,
                member: PartyRecruitMember {
                    flag: 1,
                    tamer: 2,
                    level: 3,
                    digimon: 4,
                    icon: 5,
                    name: "ab".to_string(),
                },
            }
            .encode();
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert_eq!(
                raw.payload,
                [
                    1, // flag
                    2, 0, 0, 0, // tamer
                    3, 0, 0, 0, // level
                    4, 0, 0, 0, // digimon
                    5, 0, // icon
                    2, 0, b'a', b'b', // name: u2 byte length then bytes
                ]
            );
        }
    }

    #[test]
    fn recruiter_replies_carry_one_result_word() {
        for opcode in [
            game::PARTY_RECRUIT_MAKE,
            game::PARTY_RECRUIT_APPLY,
            game::PARTY_RECRUIT_CHAT,
            game::PARTY_RECRUIT_KICKOUT,
            game::PARTY_RECRUIT_WITHDRAW,
            game::PARTY_RECRUIT_CREATE_PARTY,
        ] {
            let frame = PartyRecruitReplyPacket { opcode, result: 1 }.encode();
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert_eq!(raw.payload, [1, 0, 0, 0]);
        }

        let frame = PartyRecruitLeaveMemberPacket {
            member_type: 2,
            member: 3,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_RECRUIT_LEAVE_MEMBER);
        assert_eq!(raw.payload, [2, 3, 0, 0, 0]);
    }

    #[test]
    fn raw_notices_write_their_fixed_runs() {
        for opcode in [
            game::NOTICE_3305,
            game::NOTICE_5354,
            game::NOTICE_3172,
            game::NOTICE_14058,
        ] {
            let frame = RawNoticePacket {
                opcode,
                body: RawNoticeBody::Word(5),
            }
            .encode();
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert_eq!(raw.payload, [5, 0, 0, 0]);
        }

        let frame = RawNoticePacket {
            opcode: game::NOTICE_16055,
            body: RawNoticeBody::DoubleWord(0x0000_0002_0000_0001),
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NOTICE_16055);
        assert_eq!(raw.payload, [1, 0, 0, 0, 2, 0, 0, 0]);

        let frame = RawNoticePacket {
            opcode: game::NOTICE_9851,
            body: RawNoticeBody::Fixed32([3; 32]),
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NOTICE_9851);
        assert_eq!(raw.payload, [3u8; 32]);
    }

    #[test]
    fn small_item_notices_write_only_their_own_shape() {
        for opcode in [game::ITEM_NOTICE_5005, game::ITEM_NOTICE_5009] {
            let frame = ItemSmallNoticePacket {
                opcode,
                body: ItemNoticeBody::FlagWord { flag: 1, value: 2 },
            }
            .encode();
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert_eq!(raw.payload, [1, 2, 0]);
        }

        let frame = ItemSmallNoticePacket {
            opcode: game::ITEM_NOTICE_5006,
            body: ItemNoticeBody::FlagTwoWords {
                flag: 3,
                value: 4,
                second_value: 5,
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.payload, [3, 4, 0, 5, 0]);

        let frame = ItemSmallNoticePacket {
            opcode: game::ITEM_NOTICE_5007,
            body: ItemNoticeBody::Word { value: 6 },
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.payload, [6, 0]);

        let frame = ItemSmallNoticePacket {
            opcode: game::ITEM_NOTICE_5008,
            body: ItemNoticeBody::Flag { flag: 7 },
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.payload, [7]);
    }

    #[test]
    fn macro_protector_result_carries_its_blob_length() {
        let frame = RefreshMacroProtectorResultPacket {
            value: 1,
            blob: vec![7, 8, 9],
            tail: 2,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RESULT_REFRESH_MACRO_PROTECTOR);
        assert_eq!(raw.payload, [1, 0, 0, 0, 3, 0, 7, 8, 9, 2]);
    }

    #[test]
    fn single_byte_notice_carries_one_dropped_byte() {
        let frame = SingleByteNoticePacket { value: 9 }.encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SINGLE_BYTE_NOTICE_2318);
        assert_eq!(raw.payload, [9]);
    }

    #[test]
    fn single_word_notices_share_one_shape() {
        for opcode in [
            game::NOTICE_1309,
            game::NOTICE_1315,
            game::NOTICE_9801,
            game::NOTICE_9804,
            game::NOTICE_9806,
        ] {
            let frame = SingleWordNoticePacket { opcode, value: 7 }.encode();
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert_eq!(raw.payload, [7, 0, 0, 0]);
        }

        let frame = HalfWordNoticePacket { value: 9 }.encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NOTICE_10003);
        assert_eq!(raw.payload, [9, 0]);
    }

    #[test]
    fn progress_notice_writes_a_word_then_a_double_word() {
        let frame = ProgressNoticePacket {
            value: 1,
            second_value: 0x0000_0002_0000_0003,
        }
        .encode();
        let raw = PacketReader::from_frame(&frame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PROGRESS_NOTICE_905);
        assert_eq!(raw.payload, [1, 0, 0, 0, 3, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn no_payload_notices_emit_a_bare_header() {
        let opcodes = [
            game::NO_PAYLOAD_1031,
            game::NO_PAYLOAD_1079,
            game::NO_PAYLOAD_1080,
            game::NO_PAYLOAD_1089,
            game::NO_PAYLOAD_1103,
            game::NO_PAYLOAD_1112,
            game::NO_PAYLOAD_2121,
            game::NO_PAYLOAD_2205,
            game::NO_PAYLOAD_3003,
            game::NO_PAYLOAD_3101,
            game::NO_PAYLOAD_3906,
            game::NO_PAYLOAD_3908,
            game::NO_PAYLOAD_3919,
            game::NO_PAYLOAD_3920,
            game::NO_PAYLOAD_3921,
            game::NO_PAYLOAD_3928,
            game::NO_PAYLOAD_9904,
            game::NO_PAYLOAD_2104,
            game::NO_PAYLOAD_4128,
            game::NO_PAYLOAD_4139,
            game::NO_PAYLOAD_4142,
            game::NO_PAYLOAD_4601,
            game::NO_PAYLOAD_5358,
        ];
        for opcode in opcodes {
            let frame = PacketWriter::new(opcode).finalize();
            assert_eq!(frame.len(), 6, "opcode {opcode} must be header only");
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert!(raw.payload.is_empty(), "opcode {opcode} must carry no body");
        }
    }

    #[test]
    fn msg_scope_notices_write_their_bodies() {
        for (frame, opcode) in [
            (encode_dungeon_match_end(), game::MSG_DUNGEON_MATCH_END),
            (encode_dungeon_match_cancel(), game::MSG_DUNGEON_MATCH_CANCEL),
        ] {
            let raw = PacketReader::from_frame(&frame).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert!(raw.payload.is_empty());
        }

        let settings = UserSettingDataPacket {
            entries: vec![
                UserSettingEntry {
                    prefix: 1,
                    value: "ab".to_string(),
                },
                UserSettingEntry {
                    prefix: 2,
                    value: "c".to_string(),
                },
            ],
        }
        .encode();
        let raw = PacketReader::from_frame(&settings).expect("frame should decode");
        assert_eq!(raw.packet_type, game::USER_SETTING_DATA);
        assert_eq!(
            raw.payload,
            [
                2, 0, // count
                1, 0, 2, 0, b'a', b'b', // first row: prefix then u2 length + bytes
                2, 0, 1, 0, b'c', // second row
            ]
        );

        let balance = CashBalanceNoticePacket { value: 1 }.encode();
        let raw = PacketReader::from_frame(&balance).expect("frame should decode");
        assert_eq!(raw.packet_type, game::CASH_BALANCE_NOTICE);
        assert_eq!(raw.payload, [1, 0, 0, 0]);

        let limits = ItemMakeLimitDataPacket {
            entries: vec![
                ItemMakeLimitEntry {
                    value: 1,
                    limit: 60,
                },
                ItemMakeLimitEntry {
                    value: 2,
                    limit: 120,
                },
            ],
        }
        .encode();
        let raw = PacketReader::from_frame(&limits).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ITEM_MAKE_LIMIT_DATA);
        assert_eq!(
            raw.payload,
            [
                2, 0, // count
                1, 0, 0, 0, 60, 0, 0, 0, // first row
                2, 0, 0, 0, 120, 0, 0, 0, // second row
            ]
        );

        let reset = MakeLimitResetPacket {
            values: vec![7, 8],
        }
        .encode();
        let raw = PacketReader::from_frame(&reset).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MAKE_LIMIT_RESET);
        assert_eq!(raw.payload, [2, 0, 7, 0, 0, 0, 8, 0, 0, 0]);

        let protector = encode_refresh_macro_protector();
        let raw = PacketReader::from_frame(&protector).expect("frame should decode");
        assert_eq!(raw.packet_type, game::REFRESH_MACRO_PROTECTOR);
        assert!(raw.payload.is_empty());

        let schedule = DungeonScheduleListPacket {
            dungeon_id: 1,
            value: 2,
            second_value: 3,
            clears: vec![DungeonClearEntry { flag: 4, value: 5 }],
            schedules: vec![DungeonScheduleEntry {
                kind: 6,
                value: 7,
                second_value: 8,
                short_value: 9,
                title: "t".to_string(),
                third_value: 10,
                second_short: 11,
                note: "n".to_string(),
                members: vec![DungeonScheduleMember {
                    name: "m".to_string(),
                    value: 12,
                }],
            }],
            rows: vec![DungeonScheduleRow {
                head: 13,
                tail: 14,
                body: [15; 69],
            }],
            gifts: vec![DungeonScheduleGift {
                value: 16,
                item: 17,
            }],
        }
        .encode();
        let raw = PacketReader::from_frame(&schedule).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MSG_DUNGEON_SCHEDULE_LIST);
        let payload = raw.payload;
        assert_eq!(&payload[0..4], &[1, 0, 0, 0], "dungeon id");
        assert_eq!(&payload[4..12], &[2, 0, 0, 0, 0, 0, 0, 0], "first u8");
        assert_eq!(&payload[12..20], &[3, 0, 0, 0, 0, 0, 0, 0], "second u8");
        assert_eq!(payload[20], 1, "clear count");
        assert_eq!(&payload[21..26], &[4, 5, 0, 0, 0], "clear row");
        assert_eq!(payload[26], 1, "schedule count");
        assert_eq!(payload[27], 6, "schedule kind");
        assert_eq!(&payload[28..36], &[7, 0, 0, 0, 0, 0, 0, 0], "schedule u8");
        assert_eq!(&payload[36..40], &[8, 0, 0, 0], "schedule u4");
        assert_eq!(&payload[40..42], &[9, 0], "schedule u2");
        assert_eq!(&payload[42..45], &[1, 0, b't'], "title prefix");
        assert_eq!(&payload[45..49], &[10, 0, 0, 0], "schedule u4");
        assert_eq!(&payload[49..51], &[11, 0], "schedule u2");
        assert_eq!(&payload[51..54], &[1, 0, b'n'], "note prefix");
        assert_eq!(payload[54], 1, "member count");
        assert_eq!(&payload[55..58], &[1, 0, b'm'], "member name");
        assert_eq!(&payload[58..66], &[12, 0, 0, 0, 0, 0, 0, 0], "member u8");
        assert_eq!(payload[66], 1, "row count");
        assert_eq!(&payload[67..71], &[13, 0, 14, 0], "row head/tail");
        assert_eq!(&payload[71..140], &[15u8; 69], "row body");
        assert_eq!(payload[140], 1, "gift count");
        assert_eq!(&payload[141..147], &[16, 0, 0, 0, 17, 0], "gift row");
        assert_eq!(payload.len(), 147, "no trailing bytes");

        let stage = DungeonStageInfoPacket {
            slot: 1,
            id: 2,
            name: "ab".to_string(),
            third: 3,
            fourth: 4,
            fifth: 0x0000_0006_0000_0005,
        }
        .encode();
        let raw = PacketReader::from_frame(&stage).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MSG_DUNGEON_STAGE_INFO);
        assert_eq!(
            raw.payload,
            [
                1, 0, 0, 0, // slot
                2, 0, 0, 0, // id
                2, 0, b'a', b'b', // string: u2 byte length then bytes
                3, 0, // u2
                4, 0, 0, 0, // u4
                5, 0, 0, 0, 6, 0, 0, 0, // u8
            ]
        );

        let clear = DungeonClearInfoPacket {
            value: 7,
            clears: vec![DungeonClearEntry { flag: 1, value: 2 }],
            rewards: vec![DungeonClearReward {
                value: 3,
                item: 4,
            }],
        }
        .encode();
        let raw = PacketReader::from_frame(&clear).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MSG_DUNGEON_CLEAR_INFO);
        assert_eq!(
            raw.payload,
            vec![
                7, 0, 0, 0, // value
                1, // clear count
                1, 2, 0, 0, 0, // clear row
                1, // reward count
                3, 0, 0, 0, 4, 0, // reward row
            ]
        );

        let state = DungeonMatchStatePacket {
            value: 1,
            flag: 2,
            second_flag: 3,
            second_value: 4,
            trailing: 5,
        }
        .encode();
        let raw = PacketReader::from_frame(&state).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MSG_DUNGEON_MATCH_STATE);
        assert_eq!(raw.payload, [1, 0, 0, 0, 2, 3, 4, 0, 0, 0, 5, 0]);

        let result = DungeonMatchResultPacket {
            value: 6,
            result: 7,
            second_value: 8,
        }
        .encode();
        let raw = PacketReader::from_frame(&result).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MSG_DUNGEON_MATCH_RESULT);
        assert_eq!(raw.payload, [6, 0, 0, 0, 7, 8, 0, 0, 0]);

        let counter = DungeonMatchCounterPacket {
            value: 9,
            first: 0x1122_3344_5566_7788,
            second: u64::MAX,
        }
        .encode();
        let raw = PacketReader::from_frame(&counter).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MSG_DUNGEON_MATCH_COUNTER);
        assert_eq!(
            raw.payload,
            [
                9, 0, 0, 0, // value
                0x88, 0x77, 0x66, 0x55, 0x44, 0x33, 0x22, 0x11, // first
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, // second
            ]
        );

        let re_enter = DungeonReEnterResultPacket { result: 1 }.encode();
        let raw = PacketReader::from_frame(&re_enter).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MSG_DUNGEON_RE_ENTER_RESULT);
        assert_eq!(raw.payload, [1]);

        let progress = RankingPlayProgressPacket {
            dungeon_id: 4001,
            fixed: [7, 9, 11, 13, 15, 17],
            stages: vec![RankingPlayProgressStage {
                stage: 1,
                wave: 2,
                value: 3,
            }],
        }
        .encode();
        let raw = PacketReader::from_frame(&progress).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MSG_RANKING_PLAY_PROGRESS);
        assert_eq!(
            raw.payload,
            vec![
                161, 15, // dungeon id 4001
                7, 0, 0, 0, // u4
                9, // u1
                11, 0, 0, 0, // u4
                13, 0, // u2
                15, 0, 0, 0, // u4
                17, 0, // u2
                1, 0, // stage count
                1, 0, 2, 0, 3, 0, 0, 0, // stage row
            ]
        );
    }

    #[test]
    fn game_two_all_msg_writes_the_case_word_then_its_body() {
        let level_up = GameTwoAllMsgPacket {
            body: GameTwoAllMsgBody::ItemLevelUp {
                tamer: "Ana".to_string(),
                item_id: 7,
                level: 30,
            },
        }
        .encode();
        let level_raw = PacketReader::from_frame(&level_up).expect("frame should decode");
        assert_eq!(level_raw.packet_type, game::GAMETWO_ALL_MSG);
        assert_eq!(
            level_raw.payload,
            vec![1, 0, 3, 0, b'A', b'n', b'a', 7, 0, 0, 0, 30, 0, 0, 0]
        );

        let raid = GameTwoAllMsgPacket {
            body: GameTwoAllMsgBody::RaidMonsterDie {
                monster_id: 11,
                tamer: "Bo".to_string(),
                digimon_id: 12,
                item_id: 0,
            },
        }
        .encode();
        let raid_raw = PacketReader::from_frame(&raid).expect("frame should decode");
        assert_eq!(raid_raw.packet_type, game::GAMETWO_ALL_MSG);
        assert_eq!(
            raid_raw.payload,
            vec![69, 6, 11, 0, 0, 0, 2, 0, b'B', b'o', 12, 0, 0, 0, 0, 0, 0, 0]
        );

        for (body, case) in [
            (
                GameTwoAllMsgBody::RandomMonsterCreate {
                    monster_id: 21,
                    map_id: 22,
                },
                1606u16,
            ),
            (
                GameTwoAllMsgBody::RandomMonsterEnd {
                    monster_id: 21,
                    map_id: 22,
                },
                1609u16,
            ),
        ] {
            let packet = GameTwoAllMsgPacket { body }.encode();
            let raw = PacketReader::from_frame(&packet).expect("frame should decode");
            assert_eq!(raw.packet_type, game::GAMETWO_ALL_MSG);
            let mut expected = case.to_le_bytes().to_vec();
            expected.extend_from_slice(&[21, 0, 0, 0, 22, 0, 0, 0]);
            assert_eq!(raw.payload, expected);
        }
    }

    #[test]
    fn game_two_monster_area_attack_bodies_follow_the_row_kind() {
        let caster_only = GameTwoMonsterAreaAttackPacket {
            caster: 5,
            monster_id: 900,
            body: MonsterAreaBody::CasterOnly,
        }
        .encode();
        let caster_raw = PacketReader::from_frame(&caster_only).expect("frame should decode");
        assert_eq!(caster_raw.packet_type, game::GAMETWO_MONSTER_AREA_ATTACK);
        assert_eq!(caster_raw.payload, [5, 0, 0, 0, 132, 3, 0, 0]);

        let targets = GameTwoMonsterAreaAttackPacket {
            caster: 5,
            monster_id: 900,
            body: MonsterAreaBody::Targets(vec![7, 8]),
        }
        .encode();
        let targets_raw = PacketReader::from_frame(&targets).expect("frame should decode");
        assert_eq!(
            targets_raw.payload,
            [5, 0, 0, 0, 132, 3, 0, 0, 2, 0, 0, 0, 7, 0, 0, 0, 8, 0, 0, 0]
        );

        let pairs = GameTwoMonsterAreaAttackPacket {
            caster: 5,
            monster_id: 900,
            body: MonsterAreaBody::TargetPairs(vec![MonsterAreaTarget {
                first: 30,
                second: 40,
            }]),
        }
        .encode();
        let pairs_raw = PacketReader::from_frame(&pairs).expect("frame should decode");
        assert_eq!(
            pairs_raw.payload,
            [5, 0, 0, 0, 132, 3, 0, 0, 1, 0, 0, 0, 30, 0, 0, 0, 40, 0, 0, 0]
        );
    }

    #[test]
    fn skill_limit_open_writes_row_index_blob_and_gate_pair() {
        let packet = DigimonSkillLimitOpenPacket {
            state: 0,
            digimon_index: 3,
            skill_blob: (0..28).collect::<Vec<u8>>(),
            skill_index: 7,
            expected_value: 1,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_LIMIT_OPEN);
        assert_eq!(raw.payload.len(), 4 + 4 + 28 + 4 + 4);
        assert_eq!(&raw.payload[4..8], &[3, 0, 0, 0]);
        assert_eq!(&raw.payload[8..36], &(0..28).collect::<Vec<u8>>()[..]);
        assert_eq!(&raw.payload[36..44], &[7, 0, 0, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn macro_family_and_dropped_slot_write_fixed_bodies() {
        let failure = MacroResultNoticePacket { result: 5 }.encode_on(game::MACRO_FAILURE);
        let failure_raw = PacketReader::from_frame(&failure).expect("frame should decode");
        assert_eq!(failure_raw.packet_type, game::MACRO_FAILURE);
        assert_eq!(failure_raw.payload, [5, 0, 0, 0]);

        let result = MacroResultNoticePacket { result: 9 }.encode_on(game::MACRO_RESULT);
        let result_raw = PacketReader::from_frame(&result).expect("frame should decode");
        assert_eq!(result_raw.packet_type, game::MACRO_RESULT);
        assert_eq!(result_raw.payload, [9, 0, 0, 0]);

        let success = MacroSuccessNoticePacket.encode();
        let success_raw = PacketReader::from_frame(&success).expect("frame should decode");
        assert_eq!(success_raw.packet_type, game::MACRO_SUCCESS);
        assert_eq!(success_raw.payload, []);

        let dropped = DroppedSlotNoticePacket { reserved: 1 }.encode_on(game::MSG_ALL_FAILURE);
        let dropped_raw = PacketReader::from_frame(&dropped).expect("frame should decode");
        assert_eq!(dropped_raw.packet_type, game::MSG_ALL_FAILURE);
        assert_eq!(dropped_raw.payload, [1, 0, 0, 0]);
    }

    #[test]
    fn game_get_exp_notice_writes_five_pairs_between_two_u32() {
        let packet = GameGetExpNoticePacket {
            first: 1,
            second: 2,
            third: 3,
            flag: 7,
            fourth: 4,
            fifth: 5,
            exp_value: 88,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAME_GET_EXP);
        assert_eq!(raw.payload.len(), 48);
        assert_eq!(&raw.payload[24..28], &[7, 0, 0, 0]);
        assert_eq!(&raw.payload[44..48], &[88, 0, 0, 0]);
    }

    #[test]
    fn target_info_change_stat_and_event_fill_expected_bodies() {
        let target = GameTargetInfoPacket {
            target_handler: 0x100,
            flag: 2,
            first: 3,
            second: 4,
        }
        .encode();
        let target_raw = PacketReader::from_frame(&target).expect("frame should decode");
        assert_eq!(target_raw.packet_type, game::GAME_TARGET_INFO);
        assert_eq!(
            target_raw.payload,
            [0, 1, 0, 0, 2, 3, 0, 0, 0, 4, 0, 0, 0]
        );

        let change = GameChangeStatPacket {
            handler: 77,
            stats: vec![(1, 10), (9, 20), (12, 30)],
        }
        .encode();
        let change_raw = PacketReader::from_frame(&change).expect("frame should decode");
        assert_eq!(change_raw.packet_type, game::GAME_CHANGE_STAT);
        assert_eq!(
            change_raw.payload,
            [
                77, 0, 0, 0, // handler
                3, 0,        // count
                1, 0, 10, 0, 0, 0, // stat 1
                9, 0, 20, 0, 0, 0,  // stat 9
                12, 0, 30, 0, 0, 0, // stat 12
            ]
        );

        let event = GameEventNoticePacket { first: 5, second: 6 }.encode();
        let event_raw = PacketReader::from_frame(&event).expect("frame should decode");
        assert_eq!(event_raw.packet_type, game::GAME_EVENT);
        assert_eq!(event_raw.payload, [5, 0, 6, 0, 0, 0]);
    }

    #[test]
    fn svr_channel_info_writes_sparse_pairs_then_sentinel() {
        let packet = SvrChannelInfoPacket {
            slots: BTreeMap::from([(1u8, 30u8), (9u8, 2u8), (60u8, 7u8)]),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SVR_CHANNEL_INFO);
        // Offsets >= 32 fall outside the client's 32-byte slot buffer and are
        // rejected by the writer.
        assert_eq!(raw.payload, [1, 30, 9, 2, 0xFF]);
    }

    #[test]
    fn digi_summon_sync_response_uses_modern_wire_shape() {
        let packet = DigiSummonSyncResponsePacket {
            result: 0,
            products: vec![DigiSummonProduct {
                product_id: 9001,
                rank: 2,
                draw_count: 3,
                remaining_daily_limit: 4,
                ..DigiSummonProduct::default()
            }],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGI_SUMMON_SYNC_RESPONSE);
        // [u8 result][u16 count] + one 14-byte product = 17 bytes.
        assert_eq!(raw.payload.len(), 17);
        let mut reader = crate::PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), 0);
        assert_eq!(reader.read_u16().expect("count"), 1);
        assert_eq!(reader.read_i32().expect("product"), 9001);
        assert_eq!(reader.read_i32().expect("rank"), 2);
        assert_eq!(reader.read_u16().expect("draw count"), 3);
        assert_eq!(reader.read_i32().expect("daily limit"), 4);
    }

    #[test]
    fn digi_summon_sync_response_empty_list_round_trips() {
        let packet = DigiSummonSyncResponsePacket {
            result: 0,
            products: Vec::new(),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGI_SUMMON_SYNC_RESPONSE);
        // [u8 result][u16 count] with no products = 3 bytes.
        assert_eq!(raw.payload.len(), 3);
        let mut reader = crate::PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), 0);
        assert_eq!(reader.read_u16().expect("count"), 0);
    }

    #[test]
    fn digi_summon_purchase_response_uses_modern_wire_shape() {
        let packet = DigiSummonPurchaseResponsePacket {
            result: 0,
            product_id: 9001,
            rewards: vec![DigiSummonReward {
                item_id: 5101,
                amount: 2,
                grade: 5,
                ..DigiSummonReward::default()
            }],
            products: vec![DigiSummonProduct {
                product_id: 9001,
                rank: 1,
                draw_count: 1,
                remaining_daily_limit: 0,
                ..DigiSummonProduct::default()
            }],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGI_SUMMON_PURCHASE_RESPONSE);
        // [u8 result][i32 product][u16 reward_count] + one 8-byte reward
        // + [u16 product_count] + one 14-byte product + [u16 detail_count]
        // + [i64 trailer] = 41 bytes.
        assert_eq!(raw.payload.len(), 41);
        let mut reader = crate::PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), 0);
        assert_eq!(reader.read_i32().expect("product"), 9001);
        assert_eq!(reader.read_u16().expect("reward count"), 1);
        assert_eq!(reader.read_i32().expect("reward item"), 5101);
        assert_eq!(reader.read_u16().expect("reward amount"), 2);
        assert_eq!(reader.read_u16().expect("reward grade"), 5);
        assert_eq!(reader.read_u16().expect("product count"), 1);
        assert_eq!(reader.read_i32().expect("synced product"), 9001);
        assert_eq!(reader.read_i32().expect("synced rank"), 1);
        assert_eq!(reader.read_u16().expect("synced draw count"), 1);
        assert_eq!(reader.read_i32().expect("synced daily limit"), 0);
        assert_eq!(reader.read_u16().expect("detail count"), 0);
        assert_eq!(reader.read_u64().expect("trailer"), 0);
    }

    #[test]
    fn digi_summon_purchase_response_empty_lists_round_trip() {
        let packet = DigiSummonPurchaseResponsePacket {
            result: 1,
            product_id: 9001,
            rewards: Vec::new(),
            products: Vec::new(),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGI_SUMMON_PURCHASE_RESPONSE);
        // [u8 result][i32 product][u16 reward_count=0][u16 product_count=0]
        // [u16 detail_count=0][i64 trailer] = 19 bytes.
        assert_eq!(raw.payload.len(), 19);
        let mut reader = crate::PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), 1);
        assert_eq!(reader.read_i32().expect("product"), 9001);
        assert_eq!(reader.read_u16().expect("reward count"), 0);
        assert_eq!(reader.read_u16().expect("product count"), 0);
        assert_eq!(reader.read_u16().expect("detail count"), 0);
        assert_eq!(reader.read_u64().expect("trailer"), 0);
    }

    #[test]
    fn random_box_list_request_round_trips() {
        let mut payload = Vec::new();
        payload.push(1_u8); // flag
        payload.extend_from_slice(&(42_i32).to_le_bytes()); // index

        let request = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::RANDOM_BOX_LIST,
            payload,
        })
        .expect("request should parse");

        assert_eq!(request, GameRequest::RandomBoxList { flag: 1, index: 42 });
    }

    #[test]
    fn random_box_purchase_request_round_trips() {
        let mut payload = Vec::new();
        payload.push(1_u8); // flag
        payload.extend_from_slice(&(9001_i32).to_le_bytes()); // product_id
        payload.extend_from_slice(&(7777_i32).to_le_bytes()); // item_uid
        payload.extend_from_slice(&(3_u16).to_le_bytes()); // count
        payload.extend_from_slice(&(5_i32).to_le_bytes()); // state

        let request = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::RANDOM_BOX_PURCHASE,
            payload,
        })
        .expect("request should parse");

        assert_eq!(
            request,
            GameRequest::RandomBoxPurchase {
                flag: 1,
                product_id: 9001,
                item_uid: 7777,
                count: 3,
                state: 5,
            }
        );
    }

    #[test]
    fn random_box_list_response_uses_fixed_wire_shape() {
        let packet = RandomBoxListResponsePacket {
            field0: 11,
            entries: vec![RandomBoxListEntry {
                a: 1,
                b: 2,
                c: 3,
                d: 4,
            }],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RANDOM_BOX_LIST);
        // [i32 field0][u8 count] + one 14-byte entry = 19 bytes.
        assert_eq!(raw.payload.len(), 19);
        let mut reader = crate::PacketReader::new(raw.payload);
        assert_eq!(reader.read_i32().expect("field0"), 11);
        assert_eq!(reader.read_u8().expect("count"), 1);
        assert_eq!(reader.read_i32().expect("entry a"), 1);
        assert_eq!(reader.read_i32().expect("entry b"), 2);
        assert_eq!(reader.read_i32().expect("entry c"), 3);
        assert_eq!(reader.read_u16().expect("entry d"), 4);
    }

    #[test]
    fn random_box_list_response_empty_list_round_trips() {
        let packet = RandomBoxListResponsePacket {
            field0: 11,
            entries: Vec::new(),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RANDOM_BOX_LIST);
        // [i32 field0][u8 count] with no entries = 5 bytes.
        assert_eq!(raw.payload.len(), 5);
        let mut reader = crate::PacketReader::new(raw.payload);
        assert_eq!(reader.read_i32().expect("field0"), 11);
        assert_eq!(reader.read_u8().expect("count"), 0);
    }

    #[test]
    fn random_box_purchase_response_uses_fixed_wire_shape() {
        let packet = RandomBoxPurchaseResponsePacket {
            field0: 11,
            field1: 22,
            field2: 33,
            list_a: vec![(1, 2)],
            list_b: vec![(0x1122_3344_5566_7788, 9)],
            summary: (0x99AA_BBCC_DDEE_FF00, 7),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RANDOM_BOX_PURCHASE);
        // [i32 field0][i32 field1][u16 field2][u8 count1] + one 8-byte pair
        // + [u8 count2] + one 10-byte pair + [u64][u16] summary = 40 bytes.
        assert_eq!(raw.payload.len(), 40);
        let mut reader = crate::PacketReader::new(raw.payload);
        assert_eq!(reader.read_i32().expect("field0"), 11);
        assert_eq!(reader.read_i32().expect("field1"), 22);
        assert_eq!(reader.read_u16().expect("field2"), 33);
        assert_eq!(reader.read_u8().expect("list_a count"), 1);
        assert_eq!(reader.read_i32().expect("list_a a"), 1);
        assert_eq!(reader.read_i32().expect("list_a b"), 2);
        assert_eq!(reader.read_u8().expect("list_b count"), 1);
        assert_eq!(reader.read_u64().expect("list_b a"), 0x1122_3344_5566_7788);
        assert_eq!(reader.read_u16().expect("list_b b"), 9);
        assert_eq!(reader.read_u64().expect("summary a"), 0x99AA_BBCC_DDEE_FF00);
        assert_eq!(reader.read_u16().expect("summary b"), 7);
    }

    #[test]
    fn random_box_purchase_response_empty_lists_round_trip() {
        let packet = RandomBoxPurchaseResponsePacket {
            field0: 11,
            field1: 22,
            field2: 33,
            list_a: Vec::new(),
            list_b: Vec::new(),
            summary: (0x99AA_BBCC_DDEE_FF00, 7),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RANDOM_BOX_PURCHASE);
        // [i32 field0][i32 field1][u16 field2][u8 count1=0][u8 count2=0]
        // + [u64][u16] summary = 22 bytes.
        assert_eq!(raw.payload.len(), 22);
        let mut reader = crate::PacketReader::new(raw.payload);
        assert_eq!(reader.read_i32().expect("field0"), 11);
        assert_eq!(reader.read_i32().expect("field1"), 22);
        assert_eq!(reader.read_u16().expect("field2"), 33);
        assert_eq!(reader.read_u8().expect("list_a count"), 0);
        assert_eq!(reader.read_u8().expect("list_b count"), 0);
        assert_eq!(reader.read_u64().expect("summary a"), 0x99AA_BBCC_DDEE_FF00);
        assert_eq!(reader.read_u16().expect("summary b"), 7);
    }

    #[test]
    fn encyclopedia_open_request_reuses_3234_without_payload() {
        let packet = PacketWriter::new(game::SEAL_REMOVE_LEADER).finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");

        let request = GameRequest::try_from(raw).expect("request should decode");
        assert!(matches!(request, GameRequest::EncyclopediaLoad));
    }

    #[test]
    fn seal_remove_leader_request_uses_3234_with_u16_payload() {
        let mut writer = PacketWriter::new(game::SEAL_REMOVE_LEADER);
        writer.write_u16(1);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");

        let request = GameRequest::try_from(raw).expect("request should decode");
        assert!(matches!(request, GameRequest::SealRemoveLeader));
    }

    #[test]
    fn encyclopedia_open_request_still_accepts_3235_route() {
        let packet = PacketWriter::new(game::ENCYCLOPEDIA_LOAD).finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");

        let request = GameRequest::try_from(raw).expect("request should decode");
        assert!(matches!(request, GameRequest::EncyclopediaLoad));
    }

    #[test]
    fn care_slot_size_notice_encodes_u2_payload() {
        let packet = CareSlotSizePacket { slot_count: 12 }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_CARE_SLOT_SIZE);
        assert_eq!(raw.payload, 12u16.to_le_bytes());
    }

    #[test]
    fn battle_tag_notices_are_payloadless_frames() {
        for (packet, opcode) in [
            (BattleTagSuccessPacket.encode(), game::DIGIMON_BATTLE_TAG_SUCCESS),
            (BattleTagFailurePacket.encode(), game::DIGIMON_BATTLE_TAG_FAILURE),
        ] {
            let raw = PacketReader::from_frame(&packet).expect("frame should decode");
            assert_eq!(raw.packet_type, opcode);
            assert!(raw.payload.is_empty(), "expected empty payload");
        }
    }

    #[test]
    fn skill_point_init_encodes_count_and_blob_records() {
        let records: Vec<[u8; 28]> = (0u8..3).map(|i| core::array::from_fn(|k| i * 28 + k as u8)).collect();
        let expect: Vec<u8> = records.iter().flat_map(|r| r.iter().copied()).collect();
        let packet = DigimonSkillPointInitPacket { records }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_SKILL_POINT_INIT);
        assert_eq!(raw.payload[0], 3, "count byte");
        assert_eq!(raw.payload[1..].len(), 3 * 28, "record bytes");
        assert_eq!(&raw.payload[1..], &expect[..]);
    }

    #[test]
    fn facilitation_encodes_u8_then_u32() {
        let packet = DigimonFacilitationPacket { kind: 7, arg: 0xAABB_CCDD }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_FACILITATION);
        assert_eq!(raw.payload, [7, 0xDD, 0xCC, 0xBB, 0xAA]);
    }

    #[test]
    fn incubater_error_is_an_opaque_u4_notice() {
        let packet = DigimonIncubaterErrorPacket { code: 0x11223344 }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_INCUBATER_ERROR);
        assert_eq!(raw.payload, 0x11223344u32.to_le_bytes());
    }

    #[test]
    fn incubator_list_rows_extraction_and_slot_change_fill_bodies() {
        let list = DigimonIncubaterListPacket {
            rows: vec![(1, 2, 3), (4, 5, 6), (7, 8, 9)],
        }
        .encode();
        let list_raw = PacketReader::from_frame(&list).expect("frame should decode");
        assert_eq!(list_raw.packet_type, game::DIGIMON_INCUBATER_LIST);
        assert_eq!(list_raw.payload.len(), 36);
        assert_eq!(&list_raw.payload[..8], &[1, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&list_raw.payload[32..36], &[9, 0, 0, 0]);

        let extraction = DigimonExtractionPacket { result: 2 }.encode();
        let extraction_raw = PacketReader::from_frame(&extraction).expect("frame should decode");
        assert_eq!(extraction_raw.packet_type, game::DIGIMON_EXTRACTION);
        assert_eq!(extraction_raw.payload, [2]);

        let slot = DigimonIncubaterSlotChangePacket {
            slot: 5,
            value: 1000,
        }
        .encode();
        let slot_raw = PacketReader::from_frame(&slot).expect("frame should decode");
        assert_eq!(slot_raw.packet_type, game::DIGIMON_INCUBATER_SLOT_CHANGE);
        assert_eq!(slot_raw.payload, [5, 0, 0, 0, 232, 3, 0, 0]);
    }

    #[test]
    fn inchant_info_writes_handler_anchor_and_five_slots() {
        let packet = DigimonInchantInfoPacket {
            handler: 9,
            anchor: 3,
            entries: vec![1, 2, 3, 4, 5],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_INCHANT_INFO);
        assert_eq!(
            raw.payload,
            [
                9, 0, 0, 0, // handler
                3, 0,       // anchor
                1, 0, 2, 0, 3, 0, 4, 0, 5, 0, // five slots
            ]
        );
    }

    #[test]
    fn decrease_inchant_writes_anchor_and_two_seven_value_runs() {
        let packet = DigimonDecreaseInchantPacket {
            anchor: 4,
            first_run: (0..7).collect(),
            second_run: (10..17).collect(),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_DECREASE_INCHANT);
        assert_eq!(raw.payload.len(), 30);
        assert_eq!(&raw.payload[..2], &[4, 0]);
        assert_eq!(&raw.payload[2..16], &(0u16..7).map(|v| v.to_le_bytes()).flatten().collect::<Vec<u8>>()[..]);
        assert_eq!(&raw.payload[16..30], &(10u16..17).map(|v| v.to_le_bytes()).flatten().collect::<Vec<u8>>()[..]);
    }

    #[test]
    fn inchant_writes_handler_anchor_and_two_runs() {
        let packet = DigimonInchantPacket {
            handler: 0xAABB,
            anchor: 2,
            first_run: (0..7).collect(),
            second_run: (10..17).collect(),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_INCHANT);
        assert_eq!(raw.payload.len(), 34);
        assert_eq!(&raw.payload[..2], &[0xBB, 0xAA]);
        assert_eq!(&raw.payload[4..6], &[2, 0]);
        assert_eq!(&raw.payload[6..20], &(0u16..7).map(|v| v.to_le_bytes()).flatten().collect::<Vec<u8>>()[..]);
        assert_eq!(&raw.payload[20..34], &(10u16..17).map(|v| v.to_le_bytes()).flatten().collect::<Vec<u8>>()[..]);
    }

    #[test]
    fn coat_and_slot_size_notices_fill_expected_bodies() {
        let all = ItemCoatNoticePacket { value: 1, handler: 0x77 }.encode_on(game::ITEM_COAT_ALL);
        let all_raw = PacketReader::from_frame(&all).expect("frame should decode");
        assert_eq!(all_raw.packet_type, game::ITEM_COAT_ALL);
        assert_eq!(all_raw.payload, 0x77u32.to_le_bytes());

        let adventure = ItemCoatNoticePacket { value: 5, handler: 9 }.encode_on(game::ITEM_COAT_ADVENTURE);
        let adventure_raw = PacketReader::from_frame(&adventure).expect("frame should decode");
        assert_eq!(adventure_raw.packet_type, game::ITEM_COAT_ADVENTURE);
        assert_eq!(adventure_raw.payload, [5, 0, 0, 0, 9, 0, 0, 0]);

        let inventory = ItemSlotSizePacket { size: 64 }.encode_on(game::ITEM_INVEN_SLOT_SIZE);
        let inventory_raw = PacketReader::from_frame(&inventory).expect("frame should decode");
        assert_eq!(inventory_raw.packet_type, game::ITEM_INVEN_SLOT_SIZE);
        assert_eq!(inventory_raw.payload, [64, 0]);

        let warehouse = ItemSlotSizePacket { size: 128 }.encode_on(game::ITEM_WAREHOUSE_SLOT_SIZE);
        let warehouse_raw = PacketReader::from_frame(&warehouse).expect("frame should decode");
        assert_eq!(warehouse_raw.packet_type, game::ITEM_WAREHOUSE_SLOT_SIZE);
        assert_eq!(warehouse_raw.payload, [128, 0]);
    }

    #[test]
    fn use_failure_and_portable_fail_fill_expected_bodies() {
        let failure = ItemUseFailurePacket {
            code: 3,
            item_serial: 0x1122,
            failure_kind: 1,
        }
        .encode();
        let failure_raw = PacketReader::from_frame(&failure).expect("frame should decode");
        assert_eq!(failure_raw.packet_type, game::ITEM_USE_FAILURE);
        assert_eq!(
            failure_raw.payload,
            [3, 0, 0x22, 0x11, 0, 0, 1, 0, 0, 0]
        );

        let portable = PortableItemFailPacket { code: 7701 }.encode();
        let portable_raw = PacketReader::from_frame(&portable).expect("frame should decode");
        assert_eq!(portable_raw.packet_type, game::ITEM_PORTABLE_FAIL);
        assert_eq!(portable_raw.payload, (7701u32).to_le_bytes());
    }

    #[test]
    fn crop_money_and_limited_attribute_fill_expected_bodies() {
        let crop = ItemCropMoneyPacket {
            first_amount_lo: 100,
            first_amount_hi: 0,
            third: 40,
            fourth: 60,
        }
        .encode();
        let crop_raw = PacketReader::from_frame(&crop).expect("frame should decode");
        assert_eq!(crop_raw.packet_type, game::ITEM_CROP_MONEY);
        assert_eq!(crop_raw.payload.len(), 16);
        assert_eq!(&crop_raw.payload[..4], &100u32.to_le_bytes());
        assert_eq!(&crop_raw.payload[8..16], &40u32.to_le_bytes().iter().chain(&60u32.to_le_bytes()).copied().collect::<Vec<u8>>()[..]);

        let limited = ChangeLimitedAttributePacket { attribute: 9, flag: 1 }.encode();
        let limited_raw = PacketReader::from_frame(&limited).expect("frame should decode");
        assert_eq!(limited_raw.packet_type, game::ITEM_CHANGE_LIMITED_ATTRIBUTE);
        assert_eq!(limited_raw.payload, [9, 0, 1]);
    }

    #[test]
    fn event_guild_tamer_and_chip_notices_fill_expected_bodies() {
        let hot_time = HotTimeEventPacket { value: 11 }.encode();
        let hot_time_raw = PacketReader::from_frame(&hot_time).expect("frame should decode");
        assert_eq!(hot_time_raw.packet_type, game::EVENT_HOT_TIME);
        assert_eq!(hot_time_raw.payload, 11u32.to_le_bytes());

        let item_term = ItemTermPacket { term: 22 }.encode();
        let item_term_raw = PacketReader::from_frame(&item_term).expect("frame should decode");
        assert_eq!(item_term_raw.packet_type, game::EVENT_ITEM_TERM);
        assert_eq!(item_term_raw.payload, 22u16.to_le_bytes());

        let gotcha = SingleValueNoticePacket { value: 33 }.encode_on(game::ITEM_GOTCHA);
        let gotcha_raw = PacketReader::from_frame(&gotcha).expect("frame should decode");
        assert_eq!(gotcha_raw.packet_type, game::ITEM_GOTCHA);
        assert_eq!(gotcha_raw.payload, 33u32.to_le_bytes());

        let inc_member = SingleValueNoticePacket { value: 44 }.encode_on(game::GUILD_INC_MEMBER);
        let inc_member_raw = PacketReader::from_frame(&inc_member).expect("frame should decode");
        assert_eq!(inc_member_raw.packet_type, game::GUILD_INC_MEMBER);
        assert_eq!(inc_member_raw.payload, 44u32.to_le_bytes());

        let option = TamerOptionPacket { target: 5, value: 6 }.encode();
        let option_raw = PacketReader::from_frame(&option).expect("frame should decode");
        assert_eq!(option_raw.packet_type, game::TAMER_OPTION);
        assert_eq!(option_raw.payload, [5, 0, 0, 0, 6, 0, 0, 0]);

        let chip = SkillResultChipPacket { handler: 7, skill_id: 8 }.encode();
        let chip_raw = PacketReader::from_frame(&chip).expect("frame should decode");
        assert_eq!(chip_raw.packet_type, game::SKILL_RESULT_CHIP);
        assert_eq!(chip_raw.payload, [7, 0, 0, 0, 8, 0, 0, 0]);
    }

    #[test]
    fn cross_slot_move_clear_trade_timeout_and_make_fill_expected_bodies() {
        let cross = CrossInvenSlotSizePacket { first: 1, second: 2, third: 3, fourth: 4 }.encode();
        let cross_raw = PacketReader::from_frame(&cross).expect("frame should decode");
        assert_eq!(cross_raw.packet_type, game::ITEM_CROSS_INVEN_SLOT_SIZE);
        assert_eq!(cross_raw.payload, [1, 0, 0, 0, 2, 0, 3, 0, 0, 0, 4, 0, 0, 0]);

        let clear = ItemMoveClearPacket { handler: 9, item_id: 10 }.encode();
        let clear_raw = PacketReader::from_frame(&clear).expect("frame should decode");
        assert_eq!(clear_raw.packet_type, game::ITEM_MOVE_CLEAR);
        assert_eq!(clear_raw.payload, [9, 0, 0, 0, 10, 0, 0, 0]);

        let timeout = CashTradeTimeoutPacket { trade: 11, first: 12, second: 13 }.encode();
        let timeout_raw = PacketReader::from_frame(&timeout).expect("frame should decode");
        assert_eq!(timeout_raw.packet_type, game::ITEM_CASH_TRADE_TIMEOUT);
        assert_eq!(timeout_raw.payload, [11, 0, 12, 0, 0, 0, 13, 0, 0, 0]);

        let make = ItemMakeResultPacket { kind: 2, slot: 5 }.encode();
        let make_raw = PacketReader::from_frame(&make).expect("frame should decode");
        assert_eq!(make_raw.packet_type, game::ITEM_MAKE_RESULT);
        assert_eq!(make_raw.payload, [2, 5]);
    }

    #[test]
    fn item_skill_batches_fill_expected_bodies_per_opcode() {
        let apply = ItemSkillBatchPacket { index: 3, skill_ids: vec![77] }
            .encode_on(game::TAMER_ITEM_SKILL_APPLY);
        let apply_raw = PacketReader::from_frame(&apply).expect("frame should decode");
        assert_eq!(apply_raw.packet_type, game::TAMER_ITEM_SKILL_APPLY);
        assert_eq!(apply_raw.payload, [3, 0, 0, 0, 77, 0, 0, 0]);

        let expire = ItemSkillBatchPacket {
            index: 0,
            skill_ids: vec![10, 20],
        }
        .encode_on(game::TAMER_ITEM_SKILL_EXPIRE);
        let expire_raw = PacketReader::from_frame(&expire).expect("frame should decode");
        assert_eq!(expire_raw.packet_type, game::TAMER_ITEM_SKILL_EXPIRE);
        assert_eq!(expire_raw.payload, [2, 10, 0, 0, 0, 20, 0, 0, 0]);

        let remove = ItemSkillBatchPacket {
            index: 0,
            skill_ids: vec![30],
        }
        .encode_on(game::TAMER_ITEM_SKILL_REMOVE);
        let remove_raw = PacketReader::from_frame(&remove).expect("frame should decode");
        assert_eq!(remove_raw.packet_type, game::TAMER_ITEM_SKILL_REMOVE);
        assert_eq!(remove_raw.payload, [1, 30, 0, 0, 0]);
    }

    #[test]
    fn pinochi_vote_and_store_summon_fill_expected_bodies() {
        let vote = PinochiVotePacket {
            entries: vec![PinochiVoteRow { slot: 3, value: 5 }],
        }
        .encode();
        let vote_raw = PacketReader::from_frame(&vote).expect("frame should decode");
        assert_eq!(vote_raw.packet_type, game::EVENT_PINOCHI_VOTE);
        assert_eq!(vote_raw.payload, [1, 0, 3, 0, 5, 0, 0, 0]);

        let summon = StoreSummonUsePacket {
            items: vec![StoreSummonRow { first: 1, second: 2 }],
        }
        .encode();
        let summon_raw = PacketReader::from_frame(&summon).expect("frame should decode");
        assert_eq!(summon_raw.packet_type, game::ITEM_STORE_SUMMON_USE);
        assert_eq!(summon_raw.payload, [1, 1, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn skill_error_writes_conditional_dead_tail() {
        let plain = SkillErrorPacket { kind: 95, code: 0 }.encode();
        let plain_raw = PacketReader::from_frame(&plain).expect("frame should decode");
        assert_eq!(plain_raw.packet_type, game::SKILL_ERROR);
        assert_eq!(plain_raw.payload, [95, 0, 0]);

        let dead = SkillErrorPacket { kind: 255, code: 7 }.encode();
        let dead_raw = PacketReader::from_frame(&dead).expect("frame should decode");
        assert_eq!(dead_raw.packet_type, game::SKILL_ERROR);
        assert_eq!(dead_raw.payload, [255, 7, 0, 0, 0]);
    }

    #[test]
    fn event_active_writes_value_flags_and_rows() {
        let mut row = [0u8; 71];
        for (index, byte) in row.iter_mut().enumerate() {
            *byte = (index + 1) as u8;
        }
        let packet = EventActivePacket {
            value: 250,
            flags: [1, 2, 3, 4],
            rows: vec![row],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::EVENT_ACTIVE);
        assert_eq!(raw.payload.len(), 4 + 4 + 1 + 71);
        assert_eq!(&raw.payload[..4], &250u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &[1, 2, 3, 4]);
        assert_eq!(raw.payload[8], 1);
        assert_eq!(&raw.payload[9..80], &row);
    }

    #[test]
    fn skill_success_and_partner_scale_fill_expected_bodies() {
        let success = TamerSkillSuccessPacket { index: 3, arg: 9 }.encode();
        let success_raw = PacketReader::from_frame(&success).expect("frame should decode");
        assert_eq!(success_raw.packet_type, game::TAMER_SKILL_SUCCESS);
        assert_eq!(success_raw.payload, [3, 0, 0, 0, 9, 0, 0, 0]);

        let scale = PartnerScalePacket { handler: 5, scale: 120, arg: 7 }.encode();
        let scale_raw = PacketReader::from_frame(&scale).expect("frame should decode");
        assert_eq!(scale_raw.packet_type, game::MASTER_PARTNER_SCALE);
        assert_eq!(scale_raw.payload, [5, 0, 0, 0, 120, 0, 7, 0, 0, 0]);
    }

    #[test]
    fn master_debug_notices_fill_expected_bodies() {
        let conn = MasterConnPacket { count: 42 }.encode();
        let conn_raw = PacketReader::from_frame(&conn).expect("frame should decode");
        assert_eq!(conn_raw.packet_type, game::MASTER_CONN_COUNT);
        assert_eq!(conn_raw.payload, 42u32.to_le_bytes());

        let pay = MasterPayCashPacket { silk: 5, cash: 9 }.encode();
        let pay_raw = PacketReader::from_frame(&pay).expect("frame should decode");
        assert_eq!(pay_raw.packet_type, game::MASTER_PAY_CASH);
        assert_eq!(
            pay_raw.payload,
            5i64.to_le_bytes().iter().chain(&9i64.to_le_bytes()).copied().collect::<Vec<u8>>()
        );

        let quest = GmQuestPacket { blob: vec![7u8; 0x598] }.encode();
        let quest_raw = PacketReader::from_frame(&quest).expect("frame should decode");
        assert_eq!(quest_raw.packet_type, game::MASTER_GM_QUEST);
        assert_eq!(quest_raw.payload.len(), 0x598);
        assert!(quest_raw.payload.iter().all(|byte| *byte == 7));
    }

    #[test]
    fn guild_name_notices_fill_expected_bodies() {
        let member = GuildMemberLevelUpPacket {
            reserved: 0,
            name: "Tamer".to_string(),
        }
        .encode();
        let member_raw = PacketReader::from_frame(&member).expect("frame should decode");
        assert_eq!(member_raw.packet_type, game::GUILD_MEMBER_LEVEL_UP);
        assert_eq!(member_raw.payload, [0, 5, 0, b'T', b'a', b'm', b'e', b'r']);

        let rename = GuildChangeTNamePacket {
            name: "Ann".to_string(),
            title: "Lord".to_string(),
        }
        .encode();
        let rename_raw = PacketReader::from_frame(&rename).expect("frame should decode");
        assert_eq!(rename_raw.packet_type, game::GUILD_CHANGE_TNAME);
        assert_eq!(
            rename_raw.payload[..7].to_vec(),
            [3, 0, b'A', b'n', b'n', 4, 0].to_vec()
        );

        let deleted = GuildDeletedTamerPacket { name: "Bo".to_string() }.encode();
        let deleted_raw = PacketReader::from_frame(&deleted).expect("frame should decode");
        assert_eq!(deleted_raw.packet_type, game::GUILD_DELETED_TAMER);
        assert_eq!(deleted_raw.payload, [2, 0, b'B', b'o']);
    }

    #[test]
    fn gm_hatch_try_writes_21_fixed_slots() {
        let packet = GmHatchTryPacket {
            succ_count: [1, 2, 3, 4, 5, 6],
            fail_count: [7, 8, 9, 10, 11],
            break_count: [12, 13, 14, 15, 16],
            succ_average: [1_024, 2_048, 3_072, 4_096, 5_120],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GM_HATCH_TRY);
        assert_eq!(raw.payload.len(), 21 * 4);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[80..84], &5_120u32.to_le_bytes());
    }

    #[test]
    fn pinochi_get_info_writes_eleven_fixed_slots() {
        let values: Vec<u32> = (0..11).map(|index| 100 + index as u32).collect();
        let mut values_array = [0u32; 11];
        values_array.copy_from_slice(&values);
        let packet = PinochiGetInfoPacket { values: values_array }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::EVENT_PINOCHI_GET_INFO);
        assert_eq!(raw.payload.len(), 44);
        assert_eq!(&raw.payload[..4], &100u32.to_le_bytes());
        assert_eq!(&raw.payload[40..44], &110u32.to_le_bytes());
    }

    #[test]
    fn party_exp_writes_numeric_block_plus_label() {
        let packet = PartyExpPacket {
            first: 10,
            second: 20,
            third: 30,
            first_lo: 4,
            fifth: 50,
            sixth: 60,
            seventh: 70,
            second_lo: 8,
            label: "exp".to_string(),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_EXP);
        assert_eq!(raw.payload.len(), 56 + 2 + 3);
        assert_eq!(&raw.payload[..8], &10i64.to_le_bytes());
        assert_eq!(&raw.payload[24..28], &4u32.to_le_bytes());
        assert_eq!(&raw.payload[52..56], &8u32.to_le_bytes());
        assert_eq!(&raw.payload[56..58], &3u16.to_le_bytes());
        assert_eq!(&raw.payload[58..], b"exp");
    }

    #[test]
    fn tamer_equip_item_writes_expected_body() {
        let packet = TamerEquipItemPacket {
            handler: 12,
            kind: 5,
            first: 1,
            second: 2,
            third: 3,
            filler: 0,
            tail: 4,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::TAMER_EQUIP_ITEM);
        assert_eq!(
            raw.payload,
            [12, 0, 5, 1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 0, 4]
        );
    }

    #[test]
    fn master_user_info_not_found_writes_name() {
        let packet = MasterUserInfoNotFoundPacket { name: "Zed".to_string() }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_USER_INFO_NOT_FOUND);
        assert_eq!(raw.payload, [3, 0, b'Z', b'e', b'd']);
    }

    #[test]
    fn master_init_2nd_pass_writes_code() {
        let packet = MasterInit2ndPassPacket { code: 1 }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_INIT_2ND_PASS);
        assert_eq!(raw.payload, [1, 0]);
    }

    #[test]
    fn master_lie_kill_monster_writes_counted_money_rows() {
        let packet = MasterLieKillMonsterPacket { entries: vec![7, 11] }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_LIE_KILL_MONSTER);
        assert_eq!(raw.payload.len(), 4 + 16);
        assert_eq!(&raw.payload[..4], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[4..12], &7i64.to_le_bytes());
        assert_eq!(&raw.payload[12..20], &11i64.to_le_bytes());
    }

    #[test]
    fn master_insert_item_to_shop_writes_code() {
        let packet = MasterInsertItemToShopPacket { code: 2 }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_INSERT_ITEM_TO_SHOP);
        assert_eq!(raw.payload, [2, 0]);
    }

    #[test]
    fn master_cheat_level_writes_result_and_name() {
        let packet = MasterCheatLevelPacket {
            result: 2,
            name: "Rex".to_string(),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_CHEAT_LEVEL);
        assert_eq!(raw.payload.len(), 4 + 2 + 3);
        assert_eq!(&raw.payload[..4], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[4..], [3, 0, b'R', b'e', b'x']);
    }

    #[test]
    fn gm_cool_reset_writes_102_fixed_slots() {
        let mut entries = [0u32; 102];
        entries[0] = 5;
        entries[101] = 9;
        let packet = GmCoolResetPacket { entries }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_COOL_RESET);
        assert_eq!(raw.payload.len(), 102 * 4);
        assert_eq!(&raw.payload[..4], &5u32.to_le_bytes());
        assert_eq!(&raw.payload[404..408], &9u32.to_le_bytes());
    }

    #[test]
    fn master_chat_ban_user_writes_duration_and_name() {
        let packet = MasterChatBanUserPacket {
            duration: 7200,
            name: "Zed".to_string(),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_CHAT_BAN_USER);
        assert_eq!(raw.payload.len(), 4 + 2 + 3);
        assert_eq!(&raw.payload[..4], &7200u32.to_le_bytes());
        assert_eq!(&raw.payload[4..], [3, 0, b'Z', b'e', b'd']);
    }

    #[test]
    fn master_init_skill_stat_writes_kind_and_names() {
        let packet = MasterInitSkillStatPacket {
            kind: 3,
            tamer: "Ann".to_string(),
            digimon: "Greymon".to_string(),
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_INIT_SKILL_STAT);
        assert_eq!(raw.payload.len(), 4 + 5 + 9);
        assert_eq!(&raw.payload[..4], &3u32.to_le_bytes());
        assert_eq!(raw.payload[4..9], [3, 0, b'A', b'n', b'n']);
        assert_eq!(raw.payload[9..11], [7, 0]); // "Greymon" length
    }

    #[test]
    fn master_user_info_writes_strings_and_numeric_block() {
        let packet = MasterUserInfoPacket {
            first: "A".to_string(),
            second: String::new(),
            first_flag: 1,
            class: 2,
            head: 3,
            value: 4,
            tail: 5,
            trailing: 6,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_USER_INFO);
        assert_eq!(raw.payload.len(), 3 + 2 + 4 + 2 + 1 + 8 + 4 + 1);
        assert_eq!(&raw.payload[0..2], &1u16.to_le_bytes());
        assert_eq!(raw.payload[3..5], [0, 0]); // empty second string
        assert_eq!(&raw.payload[5..9], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[9..11], &2u16.to_le_bytes());
        assert_eq!(raw.payload[11], 3);
        assert_eq!(&raw.payload[12..20], &4i64.to_le_bytes());
        assert_eq!(&raw.payload[20..24], &5u32.to_le_bytes());
        assert_eq!(raw.payload[24], 6);
    }

    #[test]
    fn master_machine_run_writes_counted_codes() {
        let packet = MasterMachineRunPacket {
            codes: vec![165001, 165002],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_MACHINE_RUN);
        assert_eq!(raw.payload.len(), 4 + 4 + 4);
        assert_eq!(&raw.payload[..4], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &165001u32.to_le_bytes());
        assert_eq!(&raw.payload[8..12], &165002u32.to_le_bytes());
    }

    #[test]
    fn master_rank_item_run_writes_selector_count_and_codes() {
        let packet = MasterRankItemRunPacket {
            selector: 2,
            codes: vec![165001],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_RANK_ITEM_RUN);
        assert_eq!(raw.payload.len(), 2 + 4 + 4);
        assert_eq!(&raw.payload[..2], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[2..6], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[6..10], &165001u32.to_le_bytes());
    }

    #[test]
    fn master_rank_item_run_zero_selector_ends_the_frame() {
        let packet = MasterRankItemRunPacket {
            selector: 0,
            codes: vec![165001],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.payload, 0u16.to_le_bytes());
    }

    #[test]
    fn skill_range_persist_end_writes_id() {
        let packet = SkillRangePersistEndPacket { id: 7 }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_RANGE_PERSIST_END);
        assert_eq!(raw.payload, 7u32.to_le_bytes());
    }

    #[test]
    fn skill_chip_cool_time_writes_two_slots() {
        let packet = SkillChipCoolTimePacket {
            tamer_slot: 11,
            partner_slot: 22,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_CHIP_COOL_TIME);
        assert_eq!(raw.payload.len(), 8);
        assert_eq!(&raw.payload[..4], &11u32.to_le_bytes());
        assert_eq!(&raw.payload[4..], &22u32.to_le_bytes());
    }

    #[test]
    fn skill_remove_chip_writes_chip_id() {
        let packet = SkillRemoveChipPacket { chip_id: 3102 }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_REMOVE_SKILL_CHIP);
        assert_eq!(raw.payload, 3102u32.to_le_bytes());
    }

    #[test]
    fn skill_add_chip_writes_slot_chip_and_item() {
        let packet = SkillAddChipPacket {
            entity_slot: 4,
            chip_id: 3102,
            item_id: 5000,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_ADD_SKILL_CHIP);
        assert_eq!(raw.payload.len(), 12);
        assert_eq!(&raw.payload[0..4], &4u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &3102u32.to_le_bytes());
        assert_eq!(&raw.payload[8..], &5000u32.to_le_bytes());
    }

    #[test]
    fn skill_active_effect_writes_slot_skill_and_level() {
        let packet = SkillActiveEffectPacket {
            slot: 1,
            skill_id: 200,
            level: 7,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_ACTIVE_SKILL_EFF);
        assert_eq!(raw.payload.len(), 10);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &200u32.to_le_bytes());
        assert_eq!(&raw.payload[8..], &7u16.to_le_bytes());
    }

    #[test]
    fn skill_monster_buff_writes_slot_key_and_param() {
        let packet = SkillMonsterBuffPacket {
            slot: 3,
            monster_key: 1234,
            param: 5,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_MONSTER_BUFF);
        assert_eq!(raw.payload.len(), 12);
        assert_eq!(&raw.payload[..4], &3u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &1234u32.to_le_bytes());
        assert_eq!(&raw.payload[8..], &5u32.to_le_bytes());
    }

    #[test]
    fn skill_monster_skill_effect_writes_caster_skill_and_targets() {
        let packet = SkillMonsterSkillEffectPacket {
            caster: 9,
            skill_id: 44,
            targets: vec![
                SkillMonsterSkillTarget {
                    handler: 1,
                    x: 10,
                    y: 20,
                },
                SkillMonsterSkillTarget {
                    handler: 2,
                    x: 30,
                    y: 40,
                },
            ],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_MONSTER_SKILL_EFFECT);
        assert_eq!(raw.payload.len(), 12 + 24);
        assert_eq!(&raw.payload[..4], &9u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &44u32.to_le_bytes());
        assert_eq!(&raw.payload[8..12], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[12..16], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[16..20], &10u32.to_le_bytes());
        assert_eq!(&raw.payload[20..24], &20u32.to_le_bytes());
        assert_eq!(&raw.payload[24..28], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[28..32], &30u32.to_le_bytes());
        assert_eq!(&raw.payload[32..36], &40u32.to_le_bytes());
    }

    #[test]
    fn skill_around_skill_writes_head_and_counted_hits() {
        let packet = SkillAroundSkillPacket {
            caster: 77,
            skill_id: 1200,
            attack_param: 5,
            hits: vec![SkillAroundHit {
                flag: 1,
                handler: 9,
                value: 88,
                level: 3,
            }],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_AROUND_SKILL);
        assert_eq!(raw.payload.len(), 10 + 4 + 10);
        assert_eq!(&raw.payload[..4], &77u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &1200u32.to_le_bytes());
        assert_eq!(&raw.payload[8..10], &1u16.to_le_bytes());
        assert_eq!(&raw.payload[10..14], &5u32.to_le_bytes());
        assert_eq!(raw.payload[14], 1);
        assert_eq!(&raw.payload[15..19], &9u32.to_le_bytes());
        assert_eq!(&raw.payload[19..23], &88u32.to_le_bytes());
        assert_eq!(raw.payload[23], 3);
    }

    #[test]
    fn skill_range_skill_dmg_writes_rows_and_lists() {
        let packet = SkillRangeSkillDmgPacket {
            caster: 1,
            flag: 1,
            primary_target: 2,
            attackers: vec![SkillRangeAttacker {
                actor: 3,
                pad: 0,
                blob: [9u8; 40],
                stats: vec![(4, 100)],
                stats_end: 0xFF,
            }],
            targets: vec![SkillRangeTarget { handler: 5, blob: [7u8; 40] }],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_RANGE_SKILL_DMG);
        // header 4+2+1+4 + attacker (4+1+40+1+4+1) + targets 2+4+40
        assert_eq!(raw.payload.len(), 11 + 51 + 46);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..6], &1u16.to_le_bytes());
        assert_eq!(raw.payload[6], 1);
        assert_eq!(&raw.payload[7..11], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[11..15], &3u32.to_le_bytes());
        assert_eq!(raw.payload[15], 0);
        assert_eq!(&raw.payload[16..56], &[9u8; 40]);
        assert_eq!(raw.payload[56], 4);
        assert_eq!(&raw.payload[57..61], &100u32.to_le_bytes());
        assert_eq!(raw.payload[61], 0xFF);
        assert_eq!(&raw.payload[62..64], &1u16.to_le_bytes());
        assert_eq!(&raw.payload[64..68], &5u32.to_le_bytes());
        assert_eq!(&raw.payload[68..], &[7u8; 40]);
    }

    #[test]
    fn master_conn_count_info_writes_counted_chains() {
        let packet = MasterConnCountInfoPacket {
            groups: vec![
                MasterConnCountGroup {
                    opens: 0,
                    entries: vec![(12, 1), (5, 0xFF)],
                },
                MasterConnCountGroup {
                    opens: 2,
                    entries: vec![],
                },
            ],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_CONN_COUNT_INFO);
        // group count 2 + group1 (1 + 2*3 + 1) + group2 (1 + 1)
        assert_eq!(raw.payload.len(), 2 + 8 + 2);
        assert_eq!(&raw.payload[..2], &2u16.to_le_bytes());
        assert_eq!(raw.payload[2], 0);
        assert_eq!(&raw.payload[3..5], &12u16.to_le_bytes());
        assert_eq!(raw.payload[5], 1);
        assert_eq!(&raw.payload[6..8], &5u16.to_le_bytes());
        assert_eq!(raw.payload[8], 0xFF);
        assert_eq!(raw.payload[9], 0xFF);
        assert_eq!(raw.payload[10], 2);
        assert_eq!(raw.payload[11], 0xFF);
    }

    #[test]
    fn item_capsule_to_item_writes_chained_rows() {
        let packet = ItemCapsuleToItemPacket {
            rows: vec![
                ItemCapsuleRow {
                    slot: 1,
                    code: 500,
                    param: 10,
                },
                ItemCapsuleRow {
                    slot: 2,
                    code: 800,
                    param: 20,
                },
            ],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ITEM_CAPSULE_TO_ITEM);
        // header 4 + row (1+2+4+4) + row (1+2+4+4)
        assert_eq!(raw.payload.len(), 4 + 11 + 11);
        assert_eq!(&raw.payload[..4], &2u32.to_le_bytes());
        assert_eq!(raw.payload[4], 1);
        assert_eq!(&raw.payload[5..7], &500u16.to_le_bytes());
        assert_eq!(&raw.payload[7..11], &10u32.to_le_bytes());
        assert_eq!(&raw.payload[11..15], &1u32.to_le_bytes());
        assert_eq!(raw.payload[15], 2);
        assert_eq!(&raw.payload[16..18], &800u16.to_le_bytes());
        assert_eq!(&raw.payload[18..22], &20u32.to_le_bytes());
        assert_eq!(&raw.payload[22..26], &0u32.to_le_bytes());
    }

    #[test]
    fn item_timeout_writes_kinds_code_and_key() {
        let packet = ItemTimeoutPacket {
            kind_cc: 1,
            code: 3000,
            row_key: 42,
            kind_c8: 2,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ITEM_TIMEOUT);
        assert_eq!(raw.payload.len(), 8);
        assert_eq!(raw.payload[0], 1);
        assert_eq!(&raw.payload[1..3], &3000u16.to_le_bytes());
        assert_eq!(&raw.payload[3..7], &42u32.to_le_bytes());
        assert_eq!(raw.payload[7], 2);
    }

    #[test]
    fn item_party_crop_writes_strings_and_member_list() {
        let packet = ItemPartyCropPacket {
            code: 7,
            param: 99,
            sub_out: 3,
            mode: 1,
            reason: "gone".into(),
            members: vec![ItemPartyCropMember {
                kind: 2,
                name: "Tamer".into(),
            }],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ITEM_PARTY_CROP);
        // 2+4+2+1 + reason (2+4) + count 1 + member (1 + 2 + 5)
        assert_eq!(raw.payload.len(), 9 + 6 + 1 + 8);
        assert_eq!(&raw.payload[..2], &7u16.to_le_bytes());
        assert_eq!(&raw.payload[2..6], &99u32.to_le_bytes());
        assert_eq!(&raw.payload[6..8], &3u16.to_le_bytes());
        assert_eq!(raw.payload[8], 1);
        assert_eq!(&raw.payload[9..11], &4u16.to_le_bytes());
        assert_eq!(&raw.payload[11..15], b"gone");
        assert_eq!(raw.payload[15], 1);
        assert_eq!(raw.payload[16], 2);
        assert_eq!(&raw.payload[17..19], &5u16.to_le_bytes());
        assert_eq!(&raw.payload[19..], b"Tamer");
    }

    #[test]
    fn skill_around_skill_ani_writes_flag_and_seconds() {
        let packet = SkillAroundSkillAniPacket {
            flag: 1,
            seconds: 30,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SKILL_AROUND_SKILL_ANI);
        assert_eq!(raw.payload.len(), 5);
        assert_eq!(raw.payload[0], 1);
        assert_eq!(&raw.payload[1..5], &30u32.to_le_bytes());
    }

    #[test]
    fn digimon_data_change_window_writes_tagged_body() {
        let packet = DigimonDataChangePacket {
            tag: 0,
            field_a: 1,
            field_b: 2,
            magic: 30803,
            payload: DigimonDataChangePayload::Window {
                c: 3,
                d: 400,
                e: 5,
                f: 600,
            },
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_DATA_CHANGE);
        assert_eq!(raw.payload.len(), 10 + 6);
        assert_eq!(&raw.payload[..4], &0u32.to_le_bytes());
        assert_eq!(raw.payload[4], 1);
        assert_eq!(raw.payload[5], 2);
        assert_eq!(&raw.payload[6..10], &30803u32.to_le_bytes());
        assert_eq!(raw.payload[10], 3);
        assert_eq!(&raw.payload[11..13], &400u16.to_le_bytes());
        assert_eq!(raw.payload[13], 5);
        assert_eq!(&raw.payload[14..16], &600u16.to_le_bytes());
    }

    #[test]
    fn digimon_data_change_rows_interleaves_u2_and_records() {
        let record = |w: u16| DigimonDataChangeRecord14 {
            qword: w as u64 * 11,
            dword: w as u32 * 13,
            word: w,
        };
        let packet = DigimonDataChangePacket {
            tag: 1,
            field_a: 0,
            field_b: 0,
            magic: 30803,
            payload: DigimonDataChangePayload::Rows {
                g: 7,
                first: record(1),
                second: record(2),
                h: 8,
                third: record(3),
                fourth: record(4),
            },
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_DATA_CHANGE);
        // tag 4 + a/b 2 + magic 4 + u2 + 14*2 + u2 + 14*2
        assert_eq!(raw.payload.len(), 10 + 2 + 28 + 2 + 28);
        assert_eq!(&raw.payload[10..12], &7u16.to_le_bytes());
        // first record starts at 12: qword 11, dword 13, word 1
        assert_eq!(&raw.payload[12..20], &11u64.to_le_bytes());
        assert_eq!(&raw.payload[20..24], &13u32.to_le_bytes());
        assert_eq!(&raw.payload[24..26], &1u16.to_le_bytes());
        // second record at 26: qword 22
        assert_eq!(&raw.payload[26..34], &22u64.to_le_bytes());
        // h at 40
        assert_eq!(&raw.payload[40..42], &8u16.to_le_bytes());
        // third record complements word 3, fourth word 4
        assert_eq!(&raw.payload[54..56], &3u16.to_le_bytes());
        assert_eq!(&raw.payload[68..70], &4u16.to_le_bytes());
    }

    #[test]
    fn digimon_data_change_blobs_writes_counted_28_byte_rows() {
        let row = |b: u8| {
            let mut record = [0u8; 28];
            record[0] = b;
            record
        };
        let packet = DigimonDataChangePacket {
            tag: 2,
            field_a: 0,
            field_b: 0,
            magic: 30803,
            payload: DigimonDataChangePayload::Blobs {
                first: vec![row(1), row(2)],
                second: vec![row(3)],
            },
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_DATA_CHANGE);
        // tag 4 + a/b 2 + magic 4 + (1 + 56) + (1 + 28)
        assert_eq!(raw.payload.len(), 10 + 57 + 29);
        assert_eq!(&raw.payload[..4], &2u32.to_le_bytes());
        assert_eq!(raw.payload[10], 2);
        assert_eq!(&raw.payload[11..39], &row(1));
        assert_eq!(&raw.payload[39..67], &row(2));
        assert_eq!(raw.payload[67], 1);
        assert_eq!(&raw.payload[68..], &row(3));
    }

    #[test]
    fn game_macro_question_writes_prompt_answers_and_tail() {
        let packet = GameMacroQuestionPacket {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
            prompt: "who?".into(),
            answers: vec![GameMacroQuestionAnswer {
                id: 9,
                text: "me".into(),
            }],
            tail: [5, 6, 7, 8],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAME_MACRO_QUESTION);
        // 2+2+1+4 + prompt (2+4) + count 2 + answer (4 + 2+2) + tail 16
        assert_eq!(raw.payload.len(), 9 + 6 + 2 + 8 + 16);
        assert_eq!(&raw.payload[..2], &1u16.to_le_bytes());
        assert_eq!(&raw.payload[2..4], &2u16.to_le_bytes());
        assert_eq!(raw.payload[4], 3);
        assert_eq!(&raw.payload[5..9], &4u32.to_le_bytes());
        assert_eq!(&raw.payload[9..11], &4u16.to_le_bytes());
        assert_eq!(&raw.payload[11..15], b"who?");
        assert_eq!(&raw.payload[15..17], &1u16.to_le_bytes());
        assert_eq!(&raw.payload[17..21], &9u32.to_le_bytes());
        assert_eq!(&raw.payload[21..23], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[23..25], b"me");
        assert_eq!(&raw.payload[25..29], &5u32.to_le_bytes());
        assert_eq!(&raw.payload[29..33], &6u32.to_le_bytes());
        assert_eq!(&raw.payload[33..37], &7u32.to_le_bytes());
        assert_eq!(&raw.payload[37..41], &8u32.to_le_bytes());
    }

    #[test]
    fn master_set_money_writes_u64_amount() {
        let packet = MasterSetMoneyPacket { money: 123_456_789 }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_SET_MONEY);
        assert_eq!(raw.payload, 123_456_789u64.to_le_bytes());
    }

    #[test]
    fn master_gm_guild_skill_reset_writes_empty_payload() {
        let packet = MasterGmGuildSkillResetPacket.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_GM_GUILD_SKILL_RESET);
        assert!(raw.payload.is_empty());
    }

    #[test]
    fn event_masters_matching_writes_empty_payload() {
        let packet = EventMastersMatchingPacket.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::EVENT_MASTERS_MATCHING);
        assert!(raw.payload.is_empty());
    }

    #[test]
    fn buff_family_writes_fixed_envelopes() {
        let create = BuffCreatePacket {
            target: 11,
            a: 1,
            b: 2,
            c: 3,
            d: 4,
        }
        .encode();
        let raw = PacketReader::from_frame(&create).expect("frame should decode");
        assert_eq!(raw.packet_type, game::BUFF_CREATE);
        assert_eq!(raw.payload.len(), 16);
        assert_eq!(&raw.payload[..4], &11u32.to_le_bytes());
        assert_eq!(&raw.payload[4..6], &1u16.to_le_bytes());
        assert_eq!(&raw.payload[6..8], &2u16.to_le_bytes());

        let change = BuffChangePacket {
            target: 12,
            a: 1,
            b: 2,
            c: 3,
            d: 4,
            e: 5,
        }
        .encode();
        let raw = PacketReader::from_frame(&change).expect("frame should decode");
        assert_eq!(raw.packet_type, game::BUFF_BUFF_CHANGE);
        assert_eq!(raw.payload.len(), 18);
        assert_eq!(raw.payload[4], 1);
        assert_eq!(raw.payload[6], 2);
        assert_eq!(raw.payload[8], 3);

        let clear = BuffClearPacket {
            target: 13,
            buffs: vec![1, 2, 3],
        }
        .encode();
        let raw = PacketReader::from_frame(&clear).expect("frame should decode");
        assert_eq!(raw.packet_type, game::BUFF_BUFF_CLEAR);
        assert_eq!(raw.payload, {
            let mut bytes = 13u32.to_le_bytes().to_vec();
            bytes.extend(3u16.to_le_bytes());
            bytes.extend(1u16.to_le_bytes());
            bytes.extend(2u16.to_le_bytes());
            bytes.extend(3u16.to_le_bytes());
            bytes
        });

        let fail = BuffReturnFailPacket {
            kind: 1,
            code: 7,
            upgrade: Some(42),
        }
        .encode();
        let raw = PacketReader::from_frame(&fail).expect("frame should decode");
        assert_eq!(raw.packet_type, game::BUFF_RETURN_FAIL);
        assert_eq!(raw.payload.len(), 7);
        assert_eq!(raw.payload[0], 1);
        assert_eq!(&raw.payload[1..3], &7u16.to_le_bytes());
        assert_eq!(&raw.payload[3..7], &42u32.to_le_bytes());

        let damage = BuffAbnormalDamagePacket {
            handler: 1,
            actor: 2,
            target: 3,
            flag: 4,
            pair: [5, 6],
            kind: 7,
        }
        .encode();
        let raw = PacketReader::from_frame(&damage).expect("frame should decode");
        assert_eq!(raw.packet_type, game::BUFF_ABNORMAL_DAMAGE);
        assert_eq!(raw.payload.len(), 20);
        assert_eq!(&raw.payload[8..10], &3u16.to_le_bytes());
        assert_eq!(raw.payload[10], 4);
        assert_eq!(&raw.payload[11..15], &5u32.to_le_bytes());
        assert_eq!(&raw.payload[15..19], &6u32.to_le_bytes());
        assert_eq!(raw.payload[19], 7);

        let skill = BuffBuffSkillPacket {
            target: 1,
            slot: 2,
            flag: 3,
            a: 4,
            b: 5,
        }
        .encode();
        let raw = PacketReader::from_frame(&skill).expect("frame should decode");
        assert_eq!(raw.packet_type, game::BUFF_BUFF_SKILL);
        assert_eq!(raw.payload.len(), 15);
        assert_eq!(&raw.payload[4..6], &2u16.to_le_bytes());
        assert_eq!(raw.payload[6], 3);

        let stun = BuffStunPacket {
            actor: 1,
            skill: 2,
            duration: 3,
            value: 4,
        }
        .encode();
        let raw = PacketReader::from_frame(&stun).expect("frame should decode");
        assert_eq!(raw.packet_type, game::BUFF_STUN);
        assert_eq!(raw.payload.len(), 14);
        assert_eq!(&raw.payload[8..10], &3u16.to_le_bytes());
        assert_eq!(&raw.payload[10..14], &4u32.to_le_bytes());
    }

    #[test]
    fn cash_shop_family_writes_results() {
        let buy = CashShopBuyRequestPacket { a: 1, b: 2 }.encode();
        let raw = PacketReader::from_frame(&buy).expect("frame should decode");
        assert_eq!(raw.packet_type, game::CASH_SHOP_BUY_REQUEST);
        assert_eq!(raw.payload, [1u32, 2]
            .iter()
            .flat_map(|v| v.to_le_bytes())
            .collect::<Vec<u8>>());

        let gift = CashShopGiftPacket {
            a: 1,
            b: 2,
            c: 3,
            name: "tamer".into(),
            d: 4,
        }
        .encode();
        let raw = PacketReader::from_frame(&gift).expect("frame should decode");
        assert_eq!(raw.packet_type, game::CASH_SHOP_GIFT);
        // u2 + u4 + u4 + name (u16 len + 5 bytes) + u4
        assert_eq!(raw.payload.len(), 21);
        assert_eq!(&raw.payload[..2], &1u16.to_le_bytes());
        assert_eq!(&raw.payload[2..6], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[6..10], &3u32.to_le_bytes());
        assert_eq!(&raw.payload[10..12], &5u16.to_le_bytes());
        assert_eq!(&raw.payload[12..17], b"tamer");
        assert_eq!(&raw.payload[17..21], &4u32.to_le_bytes());

        let cart = CashShopCartPacket {
            blob: {
                let mut blob = [0u8; 48];
                for (i, cell) in blob.iter_mut().enumerate() {
                    *cell = i as u8;
                }
                blob
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&cart).expect("frame should decode");
        assert_eq!(raw.packet_type, game::CASH_SHOP_CART);
        assert_eq!(raw.payload.len(), 48);
        assert_eq!(raw.payload[0], 0);
        assert_eq!(raw.payload[47], 47);
    }

    #[test]
    fn queue_sweep_writes_small_envelopes() {
        let cancel = QuestCanceledPacket {
            rows: vec![
                QuestCancelRow {
                    kind: 2,
                    slot: 1,
                    a: 3,
                    b: 4,
                },
                QuestCancelRow {
                    kind: 7,
                    slot: 5,
                    a: 6,
                    b: 7,
                },
            ],
        }
        .encode();
        let raw = PacketReader::from_frame(&cancel).expect("frame should decode");
        assert_eq!(raw.packet_type, game::QUEST_CANCELED);
        assert_eq!(raw.payload.len(), 2 + 2 * 14);
        assert_eq!(&raw.payload[..2], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[2..6], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[6..8], &1u16.to_le_bytes());
        assert_eq!(&raw.payload[8..12], &3u32.to_le_bytes());
        assert_eq!(&raw.payload[12..16], &4u32.to_le_bytes());
        assert_eq!(&raw.payload[16..20], &7u32.to_le_bytes());

        let fail = InsMapDungeonFailPacket { skipped: 9 }.encode();
        let raw = PacketReader::from_frame(&fail).expect("frame should decode");
        assert_eq!(raw.packet_type, game::INSMAP_DUNGEON_FAIL);
        assert_eq!(raw.payload, 9u32.to_le_bytes());

        let portal = SvrSelectPortalFailurePacket { skipped: 4 }.encode();
        let raw = PacketReader::from_frame(&portal).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SVR_SELECT_PORTAL_FAILURE);
        assert_eq!(raw.payload, 4u32.to_le_bytes());

        let exp = MonsterNatualExpPacket { a: 1, b: 2, c: 300 }.encode();
        let raw = PacketReader::from_frame(&exp).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MONSTER_NATUAL_EXP);
        assert_eq!(raw.payload.len(), 4);
        assert_eq!(raw.payload[0], 1);
        assert_eq!(raw.payload[1], 2);
        assert_eq!(&raw.payload[2..4], &300u16.to_le_bytes());
    }

    #[test]
    fn event_family_writes_hot_time_and_item_type() {
        let hot = EventHotTimePacket {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
            e: 5,
            f: 6,
        }
        .encode();
        let raw = PacketReader::from_frame(&hot).expect("frame should decode");
        assert_eq!(raw.packet_type, game::EVENT_HOT_TIME_EVENT);
        // four u1 bytes + two u4 words
        assert_eq!(raw.payload.len(), 12);
        assert_eq!(raw.payload[0], 1);
        assert_eq!(raw.payload[1], 2);
        assert_eq!(raw.payload[2], 3);
        assert_eq!(raw.payload[3], 4);
        assert_eq!(&raw.payload[4..8], &5u32.to_le_bytes());
        assert_eq!(&raw.payload[8..12], &6u32.to_le_bytes());

        let item = EventItemTypePacket {
            name: "tree".to_string(),
            a: 7,
            b: 8,
        }
        .encode();
        let raw = PacketReader::from_frame(&item).expect("frame should decode");
        assert_eq!(raw.packet_type, game::EVENT_ITEM_TYPE);
        // name (u16 len + 4 bytes) + u4 + u2
        assert_eq!(raw.payload.len(), 12);
        assert_eq!(&raw.payload[..2], &4u16.to_le_bytes());
        assert_eq!(&raw.payload[2..6], b"tree");
        assert_eq!(&raw.payload[6..10], &7u32.to_le_bytes());
        assert_eq!(&raw.payload[10..12], &8u16.to_le_bytes());
    }

    #[test]
    fn raid_ranking_list_writes_counted_name_rows() {
        let packet = MsgRaidRankingListPacket {
            rows: vec![
                MsgRaidRankingRow {
                    a: 1,
                    name1: "Ann".to_string(),
                    name2: "Bo".to_string(),
                    b: 2,
                    c: 3,
                },
                MsgRaidRankingRow {
                    a: 4,
                    name1: String::new(),
                    name2: "Cy".to_string(),
                    b: 5,
                    c: 6,
                },
            ],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MONSTER_MSG_RAID_RANKING_LIST);
        // count + row1 (4 + 2+3 + 2+2 + 4 + 4) + row2 (4 + 2 + 2+2 + 4 + 4)
        assert_eq!(raw.payload.len(), 4 + 21 + 18);
        assert_eq!(&raw.payload[..4], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[8..13], [3, 0, b'A', b'n', b'n']);
        assert_eq!(&raw.payload[13..15], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[15..17], b"Bo");
        assert_eq!(&raw.payload[17..21], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[21..25], &3u32.to_le_bytes());
        assert_eq!(&raw.payload[25..29], &4u32.to_le_bytes());
    }

    #[test]
    fn arena_live_board_writes_self_header_then_rows() {
        let packet = ArenaRankingInfoPacket {
            ranking_type: 1,
            result: 0,
            status: 3,
            point: 0x1122_3344,
            rank: 7,
            position_delta: 2,
            rank_position_kind: 0,
            rank_percent: 40,
            renew_sec: 60,
            end_sec: 120,
            entries: vec![ArenaRankRow {
                rank: 1,
                name: "Ann".to_string(),
                guild: "Bo".to_string(),
                points: 900,
                is_new: true,
                position_delta: -4,
            }],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ARENA_REQUEST_RANK);
        assert_eq!(raw.payload.len(), 42);
        assert_eq!(raw.payload[0], 1);
        assert_eq!(raw.payload[1], 0);
        assert_eq!(raw.payload[2], 3);
        assert_eq!(&raw.payload[3..7], &0x1122_3344_u32.to_le_bytes());
        assert_eq!(&raw.payload[7..11], &7_u32.to_le_bytes());
        assert_eq!(&raw.payload[11..15], &2_u32.to_le_bytes());
        assert_eq!(raw.payload[15], 0);
        assert_eq!(raw.payload[16], 40);
        assert_eq!(&raw.payload[17..21], &60_u32.to_le_bytes());
        assert_eq!(&raw.payload[21..25], &120_u32.to_le_bytes());
        assert_eq!(raw.payload[25], 1);
        // Row: rank, name string, guild string, point, is_new, signed delta.
        assert_eq!(raw.payload[26], 1);
        assert_eq!(&raw.payload[27..29], &3u16.to_le_bytes());
        assert_eq!(&raw.payload[29..32], b"Ann");
        assert_eq!(&raw.payload[32..34], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[34..36], b"Bo");
        assert_eq!(&raw.payload[36..40], &900_u32.to_le_bytes());
        assert_eq!(raw.payload[40], 1);
        assert_eq!(raw.payload[41] as i8, -4);
    }

    #[test]
    fn arena_old_rank_writes_self_rank_then_archived_rows() {
        let packet = ArenaOldRankingInfoPacket {
            ranking_type: 2,
            result: 0,
            rank: 11,
            entries: vec![ArenaArchiveRankRow {
                rank: 1,
                name: "Ann".to_string(),
                guild: "Bo".to_string(),
                points: 900,
            }],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ARENA_REQUEST_OLD_RANK);
        assert_eq!(raw.payload.len(), 21);
        assert_eq!(raw.payload[0], 2);
        assert_eq!(raw.payload[1], 0);
        assert_eq!(&raw.payload[2..6], &11_u32.to_le_bytes());
        assert_eq!(raw.payload[6], 1);
        assert_eq!(raw.payload[7], 1);
        assert_eq!(&raw.payload[8..10], &3u16.to_le_bytes());
        assert_eq!(&raw.payload[10..13], b"Ann");
        assert_eq!(&raw.payload[13..15], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[15..17], b"Bo");
        assert_eq!(&raw.payload[17..21], &900_u32.to_le_bytes());
    }

    #[test]
    fn arena_fame_writes_echoed_season_then_rows() {
        let packet = ArenaFameInfoPacket {
            season: 5,
            result: 0,
            entries: vec![ArenaArchiveRankRow {
                rank: 1,
                name: "Ann".to_string(),
                guild: "Bo".to_string(),
                points: 900,
            }],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ARENA_REQUEST_FAME);
        assert_eq!(raw.payload.len(), 18);
        assert_eq!(&raw.payload[..2], &5_u16.to_le_bytes());
        assert_eq!(raw.payload[2], 0);
        assert_eq!(raw.payload[3], 1);
        assert_eq!(raw.payload[4], 1);
        assert_eq!(&raw.payload[5..7], &3u16.to_le_bytes());
        assert_eq!(&raw.payload[7..10], b"Ann");
        assert_eq!(&raw.payload[10..12], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[12..14], b"Bo");
        assert_eq!(&raw.payload[14..18], &900_u32.to_le_bytes());
    }

    #[test]
    fn arena_fame_season_list_writes_u16_seasons() {
        let packet = ArenaFameSeasonListPacket {
            seasons: vec![3, 7],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ARENA_REQUEST_FAME_LIST);
        assert_eq!(raw.payload.len(), 6);
        assert_eq!(&raw.payload[..2], &2_u16.to_le_bytes());
        assert_eq!(&raw.payload[2..4], &3_u16.to_le_bytes());
        assert_eq!(&raw.payload[4..6], &7_u16.to_le_bytes());
    }

    #[test]
    fn chat_and_party_name_requests_decode_modern_text() {
        let mut chat = Vec::new();
        chat.extend_from_slice(&5_u16.to_le_bytes());
        chat.extend_from_slice(b"hello");
        assert_eq!(
            GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: game::CHAT_MESSAGE,
                payload: chat,
            })
            .expect("chat should parse"),
            GameRequest::ChatMessage {
                message: "hello".to_string()
            }
        );

        let mut name = Vec::new();
        name.extend_from_slice(&4_u16.to_le_bytes());
        name.extend_from_slice(b"Agun");
        for opcode in [game::PARTY_INVITE, game::PARTY_KICK] {
            let parsed = GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: opcode,
                payload: name.clone(),
            })
            .unwrap_or_else(|error| panic!("opcode {opcode} should parse: {error}"));
            match parsed {
                GameRequest::PartyInvite { target_name }
                | GameRequest::PartyKick { target_name } => assert_eq!(target_name, "Agun"),
                other => panic!("opcode {opcode} decoded as {other:?}"),
            }
        }

        let mut response = Vec::new();
        response.extend_from_slice(&1_i32.to_le_bytes());
        response.extend_from_slice(&4_u16.to_le_bytes());
        response.extend_from_slice(b"Agun");
        assert_eq!(
            GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: game::PARTY_INVITE_RESPONSE,
                payload: response,
            })
            .expect("invite response should parse"),
            GameRequest::PartyInviteResponse {
                result_type: 1,
                inviter_name: "Agun".to_string(),
            }
        );
    }

    #[test]
    fn message_family_requests_decode_modern_text() {
        // Whisper carries the target before the message, so a legacy-shaped body
        // desyncs the whole pair instead of only truncating one field.
        let mut whisper = Vec::new();
        whisper.extend_from_slice(&7_u16.to_le_bytes());
        whisper.extend_from_slice(b"Dracmon");
        whisper.extend_from_slice(&5_u16.to_le_bytes());
        whisper.extend_from_slice(b"hello");
        assert_eq!(
            GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: game::WHISPER_MESSAGE,
                payload: whisper,
            })
            .expect("whisper should parse"),
            GameRequest::WhisperMessage {
                target_name: "Dracmon".to_string(),
                message: "hello".to_string(),
            }
        );

        let mut message = Vec::new();
        message.extend_from_slice(&5_u16.to_le_bytes());
        message.extend_from_slice(b"hello");
        for opcode in [
            game::GUILD_MESSAGE,
            game::GUILD_NOTICE,
            game::PARTY_CHAT,
        ] {
            let request = GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: opcode,
                payload: message.clone(),
            })
            .unwrap_or_else(|error| panic!("opcode {opcode} should parse: {error}"));
            let text = match request {
                GameRequest::ShoutMessage { message }
                | GameRequest::GuildMessage { message }
                | GameRequest::GuildNotice { notice: message }
                | GameRequest::PartyChat { message } => message,
                other => panic!("opcode {opcode} decoded as {other:?}"),
            };
            assert_eq!(text, "hello");
        }
    }

    #[test]
    fn megaphone_request_carries_mode_item_and_slot() {
        // The shout request leads with the consumption mode and the megaphone item
        // code, so the text sits after eight scalar bytes rather than at the front.
        let mut payload = Vec::new();
        payload.push(0_u8);
        payload.extend_from_slice(&160_347_u32.to_le_bytes());
        payload.extend_from_slice(&5_u16.to_le_bytes());
        payload.extend_from_slice(b"hello");
        payload.extend_from_slice(&17_u16.to_le_bytes());

        let request = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::MEGAPHONE_MESSAGE,
            payload,
        })
        .unwrap_or_else(|error| panic!("megaphone should parse: {error}"));

        assert_eq!(
            request,
            GameRequest::MegaphoneMessage {
                mode: 0,
                item_code: 160_347,
                message: "hello".to_string(),
                item_slot: 17,
            }
        );

        // An entitlement shout owns no inventory slot and reports the sentinel.
        let mut entitlement_payload = vec![1_u8];
        entitlement_payload.extend_from_slice(&160_347_u32.to_le_bytes());
        entitlement_payload.extend_from_slice(&0_u16.to_le_bytes());
        entitlement_payload.extend_from_slice(&0xfffe_u16.to_le_bytes());
        let entitlement = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::MEGAPHONE_MESSAGE,
            payload: entitlement_payload,
        })
        .unwrap_or_else(|error| panic!("entitlement shout should parse: {error}"));

        assert_eq!(
            entitlement,
            GameRequest::MegaphoneMessage {
                mode: 1,
                item_code: 160_347,
                message: String::new(),
                item_slot: 0xfffe,
            }
        );
    }

    #[test]
    fn guild_member_name_requests_decode_modern_text() {
        for opcode in [
            game::GUILD_KICK,
            game::GUILD_AUTHORITY_MASTER,
            game::GUILD_AUTHORITY_SUBMASTER,
            game::GUILD_AUTHORITY_DATS,
            game::GUILD_AUTHORITY_MEMBER,
            game::GUILD_AUTHORITY_NEW_MEMBER,
        ] {
            let mut payload = Vec::new();
            payload.extend_from_slice(&7_u16.to_le_bytes());
            payload.extend_from_slice(b"Dracmon");

            let request = GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: opcode,
                payload,
            })
            .unwrap_or_else(|error| panic!("opcode {opcode} should parse: {error}"));

            let name = match request {
                GameRequest::GuildKick { target_name } => target_name,
                GameRequest::GuildAuthorityMaster { target_name }
                | GameRequest::GuildAuthoritySubMaster { target_name }
                | GameRequest::GuildAuthorityDats { target_name }
                | GameRequest::GuildAuthorityMember { target_name }
                | GameRequest::GuildAuthorityNewMember { target_name } => target_name,
                other => panic!("opcode {opcode} decoded as {other:?}"),
            };
            assert_eq!(name, "Dracmon");
        }
    }

    #[test]
    fn guild_join_requests_decode_modern_text_and_scalar_widths() {
        // The creation slot is a 16-bit field, so reading it as 32 bits would also
        // consume the first two bytes of the following npc id.
        let mut create = Vec::new();
        create.extend_from_slice(&5_u16.to_le_bytes());
        create.extend_from_slice(b"Trust");
        create.extend_from_slice(&12_u16.to_le_bytes());
        create.extend_from_slice(&4001_u32.to_le_bytes());
        assert_eq!(
            GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: game::GUILD_CREATE,
                payload: create,
            })
            .expect("guild create should parse"),
            GameRequest::GuildCreate {
                guild_name: "Trust".to_string(),
                inven_slot: 12,
                npc_id: 4001,
            }
        );

        let mut invite = Vec::new();
        invite.extend_from_slice(&7_u16.to_le_bytes());
        invite.extend_from_slice(b"Dracmon");
        assert_eq!(
            GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: game::GUILD_INVITE,
                payload: invite,
            })
            .expect("guild invite should parse"),
            GameRequest::GuildInvite {
                target_name: "Dracmon".to_string()
            }
        );

        // Allow and reject share `[u4 certified_code][text tamer_name]`.
        for opcode in [game::GUILD_INVITE_ACCEPT, game::GUILD_INVITE_DENY] {
            let mut payload = Vec::new();
            payload.extend_from_slice(&0x1234_u32.to_le_bytes());
            payload.extend_from_slice(&7_u16.to_le_bytes());
            payload.extend_from_slice(b"Dracmon");

            let request = GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: opcode,
                payload,
            })
            .unwrap_or_else(|error| panic!("opcode {opcode} should parse: {error}"));

            match request {
                GameRequest::GuildInviteAccept {
                    certified_code,
                    target_name,
                }
                | GameRequest::GuildInviteDeny {
                    certified_code,
                    target_name,
                } => {
                    assert_eq!(certified_code, 0x1234);
                    assert_eq!(target_name, "Dracmon");
                }
                other => panic!("opcode {opcode} decoded as {other:?}"),
            }
        }
    }

    #[test]
    fn guild_join_requests_reject_the_legacy_text_form() {
        // `[u1 len][bytes][0x00]` reads as an out-of-bound u16 count, so a stream that
        // was framed the old way fails loudly instead of decoding a truncated name.
        let mut payload = vec![5_u8];
        payload.extend_from_slice(b"Trust");
        payload.push(0);
        payload.extend_from_slice(&12_u16.to_le_bytes());
        payload.extend_from_slice(&4001_u32.to_le_bytes());

        let error = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::GUILD_CREATE,
            payload,
        })
        .expect_err("legacy framing must not decode");
        assert!(matches!(
            error,
            ProtocolError::InvalidTextLength(length) if length > 512
        ));
    }

    #[test]
    fn rename_and_buddy_requests_decode_modern_text() {
        // Both renames lead with the rename item's inventory slot, which has to be
        // consumed before the text or the name decodes from the wrong offset.
        for opcode in [game::TAMER_NAME_CHANGE, game::DIGIMON_CHANGE_NAME] {
            let mut payload = Vec::new();
            payload.extend_from_slice(&7_i32.to_le_bytes());
            payload.extend_from_slice(&6_u16.to_le_bytes());
            payload.extend_from_slice(b"Veemon");

            let request = GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: opcode,
                payload,
            })
            .unwrap_or_else(|error| panic!("opcode {opcode} should parse: {error}"));

            let name = match request {
                GameRequest::TamerNameChange {
                    inven_slot,
                    new_name,
                }
                | GameRequest::DigimonChangeName {
                    inven_slot,
                    new_name,
                } => {
                    assert_eq!(inven_slot, 7);
                    new_name
                }
                other => panic!("opcode {opcode} decoded as {other:?}"),
            };
            assert_eq!(name, "Veemon");
        }

        let mut buddy = Vec::new();
        buddy.extend_from_slice(&4_u16.to_le_bytes());
        buddy.extend_from_slice(b"Kari");
        assert_eq!(
            GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: game::ADD_FRIEND,
                payload: buddy,
            })
            .expect("buddy add should parse"),
            GameRequest::AddFriend {
                friend_name: "Kari".to_string()
            }
        );
    }

    #[test]
    fn buddy_delete_and_memo_follow_the_modern_text_form() {
        fn modern(text: &str) -> Vec<u8> {
            let mut out = Vec::new();
            out.extend_from_slice(&(text.len() as u16).to_le_bytes());
            out.extend_from_slice(text.as_bytes());
            out
        }
        fn decode(opcode: i16, payload: Vec<u8>) -> GameRequest {
            GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: opcode,
                payload,
            })
            .unwrap_or_else(|error| panic!("opcode {opcode} should parse: {error}"))
        }

        assert_eq!(
            decode(game::BUDDY_DELETE, modern("Kari")),
            GameRequest::DeleteFriend {
                friend_name: "Kari".to_string()
            }
        );
        assert_eq!(
            decode(game::BUDDY_MEMO, [modern("Kari"), modern("partner")].concat()),
            GameRequest::FriendMemo {
                tamer_name: "Kari".to_string(),
                memo: "partner".to_string()
            }
        );
        // The memo body carries two strings, so a lone name must not decode.
        assert!(GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::BUDDY_MEMO,
            payload: modern("Kari"),
        })
        .is_err());
    }

    #[test]
    fn guild_invite_failure_carries_only_the_inviter_name() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&3_u16.to_le_bytes());
        payload.extend_from_slice(b"Tai");

        assert_eq!(
            GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: game::GUILD_INVITE_FAIL,
                payload,
            })
            .expect("invite failure should parse"),
            GameRequest::GuildInviteFailure {
                inviter_name: "Tai".to_string()
            }
        );
    }

    #[test]
    fn tamer_summon_request_follows_its_case_byte() {
        fn decode(payload: &[u8]) -> Result<GameRequest, ProtocolError> {
            GameRequest::try_from(RawPacket {
                length: 0,
                packet_type: game::TAMER_SUMMON,
                payload: payload.to_vec(),
            })
        }

        // Case 1 is the only body carrying text: `[u1 1][text name][u4 skill index]`.
        let mut by_name = vec![1_u8];
        by_name.extend_from_slice(&5_u16.to_le_bytes());
        by_name.extend_from_slice(b"Ricky");
        by_name.extend_from_slice(&7_u32.to_le_bytes());
        assert_eq!(
            decode(&by_name).expect("summon by name should parse"),
            GameRequest::TamerSummon(TamerSummonRequest::ByName {
                target_name: "Ricky".to_string(),
                skill_index: 7,
            })
        );

        // Case 4 splits on a sub-mode: one byte of value, or one dword of it.
        assert_eq!(
            decode(&[4, 1, 2]).expect("state check should parse"),
            GameRequest::TamerSummon(TamerSummonRequest::StateCheck {
                submode: 1,
                value: 2,
            })
        );
        let mut dword_state = vec![4_u8, 2];
        dword_state.extend_from_slice(&300_u32.to_le_bytes());
        assert_eq!(
            decode(&dword_state).expect("state check should parse"),
            GameRequest::TamerSummon(TamerSummonRequest::StateCheck {
                submode: 2,
                value: 300,
            })
        );

        // Any other case is a two-byte accept frame.
        assert_eq!(
            decode(&[2, 5]).expect("accept should parse"),
            GameRequest::TamerSummon(TamerSummonRequest::Accept { case: 2, value: 5 })
        );

        // An unknown sub-mode under case 4 cannot be guessed: the remaining bytes would
        // be either one or four, so the request fails instead of desyncing.
        assert!(matches!(
            decode(&[4, 9, 1]),
            Err(ProtocolError::UnsupportedSummonCase(9))
        ));
    }

    #[test]
    fn sync_chat_records_write_one_typed_record() {
        fn payload_of(frame: &[u8]) -> Vec<u8> {
            PacketReader::from_frame(frame)
                .expect("record frame should frame")
                .payload
        }

        let chat = SyncChatRecord::Chat(vec![SyncChatLine {
            handler: 7,
            message: "hello".to_string(),
        }])
        .encode();
        let mut expected_chat = vec![7_u8, 1, 7, 0, 0, 0, 5, 0];
        expected_chat.extend_from_slice(b"hello");
        expected_chat.push(0);
        assert_eq!(payload_of(&chat), expected_chat);

        let whisper = SyncChatRecord::Whisper(vec![SyncWhisperLine {
            result: 0,
            sender: "Tai".to_string(),
            receiver: "Kari".to_string(),
            message: "hi".to_string(),
        }])
        .encode();
        let mut expected_whisper = vec![8_u8, 1, 0, 3, 0];
        expected_whisper.extend_from_slice(b"Tai");
        expected_whisper.extend_from_slice(&4_u16.to_le_bytes());
        expected_whisper.extend_from_slice(b"Kari");
        expected_whisper.extend_from_slice(&2_u16.to_le_bytes());
        expected_whisper.extend_from_slice(b"hi");
        expected_whisper.push(0);
        assert_eq!(payload_of(&whisper), expected_whisper);

        // A shout carries no handler: the client resolves the speaker from the name.
        let shout = SyncChatRecord::Shout(vec![SyncNamedLine {
            sender: "Matt".to_string(),
            message: "look".to_string(),
        }])
        .encode();
        let mut expected_shout = vec![11_u8, 1, 4, 0];
        expected_shout.extend_from_slice(b"Matt");
        expected_shout.extend_from_slice(&4_u16.to_le_bytes());
        expected_shout.extend_from_slice(b"look");
        expected_shout.push(0);
        assert_eq!(payload_of(&shout), expected_shout);
    }

    #[test]
    fn guild_set_title_decodes_modern_text_fields() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&2_i32.to_le_bytes());
        payload.extend_from_slice(&4_u16.to_le_bytes());
        payload.extend_from_slice(b"Lord");
        payload.extend_from_slice(&5_u16.to_le_bytes());
        payload.extend_from_slice(b"Leads");

        let request = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::GUILD_SET_TITLE,
            payload,
        })
        .expect("title request should parse");
        assert_eq!(
            request,
            GameRequest::GuildSetTitle {
                authority_class: 2,
                title: "Lord".to_string(),
                duty: "Leads".to_string(),
            }
        );
    }

    #[test]
    fn guild_set_title_rejects_text_above_the_client_bound() {
        let mut payload = Vec::new();
        payload.extend_from_slice(&0_i32.to_le_bytes());
        payload.extend_from_slice(&1000_u16.to_le_bytes());

        let error = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::GUILD_SET_TITLE,
            payload,
        })
        .expect_err("a length beyond the read bound must be rejected");
        assert!(matches!(error, ProtocolError::InvalidTextLength(1000)));
    }

    #[test]
    fn parse_arena_requests_covers_all_four_ids() {
        let live = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::ARENA_REQUEST_RANK,
            payload: vec![2],
        })
        .expect("live rank request should parse");
        assert_eq!(live, GameRequest::ArenaRequestRank { ranking_type: 2 });

        let old = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::ARENA_REQUEST_OLD_RANK,
            payload: vec![3],
        })
        .expect("old rank request should parse");
        assert_eq!(old, GameRequest::ArenaRequestOldRank { ranking_type: 3 });

        let fame = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::ARENA_REQUEST_FAME,
            payload: 5_u16.to_le_bytes().to_vec(),
        })
        .expect("fame request should parse");
        assert_eq!(fame, GameRequest::ArenaRequestFame { season: 5 });

        let list = GameRequest::try_from(RawPacket {
            length: 0,
            packet_type: game::ARENA_REQUEST_FAME_LIST,
            payload: Vec::new(),
        })
        .expect("season list request should parse");
        assert_eq!(list, GameRequest::ArenaRequestFameList);
    }

    #[test]
    fn game_two_family_writes_doors_guards_actions_and_guage() {
        let door = GameTwoDoorObjectOpenPacket {
            monster_id: 7,
            opened: 1,
        }
        .encode();
        let raw = PacketReader::from_frame(&door).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_DOOR_OBJECT_OPEN);
        assert_eq!(raw.payload.len(), 5);
        assert_eq!(&raw.payload[..4], &7u32.to_le_bytes());
        assert_eq!(raw.payload[4], 1);

        let guard = GameTwoGameGuardCheckPacket {
            a: 1,
            b: 2,
            rows: vec![
                GameGuardRow {
                    kind: 1,
                    c: 3,
                    d: 4,
                    e: 5,
                },
                GameGuardRow {
                    kind: 0,
                    c: 6,
                    d: 7,
                    e: 8,
                },
            ],
        }
        .encode();
        let raw = PacketReader::from_frame(&guard).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_GAME_GUARD_CHECK);
        assert_eq!(raw.payload.len(), 10 + 2 * 10);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[8..10], &2u16.to_le_bytes());
        // kind + c + d + e
        assert_eq!(&raw.payload[10..11], [1]);
        assert_eq!(&raw.payload[11..15], &3u32.to_le_bytes());
        assert_eq!(&raw.payload[15..19], &4u32.to_le_bytes());
        assert_eq!(raw.payload[19], 5);
        assert_eq!(raw.payload[20], 0);
        assert_eq!(&raw.payload[21..25], &6u32.to_le_bytes());

        let action = GameTwoSkillActionPacket { kind: 4 }.encode();
        let raw = PacketReader::from_frame(&action).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_SKILL_ACTION);
        assert_eq!(raw.payload, 4u32.to_le_bytes());

        let guage = GameTwoXGuageUpdatePacket {
            a: 1,
            name: "bar".to_string(),
            b: 2,
            c: 3,
        }
        .encode();
        let raw = PacketReader::from_frame(&guage).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_X_GUAGE_UPDATE);
        // u2 + name (u16 len + 3 bytes) + u4 + u4
        assert_eq!(raw.payload.len(), 2 + 5 + 8);
        assert_eq!(&raw.payload[..2], &1u16.to_le_bytes());
        assert_eq!(&raw.payload[2..4], &3u16.to_le_bytes());
        assert_eq!(&raw.payload[4..7], b"bar");
        assert_eq!(&raw.payload[7..11], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[11..15], &3u32.to_le_bytes());
    }

    #[test]
    fn time_charge_family_writes_charge_rows_and_slot_lists() {
        let immediate = TimeChargeImmediatePacket {
            a: 1,
            b: 2,
            c: 3,
            charge_row: {
                let mut blob = [0u8; 69];
                for (i, cell) in blob.iter_mut().enumerate() {
                    *cell = i as u8;
                }
                blob
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&immediate).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_TIME_CHARGE_IMMEDIATE);
        // u4 + u2 + u4 + 69-byte row
        assert_eq!(raw.payload.len(), 10 + 69);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..6], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[6..10], &3u32.to_le_bytes());
        assert_eq!(raw.payload[10], 0);
        assert_eq!(raw.payload[78], 68);

        let cancel = TimeChargeCancelPacket::default().encode();
        let raw = PacketReader::from_frame(&cancel).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_TIME_CHARGE_CANCEL);
        assert_eq!(raw.payload.len(), 10 + 69);
        assert!(raw.payload.iter().all(|byte| *byte == 0));

        let list = TimeChargeItemListPacket {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
            e: 5,
            charge_row: [7u8; 69],
            f: 6,
        }
        .encode();
        let raw = PacketReader::from_frame(&list).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_TIME_CHARGE_ITEM_LIST);
        // u4 + u2 + u4 + u4 + u8 + 69 + u4
        assert_eq!(raw.payload.len(), 22 + 69 + 4);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..6], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[6..10], &3u32.to_le_bytes());
        assert_eq!(&raw.payload[10..14], &4u32.to_le_bytes());
        assert_eq!(&raw.payload[14..22], &5u64.to_le_bytes());
        assert_eq!(raw.payload[22], 7);
        assert_eq!(raw.payload[90], 7);
        assert_eq!(&raw.payload[91..95], &6u32.to_le_bytes());

        let normal = TimeChargeNormalPacket {
            a: 1,
            b: 2,
            c: 3,
            charge_row: [7u8; 69],
            d: 4,
            rows: vec![TimeChargeSlot { e: 5, f: 6 }, TimeChargeSlot { e: 7, f: 8 }],
        }
        .encode();
        let raw = PacketReader::from_frame(&normal).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_TIME_CHARGE_NORMAL);
        // u4 x3 + 69 + u8 + count + 2 rows x 4 bytes
        assert_eq!(raw.payload.len(), 12 + 69 + 8 + 2 + 8);
        assert_eq!(&raw.payload[4..8], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[8..12], &3u32.to_le_bytes());
        assert_eq!(raw.payload[12], 7);
        assert_eq!(&raw.payload[81..89], &4u64.to_le_bytes());
        assert_eq!(&raw.payload[89..91], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[91..93], &5u16.to_le_bytes());
        assert_eq!(&raw.payload[93..95], &6u16.to_le_bytes());
        assert_eq!(&raw.payload[95..97], &7u16.to_le_bytes());
        assert_eq!(&raw.payload[97..99], &8u16.to_le_bytes());
    }

    #[test]
    fn satellite_pins_writes_withdraw_piemon_and_play_time() {
        let withdraw = TimeChargeWithdrawPacket {
            a: 1,
            c: 2,
            d: 3,
        }
        .encode();
        let raw = PacketReader::from_frame(&withdraw).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_TIME_CHARGE_WITHDRAW);
        // u4 + u8 + u2
        assert_eq!(raw.payload.len(), 14);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..12], &2u64.to_le_bytes());
        assert_eq!(&raw.payload[12..14], &3u16.to_le_bytes());

        let start = PiemonGameStartPacket {
            a: 1,
            b: 2,
            c: 3,
        }
        .encode();
        let raw = PacketReader::from_frame(&start).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MINIEVENT_PIEMON_GAME_START);
        assert_eq!(raw.payload.len(), 16);
        assert_eq!(&raw.payload[..8], &1u64.to_le_bytes());
        assert_eq!(&raw.payload[8..12], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[12..16], &3u32.to_le_bytes());

        let end = PiemonGameEndPacket { value: 9 }.encode();
        let raw = PacketReader::from_frame(&end).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MINIEVENT_PIEMON_GAME_END);
        assert_eq!(raw.payload, 9u32.to_le_bytes());

        let play_time = MgrPlayTimeSetPacket { skipped: 5 }.encode();
        let raw = PacketReader::from_frame(&play_time).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MGR_PLAY_TIME_SET);
        assert_eq!(raw.payload, 5u32.to_le_bytes());
    }

    #[test]
    fn debug_scanner_and_unnamed_pins_writes_encoded_bodies() {
        let range = DebugInsufficientRangePacket {
            first: 1,
            second: 2,
        }
        .encode();
        let raw = PacketReader::from_frame(&range).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DEBUG_INSUFFICIENT_RANGE);
        assert_eq!(raw.payload, [1, 0, 0, 0, 2, 0, 0, 0]);

        let lose = DebugLoseTargetPacket { skipped: 3 }.encode();
        let raw = PacketReader::from_frame(&lose).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DEBUG_LOSE_TARGET);
        assert_eq!(raw.payload, 3u32.to_le_bytes());

        // Chrome read without link values
        let read = ScannerReadPacket {
            anchor: 1,
            mode: 2,
            value: None,
            value2: None,
        }
        .encode();
        let raw = PacketReader::from_frame(&read).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SCANNER_READ);
        assert_eq!(raw.payload.len(), 6);
        assert_eq!(&raw.payload[..2], &1u16.to_le_bytes());
        assert_eq!(&raw.payload[2..6], &2u32.to_le_bytes());

        // Chrome read, linked and with the 30001 extra value
        let read = ScannerReadPacket {
            anchor: 1,
            mode: 30001,
            value: Some(3),
            value2: Some(4),
        }
        .encode();
        let raw = PacketReader::from_frame(&read).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SCANNER_READ);
        assert_eq!(raw.payload.len(), 14);
        assert_eq!(&raw.payload[6..10], &3u32.to_le_bytes());
        assert_eq!(&raw.payload[10..14], &4u32.to_le_bytes());

        let release = ScannerReleasePacket { flag: 1, a: 2 }.encode();
        let raw = PacketReader::from_frame(&release).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SCANNER_RELEASE);
        assert_eq!(raw.payload, [1, 0, 0, 0, 2, 0, 0, 0]);

        let error = ScannerErrorPacket { skipped: 4 }.encode();
        let raw = PacketReader::from_frame(&error).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SCANNER_ERROR);
        assert_eq!(raw.payload, 4u32.to_le_bytes());

        let unnamed = Unknown1342Packet {}.encode();
        let raw = PacketReader::from_frame(&unnamed).expect("frame should decode");
        assert_eq!(raw.packet_type, game::UNKNOWN_1342);
        assert!(raw.payload.is_empty());
    }

    #[test]
    fn notice_family_and_skill_pairing_writes_encoded_bodies() {
        let unnamed1343 = Unknown1343Packet {}.encode();
        let raw = PacketReader::from_frame(&unnamed1343).expect("frame should decode");
        assert_eq!(raw.packet_type, game::UNKNOWN_1343);
        assert!(raw.payload.is_empty());

        let blob = {
            let mut blob = [0u8; 71];
            for (i, cell) in blob.iter_mut().enumerate() {
                *cell = i as u8;
            }
            blob
        };
        let notice2140 = Modern2140Packet {
            a: 1,
            blob,
            name: "go".to_string(),
        }
        .encode();
        let raw = PacketReader::from_frame(&notice2140).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MODERN_2140);
        // u4 + 71 + name (u16 len + 2 bytes)
        assert_eq!(raw.payload.len(), 4 + 71 + 4);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(raw.payload[4], 0);
        assert_eq!(raw.payload[74], 70);
        assert_eq!(&raw.payload[75..77], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[77..79], b"go");

        let notice2141 = Modern2141Packet {
            a: 1,
            name: "stay".to_string(),
        }
        .encode();
        let raw = PacketReader::from_frame(&notice2141).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MODERN_2141);
        assert_eq!(raw.payload.len(), 4 + 6);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..6], &4u16.to_le_bytes());
        assert_eq!(&raw.payload[6..10], b"stay");

        let notice2143 = Modern2143Packet {
            values: vec![7, 9, 11],
            name: "g".to_string(),
        }
        .encode();
        let raw = PacketReader::from_frame(&notice2143).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MODERN_2143);
        assert_eq!(raw.payload.len(), 1 + 12 + 3);
        assert_eq!(raw.payload[0], 3);
        assert_eq!(&raw.payload[1..5], &7u32.to_le_bytes());
        assert_eq!(&raw.payload[5..9], &9u32.to_le_bytes());
        assert_eq!(&raw.payload[9..13], &11u32.to_le_bytes());
        assert_eq!(&raw.payload[13..15], &1u16.to_le_bytes());
        assert_eq!(raw.payload[15], b'g');

        let unknown3247 = Unknown3247Packet {
            handler: 1,
            a: 2,
            b: 3,
            skipped: 4,
            c: 5,
            d: 6,
        }
        .encode();
        let raw = PacketReader::from_frame(&unknown3247).expect("frame should decode");
        assert_eq!(raw.packet_type, game::UNKNOWN_3247);
        assert_eq!(raw.payload.len(), 17);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..6], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[6..10], &3u32.to_le_bytes());
        assert_eq!(&raw.payload[10..12], &4u16.to_le_bytes());
        assert_eq!(raw.payload[12], 5);
        assert_eq!(&raw.payload[13..17], &6u32.to_le_bytes());

        let unknown3248 = Unknown3248Packet {
            handler: 1,
            value: 2,
        }
        .encode();
        let raw = PacketReader::from_frame(&unknown3248).expect("frame should decode");
        assert_eq!(raw.packet_type, game::UNKNOWN_3248);
        assert_eq!(raw.payload, [1, 0, 0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn quad_and_cash_list_snapshots_write_encoded_bodies() {
        let quad = Unknown3249Packet {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
            e: 5,
            f: 6,
        }
        .encode();
        let raw = PacketReader::from_frame(&quad).expect("frame should decode");
        assert_eq!(raw.packet_type, game::UNKNOWN_3249);
        assert_eq!(raw.payload.len(), 12);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[8..12], [3, 4, 5, 6]);

        let mut rows = Vec::new();
        for i in 0..2 {
            rows.push(Modern3415Row {
                c: (i + 1) as u32,
                d: (i + 9) as u16,
            });
        }
        let list3415 = Modern3415Packet { a: 1, b: 2, rows }.encode();
        let raw = PacketReader::from_frame(&list3415).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MODERN_3415);
        // u4 + u4 + u2 count + 2 x (u4 c; u2 d)
        assert_eq!(raw.payload.len(), 10 + 2 * 6);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..8], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[8..10], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[10..14], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[14..16], &9u16.to_le_bytes());
        assert_eq!(&raw.payload[16..20], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[20..22], &10u16.to_le_bytes());

        let mut rows = Vec::new();
        for i in 0..2 {
            rows.push(Modern3417Row {
                c: (i + 1) as u32,
                d: (i + 2) as u32,
                e: (i + 3) as u32,
                f: 4,
                g: 5,
                h: 6,
                i: 7,
                j: 8,
                k: 9,
                l: 10,
                m: 11,
                n: 12,
            });
        }
        let list3417 = Modern3417Packet { a: 1, b: 2, rows }.encode();
        let raw = PacketReader::from_frame(&list3417).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MODERN_3417);
        // u4 + u4 + u2 count + 2 x 24-byte rows
        assert_eq!(raw.payload.len(), 10 + 2 * 24);
        assert_eq!(&raw.payload[10..14], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[14..18], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[18..22], &3u32.to_le_bytes());
        assert_eq!(raw.payload[22], 4);
        assert_eq!(&raw.payload[23..25], &5u16.to_le_bytes());
        assert_eq!(&raw.payload[25..27], &6u16.to_le_bytes());
        assert_eq!(&raw.payload[27..29], &7u16.to_le_bytes());
        assert_eq!(&raw.payload[29..34], [8, 9, 10, 11, 12]);
        assert_eq!(&raw.payload[34..38], &2u32.to_le_bytes());

        let mut rows = Vec::new();
        for i in 0..2 {
            rows.push(Modern3418Row {
                a: (i + 1) as u32,
                b: (i + 7) as u16,
            });
        }
        let list3418 = Modern3418Packet { rows }.encode();
        let raw = PacketReader::from_frame(&list3418).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MODERN_3418);
        assert_eq!(raw.payload.len(), 2 + 2 * 6);
        assert_eq!(&raw.payload[..2], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[2..6], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[6..8], &7u16.to_le_bytes());
        assert_eq!(&raw.payload[8..12], &2u32.to_le_bytes());
        assert_eq!(&raw.payload[12..14], &8u16.to_le_bytes());

        let mut entries = Vec::new();
        for i in 0..2 {
            entries.push((i + 1) as u32);
        }
        let list3419 = Modern3419Packet { a: 9, entries }.encode();
        let raw = PacketReader::from_frame(&list3419).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MODERN_3419);
        assert_eq!(raw.payload.len(), 4 + 2 + 2 * 4);
        assert_eq!(&raw.payload[..4], &9u32.to_le_bytes());
        assert_eq!(&raw.payload[4..6], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[6..10], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[10..14], &2u32.to_le_bytes());
    }

    #[test]
    fn arena_fame_list_writes_its_text_rows() {
        let fame = ArenaFameListPacket {
            season: 3,
            result: 1,
            rows: vec![
                ArenaFameRow {
                    rank: 1,
                    name: "Ana".to_string(),
                    guild: "GK".to_string(),
                    point: 900,
                },
                ArenaFameRow::default(),
            ],
        }
        .encode();
        let raw = PacketReader::from_frame(&fame).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ARENA_REQUEST_FAME);
        assert_eq!(
            raw.payload,
            vec![
                3, 0, // season
                1, // result
                2, // row count
                1, 3, 0, b'A', b'n', b'a', 2, 0, b'G', b'K', 132, 3, 0, 0, // first row
                0, 0, 0, 0, 0, 0, 0, 0, 0, // default row
            ]
        );
    }

    #[test]
    fn raid_skill_chain_closes_on_a_zero_link() {
        let packet = GameTwoRaidSkillChainPacket {
            handler: 7,
            skill_id: 42,
            entries: vec![
                RaidSkillChainEntry {
                    value: 500,
                    next: 9,
                    position: RaidSkillChainPosition { x: 10, y: -4 },
                },
                RaidSkillChainEntry {
                    value: 600,
                    next: 0,
                    position: RaidSkillChainPosition { x: 12, y: 8 },
                },
            ],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_RAID_SKILL_CHAIN);
        assert_eq!(raw.payload.len(), 8 + 2 + 2 * 16);
        assert_eq!(&raw.payload[..8], &[7, 0, 0, 0, 42, 0, 0, 0]);
        assert_eq!(&raw.payload[8..10], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[10..14], &500u32.to_le_bytes());
        assert_eq!(&raw.payload[14..18], &9u32.to_le_bytes());
        assert_eq!(&raw.payload[18..22], &10i32.to_le_bytes());
        assert_eq!(&raw.payload[22..26], &(-4i32).to_le_bytes());
        assert_eq!(&raw.payload[26..30], &600u32.to_le_bytes());
        assert_eq!(&raw.payload[30..34], &0u32.to_le_bytes());
        assert_eq!(&raw.payload[34..38], &12i32.to_le_bytes());
        assert_eq!(&raw.payload[38..42], &8i32.to_le_bytes());
    }

    #[test]
    fn fame_region_and_extended_cash_snapshot_writes_encoded_bodies() {
        let region = GamTwoRegionBuffPacket {
            a: 1,
            blob: [5, 6, 7, 8, 9, 10, 11, 12],
            flag: 2,
            rows: Vec::new(),
        }
        .encode();
        let raw = PacketReader::from_frame(&region).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GAMETWO_REGION_BUFF);
        assert_eq!(raw.payload.len(), 15);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..12], [5, 6, 7, 8, 9, 10, 11, 12]);
        assert_eq!(raw.payload[12], 2);
        assert_eq!(&raw.payload[13..15], &[0, 0]);

        let flag3990 = Unknown3990Packet { flag: 7 }.encode();
        let raw = PacketReader::from_frame(&flag3990).expect("frame should decode");
        assert_eq!(raw.packet_type, game::UNKNOWN_3990);
        assert_eq!(raw.payload, [7]);

        let mut rows = Vec::new();
        for i in 0..2 {
            rows.push(Modern3420Row {
                c: (i + 1) as u32,
                d: 2,
                e: 3,
                f: 4,
                g: 5,
                h: 6,
                i: 7,
                j: 8,
                k: 9,
                l: 10,
                m: 11,
                n: 12,
            });
        }
        let list3420 = Modern3420Packet { a: 1, rows }.encode();
        let raw = PacketReader::from_frame(&list3420).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MODERN_3420);
        // u4 a + u2 count + 2 x 24-byte rows
        assert_eq!(raw.payload.len(), 6 + 2 * 24);
        assert_eq!(&raw.payload[..4], &1u32.to_le_bytes());
        assert_eq!(&raw.payload[4..6], &2u16.to_le_bytes());
        assert_eq!(&raw.payload[6..10], &1u32.to_le_bytes());
        assert_eq!(raw.payload[6], 1);
        assert_eq!(raw.payload[18], 4);
        assert_eq!(&raw.payload[19..21], &5u16.to_le_bytes());
        assert_eq!(raw.payload[29], 12);
        assert_eq!(&raw.payload[30..34], &2u32.to_le_bytes());
    }

    #[test]
    fn pinochi_item_delete_writes_three_counted_blocks() {
        let lists = [
            PinochiDeleteList {
                rows: vec![PinochiDeleteRow { slot: 1, value: 2 }],
            },
            PinochiDeleteList::default(),
            PinochiDeleteList {
                rows: vec![
                    PinochiDeleteRow { slot: 3, value: 4 },
                    PinochiDeleteRow { slot: 5, value: 6 },
                ],
            },
        ];
        let packet = PinochiItemDeletePacket { key: 9, lists }.encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::EVENT_PINOCHI_ITEM_DELETE);
        assert_eq!(raw.payload.len(), 4 + (2 + 6) + (2) + (2 + 12));
        assert_eq!(&raw.payload[..4], &9u32.to_le_bytes());
        assert_eq!(raw.payload[4..6], [1, 0]);
        assert_eq!(raw.payload[6..8], [1, 0]); // one row
        assert_eq!(raw.payload[10..12], [0, 0]); // empty second block
    }

    #[test]
    fn master_user_list_writes_counted_name_rows() {
        let packet = MasterUserListPacket {
            users: vec![
                MasterUserRow {
                    first: "Ann".to_string(),
                    second: "Bo".to_string(),
                },
                MasterUserRow {
                    first: "Cy".to_string(),
                    second: String::new(),
                },
            ],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::MASTER_USER_LIST);
        assert_eq!(raw.payload.len(), 2 + (2 + 3 + 2 + 2) + (2 + 2 + 2));
        assert_eq!(&raw.payload[..2], &2u16.to_le_bytes());
        assert_eq!(raw.payload[2..7], [3, 0, b'A', b'n', b'n']);
    }

    #[test]
    fn npc_sell_result_writes_money_and_item_rows() {
        let packet = NpcSellResultPacket {
            remaining_bits: 3500,
            rows: vec![
                NpcSellRow {
                    kind: 1,
                    serial: 0xCAFEBABE,
                    stack: 2,
                },
                NpcSellRow {
                    kind: 0,
                    serial: 7,
                    stack: 9,
                },
            ],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::NPC_SELL);
        assert_eq!(raw.payload.len(), 8 + 1 + 7 * 2);
        assert_eq!(&raw.payload[..8], &3500i64.to_le_bytes());
        assert_eq!(raw.payload[8], 2); // row count
        assert_eq!(&raw.payload[9..16], &[1, 0xBE, 0xBA, 0xFE, 0xCA, 2, 0]);
        assert_eq!(&raw.payload[16..23], &[0, 7, 0, 0, 0, 9, 0]);
    }

    #[test]
    fn deck_buff_notice_encodes_u2_option_then_u4_value() {
        // option_index 100 = clear-buffs sentinel (consumer).
        let packet = EncyclopediaDeckBuffUsePacket { option_index: 100, value: 7 }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ENCYCLOPEDIA_DECK_BUFF);
        assert_eq!(raw.payload, [100, 0, 7, 0, 0, 0]);
    }

    #[test]
    fn deck_buff_request_reads_u2_option_index() {
        let mut writer = PacketWriter::new(game::ENCYCLOPEDIA_DECK_BUFF);
        writer.write_u16(3);
        let raw = PacketReader::from_frame(&writer.finalize()).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert!(matches!(request, GameRequest::EncyclopediaDeckBuff { deck_idx: 3 }));
    }

    #[test]
    fn initial_info_packet_uses_expected_opcode() {        let packet = GameInitialInfoPacket {
            character: CharacterSummary {
                id: 1,
                account_id: 1,
                slot: 0,
                name: "Admin".to_string(),
                partner_name: "Agumon".to_string(),
                model: DEFAULT_TAMER_MODEL_ID,
                partner_model: DEFAULT_PARTNER_MODEL_ID,
                ..CharacterSummary::default()
            },
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::INITIAL_INFO_RESPONSE);
        assert!(
            packet.len() > 2_500,
            "packet should resemble the large legacy bootstrap"
        );
    }

    #[test]
    fn load_inventory_packet_uses_expected_opcode() {
        let packet = LoadInventoryPacket {
            inventory: InventorySnapshot {
                bits: 0,
                size: 2,
                items: vec![ItemRecord::default(), ItemRecord::default()],
            },
            inventory_type: InventoryType::Inventory,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_INVENTORY);
    }

    #[test]
    fn load_tamer_packet_uses_expected_opcode() {
        let packet = LoadTamerPacket {
            character: CharacterSummary {
                id: 1,
                account_id: 1,
                name: "Admin".to_string(),
                partner_name: "Agumon".to_string(),
                partner_current_type: 31_001,
                ..CharacterSummary::default()
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_UNLOAD_ENTITY);
    }

    #[test]
    fn unload_tamer_packet_uses_expected_opcode() {
        let packet = UnloadTamerPacket {
            character: CharacterSummary {
                id: 1,
                account_id: 1,
                name: "Admin".to_string(),
                partner_name: "Agumon".to_string(),
                ..CharacterSummary::default()
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_UNLOAD_ENTITY);
    }

    #[test]
    fn load_buffs_packet_uses_expected_opcode() {
        let packet = LoadBuffsPacket {
            character: CharacterSummary {
                id: 1,
                account_id: 1,
                general_handler: 11_000,
                partner_handler: 21_000,
                active_buffs: vec![ActiveBuffSnapshot {
                    buff_id: 500,
                    buff_class: 1,
                    skill_id: 8001001,
                    remaining_seconds: 60,
                }],
                partner_active_buffs: vec![ActiveBuffSnapshot {
                    buff_id: 600,
                    buff_class: 1,
                    skill_id: 8002001,
                    remaining_seconds: 30,
                }],
                partner_active_debuffs: vec![ActiveBuffSnapshot {
                    buff_id: 700,
                    buff_class: 1,
                    skill_id: 8003001,
                    remaining_seconds: 15,
                }],
                ..CharacterSummary::default()
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_BUFFS);
    }

    #[test]
    fn load_mobs_packet_uses_expected_opcode() {
        let packet = LoadMobsPacket {
            mob: MobSummary {
                id: 900,
                handler: 44_001,
                type_id: 51_001,
                x: 15_000,
                y: 10_000,
                previous_x: 14_980,
                previous_y: 9_980,
                level: 25,
                ..MobSummary::default()
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_UNLOAD_ENTITY);
        assert_eq!(raw.payload[0], 3);
    }

    #[test]
    fn unload_mobs_packet_uses_expected_opcode() {
        let packet = UnloadMobsPacket {
            mob: MobSummary {
                id: 900,
                handler: 44_001,
                x: 15_000,
                y: 10_000,
                ..MobSummary::default()
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_UNLOAD_ENTITY);
        assert_eq!(raw.payload[0], 4);
    }

    #[test]
    fn load_mob_buffs_packet_uses_expected_opcode() {
        let packet = LoadMobBuffsPacket {
            mob: MobSummary {
                id: 900,
                handler: 44_001,
                active_debuffs: vec![ActiveBuffSnapshot {
                    buff_id: 88,
                    buff_class: 1,
                    skill_id: 7001,
                    remaining_seconds: 30,
                }],
                ..MobSummary::default()
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_BUFFS);
        assert_eq!(raw.payload[0], 16);
    }

    #[test]
    fn load_drops_packet_uses_expected_opcode() {
        let packet = LoadDropsPacket {
            drop: DropSummary {
                id: 990,
                handler: 49_200,
                item_id: 90600,
                owner_handler: 11_000,
                x: 15_010,
                y: 10_020,
                ..DropSummary::default()
            },
            viewer_handler: 11_000,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_UNLOAD_ENTITY);
        assert_eq!(raw.payload[0], 3);
    }

    #[test]
    fn unload_drops_packet_uses_expected_opcode() {
        let packet = UnloadDropsPacket {
            drop: DropSummary {
                id: 990,
                handler: 49_200,
                x: 15_010,
                y: 10_020,
                ..DropSummary::default()
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_UNLOAD_ENTITY);
        assert_eq!(raw.payload[0], 4);
    }

    #[test]
    fn pick_item_packet_uses_expected_opcode() {
        let packet = PickItemPacket {
            appearance_handler: 11_000,
            item_id: 5101,
            amount: 2,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOOT_ITEM);
    }

    #[test]
    fn pick_bits_packet_uses_expected_opcode() {
        let packet = PickBitsPacket {
            appearance_handler: 11_000,
            value: 123,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PICK_BITS);
    }

    #[test]
    fn pick_item_fail_packet_uses_expected_opcode() {
        let packet = PickItemFailPacket {
            reason: PickItemFailReason::NotTheOwner,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PICK_ITEM_FAIL);
    }

    #[test]
    fn seals_packet_uses_expected_opcode() {
        let packet = SealsPacket {
            seal_list: SealListSnapshot {
                seal_leader_id: 2,
                seals: vec![odmo_types::SealRecord {
                    seal_id: 5101,
                    amount: 12,
                    sequential_id: 7,
                    favorite: true,
                }],
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SEALS);
    }

    #[test]
    fn time_reward_packet_uses_expected_opcode() {
        let packet = TimeRewardPacket {
            reward: DailyRewardStatus {
                event_no: 101,
                remaining_seconds: 30,
                total_seconds: 60,
                week: 4,
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::TIME_REWARD);
    }

    #[test]
    fn attendance_packet_uses_expected_opcode() {
        let packet = TamerAttendancePacket {
            attendance: AttendanceStatus {
                event_no: u8::MAX,
                attendance_count: 0,
                notify: false,
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::TAMER_ATTENDANCE);
    }

    #[test]
    fn relations_packet_uses_expected_opcode() {
        let packet = TamerRelationsPacket {
            friends: vec![RelationEntry {
                name: "Tai".to_string(),
                connected: true,
                annotation: String::new(),
            }],
            foes: vec![RelationEntry {
                name: "Etemon".to_string(),
                connected: false,
                annotation: "Nemesis".to_string(),
            }],
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::AVAILABLE_RELATIONS);
        // Modern strings: {u2 len; bytes}; an empty annotation still consumes
        // its 2-byte length prefix.
        assert_eq!(
            raw.payload,
            [
                1, 0, // friends count
                1, 3, 0, b'T', b'a', b'i', 0, 0, // flag + name + empty annotation
                1, 0, // foes count
                6, 0, b'E', b't', b'e', b'm', b'o', b'n', 7, 0,
                b'N', b'e', b'm', b'e', b's', b'i', b's',
            ]
        );
    }

    #[test]
    fn friend_connect_packet_uses_expected_opcode() {
        let packet = FriendConnectPacket {
            name: "Tai".to_string(),
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::FRIEND_CONNECT);
        // Modern wire: u16 length prefix, no NUL terminator.
        assert_eq!(raw.payload, [3, 0, b'T', b'a', b'i']);
    }

    #[test]
    fn buddy_family_notices_write_modern_strings() {
        let add = BuddyAddPacket {
            flag: 1,
            name: "Sora".into(),
        }
        .encode();
        let add_raw = PacketReader::from_frame(&add).expect("frame should decode");
        assert_eq!(add_raw.packet_type, game::BUDDY_ADD);
        assert_eq!(add_raw.payload, [1, 4, 0, b'S', b'o', b'r', b'a']);

        let ban = BuddyBanPacket { name: "Kari".into() }.encode();
        let ban_raw = PacketReader::from_frame(&ban).expect("frame should decode");
        assert_eq!(ban_raw.packet_type, game::BUDDY_BAN);
        assert_eq!(ban_raw.payload, [4, 0, b'K', b'a', b'r', b'i']);

        let missing = BuddyNotFoundPacket { name: "Jp".into() }.encode();
        let missing_raw = PacketReader::from_frame(&missing).expect("frame should decode");
        assert_eq!(missing_raw.packet_type, game::BUDDY_NOT_FOUND);
        assert_eq!(missing_raw.payload, [2, 0, b'J', b'p']);

        let off = BuddyOffPacket { name: "Matt".into() }.encode();
        let off_raw = PacketReader::from_frame(&off).expect("frame should decode");
        assert_eq!(off_raw.packet_type, game::BUDDY_OFF);
        assert_eq!(off_raw.payload, [4, 0, b'M', b'a', b't', b't']);

        let rename = BuddyChangeNamePacket {
            flag: 2,
            original_name: "Aa".into(),
            updated_name: "Bb".into(),
        }
        .encode();
        let rename_raw = PacketReader::from_frame(&rename).expect("frame should decode");
        assert_eq!(rename_raw.packet_type, game::BUDDY_CHANGE_NAME);
        assert_eq!(
            rename_raw.payload,
            [2, 2, 0, b'A', b'a', 2, 0, b'B', b'b']
        );
    }

    #[test]
    fn party_invite_packet_uses_expected_opcode() {
        let packet = PartyInvitePacket {
            inviter_name: "AdminTamer".to_string(),
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_INVITE);
    }

    #[test]
    fn party_created_packet_uses_expected_opcode() {
        let packet = PartyCreatedPacket {
            party_id: 77,
            loot_type: 0,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_CREATED);
    }

    #[test]
    fn party_member_list_packet_uses_expected_opcode() {
        let packet = PartyMemberListPacket {
            party_id: 77,
            my_slot: 1,
            leader_slot: 0,
            loot_type: 0,
            rare_rate: 0,
            disp_rare_grade: 0,
            members: vec![PartyMemberListEntry {
                party_slot: 0,
                character: CharacterSummary {
                    name: "AdminTamer".to_string(),
                    model: DEFAULT_TAMER_MODEL_ID,
                    level: 70,
                    partner_current_type: 31_001,
                    partner_level: 65,
                    partner_name: "Agumon".to_string(),
                    map_id: DEFAULT_START_MAP_ID,
                    channel: 0,
                    general_handler: 11_000,
                    partner_handler: 21_000,
                    ..CharacterSummary::default()
                },
            }],
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_MEMBER_LIST);
    }

    #[test]
    fn party_leave_packet_uses_expected_opcode() {
        let packet = PartyLeavePacket { member_slot: 1 }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_LEAVE);
    }

    #[test]
    fn party_kick_packet_uses_expected_opcode() {
        let packet = PartyKickPacket { member_slot: 1 }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_KICK);
    }

    #[test]
    fn party_leader_changed_packet_uses_expected_opcode() {
        let packet = PartyLeaderChangedPacket { new_leader_slot: 2 }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_CHANGE_MASTER);
    }

    #[test]
    fn party_change_loot_packet_uses_expected_opcode() {
        let packet = PartyChangeLootTypePacket {
            loot_type: 2,
            rare_type: 3,
            disp_rare_grade: 4,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_CHANGE_LOOT);
    }

    #[test]
    fn party_member_info_packet_uses_expected_opcode() {
        let packet = PartyMemberInfoPacket {
            member_slot: 1,
            digimon_type: 31_001,
            tamer_hp: 1000,
            tamer_max_hp: 1200,
            tamer_ds: 500,
            tamer_max_ds: 700,
            digimon_hp: 800,
            digimon_max_hp: 900,
            digimon_ds: 300,
            digimon_max_ds: 400,
            tamer_level: 70,
            digimon_level: 65,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_MEMBER_INFO);
    }

    #[test]
    fn party_member_position_packet_uses_expected_opcode() {
        let packet = PartyMemberPositionPacket {
            member_slot: 1,
            tamer_x: 100,
            tamer_y: 200,
            digimon_x: 110,
            digimon_y: 210,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_MEMBER_POSITION);
    }

    fn read_fixed_wide(reader: &mut PacketReader, units: usize) -> String {
        let bytes = reader.read_bytes(units * 2).expect("fixed wide block");
        let codes: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        String::from_utf16(&codes).expect("utf16")
    }

    fn read_modern_utf8(reader: &mut PacketReader) -> String {
        let len = reader.read_u16().expect("utf8 len") as usize;
        let bytes = reader.read_bytes(len).expect("utf8 bytes");
        String::from_utf8(bytes).expect("utf8")
    }

    #[test]
    fn other_tamer_detail_info_packet_matches_modern_wire_layout() {
        let mut packet = OtherTamerDetailInfoPacket::empty(33_480);
        packet.tamer_name = "SmokeTamer".to_string();
        packet.guild_name = "Alpha".to_string();
        packet.stats = [70, 130, 1200, 800, 155, 90, 410, 0, 0, 0, 0, 0, 0];
        packet.equipment[0].push(DetailItemRecord {
            item_id: 700_001,
            slot: 5,
            stack: 1,
            flags: [1, 2, 3, 4],
            serial: 0xDEAD,
            bound: 7,
            durability: 100,
            grade: 3,
            sockets: [0u8; 16],
            enchant: [0u8; 16],
            expire_low: 0,
            expire_high: 0,
            extra_a: 9,
            extra_b: 10,
            tag_a: 1,
            tag_b: 2,
        });
        packet.digimon.model = 51_001;
        packet.digimon.name = "Agumon".to_string();
        packet.digimon.hatch_level = 4;
        packet.digimon.skills = vec![
            DetailStatPair { key: 0, value: 3 },
            DetailStatPair { key: 1, value: 2 },
        ];
        packet.digimon.enchants = vec![
            DetailStatPair { key: 0, value: 100 },
            DetailStatPair { key: 1, value: 50 },
        ];

        let encoded = packet.encode();
        let raw = PacketReader::from_frame(&encoded).expect("frame should decode");
        assert_eq!(raw.packet_type, game::OTHER_TAMER_DETAIL_INFO_RESPONSE);

        let mut reader = PacketReader::new(raw.payload);
        assert_eq!(reader.read_u32().expect("handler"), 33_480);
        assert_eq!(read_fixed_wide(&mut reader, 36), "SmokeTamer");
        assert_eq!(read_fixed_wide(&mut reader, 36), "Alpha");
        for expected in [70, 130, 1200, 800, 155, 90, 410, 0, 0, 0, 0, 0, 0] {
            assert_eq!(reader.read_i32().expect("stat"), expected);
        }
        assert_eq!(reader.read_u16().expect("field_200"), 0);
        // equipment list 0 holds one record; lists 1 and 2 are empty.
        assert_eq!(reader.read_u16().expect("equip0 count"), 1);
        assert_eq!(reader.read_u32().expect("item_id"), 700_001);
        let _ = reader.read_bytes(71 - 4).expect("rest of item record");
        assert_eq!(reader.read_u16().expect("equip1 count"), 0);
        assert_eq!(reader.read_u16().expect("equip2 count"), 0);
        assert_eq!(reader.read_i32().expect("field_238"), 0);
        assert_eq!(read_modern_utf8(&mut reader), "SmokeTamer");
        // embedded partner block
        assert_eq!(reader.read_u32().expect("model"), 51_001);
        assert_eq!(read_modern_utf8(&mut reader), "Agumon");
        let _ = reader.read_bytes(72).expect("appearance");
        assert_eq!(reader.read_u16().expect("d104"), 0);
        assert_eq!(reader.read_u16().expect("d106"), 0);
        assert_eq!(reader.read_u8().expect("hatch_level"), 4);
        assert_eq!(reader.read_u16().expect("d109"), 0);
        assert_eq!(reader.read_u32().expect("d111"), 0);
        assert_eq!(reader.read_u32().expect("d115"), 0);
        let _ = reader.read_bytes(7 + 7).expect("two 7-byte groups");
        assert_eq!(reader.read_u16().expect("d133"), 0);
        let _ = reader.read_bytes(14 * 3).expect("three 14-byte records");
        let _ = reader.read_bytes(6).expect("6-byte group");
        let _ = reader.read_bytes(20).expect("20-byte group");
        let _ = reader.read_bytes(8).expect("8-byte group");
        assert_eq!(reader.read_u16().expect("skill count"), 2);
        assert_eq!(reader.read_u32().expect("skill0 key"), 0);
        assert_eq!(reader.read_u32().expect("skill0 value"), 3);
        assert_eq!(reader.read_u32().expect("skill1 key"), 1);
        assert_eq!(reader.read_u32().expect("skill1 value"), 2);
        assert_eq!(reader.read_u16().expect("enchant count"), 2);
        assert_eq!(reader.read_u32().expect("enchant0 key"), 0);
        assert_eq!(reader.read_u32().expect("enchant0 value"), 100);
        assert_eq!(reader.read_u32().expect("enchant1 key"), 1);
        assert_eq!(reader.read_u32().expect("enchant1 value"), 50);
        assert_eq!(reader.read_u16().expect("field_493"), 0);
        for _ in 0..19 {
            assert_eq!(reader.read_i32().expect("attribute"), 0);
        }
        assert_eq!(reader.remaining_len(), 0);
    }

    #[test]
    fn party_member_map_change_packet_uses_expected_opcode() {
        let packet = PartyMemberMapChangePacket {
            member_slot: 1,
            map_id: 1,
            channel: 2,
            tamer_handler: 11_000,
            digimon_handler: 21_000,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_MEMBER_MAP_CHANGE);
    }

    #[test]
    fn party_member_digimon_change_packet_uses_expected_opcode() {
        let packet = PartyMemberDigimonChangePacket {
            member_slot: 1,
            digimon_type: 31_001,
            digimon_name: "Agumon".to_string(),
            digimon_hp: 800,
            digimon_max_hp: 900,
            digimon_ds: 300,
            digimon_max_ds: 400,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_MEMBER_DIGIMON_CHANGE);
    }

    #[test]
    fn party_member_buff_change_packet_uses_expected_opcode() {
        let packet = PartyMemberBuffChangePacket {
            member_slot: 1,
            buffs: vec![
                PartyMemberBuffEntry {
                    status: 1,
                    buff_code: 700,
                },
                PartyMemberBuffEntry {
                    status: 0,
                    buff_code: 701,
                },
            ],
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_MEMBER_BUFF_CHANGE);
    }

    #[test]
    fn party_member_disconnected_packet_uses_expected_opcode() {
        let packet = PartyMemberDisconnectedPacket { member_slot: 1 }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTY_MEMBER_DISCONNECTED);
    }

    #[test]
    fn digimon_evolution_fail_packet_uses_expected_opcode() {
        let packet = DigimonEvolutionFailPacket.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::EVOLUTION_FAILURE);
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_i32().expect("fail payload"), 0);
    }

    #[test]
    fn digimon_evolution_success_packet_uses_expected_opcode() {
        let packet = DigimonEvolutionSuccessPacket {
            digimon_handler: 21_000,
            tamer_handler: 11_000,
            new_type: 31_005,
            evolution_slot: 4,
            hp_rate: 255,
            parts_type: 0,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::EVOLUTION);
    }

    #[test]
    fn partner_switch_failure_packet_uses_expected_opcode_and_zero_uid() {
        let packet = PartnerSwitchFailurePacket.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTNER_SWITCH_RESPONSE);
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u32().expect("uid"), 0);
    }

    #[test]
    fn partner_switch_packet_uses_expected_opcode() {
        let packet = PartnerSwitchPacket {
            handler: 21_000,
            old_partner_current_type: 31_001,
            slot: 2,
            partner: odmo_types::PartnerSlotSnapshot {
                slot: 2,
                digimon_type: 31_002,
                model: 31_002,
                level: 11,
                name: "Greymon".to_string(),
                active_buffs: vec![ActiveBuffSnapshot {
                    buff_id: 500,
                    buff_class: 1,
                    skill_id: 8_001_001,
                    remaining_seconds: 30,
                }],
                ..odmo_types::PartnerSlotSnapshot::default()
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTNER_SWITCH_RESPONSE);
    }

    #[test]
    fn spirit_to_digimon_result_packet_uses_expected_opcode() {
        let packet = SpiritToDigimonResultPacket {
            digimon_id: 31_004,
            remaining_bits: 450,
            consumed_items: vec![(1, 81_001), (1, 81_002)],
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SPIRIT_TO_DIGIMON);
        // [u32 id][i64 bits] + two 5-byte consumed blocks + [u8 0] = 23 bytes.
        assert_eq!(raw.payload.len(), 23);
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u32().expect("digimon id"), 31_004);
        assert_eq!(payload.read_u64().expect("bits"), 450);
        assert_eq!(payload.read_u8().expect("count1"), 1);
        assert_eq!(payload.read_u32().expect("item1"), 81_001);
        assert_eq!(payload.read_u8().expect("count2"), 1);
        assert_eq!(payload.read_u32().expect("item2"), 81_002);
        assert_eq!(payload.read_u8().expect("end"), 0);
    }

    #[test]
    fn spirit_to_digimon_result_empty_list_round_trips() {
        let packet = SpiritToDigimonResultPacket {
            digimon_id: 31_004,
            remaining_bits: 450,
            consumed_items: Vec::new(),
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SPIRIT_TO_DIGIMON);
        // [u32 id][i64 bits] + [u8 0] terminator = 13 bytes.
        assert_eq!(raw.payload.len(), 13);
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u32().expect("digimon id"), 31_004);
        assert_eq!(payload.read_u64().expect("bits"), 450);
        assert_eq!(payload.read_u8().expect("end"), 0);
    }

    #[test]
    fn spirit_to_digimon_result_single_entry_round_trips() {
        let packet = SpiritToDigimonResultPacket {
            digimon_id: 31_004,
            remaining_bits: 450,
            consumed_items: vec![(2, 81_001)],
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::SPIRIT_TO_DIGIMON);
        // [u32 id][i64 bits] + one 5-byte consumed block + [u8 0] = 18 bytes.
        assert_eq!(raw.payload.len(), 18);
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u32().expect("digimon id"), 31_004);
        assert_eq!(payload.read_u64().expect("bits"), 450);
        assert_eq!(payload.read_u8().expect("count1"), 2);
        assert_eq!(payload.read_u32().expect("item1"), 81_001);
        assert_eq!(payload.read_u8().expect("end"), 0);
    }

    #[test]
    fn digimon_to_spirit_result_packet_uses_expected_opcode() {
        let packet = DigimonToSpiritResultPacket {
            slot: 2,
            remaining_bits: 250,
            consumed_items: vec![(1, 81_001)],
            gained_items: vec![(1, 81_003)],
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_TO_SPIRIT);
        // [u8 slot][i64 bits] + one 5-byte consumed block + [u8 0]
        // + one 5-byte gained block + [u8 0] = 21 bytes.
        assert_eq!(raw.payload.len(), 21);
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u8().expect("slot"), 2);
        assert_eq!(payload.read_u64().expect("bits"), 250);
        assert_eq!(payload.read_u8().expect("consumed count"), 1);
        assert_eq!(payload.read_u32().expect("consumed item"), 81_001);
        assert_eq!(payload.read_u8().expect("consumed end"), 0);
        assert_eq!(payload.read_u8().expect("gained count"), 1);
        assert_eq!(payload.read_u32().expect("gained item"), 81_003);
        assert_eq!(payload.read_u8().expect("gained end"), 0);
    }

    #[test]
    fn digimon_to_spirit_result_empty_lists_round_trip() {
        let packet = DigimonToSpiritResultPacket {
            slot: 2,
            remaining_bits: 250,
            consumed_items: Vec::new(),
            gained_items: Vec::new(),
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGIMON_TO_SPIRIT);
        // [u8 slot][i64 bits] + [u8 0] consumed terminator + [u8 0] gained
        // terminator = 11 bytes.
        assert_eq!(raw.payload.len(), 11);
        let mut payload = PacketReader::new(raw.payload);
        assert_eq!(payload.read_u8().expect("slot"), 2);
        assert_eq!(payload.read_u64().expect("bits"), 250);
        assert_eq!(payload.read_u8().expect("consumed end"), 0);
        assert_eq!(payload.read_u8().expect("gained end"), 0);
    }

    #[test]
    fn combine_sync_response_digi_empty_ceiling_round_trips() {
        let packet = CombineSyncResponsePacket::digi(0, Vec::new()).encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGI_COMBINE_SYNC);
        // [u8 result][u16 count=0] = 3 bytes.
        assert_eq!(raw.payload.len(), 3);
        let mut reader = PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), 0);
        assert_eq!(reader.read_u16().expect("ceiling count"), 0);
    }

    #[test]
    fn combine_sync_response_digi_single_ceiling_round_trips() {
        let packet = CombineSyncResponsePacket::digi(
            1,
            vec![CombineCeilingEntry {
                tier: 3,
                value_a: 7,
                value_b: 0x1234,
            }],
        )
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGI_COMBINE_SYNC);
        // [u8 result][u16 count] + one 4-byte ceiling entry = 7 bytes.
        assert_eq!(raw.payload.len(), 7);
        let mut reader = PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), 1);
        assert_eq!(reader.read_u16().expect("ceiling count"), 1);
        assert_eq!(reader.read_u8().expect("tier"), 3);
        assert_eq!(reader.read_u8().expect("value_a"), 7);
        assert_eq!(reader.read_u16().expect("value_b"), 0x1234);
    }

    #[test]
    fn combine_sync_response_union_uses_union_opcode() {
        let packet = CombineSyncResponsePacket::union(
            0,
            vec![CombineCeilingEntry {
                tier: 5,
                value_a: 9,
                value_b: 0x00AB,
            }],
        )
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::UNION_COMBINE_SYNC);
        assert_eq!(raw.payload.len(), 7);
        let mut reader = PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), 0);
        assert_eq!(reader.read_u16().expect("ceiling count"), 1);
        assert_eq!(reader.read_u8().expect("tier"), 5);
        assert_eq!(reader.read_u8().expect("value_a"), 9);
        assert_eq!(reader.read_u16().expect("value_b"), 0x00AB);
    }

    #[test]
    fn combine_result_response_digi_result_empty_lists_round_trip() {
        let packet =
            CombineResultResponsePacket::digi_result(0, Vec::new(), Vec::new(), Vec::new())
                .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGI_COMBINE);
        // [u8 result][u16 ceiling=0][u16 materials=0][u16 rewards=0] = 7 bytes.
        assert_eq!(raw.payload.len(), 7);
        let mut reader = PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), 0);
        assert_eq!(reader.read_u16().expect("ceiling count"), 0);
        assert_eq!(reader.read_u16().expect("material count"), 0);
        assert_eq!(reader.read_u16().expect("reward count"), 0);
    }

    #[test]
    fn combine_result_response_digi_result_single_entries_round_trip() {
        let packet = CombineResultResponsePacket::digi_result(
            1,
            vec![CombineCeilingEntry {
                tier: 2,
                value_a: 4,
                value_b: 0x0102,
            }],
            vec![CombineItemRef {
                item_uid: 0xAABB_CCDD,
                item_type: 0x0011,
                count: 0x0022,
            }],
            vec![DigiCombineReward {
                item_id: 5101,
                amount: 3,
                grade: 6,
            }],
        )
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGI_COMBINE);
        // [u8 result] + [u16 count + 4B ceiling] + [u16 count + 8B material]
        // + [u16 count + 71B reward node] = 1 + 6 + 10 + 73 = 90 bytes.
        assert_eq!(raw.payload.len(), 90);
        let mut reader = PacketReader::new(raw.payload);
        assert_eq!(reader.read_u8().expect("result"), 1);
        assert_eq!(reader.read_u16().expect("ceiling count"), 1);
        assert_eq!(reader.read_u8().expect("tier"), 2);
        assert_eq!(reader.read_u8().expect("value_a"), 4);
        assert_eq!(reader.read_u16().expect("value_b"), 0x0102);
        assert_eq!(reader.read_u16().expect("material count"), 1);
        assert_eq!(reader.read_u32().expect("item_uid"), 0xAABB_CCDD);
        assert_eq!(reader.read_u16().expect("item_type"), 0x0011);
        assert_eq!(reader.read_u16().expect("material count field"), 0x0022);
        assert_eq!(reader.read_u16().expect("reward count"), 1);
        // Reward node: leading 7 decoded bytes, then 64 reserved zero bytes.
        assert_eq!(reader.read_i32().expect("reward item id"), 5101);
        assert_eq!(reader.read_u16().expect("reward amount"), 3);
        assert_eq!(reader.read_u8().expect("reward grade"), 6);
        let reserved = reader.read_bytes(64).expect("reserved reward block");
        assert!(
            reserved.iter().all(|&b| b == 0),
            "reserved reward bytes must be zero"
        );
    }

    #[test]
    fn combine_result_response_digi_reward_uses_reward_opcode() {
        let packet =
            CombineResultResponsePacket::digi_reward(0, Vec::new(), Vec::new(), Vec::new())
                .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::DIGI_COMBINE_REWARD);
        assert_eq!(raw.payload.len(), 7);
    }

    #[test]
    fn combine_result_response_union_result_uses_union_opcode() {
        let packet =
            CombineResultResponsePacket::union_result(0, Vec::new(), Vec::new(), Vec::new())
                .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::UNION_COMBINE);
        assert_eq!(raw.payload.len(), 7);
    }

    #[test]
    fn combine_result_response_union_reward_uses_union_opcode() {
        let packet =
            CombineResultResponsePacket::union_reward(0, Vec::new(), Vec::new(), Vec::new())
                .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::UNION_COMBINE_REWARD);
        assert_eq!(raw.payload.len(), 7);
    }

    #[test]
    fn party_invite_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::PARTY_INVITE);
        writer.write_modern_utf8_string("Matt");
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::PartyInvite {
                target_name: "Matt".to_string(),
            }
        );
    }

    #[test]
    fn partner_evolution_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::EVOLUTION);
        writer.write_u32(21_000);
        writer.write_u8(4);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::PartnerEvolution {
                digimon_handler: 21_000,
                evolution_slot: 4,
            }
        );
    }

    #[test]
    fn evolution_unlock_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::EVOLUTION_UNLOCK);
        writer.write_i32(1);
        writer.write_u16(1);
        writer.write_u32(6_100);
        writer.write_u32(2);
        writer.write_i16(-1);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::EvolutionUnlock {
                evo_slot: 1,
                items: vec![EvolutionUnlockItem {
                    item_id: 6_100,
                    item_count: 2,
                }],
                selected_inven_idx: -1,
            }
        );
    }

    #[test]
    fn party_invite_response_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::PARTY_INVITE_RESPONSE);
        writer.write_i32(1);
        writer.write_modern_utf8_string("AdminTamer");
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::PartyInviteResponse {
                result_type: 1,
                inviter_name: "AdminTamer".to_string(),
            }
        );
    }

    #[test]
    fn modern_string_notice_family_writes_updated_shapes() {
        // 2301 party invite: single modern string.
        let invite = PartyInvitePacket {
            inviter_name: "Matt".into(),
        }
        .encode();
        let invite_raw = PacketReader::from_frame(&invite).expect("frame should decode");
        assert_eq!(invite_raw.payload, [4, 0, b'M', b'a', b't', b't']);

        // 2114 guild message: two strings, no leading handler u4.
        let message = GuildMessagePacket {
            sender_name: "Tai".into(),
            message: "hi".into(),
        }
        .encode();
        let message_raw = PacketReader::from_frame(&message).expect("frame should decode");
        assert_eq!(
            message_raw.payload,
            [
                3, 0, b'T', b'a', b'i', // sender
                2, 0, b'h', b'i',   // message
            ]
        );

        // 1311 name change: the result code selects the body, and a rejection ends
        // after the code so nothing else is read.
        let rejected = TamerChangeNamePacket::Rejected { result: 1 }.encode();
        let rejected_raw = PacketReader::from_frame(&rejected).expect("frame should decode");
        assert_eq!(rejected_raw.payload, [1, 0, 0, 0]);

        let nearby = TamerChangeNamePacket::Nearby {
            previous: "Aa".into(),
            next: "Bb".into(),
        }
        .encode();
        let nearby_raw = PacketReader::from_frame(&nearby).expect("frame should decode");
        assert_eq!(
            nearby_raw.payload,
            [
                3, 0, 0, 0,          // result
                2, 0, b'A', b'a',    // previous name
                2, 0, b'B', b'b',    // next name
            ]
        );

        let applied = TamerChangeNamePacket::Applied {
            item_slot: -1,
            name: "Bb".into(),
        }
        .encode();
        let applied_raw = PacketReader::from_frame(&applied).expect("frame should decode");
        assert_eq!(
            applied_raw.payload,
            [
                0, 0, 0, 0,                // result
                0xFF, 0xFF, 0xFF, 0xFF,    // item slot
                2, 0, b'B', b'b',          // new name
            ]
        );

        // 1709 map swap: map id widened to u4.
        let swap = MapSwapPacket {
            address: "127.0.0.1".into(),
            port: 7607,
            map_id: 9,
            x: 100,
            y: 200,
        }
        .encode();
        let swap_raw = PacketReader::from_frame(&swap).expect("frame should decode");
        let swap_body = &swap_raw.payload;
        assert_eq!(&swap_body[..7], &[9, 0, b'1', b'2', b'7', b'.', b'0']);
        assert_eq!(swap_body.len(), 2 + 9 + 4 * 4);

        // 2129 guild set title: no trailing byte after the two strings.
        let title = GuildAuthorityUpdatePacket {
            authority_class: 2,
            title: "T1".into(),
            duty: "D".into(),
        }
        .encode();
        let title_raw = PacketReader::from_frame(&title).expect("frame should decode");
        assert_eq!(
            title_raw.payload,
            [2, 2, 0, b'T', b'1', 1, 0, b'D']
        );
    }

    #[test]
    fn party_kick_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::PARTY_KICK);
        writer.write_modern_utf8_string("Matt");
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::PartyKick {
                target_name: "Matt".to_string(),
            }
        );
    }

    #[test]
    fn party_change_master_request_decodes_int_slot() {
        let mut writer = PacketWriter::new(game::PARTY_CHANGE_MASTER);
        writer.write_i32(2);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::PartyChangeMaster { new_leader_slot: 2 }
        );
    }

    #[test]
    fn party_change_loot_request_decodes_full_payload() {
        let mut writer = PacketWriter::new(game::PARTY_CHANGE_LOOT);
        writer.write_i32(2);
        writer.write_u8(3);
        writer.write_u8(4);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::PartyChangeLoot {
                loot_type: 2,
                rare_type: 3,
                disp_rare_grade: 4,
            }
        );
    }

    #[test]
    fn digimon_to_spirit_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::DIGIMON_TO_SPIRIT);
        writer.write_u8(2);
        writer.write_modern_utf8_string("4321");
        writer.write_i32(91001);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::DigimonToSpirit {
                slot: 2,
                validation: "4321".to_string(),
                npc_id: 91001,
            }
        );
    }

    #[test]
    fn spirit_to_digimon_request_decodes_modern_text_name() {
        let mut writer = PacketWriter::new(game::SPIRIT_TO_DIGIMON);
        writer.write_i32(31_004);
        writer.write_modern_utf8_string("Agumon");
        writer.write_i32(91001);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::SpiritToDigimon {
                model_id: 31_004,
                name: "Agumon".to_string(),
                npc_id: 91001,
            }
        );
    }

    #[test]
    fn hatch_finish_request_decodes_modern_text_name() {
        let mut writer = PacketWriter::new(game::HATCH_FINISH);
        writer.write_u8(1);
        writer.write_u32(7);
        writer.write_modern_utf8_string("Impmon");
        writer.write_i32(91_001);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::HatchFinish {
                flag: 1,
                portable_pos: 7,
                name: "Impmon".to_string(),
                npc_idx: 91_001,
            }
        );
    }

    #[test]
    fn channel_switch_and_seal_close_read_their_modern_widths() {
        let mut change = PacketWriter::new(game::CHANGE_CHANNEL);
        change.write_u32(300);
        let raw = PacketReader::from_frame(&change.finalize()).expect("frame should decode");
        assert_eq!(
            GameRequest::try_from(raw).expect("request should decode"),
            GameRequest::ChangeChannel { channel: 300 }
        );

        let mut close = PacketWriter::new(game::SEAL_CLOSE);
        close.write_i16(7);
        close.write_u32(42);
        let raw = PacketReader::from_frame(&close.finalize()).expect("frame should decode");
        assert_eq!(
            GameRequest::try_from(raw).expect("request should decode"),
            GameRequest::SealClose {
                seal_idx: 7,
                value: 42,
            }
        );
    }

    #[test]
    fn hatch_requests_lead_with_the_flag_byte() {
        let mut insert = PacketWriter::new(game::HATCH_INSERT_EGG);
        insert.write_u8(0);
        insert.write_u32(12);
        insert.write_i32(91_002);
        let raw = PacketReader::from_frame(&insert.finalize()).expect("frame should decode");
        assert_eq!(
            GameRequest::try_from(raw).expect("request should decode"),
            GameRequest::HatchInsertEgg {
                vip: 0,
                inven_slot: 12,
                npc_idx: 91_002,
            }
        );

        let mut increase = PacketWriter::new(game::HATCH_INCREASE);
        increase.write_u8(1);
        increase.write_i32(91_002);
        increase.write_u8(3);
        let raw = PacketReader::from_frame(&increase.finalize()).expect("frame should decode");
        assert_eq!(
            GameRequest::try_from(raw).expect("request should decode"),
            GameRequest::HatchIncrease {
                vip: 1,
                npc_idx: 91_002,
                data_level: 3,
            }
        );

        let mut remove = PacketWriter::new(game::HATCH_REMOVE_EGG);
        remove.write_u8(0);
        remove.write_i32(91_002);
        let raw = PacketReader::from_frame(&remove.finalize()).expect("frame should decode");
        assert_eq!(
            GameRequest::try_from(raw).expect("request should decode"),
            GameRequest::HatchRemoveEgg {
                vip: 0,
                npc_idx: 91_002,
            }
        );
    }

    #[test]
    fn guild_information_packet_uses_expected_opcode() {
        let packet = GuildInformationPacket {
            guild: GuildSnapshot {
                id: 77,
                name: "Tamers".to_string(),
                members: vec![odmo_types::GuildMemberSnapshot {
                    authority: 1,
                    character_name: "Tai".to_string(),
                    character_level: 60,
                    character_model: 80_003,
                    map_id: DEFAULT_START_MAP_ID,
                    channel: 2,
                    state: odmo_types::CharacterConnectionState::Ready,
                    ..odmo_types::GuildMemberSnapshot::default()
                }],
                ..GuildSnapshot::default()
            },
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GUILD_INFORMATION);
    }

    #[test]
    fn guild_historic_packet_uses_expected_opcode() {
        let packet = GuildHistoricPacket {
            entries: vec![GuildHistoricEntry {
                historic_type: 101,
                date_utc_seconds: 1234,
                master_class: 1,
                master_name: "Tai".to_string(),
                member_class: 5,
                member_name: "TK".to_string(),
            }],
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GUILD_HISTORIC);
    }

    #[test]
    fn guild_rank_packet_uses_expected_opcode() {
        let packet = GuildRankPacket { position: 7 }.encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::GUILD_RANK);
    }

    #[test]
    fn xai_info_packet_uses_expected_opcode() {
        let packet = XaiInfoPacket {
            xai: Some(XaiSnapshot {
                item_id: 131063,
                max_xgauge: 2000,
                max_xcrystals: 3,
            }),
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::XAI_INFO);
    }

    #[test]
    fn tamer_xai_resources_packet_uses_expected_opcode() {
        let packet = TamerXaiResourcesPacket {
            current_xgauge: 500,
            current_xcrystals: 2,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::TAMER_XAI_RESOURCES);
    }

    #[test]
    fn hit_packet_uses_expected_opcode_and_layout() {
        let packet = HitPacket {
            attacker_handler: 11_000,
            target_handler: 22_000,
            final_damage: 250,
            hp_before_hit: 1_000,
            hp_after_hit: 750,
            hit_type: HitType::Normal,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTNER_ATTACK_RESPONSE);
        // [u32 attacker][u32 target][10 * i32 damage block][i32 hit type][i64 hp_after][i64 hp_before]
        // = 4 + 4 + 40 + 4 + 8 + 8 = 68 bytes payload
        assert_eq!(raw.payload.len(), 68);
    }

    #[test]
    fn miss_hit_packet_uses_expected_opcode() {
        let packet = MissHitPacket {
            attacker_handler: 11_000,
            target_handler: 22_000,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::ATTACK_MISS);
        assert_eq!(raw.payload.len(), 8);
    }

    #[test]
    fn kill_on_hit_packet_uses_expected_opcode() {
        let packet = KillOnHitPacket {
            attacker_handler: 11_000,
            target_handler: 22_000,
            final_damage: 9999,
            hit_type: HitType::Critical,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::KILL_ON_HIT);
        // [u32 attacker][u32 target][10 * i32 damage block][i32 hit type] = 4 + 4 + 40 + 4 = 52
        assert_eq!(raw.payload.len(), 52);
    }

    #[test]
    fn cast_skill_packet_uses_expected_opcode() {
        let packet = CastSkillPacket {
            skill_slot: 2,
            attacker_handler: 11_000,
            target_handler: 22_000,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTNER_SKILL_RESPONSE);
        // [u8 slot][u32 attacker][u32 target] = 1 + 4 + 4 = 9
        assert_eq!(raw.payload.len(), 9);
    }

    #[test]
    fn kill_on_skill_packet_uses_expected_opcode() {
        let packet = KillOnSkillPacket {
            attacker_handler: 11_000,
            target_handler: 22_000,
            skill_slot: 3,
            final_damage: 5000,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::KILL_ON_SKILL);
        // [u32 attacker][u32 target][u32 skill][10 * i32 damage block] = 4 + 4 + 4 + 40 = 52
        assert_eq!(raw.payload.len(), 52);
    }

    #[test]
    fn partner_skill_error_packet_uses_expected_opcode() {
        let packet = PartnerSkillErrorPacket {
            attacker_handler: 11_000,
            parameter: 1,
            value: 5,
            value2: 0,
            context: 0,
        }
        .encode();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::PARTNER_SKILL_ERROR);
        // [u32 attacker][u8][u8][u8][i32 context] = 4 + 1 + 1 + 1 + 4 = 11
        assert_eq!(raw.payload.len(), 11);
    }

    #[test]
    fn hit_packet_damage_block_carries_negative_value() {
        let packet = HitPacket {
            attacker_handler: 11_000,
            target_handler: 22_000,
            final_damage: 250,
            hp_before_hit: 1_000,
            hp_after_hit: 750,
            hit_type: HitType::Normal,
        }
        .encode();
        // skip [length:u2][opcode:i2] = 4 bytes header, then [u32 attacker][u32 target] = 8 bytes payload
        // damage block starts at offset 4 + 4 + 4 = 12 (within frame, where length is at 0..2 and opcode is at 2..4)
        let damage_offset_in_frame = 4 + 4 + 4;
        let damage_bytes = [
            packet[damage_offset_in_frame],
            packet[damage_offset_in_frame + 1],
            packet[damage_offset_in_frame + 2],
            packet[damage_offset_in_frame + 3],
        ];
        let damage = i32::from_le_bytes(damage_bytes);
        assert_eq!(damage, -250);
    }

    #[test]
    fn move_item_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::MOVE_ITEM);
        writer.write_u16(12);
        writer.write_u16(34);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::MoveItem {
                origin_slot: 12,
                destination_slot: 34,
            }
        );
    }

    #[test]
    fn split_item_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::SPLIT_ITEM);
        writer.write_u16(5);
        writer.write_u16(9);
        writer.write_u16(20);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::SplitItem {
                origin_slot: 5,
                destination_slot: 9,
                amount: 20,
            }
        );
    }

    #[test]
    fn item_remove_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::ITEM_REMOVE);
        writer.write_u16(7);
        writer.write_i32(120);
        writer.write_i32(-30);
        writer.write_u16(3);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::RemoveItem {
                slot: 7,
                x: 120,
                y: -30,
                amount: 3,
            }
        );
    }

    #[test]
    fn npc_purchase_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::NPC_PURCHASE);
        writer.write_u8(1);
        writer.write_i32(9001);
        writer.write_u8(0x38);
        writer.write_i32(4);
        writer.write_u16(2);
        writer.write_u8(0x38);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::NpcPurchase {
                vip: 1,
                npc_id: 9001,
                marker: 0x38,
                shop_slot: 4,
                purchase_count: 2,
            }
        );
    }

    #[test]
    fn npc_sell_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::NPC_SELL);
        writer.write_u8(1);
        writer.write_i32(9001);
        writer.write_u8(0x38);
        writer.write_u8(6);
        writer.write_u16(2);
        writer.write_u8(0x38);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::NpcSell {
                vip: 1,
                npc_id: 9001,
                marker: 0x38,
                item_slot: 6,
                sell_amount: 2,
            }
        );
    }

    #[test]
    fn item_socket_in_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::ITEM_SOCKET_IN);
        writer.write_u8(1);
        writer.write_u32(40_010);
        writer.write_i32(9001);
        writer.write_u16(3);
        writer.write_u16(7);
        writer.write_u8(2);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::ItemSocketIn {
                vip: 1,
                inven_portable_pos: 40_010,
                npc_idx: 9001,
                src_inven_pos: 3,
                dst_inven_pos: 7,
                socket_order: 2,
            }
        );
    }

    #[test]
    fn item_socket_out_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::ITEM_SOCKET_OUT);
        writer.write_u8(1);
        writer.write_u32(40_010);
        writer.write_i32(9001);
        writer.write_u16(3);
        writer.write_u16(7);
        writer.write_u8(2);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::ItemSocketOut {
                vip: 1,
                inven_portable_pos: 40_010,
                npc_idx: 9001,
                src_inven_pos: 3,
                dst_inven_pos: 7,
                socket_order: 2,
            }
        );
    }

    #[test]
    fn item_socket_identify_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::ITEM_SOCKET_IDENTIFY);
        writer.write_u8(1);
        writer.write_i32(9001);
        writer.write_u32(40_010);
        writer.write_u16(5);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::ItemSocketIdentify {
                vip: 1,
                npc_idx: 9001,
                inven_portable_pos: 40_010,
                inven_pos: 5,
            }
        );
    }

    #[test]
    fn partner_attack_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::PARTNER_ATTACK);
        writer.write_u32(21_000);
        writer.write_u32(48_500);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::PartnerAttack {
                attacker_handler: 21_000,
                target_handler: 48_500,
            }
        );
    }

    #[test]
    fn partner_skill_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::PARTNER_SKILL);
        writer.write_u8(2);
        writer.write_u32(21_000);
        writer.write_u32(48_500);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::PartnerSkill {
                skill_slot: 2,
                attacker_handler: 21_000,
                target_handler: 48_500,
            }
        );
    }

    #[test]
    fn partner_switch_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::PARTNER_SWITCH);
        writer.write_u8(3);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(request, GameRequest::PartnerSwitch { slot: 3 });
    }

    #[test]
    fn partner_delete_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::PARTNER_DELETE);
        writer.write_u8(2);
        writer.write_modern_utf8_string("secret42");
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::PartnerDelete {
                slot: 2,
                validation: "secret42".to_string(),
            }
        );
    }

    #[test]
    fn open_ride_mode_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::OPEN_RIDE_MODE);
        writer.write_u32(7);
        writer.write_i32(101);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::OpenRideMode {
                evo_unit_idx: 7,
                item_type: 101,
            }
        );
    }

    #[test]
    fn ride_mode_start_request_decodes_empty_payload() {
        let writer = PacketWriter::new(game::RIDE_MODE_START);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(request, GameRequest::RideModeStart);
    }

    #[test]
    fn ride_mode_stop_request_decodes_empty_payload() {
        let writer = PacketWriter::new(game::RIDE_MODE_STOP);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(request, GameRequest::RideModeStop);
    }

    #[test]
    fn open_region_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::REGION_UNLOCK);
        writer.write_u8(5);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(request, GameRequest::RegionUnlock { region_idx: 5 });
    }

    #[test]
    fn set_target_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::SET_TARGET);
        writer.write_u32(21_000);
        writer.write_u32(48_500);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::SetTarget {
                attacker_handler: 21_000,
                target_handler: 48_500,
            }
        );
    }

    #[test]
    fn away_time_request_decodes_empty_payload() {
        let writer = PacketWriter::new(game::AWAY_TIME);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(request, GameRequest::AwayTime);
    }

    #[test]
    fn change_tamer_model_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::CHANGE_TAMER_MODEL);
        writer.write_i32(8001);
        writer.write_i32(4);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::ChangeTamerModel {
                model_id: 8001,
                inven_slot: 4,
            }
        );
    }

    #[test]
    fn cash_shop_reload_request_decodes_empty_payload() {
        let writer = PacketWriter::new(game::CASHSHOP_RELOAD);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(request, GameRequest::CashShopReload);
    }

    #[test]
    fn cash_shop_buy_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::CASHSHOP_BUY);
        writer.write_u8(2);
        writer.write_i32(1500);
        writer.write_u64(0x0102_0304_0506_0708);
        writer.write_i32(40_010);
        writer.write_i32(40_020);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::CashShopBuy {
                amount: 2,
                total_price: 1500,
                order_id: 0x0102_0304_0506_0708,
                product_ids: vec![40_010, 40_020],
            }
        );
    }

    #[test]
    fn cash_shop_buy_request_decodes_u16_order_id_payload() {
        let mut writer = PacketWriter::new(game::CASHSHOP_BUY);
        writer.write_u8(1);
        writer.write_i32(185);
        writer.write_u16(0);
        writer.write_i32(31_030_042);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::CashShopBuy {
                amount: 1,
                total_price: 185,
                order_id: 0,
                product_ids: vec![31_030_042],
            }
        );
    }

    #[test]
    fn hatch_backup_insert_request_decodes_modern_client_payload() {
        let mut writer = PacketWriter::new(game::HATCH_BACKUP_INSERT);
        writer.write_u8(1);
        writer.write_u32(12);
        writer.write_i32(9001);
        let packet = writer.finalize();
        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        let request = GameRequest::try_from(raw).expect("request should decode");
        assert_eq!(
            request,
            GameRequest::HatchBackupInsert {
                vip: 1,
                inven_slot: 12,
                npc_idx: 9001,
            }
        );
    }
}

#[cfg(test)]
mod sync_1006_exploration {
    //! Exploration test for the opcode-1006 (pGame::Sync / MAP_ENTITY) entity-load
    //! misalignment.
    //!
    //! This encodes a faithful model of the client's 1006 dispatcher and its
    //! subtype-3 handler, then asserts every 1006 entity-load payload is consumed
    //! to its exact end. It is the executable form of the wire contract: the
    //! dispatcher reads one `subtype` byte; the subtype-3 handler reads a
    //! `[u2 count]` and then walks `count` fixed 16-byte entries.
    //!
    //! Property 1 (Bug Condition): a 1006 sub-packet must be consumed exactly by
    //! the client parser - no underrun ("need N bytes, have M"), no leftover bytes.
    //!
    //! Validates: Requirements 1.1, 1.2, 1.3

    use super::*;
    use crate::reader::PacketReader;
    use odmo_types::{CharacterSummary, DropSummary, MobSummary};

    /// Result of running the client parser model over a 1006 payload.
    #[derive(Debug, PartialEq, Eq)]
    enum ParseOutcome {
        /// The handler consumed the payload to its exact end.
        Exact,
        /// The handler read past the buffer end (mirrors the client's
        /// "Insufficient data (need N bytes, have M)" exception).
        Underrun {
            need: usize,
            have: usize,
            offset: usize,
        },
        /// The handler returned before the payload end, leaving bytes unconsumed.
        Leftover {
            consumed: usize,
            total: usize,
            next_byte: u8,
        },
    }

    /// Bounds-checked cursor that mirrors the client's positional reads. A read
    /// past the end records the shortfall instead of consuming, matching the
    /// client's `cPacket::pop: Insufficient data` behavior.
    struct ModelCursor<'a> {
        buf: &'a [u8],
        pos: usize,
        underrun: Option<(usize, usize, usize)>,
    }

    impl<'a> ModelCursor<'a> {
        fn new(buf: &'a [u8]) -> Self {
            Self {
                buf,
                pos: 0,
                underrun: None,
            }
        }

        fn take(&mut self, n: usize) -> Option<&'a [u8]> {
            if self.pos + n > self.buf.len() {
                self.underrun = Some((n, self.buf.len() - self.pos, self.pos));
                return None;
            }
            let slice = &self.buf[self.pos..self.pos + n];
            self.pos += n;
            Some(slice)
        }

        fn read_u8(&mut self) -> Option<u8> {
            self.take(1).map(|s| s[0])
        }

        fn read_u16(&mut self) -> Option<u16> {
            self.take(2).map(|s| u16::from_le_bytes([s[0], s[1]]))
        }

        fn read_u32(&mut self) -> Option<u32> {
            self.take(4)
                .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
        }

        /// `[u2 len LE][len ASCII bytes]`, no terminator. The client caps the
        /// length at 0x200 and treats an out-of-range length as empty; a
        /// misaligned length that points past the buffer underruns - the
        /// observed `need N bytes` symptom.
        fn read_entity_string(&mut self) -> Option<()> {
            let len = self.read_u16()? as usize;
            if len != 0 && len <= ENTITY_STRING_MAX {
                self.take(len)?;
            }
            Some(())
        }
    }

    /// Consume a digimon (kind 1) body in the order the client reads it:
    /// destination pair, name, scale, level, facing, move speed, attack speed,
    /// spawn flag, tamer handle, hp rate, condition, enchant level, the seven
    /// extend-stat levels, and the appearance item id.
    fn consume_digimon_body(cur: &mut ModelCursor) -> Option<()> {
        cur.read_u32()?; // destination x, y
        cur.read_u32()?;
        cur.read_entity_string()?; // name
        cur.read_u16()?; // scale
        cur.read_u8()?; // level
        cur.read_u32()?; // facing direction
        cur.read_u16()?; // move speed
        cur.read_u16()?; // attack speed
        cur.read_u8()?; // spawn flag
        cur.read_u32()?; // tamer handle
        cur.read_u8()?; // hp rate
        cur.read_u32()?; // condition
        cur.read_u16()?; // enchant level
        for _ in 0..7 {
            cur.read_u16()?; // extend-stat levels
        }
        cur.read_u32()?; // appearance item id
        Some(())
    }

    /// Consume a tamer (kind 2) body in the order the client reads it:
    /// destination pair, name, level, facing, move speed, hp rate, the 16x69
    /// equipment block plus a 69-byte digivice record, condition/sync/partner
    /// dwords, scale, guild class (with id and name when set), achievement,
    /// match team, seal id, an optional shop title, and the costume id.
    fn consume_tamer_body(cur: &mut ModelCursor) -> Option<()> {
        cur.read_u32()?; // destination x, y
        cur.read_u32()?;
        cur.read_entity_string()?; // name
        cur.read_u8()?; // level
        cur.read_u32()?; // facing direction
        cur.read_u16()?; // move speed
        cur.read_u8()?; // hp rate
        cur.take(TAMER_EQUIPMENT_SLOTS * VISUAL_SLOT_LEN)?; // equipment slots
        cur.take(VISUAL_SLOT_LEN)?; // digivice record
        let condition = cur.read_u32()?; // condition bitfield
        cur.read_u32()?; // sync
        cur.read_u32()?; // partner handle
        cur.read_u16()?; // scale
        let guild_class = cur.read_u8()?;
        if guild_class != 0 {
            cur.read_u32()?; // guild id
            cur.read_entity_string()?; // guild name
        }
        cur.read_u16()?; // achievement
        cur.read_u8()?; // match team
        cur.read_u16()?; // seal id
        if condition & 0x4 != 0 {
            cur.read_entity_string()?; // shop title
        }
        cur.read_u32()?; // costume id
        Some(())
    }

    /// Consume an item/drop (kind 3) body: `[u4 owner handle][u1 state]`.
    fn consume_item_body(cur: &mut ModelCursor) -> Option<()> {
        cur.read_u32()?; // owner handle
        cur.read_u8()?; // state
        Some(())
    }

    /// Consume a monster (kind 4) body: destination pair, hp rate, level, growth
    /// skill id, stack count, condition, an extra skill id when the condition
    /// requests it, then the skill/seed record count.
    fn consume_monster_body(cur: &mut ModelCursor) -> Option<()> {
        cur.read_u32()?; // destination x, y
        cur.read_u32()?;
        cur.read_u8()?; // hp rate
        cur.read_u8()?; // level
        cur.read_u32()?; // growth skill id
        cur.read_u32()?; // stack count
        let condition = cur.read_u32()?;
        if condition == 6 {
            cur.read_u32()?; // casting skill id
        }
        let record_count = cur.read_u32()?; // skill/seed count
        for _ in 0..record_count {
            cur.read_u32()?;
            cur.read_u32()?;
            cur.read_u32()?;
            cur.read_u32()?;
        }
        Some(())
    }

    /// Consume one typed entry: a 16-byte header whose third dword carries the
    /// entity kind in its high word, then the kind-specific body. An unknown kind
    /// consumes no body (the client's `default` branch), which strands the cursor
    /// and surfaces as leftover/underrun downstream - exactly the drift the bug
    /// produces.
    fn consume_entry(cur: &mut ModelCursor) -> Option<()> {
        cur.read_u32()?; // header word 0
        cur.read_u32()?; // header word 1
        let kind_handle = cur.read_u32()?; // header word 2: HIWORD = kind
        cur.read_u32()?; // header word 3
        let kind = (kind_handle >> 16) as u16;
        match kind {
            1 => consume_digimon_body(cur),
            2 => consume_tamer_body(cur),
            3 => consume_item_body(cur),
            4 => consume_monster_body(cur),
            _ => Some(()),
        }
    }

    /// Model of a `New`/`In` count-framed record: read `[u2 count]`, then
    /// consume `count` typed entries.
    fn consume_count_framed(cur: &mut ModelCursor) -> Option<()> {
        let count = cur.read_u16()?;
        for _ in 0..count {
            consume_entry(cur)?;
        }
        Some(())
    }

    /// Model of the 1006 dispatcher: a loop that reads `[u1 action]`, runs the
    /// matching handler, then reads the next action
    /// byte until a `0x00` terminator ends the block. Returns whether the payload
    /// was consumed exactly, underran, or left trailing bytes.
    fn client_parse_1006(payload: &[u8]) -> ParseOutcome {
        let mut cur = ModelCursor::new(payload);
        loop {
            let action = match cur.read_u8() {
                Some(b) => b,
                None => {
                    let (need, have, offset) = cur.underrun.expect("underrun recorded on None");
                    return ParseOutcome::Underrun { need, have, offset };
                }
            };

            if action == ENTITY_BLOCK_END {
                break;
            }

            // New (1) and In (3) carry count-framed typed entries; other
            // lifecycle actions are not produced by these encoders.
            let handled = match action {
                ENTITY_ACTION_NEW | ENTITY_ACTION_IN => consume_count_framed(&mut cur),
                _ => Some(()),
            };

            if handled.is_none() {
                let (need, have, offset) = cur.underrun.expect("underrun recorded on None");
                return ParseOutcome::Underrun { need, have, offset };
            }
        }

        if cur.pos == payload.len() {
            ParseOutcome::Exact
        } else {
            ParseOutcome::Leftover {
                consumed: cur.pos,
                total: payload.len(),
                next_byte: payload[cur.pos],
            }
        }
    }

    /// Decode a finalized frame and run the client 1006 parser model over its
    /// payload (payload starts at frame offset 4, after length + opcode).
    fn parse_encoded_1006(frame: &[u8]) -> ParseOutcome {
        let raw = PacketReader::from_frame(frame).expect("frame should decode");
        assert_eq!(
            raw.packet_type,
            game::LOAD_UNLOAD_ENTITY,
            "opcode must be 1006"
        );
        client_parse_1006(&raw.payload)
    }

    /// Assert exact consumption, formatting the counterexample like the client's
    /// own diagnostic when the payload is misaligned.
    fn assert_consumed_exactly(label: &str, frame: &[u8]) {
        let outcome = parse_encoded_1006(frame);
        assert!(
            outcome == ParseOutcome::Exact,
            "{label}: client 1006 parser did not consume the payload exactly: {outcome:?}"
        );
    }

    fn tamer_fixture(seed: u32) -> CharacterSummary {
        let name = match seed % 3 {
            0 => String::new(),
            1 => "Admin".to_string(),
            _ => "LongTamerName".to_string(),
        };
        CharacterSummary {
            id: 1 + seed as u64,
            account_id: 1,
            name,
            partner_name: "Agumon".to_string(),
            x: 15_000 + seed as i32,
            y: 10_000 + seed as i32,
            level: (1 + seed % 99) as u8,
            general_handler: if seed.is_multiple_of(2) {
                0
            } else {
                11_000 + seed
            },
            partner_handler: if seed.is_multiple_of(2) {
                0
            } else {
                21_000 + seed
            },
            partner_current_type: 31_001,
            ..CharacterSummary::default()
        }
    }

    fn mob_fixture(seed: u32) -> MobSummary {
        MobSummary {
            id: 900 + seed as u64,
            handler: if seed.is_multiple_of(2) {
                0
            } else {
                44_000 + seed
            },
            type_id: 51_001 + seed as i32,
            x: 15_000 + seed as i32,
            y: 10_000 + seed as i32,
            previous_x: 14_980 + seed as i32,
            previous_y: 9_980 + seed as i32,
            level: (1 + seed % 99) as u8,
            grow_stack: (seed % 7) as u8,
            disposed_objects: (seed % 5) as u8,
            respawn: false,
            ..MobSummary::default()
        }
    }

    fn drop_fixture(seed: u32) -> DropSummary {
        DropSummary {
            id: 990 + seed as u64,
            handler: if seed.is_multiple_of(2) {
                0
            } else {
                49_000 + seed
            },
            item_id: 20_000 + seed as i32,
            x: 15_010 + seed as i32,
            y: 10_020 + seed as i32,
            owner_id: seed as u64,
            owner_handler: if seed.is_multiple_of(3) {
                0
            } else {
                60_000 + seed
            },
            no_owner: seed.is_multiple_of(2),
            ..DropSummary::default()
        }
    }

    #[test]
    fn tamer_subtype3_payload_is_consumed_exactly() {
        let frame = LoadTamerPacket {
            character: tamer_fixture(1),
        }
        .encode();
        assert_consumed_exactly("LoadTamerPacket (subtype 3)", &frame);
    }

    #[test]
    fn mob_subtype3_payload_is_consumed_exactly() {
        let frame = LoadMobsPacket {
            mob: mob_fixture(1),
        }
        .encode();
        assert_consumed_exactly("LoadMobsPacket (subtype 3)", &frame);
    }

    #[test]
    fn drop_subtype3_payload_is_consumed_exactly() {
        let frame = LoadDropsPacket {
            drop: drop_fixture(1),
            viewer_handler: 11_000,
        }
        .encode();
        assert_consumed_exactly("LoadDropsPacket (subtype 3)", &frame);
    }

    #[test]
    fn empty_name_tamer_is_consumed_exactly() {
        let frame = LoadTamerPacket {
            character: tamer_fixture(0),
        }
        .encode();
        assert_consumed_exactly("LoadTamerPacket (empty name)", &frame);
    }

    /// Scoped property: across many generated tamer/mob/drop inputs, every 1006
    /// entity-load payload must be consumed exactly by the client parser model.
    /// The first counterexample is reported in the client's own diagnostic shape.
    ///
    /// Validates: Requirements 1.1, 1.2, 1.3
    #[test]
    fn prop_all_1006_subpackets_consumed_exactly() {
        for seed in 0..64u32 {
            let tamer = LoadTamerPacket {
                character: tamer_fixture(seed),
            }
            .encode();
            assert_consumed_exactly(&format!("LoadTamerPacket seed={seed}"), &tamer);

            let mob = LoadMobsPacket {
                mob: mob_fixture(seed),
            }
            .encode();
            assert_consumed_exactly(&format!("LoadMobsPacket seed={seed}"), &mob);

            let drop = LoadDropsPacket {
                drop: drop_fixture(seed),
                viewer_handler: 11_000 + seed,
            }
            .encode();
            assert_consumed_exactly(&format!("LoadDropsPacket seed={seed}"), &drop);
        }
    }
}

#[cfg(test)]
mod sync_1006_preservation {
    //! Preservation tests for the opcode-1006 entity-load misalignment fix.
    //!
    //! Property 2 (Preservation): every encoder that is NOT a bug target must
    //! produce byte-identical output before and after the fix. These tests
    //! capture the current byte output as golden baselines so the upcoming fix
    //! is proven not to touch them.
    //!
    //! Two groups are covered:
    //!   1. Non-1006 encoders (distinct opcodes), which the fix never touches.
    //!   2. The 1006 unload/buff sub-variants, which the client already parses
    //!      and which the fix must leave alone (only LoadTamer/LoadMobs/LoadDrops
    //!      are the bug targets).
    //!
    //! Validates: Requirements 3.1, 3.2

    use super::*;
    use odmo_types::{
        ActiveBuffSnapshot, CharacterSummary, DropSummary, InventorySnapshot, ItemRecord,
        MobSummary,
    };

    /// Render bytes as lowercase hex for golden capture and counterexamples.
    fn to_hex(bytes: &[u8]) -> String {
        let mut s = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }

    // ---- deterministic fixtures -------------------------------------------

    fn inventory_packet() -> LoadInventoryPacket {
        LoadInventoryPacket {
            inventory: InventorySnapshot {
                bits: 0x0102_0304_0506_0708,
                size: 2,
                items: vec![ItemRecord::new(5101, 3)],
            },
            inventory_type: InventoryType::Inventory,
        }
    }

    fn server_experience_packet() -> ServerExperiencePacket {
        ServerExperiencePacket {
            experience: 123_456,
        }
    }

    fn pick_item_packet() -> PickItemPacket {
        PickItemPacket {
            appearance_handler: 11_000,
            item_id: 5101,
            amount: 2,
        }
    }

    fn pick_bits_packet() -> PickBitsPacket {
        PickBitsPacket {
            appearance_handler: 11_000,
            value: 123,
        }
    }

    fn receive_exp_packet() -> ReceiveExpPacket {
        ReceiveExpPacket {
            tamer_exp: 1_000,
            tamer_bonus: 100,
            tamer_total: 1_100,
            partner_handler: 21_000,
            partner_exp: 2_000,
            partner_bonus: 200,
            partner_total: 2_200,
            skill_exp: 50,
        }
    }

    fn level_up_packet() -> LevelUpPacket {
        LevelUpPacket {
            handler: 11_000,
            level: 42,
        }
    }

    fn tamer_fixture() -> CharacterSummary {
        CharacterSummary {
            id: 1,
            name: "Admin".to_string(),
            x: 15_000,
            y: 10_000,
            general_handler: 11_000,
            partner_handler: 21_000,
            partner_x: 15_010,
            partner_y: 10_020,
            active_buffs: vec![ActiveBuffSnapshot {
                buff_id: 500,
                buff_class: 1,
                skill_id: 8_001_001,
                remaining_seconds: 30,
            }],
            partner_active_buffs: vec![ActiveBuffSnapshot {
                buff_id: 600,
                buff_class: 1,
                skill_id: 8_002_002,
                remaining_seconds: 45,
            }],
            ..CharacterSummary::default()
        }
    }

    fn mob_fixture() -> MobSummary {
        MobSummary {
            id: 900,
            handler: 44_001,
            type_id: 51_001,
            x: 15_000,
            y: 10_000,
            previous_x: 14_980,
            previous_y: 9_980,
            level: 25,
            active_debuffs: vec![ActiveBuffSnapshot {
                buff_id: 88,
                buff_class: 1,
                skill_id: 7001,
                remaining_seconds: 30,
            }],
            ..MobSummary::default()
        }
    }

    fn drop_fixture() -> DropSummary {
        DropSummary {
            id: 990,
            handler: 49_200,
            item_id: 90_600,
            owner_handler: 11_000,
            x: 15_010,
            y: 10_020,
            ..DropSummary::default()
        }
    }

    fn unload_tamer_packet() -> UnloadTamerPacket {
        UnloadTamerPacket {
            character: tamer_fixture(),
        }
    }

    fn unload_mobs_packet() -> UnloadMobsPacket {
        UnloadMobsPacket { mob: mob_fixture() }
    }

    fn unload_drops_packet() -> UnloadDropsPacket {
        UnloadDropsPacket {
            drop: drop_fixture(),
        }
    }

    fn load_buffs_packet() -> LoadBuffsPacket {
        LoadBuffsPacket {
            character: tamer_fixture(),
        }
    }

    fn load_mob_buffs_packet() -> LoadMobBuffsPacket {
        LoadMobBuffsPacket { mob: mob_fixture() }
    }

    /// Parse a lowercase-hex string into its byte vector.
    fn from_hex(hex: &str) -> Vec<u8> {
        assert!(hex.len().is_multiple_of(2), "hex length must be even");
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect("valid hex byte"))
            .collect()
    }

    // ---- golden baselines (captured from the current encoders) ------------
    //
    // These byte vectors are the baseline F(X). The fix must reproduce them
    // exactly (F(X) == F'(X)) for every encoder below.

    const GOLDEN_INVENTORY: &str = "9f00893e000000000807060504030201000200ed1306000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a31a";
    const GOLDEN_SERVER_EXPERIENCE: &str = "1a001e040100000040e20100010000000000000040e20100261a";
    const GOLDEN_PICK_ITEM: &str = "1500460ff82a0000ed13000002000000000000291a";
    const GOLDEN_PICK_BITS: &str = "1a00470ff82a00007b000000000000000000000000000000261a";
    const GOLDEN_RECEIVE_EXP: &str = "4200fa03e80300000000000064000000000000004c0400000000000008520000d007000000000000c800000000000000980800000000000032000000000000007e1a";
    const GOLDEN_LEVEL_UP: &str = "0b00fb03f82a00002a371a";
    const GOLDEN_UNLOAD_TAMER: &str =
        "2500ee03040200f82a0000983a00001027000008520000a23a00002427000000000000191a";
    const GOLDEN_UNLOAD_MOBS: &str = "1900ee03040100e1ab0000983a00001027000000000000251a";
    const GOLDEN_UNLOAD_DROPS: &str = "1900ee0304010030c00000a23a00002427000000000000251a";
    const GOLDEN_LOAD_BUFFS: &str = "3000ee03100100f82a000001f40101001e000000e9157a0001000852000001580201002d000000d2197a000000000c1a";
    const GOLDEN_LOAD_MOB_BUFFS: &str =
        "1f00ee0310000000000100e1ab000001580001001e000000591b000000231a";

    /// Assert an encoder reproduces its captured golden bytes, reporting the
    /// divergence as hex when it does not.
    fn assert_golden(label: &str, golden_hex: &str, actual: &[u8]) {
        let expected = from_hex(&golden_hex.replace(' ', ""));
        assert!(
            actual == expected.as_slice(),
            "{label}: encoder output changed.\n expected={}\n actual  ={}",
            to_hex(&expected),
            to_hex(actual)
        );
    }

    // ---- non-1006 byte-identity (distinct opcodes, never touched) ---------

    #[test]
    fn load_inventory_matches_golden() {
        assert_golden(
            "LoadInventoryPacket",
            GOLDEN_INVENTORY,
            &inventory_packet().encode(),
        );
    }

    #[test]
    fn server_experience_matches_golden() {
        assert_golden(
            "ServerExperiencePacket",
            GOLDEN_SERVER_EXPERIENCE,
            &server_experience_packet().encode(),
        );
    }

    #[test]
    fn pick_item_matches_golden() {
        assert_golden(
            "PickItemPacket",
            GOLDEN_PICK_ITEM,
            &pick_item_packet().encode(),
        );
    }

    #[test]
    fn pick_bits_matches_golden() {
        assert_golden(
            "PickBitsPacket",
            GOLDEN_PICK_BITS,
            &pick_bits_packet().encode(),
        );
    }

    #[test]
    fn receive_exp_matches_golden() {
        assert_golden(
            "ReceiveExpPacket",
            GOLDEN_RECEIVE_EXP,
            &receive_exp_packet().encode(),
        );
    }

    #[test]
    fn level_up_matches_golden() {
        assert_golden(
            "LevelUpPacket",
            GOLDEN_LEVEL_UP,
            &level_up_packet().encode(),
        );
    }

    // ---- already-correct 1006 sub-variants (unload/buff, not bug targets) -

    #[test]
    fn unload_tamer_matches_golden() {
        assert_golden(
            "UnloadTamerPacket",
            GOLDEN_UNLOAD_TAMER,
            &unload_tamer_packet().encode(),
        );
    }

    #[test]
    fn unload_mobs_matches_golden() {
        assert_golden(
            "UnloadMobsPacket",
            GOLDEN_UNLOAD_MOBS,
            &unload_mobs_packet().encode(),
        );
    }

    #[test]
    fn unload_drops_matches_golden() {
        assert_golden(
            "UnloadDropsPacket",
            GOLDEN_UNLOAD_DROPS,
            &unload_drops_packet().encode(),
        );
    }

    #[test]
    fn load_buffs_matches_golden() {
        assert_golden(
            "LoadBuffsPacket",
            GOLDEN_LOAD_BUFFS,
            &load_buffs_packet().encode(),
        );
    }

    #[test]
    fn load_mob_buffs_matches_golden() {
        assert_golden(
            "LoadMobBuffsPacket",
            GOLDEN_LOAD_MOB_BUFFS,
            &load_mob_buffs_packet().encode(),
        );
    }

    // ---- frame-wire invariants --------------------------------------------

    /// Verify the frame envelope: `[Length u16 LE][Opcode i16 LE][..][Checksum u16 LE]`,
    /// with `checksum = length XOR 6716`.
    fn assert_frame_invariants(label: &str, frame: &[u8], expected_opcode: i16) {
        assert!(
            frame.len() >= 6,
            "{label}: frame too short: {}",
            frame.len()
        );
        let length = u16::from_le_bytes([frame[0], frame[1]]) as usize;
        assert_eq!(
            length,
            frame.len(),
            "{label}: length field must equal frame size"
        );
        let opcode = i16::from_le_bytes([frame[2], frame[3]]);
        assert_eq!(opcode, expected_opcode, "{label}: opcode mismatch");
        let checksum = i16::from_le_bytes([frame[length - 2], frame[length - 1]]);
        let expected = (length as i16) ^ crate::opcode::CHECKSUM_VALIDATION;
        assert_eq!(checksum, expected, "{label}: checksum mismatch");
    }

    // ---- property-based determinism over generated inputs -----------------
    //
    // There is no fixed F' yet, so the oracle for random inputs is a
    // deterministic re-encode equality: a pure encoder must produce identical
    // bytes for identical input, and a well-formed frame envelope every time.
    // After the fix lands, re-running these proves the non-bug encoders are
    // still byte-stable for the same inputs.

    fn seeded_string(seed: u32) -> String {
        match seed % 4 {
            0 => String::new(),
            1 => "A".to_string(),
            2 => "Admin".to_string(),
            _ => "LongTamerNameXYZ".to_string(),
        }
    }

    fn seeded_handler(seed: u32) -> u32 {
        match seed % 3 {
            0 => 0,
            1 => 11_000 + seed,
            _ => u32::MAX,
        }
    }

    fn seeded_i32(seed: u32) -> i32 {
        match seed % 4 {
            0 => 0,
            1 => seed as i32,
            2 => i32::MAX,
            _ => i32::MIN,
        }
    }

    fn seeded_buffs(seed: u32) -> Vec<ActiveBuffSnapshot> {
        match seed % 3 {
            0 => Vec::new(),
            1 => vec![ActiveBuffSnapshot {
                buff_id: (seed % u16::MAX as u32) as u16,
                buff_class: 1,
                skill_id: seed as i32,
                remaining_seconds: (seed % 600) as i32,
            }],
            _ => vec![
                ActiveBuffSnapshot {
                    buff_id: 500,
                    buff_class: 1,
                    skill_id: 8_001_001,
                    remaining_seconds: 30,
                },
                ActiveBuffSnapshot {
                    buff_id: 600,
                    buff_class: 1,
                    skill_id: 8_002_002,
                    remaining_seconds: -5,
                },
            ],
        }
    }

    /// Property: non-1006 encoders are byte-stable and emit well-formed frames
    /// across generated inputs (empty strings, boundary handlers, extreme ints).
    ///
    /// Validates: Requirements 3.1
    #[test]
    fn prop_non_1006_encoders_are_byte_stable() {
        for seed in 0..96u32 {
            let server_exp = ServerExperiencePacket {
                experience: seeded_i32(seed),
            };
            let a = server_exp.encode();
            assert_eq!(
                a,
                server_exp.encode(),
                "ServerExperiencePacket seed={seed} not stable"
            );
            assert_frame_invariants("ServerExperiencePacket", &a, game::SERVER_EXPERIENCE);

            let pick = PickItemPacket {
                appearance_handler: seeded_handler(seed),
                item_id: seeded_i32(seed),
                amount: (seed % i16::MAX as u32) as i16,
            };
            let b = pick.encode();
            assert_eq!(b, pick.encode(), "PickItemPacket seed={seed} not stable");
            assert_frame_invariants("PickItemPacket", &b, game::LOOT_ITEM);

            let bits = PickBitsPacket {
                appearance_handler: seeded_handler(seed),
                value: seeded_i32(seed),
            };
            let c = bits.encode();
            assert_eq!(c, bits.encode(), "PickBitsPacket seed={seed} not stable");
            assert_frame_invariants("PickBitsPacket", &c, game::PICK_BITS);

            let inventory = LoadInventoryPacket {
                inventory: InventorySnapshot {
                    bits: seed as i64,
                    size: (seed % 5) as u16,
                    items: if seed % 2 == 0 {
                        Vec::new()
                    } else {
                        vec![ItemRecord::new(5101 + seed as i32, (seed % 100) as i32)]
                    },
                },
                inventory_type: InventoryType::Inventory,
            };
            let d = inventory.encode();
            assert_eq!(
                d,
                inventory.encode(),
                "LoadInventoryPacket seed={seed} not stable"
            );
            assert_frame_invariants("LoadInventoryPacket", &d, game::LOAD_INVENTORY);
        }
    }

    /// Property: the 1006 unload/buff sub-variants (already-correct, not bug
    /// targets) are byte-stable and emit well-formed 1006 frames across
    /// generated inputs (empty names, empty buff lists, boundary handlers).
    ///
    /// Validates: Requirements 3.2
    #[test]
    fn prop_unload_and_buff_encoders_are_byte_stable() {
        for seed in 0..96u32 {
            let character = CharacterSummary {
                id: 1 + seed as u64,
                name: seeded_string(seed),
                general_handler: seeded_handler(seed),
                partner_handler: seeded_handler(seed + 1),
                x: seeded_i32(seed),
                y: seeded_i32(seed + 1),
                partner_x: seeded_i32(seed + 2),
                partner_y: seeded_i32(seed + 3),
                active_buffs: seeded_buffs(seed),
                partner_active_buffs: seeded_buffs(seed + 1),
                partner_active_debuffs: seeded_buffs(seed + 2),
                ..CharacterSummary::default()
            };
            let mob = MobSummary {
                id: 900 + seed as u64,
                handler: seeded_handler(seed),
                x: seeded_i32(seed),
                y: seeded_i32(seed + 1),
                active_debuffs: seeded_buffs(seed),
                ..MobSummary::default()
            };
            let drop = DropSummary {
                id: 990 + seed as u64,
                handler: seeded_handler(seed),
                x: seeded_i32(seed),
                y: seeded_i32(seed + 1),
                ..DropSummary::default()
            };

            let ut = UnloadTamerPacket {
                character: character.clone(),
            }
            .encode();
            assert_eq!(
                ut,
                UnloadTamerPacket {
                    character: character.clone()
                }
                .encode(),
                "UnloadTamerPacket seed={seed} not stable"
            );
            assert_frame_invariants("UnloadTamerPacket", &ut, game::LOAD_UNLOAD_ENTITY);

            let um = UnloadMobsPacket { mob: mob.clone() }.encode();
            assert_eq!(
                um,
                UnloadMobsPacket { mob: mob.clone() }.encode(),
                "UnloadMobsPacket seed={seed} not stable"
            );
            assert_frame_invariants("UnloadMobsPacket", &um, game::LOAD_UNLOAD_ENTITY);

            let ud = UnloadDropsPacket { drop: drop.clone() }.encode();
            assert_eq!(
                ud,
                UnloadDropsPacket { drop: drop.clone() }.encode(),
                "UnloadDropsPacket seed={seed} not stable"
            );
            assert_frame_invariants("UnloadDropsPacket", &ud, game::LOAD_UNLOAD_ENTITY);

            let lb = LoadBuffsPacket {
                character: character.clone(),
            }
            .encode();
            assert_eq!(
                lb,
                LoadBuffsPacket {
                    character: character.clone()
                }
                .encode(),
                "LoadBuffsPacket seed={seed} not stable"
            );
            assert_frame_invariants("LoadBuffsPacket", &lb, game::LOAD_BUFFS);

            let lmb = LoadMobBuffsPacket { mob: mob.clone() }.encode();
            assert_eq!(
                lmb,
                LoadMobBuffsPacket { mob: mob.clone() }.encode(),
                "LoadMobBuffsPacket seed={seed} not stable"
            );
            assert_frame_invariants("LoadMobBuffsPacket", &lmb, game::LOAD_BUFFS);
        }
    }

    #[test]
    fn cash_shop_buy_response_packet_encodes_multibuy_wire_layout() {
        let packet = CashShopBuyResponsePacket {
            result: 0,
            real_cash: 5000,
            bonus_cash: 200,
            success_product_ids: vec![10001, 10002],
            failed_product_ids: vec![10003],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::CASHSHOP_BUY);
        let mut reader = PacketReader::new(raw.payload);
        assert_eq!(reader.read_u16().unwrap(), 0);
        assert_eq!(reader.read_i32().unwrap(), 5000);
        assert_eq!(reader.read_i32().unwrap(), 200);
        assert_eq!(reader.read_u8().unwrap(), 2);
        assert_eq!(reader.read_i32().unwrap(), 10001);
        assert_eq!(reader.read_i32().unwrap(), 10002);
        assert_eq!(reader.read_u8().unwrap(), 1);
        assert_eq!(reader.read_i32().unwrap(), 10003);
    }

    #[test]
    fn cash_shop_crop_response_packet_encodes_wire_layout() {
        let packet = CashShopCropResponsePacket {
            result: 100,
            rtime: 0,
            cash_warehouse_slot: 3,
            slot_type: 0,
            slot_no: 5,
            item_type: 9001,
            item_count: 10,
            item_rate: 100,
            item_end_time: 0,
            remain_trade_limit_time: 0,
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::RETRIEVE_ACCOUNT_WAREHOUSE);
        let mut reader = PacketReader::new(raw.payload);
        assert_eq!(reader.read_i32().unwrap(), 100);
        assert_eq!(reader.read_i32().unwrap(), 0);
        assert_eq!(reader.read_u16().unwrap(), 3);
        assert_eq!(reader.read_u8().unwrap(), 0);
        assert_eq!(reader.read_u8().unwrap(), 5);
        assert_eq!(reader.read_u32().unwrap(), 9001);
        assert_eq!(reader.read_u32().unwrap(), 10);
        assert_eq!(reader.read_u8().unwrap(), 100);
        assert_eq!(reader.read_u32().unwrap(), 0);
        assert_eq!(reader.read_u32().unwrap(), 0);
    }

    #[test]
    fn cash_shop_warehouse_packet_encodes_wire_layout() {
        let packet = CashShopWarehousePacket {
            items: vec![ItemRecord::new(5001, 2), ItemRecord::new(5002, 1)],
        }
        .encode();

        let raw = PacketReader::from_frame(&packet).expect("frame should decode");
        assert_eq!(raw.packet_type, game::LOAD_ACCOUNT_WAREHOUSE);
        let mut reader = PacketReader::new(raw.payload);
        assert_eq!(reader.read_i16().unwrap(), 2);
        let item1 = reader.read_bytes(69).unwrap();
        assert_eq!(item1.len(), 69);
        let item2 = reader.read_bytes(69).unwrap();
        assert_eq!(item2.len(), 69);
    }
}
