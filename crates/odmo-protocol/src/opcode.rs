pub const CHECKSUM_VALIDATION: i16 = 6716;

pub mod account {
    pub const KEEP_CONNECTION: i16 = -3;
    pub const CONNECTION: i16 = -1;
    pub const CONNECTED_EVENT: i16 = -1;
    pub const CONNECTION_RESPONSE: i16 = -2;
    pub const LOGIN_REQUEST: i16 = 3301;
    pub const LOAD_SERVER_LIST: i16 = 1701;
    pub const CONNECT_CHARACTER_SERVER: i16 = 1702;
}

pub mod character {
    pub const KEEP_CONNECTION: i16 = -3;
    pub const CONNECTION: i16 = -1;
    pub const CONNECTION_RESPONSE: i16 = -2;
    pub const CREATE_CHARACTER: i16 = 1303;
    pub const DELETE_CHARACTER: i16 = 1304;
    pub const CHECK_NAME_DUPLICITY: i16 = 1302;
    pub const GET_CHARACTER_POSITION: i16 = 1305;
    pub const REQUEST_CHARACTERS: i16 = 1706;
    pub const CONNECT_GAME_SERVER: i16 = 1703;
    pub const CHARACTER_LIST: i16 = 1301;
    pub const CHARACTER_CREATED: i16 = 1306;
    pub const CHARACTER_CREATION_FAILED: i16 = 1307;
    pub const CHARACTER_DELETED: i16 = 1304;
    pub const CONNECT_GAME_SERVER_INFO: i16 = 1308;
}

pub mod game {
    pub const KEEP_CONNECTION: i16 = -3;
    pub const CONNECTION: i16 = -1;
    pub const CONNECTION_RESPONSE: i16 = -2;
    pub const COMPLEMENTAR_INFORMATION: i16 = 1001;
    pub const TAMER_MOVIMENTATION: i16 = 1004;
    // pGame::Sync (1006) is a multiplexed entity channel. The leading byte
    // discriminates the sub-packet: 1=mob, 3=tamer, 4=unload, 5=tamer-walk,
    // 6=digimon-walk, 16=buffs. Walk, load/unload and buff packets all ride
    // opcode 1006; the client switches on that first byte. (pGame::HpRate=1007
    // is a different, unrelated packet — do not put entity loads there.)
    pub const MAP_ENTITY: i16 = 1006;
    pub const LOAD_UNLOAD_ENTITY: i16 = 1006;
    pub const CONSIGNED_SHOP_ENTITY: i16 = 1008;
    pub const LOAD_BUFFS: i16 = 1006;
    pub const NOTICE_MESSAGE: i16 = 1010;
    pub const CHAT_MESSAGE: i16 = 1008;
    pub const CHAT_MESSAGE_RESPONSE: i16 = 1006;
    pub const INITIAL_INFORMATION: i16 = 1706;
    pub const INITIAL_INFO_RESPONSE: i16 = 1003;
    // Target HP-rate notice {u4 handler_id, u1 rate} -> event 42 (builder case
    // pGame::HpRate).
    pub const GAME_HP_RATE: i16 = 1007;
    // Level-up notice {u4 handler_id, u1 level} -> event 667 (builder case
    // pGame::LevelUp).
    pub const GAME_LEVEL_UP: i16 = 1019;
    // Battle-state notices {u4 handler_id} resolved through the entity table.
    pub const GAME_BATTLE_ON: i16 = 1034;
    pub const GAME_BATTLE_OFF: i16 = 1035;
    // Partner stat-restore notice {u4 handler_id; u4 x3} hitting three sequential
    // status setters.
    pub const GAME_HEAL_UP: i16 = 1023;
    // Ting visual notice {u4 handler_id}.
    pub const GAME_TING: i16 = 1053;
    // Emoticon notice {u4 handler_id; u4 emotion_id; u4 flag} resolved through
    // table 17 (CEmotionTable); flag 1 selects the aura variant.
    pub const GAME_EMOTICON: i16 = 1058;
    // Start-message UI notice {u1 count; count x {u2 len; len x bytes}} feeding
    // event 42 per message.
    pub const GAME_START_MESSAGE: i16 = 1047;
    // Death-state reset notice; client advances a single u2 and clears the
    // corpse/party-requeue state.
    pub const GAME_DIED_ALREADY: i16 = 1052;
    // Advert-boss notices: 1059 count/upsert {u4; u4; u4} (lookup-then-create
    // dispatch), 1061 die {u4; u4} (direct-removal dispatch). Create 1060 {u4 map;
    // u4; u4 monster} resolves CMapTable(33) and CMonsterTable(27) rows, pings
    // event 121 and registers the boss entry conditionally.
    pub const ADVERT_BOSS_COUNT: i16 = 1059;
    pub const ADVERT_BOSS_CREATE: i16 = 1060;
    pub const ADVERT_BOSS_DIE: i16 = 1061;
    // Sparse channel-status map fill: n x {u1 offset; u1 byte}, terminated by an
    // 0xFF offset; builds the 32-byte slot buffer for event 118.
    pub const SVR_CHANNEL_INFO: i16 = 1713;
    // Buddy-notice family. Modern parsers read modern UTF-8 strings
    // ({u2 len; len x bytes}) before dispatching into the friends UI:
    // 2401 add ({u1 flag; str}), 2403 ban (str), 2407 not-found (str),
    // 2408 on (str; see FRIEND_CONNECT), 2409 off (str),
    // 2410 name change ({u1 flag; str; str}). The receive table registers no
    // reader for 2402 or 2405, so those two only ever travel client to server.
    pub const BUDDY_ADD: i16 = 2401;
    /// `pBuddy::Delete` request — builder @0x00D9EEB0 sends one modern string.
    pub const BUDDY_DELETE: i16 = 2402;
    pub const BUDDY_BAN: i16 = 2403;
    /// `pBuddy::Memo` request — builder @0x00D9EFB0 sends tamer then memo, both
    /// modern strings.
    pub const BUDDY_MEMO: i16 = 2405;
    pub const BUDDY_NOT_FOUND: i16 = 2407;
    pub const BUDDY_OFF: i16 = 2409;
    pub const BUDDY_CHANGE_NAME: i16 = 2410;
    // Item-scan notice on the receive table: {u4; i64; u2 count; count x u2 rows}
    // feeding the scan list through event 96. The fork's C2S ItemScan request uses
    // 3987 on the send table, so this notice keeps a distinct constant.
    pub const ITEM_SCAN_NOTICE: i16 = 3988;
    pub const UPDATE_STATUS: i16 = 1043;
    pub const SERVER_EXPERIENCE: i16 = 1054;
    pub const SEALS: i16 = 1333;
    pub const AVAILABLE_RELATIONS: i16 = 2404;
    pub const FRIEND_CONNECT: i16 = 2408;
    pub const AVAILABLE_CHANNELS: i16 = 1713;
    pub const GUILD_INFORMATION: i16 = 2113;
    pub const GUILD_RANK: i16 = 2127;
    pub const GUILD_HISTORIC: i16 = 2128;
    pub const TIME_REWARD: i16 = 3106;
    pub const TAMER_ATTENDANCE: i16 = 3133;
    pub const CASH_SHOP_COINS: i16 = 3404;
    pub const MEMBERSHIP: i16 = 3414;
    pub const PICK_BITS: i16 = 3911;
    pub const PICK_ITEM_FAIL: i16 = 3913;
    pub const LOOT_ITEM: i16 = 3910;
    pub const UPDATE_MOVEMENT_SPEED: i16 = 9905;
    pub const LOAD_INVENTORY: i16 = 16009;
    pub const TAMER_XAI_RESOURCES: i16 = 16032;
    pub const XAI_INFO: i16 = 16033;
    pub const SYNC_CONDITION: i16 = 1070;
    pub const WARP_GATE: i16 = 1709;
    pub const LOCAL_MAP_SWAP: i16 = 1711;
    pub const CONSUME_ITEM: i16 = 3901;
    pub const MOVE_ITEM: i16 = 3904;
    pub const MOVE_ITEM_FAILURE: i16 = 3905;
    pub const SPLIT_ITEM: i16 = 3907;
    pub const ITEM_REMOVE: i16 = 3909;
    pub const NPC_PURCHASE: i16 = 3915;
    // C2S sell request; the client also parses 3916 frames on GS2C as the sell
    // result {i64 money; u1 count; count x {u1 kind; u4 serial; u2 stack rows}}
    // pinging ev89.
    pub const NPC_SELL: i16 = 3916;
    // Event notices: 3135 hot-time {r4 value} -> ev454 {3135, value}; 3139 item
    // term {u2} -> ev527 {3139, value}.
    pub const EVENT_HOT_TIME: i16 = 3135;
    pub const EVENT_ITEM_TERM: i16 = 3139;
    // Gotcha {r4 value} -> ev492 {value}.
    pub const ITEM_GOTCHA: i16 = 3959;
    // Guild member join {r4 value} -> ev261 {value}.
    pub const GUILD_INC_MEMBER: i16 = 2130;
    // Tamer option {r4 target handler; r4 value}; the client looks the target up
    // and applies the option through it.
    pub const TAMER_OPTION: i16 = 1313;
    // Skill-chip result {r4 handler; r4 skill_id}: the CSkillTable row keyed by
    // skill_id with kind 1 or 47 pops the buff message.
    pub const SKILL_RESULT_CHIP: i16 = 1122;
    // Cross-inventory slot-size update {r4; u2; r4; r4} -> ev491.
    pub const ITEM_CROSS_INVEN_SLOT_SIZE: i16 = 3966;
    // Move-clear removal {r4 handler; r4 item_id}: the client checks the CItemTable
    // row (+204) and pops the "Event\%s.nif" visual via 901112.
    pub const ITEM_MOVE_CLEAR: i16 = 3962;
    // Cash trade timeout {u2; r4; r4} -> ev467.
    pub const ITEM_CASH_TRADE_TIMEOUT: i16 = 3984;
    // Make result {u1 kind; u1 slot}: 0 clears a digimon skill-table row (offset 65
    // flag = -1), 1/3/4 fire ev586 with 1000/3000/4000 + slot, 2 fires ev68 {slot},
    // 5 fires ev586 {5000} and 9 fires ev68 {1}.
    pub const ITEM_MAKE_RESULT: i16 = 3983;
    // Tamer item-skill flow: 1330 apply {r4 index; r4 skill ref} through the client
    // list at +1092 plus a CSkillTable row check; 1331 expire and 1332 remove are
    // {u1 count; count x r4 skill ids} batches.
    pub const TAMER_ITEM_SKILL_APPLY: i16 = 1330;
    pub const TAMER_ITEM_SKILL_EXPIRE: i16 = 1331;
    pub const TAMER_ITEM_SKILL_REMOVE: i16 = 1332;
    // Pinochi vote {u2 count; count x {u2 slot; r4 value}}: each entry decrements a
    // digimon skill-table row (+4, zeroing the 69-byte row on underflow) and ends
    // with the 10019 event window.
    pub const EVENT_PINOCHI_VOTE: i16 = 3127;
    // Store-summon use {u1 count; count x {r4; r4}} -> ev466.
    pub const ITEM_STORE_SUMMON_USE: i16 = 3953;
    // Skill error {u1 kind; u2 code; dead u2 when kind = 255}: kind 95 resets only
    // on code 0, kind 100 closes the cast window on code 1 and other kinds fall
    // through to the digimon vtable reset.
    pub const SKILL_ERROR: i16 = 1106;
    // Event active banner {r4; u1 x4; u1 count; count x 71-byte rows} -> ev130;
    // each row is 68 opaque bytes plus a u2 and a u1 tail.
    pub const EVENT_ACTIVE: i16 = 3138;
    // Tamer skill success {r4 skill index; r4 argument}: the client looks up the
    // slide-list row and rebuilds the skill presentation struct (reads stop at the
    // two u4s).
    pub const TAMER_SKILL_SUCCESS: i16 = 1328;
    // Partner scale {r4 handler; u2 scale; r4 arg}: entity lookup + scale
    // application through the resolved digimon.
    pub const MASTER_PARTNER_SCALE: i16 = 9942;
    // Master/GM debug notices: 9963 conn count {r4} pops "Connect\n%d"; 9964 cash
    // payout {i64; i64} pops the premium text; 9972 GM quest copies a fixed
    // 0x598-byte blob straight into the client table at +0x766C.
    pub const MASTER_CONN_COUNT: i16 = 9963;
    pub const MASTER_PAY_CASH: i16 = 9964;
    pub const MASTER_GM_QUEST: i16 = 9972;
    // Guild name-flow notices: 2122 member level-up {u8 dead; s2 string} -> ev268
    // with the string; 2123 tamer-name change {s2; s2} -> ev275 with the first
    // string; 2124 deleted tamer {s2} -> ev274.
    pub const GUILD_MEMBER_LEVEL_UP: i16 = 2122;
    pub const GUILD_CHANGE_TNAME: i16 = 2123;
    pub const GUILD_DELETED_TAMER: i16 = 2124;
    // GM hatch stat table {r4 x6 succ; r4 x5 fail; r4 x5 break; r4 x5 succ-average
    // floats}: the client renders _HatchTry.txt and opens notepad.
    pub const GM_HATCH_TRY: i16 = 9966;
    // Pinochi get-info {r4 x11}: one value lands at container +0x48, five at
    // +0x60.+0x70 and five at +0x4C.+0x5C; the render list is refreshed through the
    // 0x3138 slots and the fallback path pings ev42.
    pub const EVENT_PINOCHI_GET_INFO: i16 = 3126;
    // Party exp split {i64 x3; r4; i64 x3; r4; s2 label} -> ev196; the label string
    // is consumed but never reaches the event struct.
    pub const PARTY_EXP: i16 = 2311;
    // Equip flow {u2 handler; u1 kind; r4 x3; u1 dead; u1}: kind 5 runs the equip
    // slot push through the entity slot and kind 10 swaps the digimon aura
    // (item-table row kind 31 with the T_Aura visual pair).
    pub const TAMER_EQUIP_ITEM: i16 = 1310;
    // Master user-info lookup miss {s2 name}: the client answers the GM console
    // with the "(name) is not found" line on ev42.
    pub const MASTER_USER_INFO_NOT_FOUND: i16 = 9921;
    // Master second-password reset {u2 code}: 0 resolves the misc table id 34
    // (CHAT_SECOND_PASSWORD_RESET_SUCCESS) and 1 id 17 (CHAT_NOT_EXIST_ID), both
    // popped as an ev42 console line.
    pub const MASTER_INIT_2ND_PASS: i16 = 9945;
    // Master kill-stat dump {u4 count; count x i64}: per-entry money totals are
    // rendered into _KillMon.txt and popped open in notepad.
    pub const MASTER_LIE_KILL_MONSTER: i16 = 9958;
    // Master cash-shop insert result {u2 code}: resolves misc ids 30 (added to cash
    // shop), 23 (item code missing), 29 (store full) and more, popped as an ev42
    // console line.
    pub const MASTER_INSERT_ITEM_TO_SHOP: i16 = 9944;
    // Master cheat-level result {u4 result; s2 name}: result 2 resolves misc id 24
    // (CHAT_CHEAT_LEVEL_CHANGED), other values resolve ids 23 and friends as
    // failure lines, popped through ev42.
    pub const MASTER_CHEAT_LEVEL: i16 = 9932;
    // GM cooldown reset {r4 x102 fixed blob}: entry i adjusts the cooldown modifier
    // of slot i+1 across the tamer and partner skill icon lists; the tail pings
    // ev42 through a static text.
    pub const MASTER_COOL_RESET: i16 = 9975;
    // Master chat ban {u4 duration; s2 name}: 0 resolves misc id 31
    // (CHAT_UNLIMIT_SERVER_BAN_MESSAGE), other values resolve id 29
    // (CHAT_LIMIT_SERVER_BAN_MESSAGE) with the duration split into hours through
    // the #Hour# template.
    pub const MASTER_CHAT_BAN_USER: i16 = 9983;
    // Master skill-stat init {u4 kind; s2; s2}: renders the misc id 37 line with
    // the "%s tn(%s) dn(%s)" template, then walks the per-slot stat reset switch
    // before the ev42 console pop.
    pub const MASTER_INIT_SKILL_STAT: i16 = 9941;
    // Master user info {s2; s2; u4; u2; u1; r4; r4(hi); u1}: numeric and string
    // fields feed the console info branches, each popping through ev42 (semantics
    // unresolved).
    pub const MASTER_USER_INFO: i16 = 9920;
    // Master rare-machine census {u4 count; count x u4 item_code}: count 0 pops misc id
    // 39 (CHAT_RAREMACHINE_CHEAT_WRONG_INPUT_DATA) and codes that miss the item
    // table pop misc id 38 (WRONG_RECV_DATA) through ev42; hits are rolled into
    // Cheat_RareMachine.txt and opened in notepad.
    pub const MASTER_MACHINE_RUN: i16 = 9981;
    // Master item-rank census {u2 selector; u4 count; count x u4 item_code}: a zero
    // selector ends the frame and pops misc id 36 (CHAT_ITEMRANK_CHEAT_WRONG_INPUT_DATA);
    // codes that miss the item table pop misc id 33
    // (CHAT_CHEAT_MESSAGE_ITEMRANK_ERROR) through ev42; hits roll into
    // Cheat_ItemRank.txt + notepad.
    pub const MASTER_RANK_ITEM_RUN: i16 = 9982;
    // Skill range-persist end {u4 id}: drops the tracked range-persist effect entry
    // whose entity carries the id and respawns its tail particle.
    pub const SKILL_RANGE_PERSIST_END: i16 = 1124;
    // Skill chip cooldown {u4 tamer_slot; u4 partner_slot}: the tamer skill icon
    // list is scanned for the first slot and the partner list for the second, both
    // cooldowns re-anchored from the skill-table row.
    pub const SKILL_CHIP_COOL_TIME: i16 = 1121;
    // Skill remove chip {u4 chip_id}: erases the chip from the tamer skill icon
    // list, zeroes its chip slots in the +0x74C7 table, and stops the row effect
    // when the CSkillTable row carries one.
    pub const SKILL_REMOVE_SKILL_CHIP: i16 = 1119;
    // Skill add chip {u4 slot; u4 chip; u4 item_id}: anchors the entity, registers
    // the chip in the chip table, feeds the buff particle row (Buff %s.nif) and
    // pops the console line.
    pub const SKILL_ADD_SKILL_CHIP: i16 = 1118;
    // Skill active effect {u4 slot; u4 skill_id; u2 level}: resolves the entity and
    // the CSkillTable row, then branches on the handler kind popping message lines
    // through ev42.
    pub const SKILL_ACTIVE_SKILL_EFF: i16 = 1117;
    // Skill monster buff {u4 slot; u4 monster_key; u4 param}: resolves the entity
    // and the CMonsterTable row (kind 4/13/14 switch), then applies the 0x9CE2 buff
    // text and pops the console line through ev42.
    pub const SKILL_MONSTER_BUFF: i16 = 1115;
    // Skill monster skill-effect {u4 caster; u4 skill_id; u4 count; count x {u4
    // handler; u4 x; u4 y}}: the trailing reads exist only on the CMonsterTable row
    // kind-18 branch, spawning the raid dot-damage circle per target; kind 19 stops
    // the effect, other kinds pop the ev42 error line.
    pub const SKILL_MONSTER_SKILL_EFFECT: i16 = 1123;
    // Skill around-skill {u4 caster; u4 skill_id; u2 count; u4 param; count x {u1
    // flag; u4 handler; u4 value; u1 level}}: every hit is resolved through the
    // handler and pushed into the damage dispatch together with the param; flag
    // hits also pop the wide-skill console line (event 42) and refresh the caster
    // marker. CMonsterTable row kinds 16 and 17 are a variant that consumes an
    // extra leading count/list block first (unresolved).
    pub const SKILL_AROUND_SKILL: i16 = 1110;
    // Skill range skill-damage {u4 caster; u2 atk_count; u1 flag; u4 target;
    // atk_count x {u4 actor; u1 pad; blob40; stat-pair list (u1 slot; u4 value) x
    // n, closed by a u1 sentinel >= 0x12}; u2 tgt_count; tgt_count x {u4 handler;
    // blob40}}: rows feed the damage dispatch.
    pub const SKILL_RANGE_SKILL_DMG: i16 = 1116;
    // Master connection-count info {u2 group_count; group_count x chain of (u1
    // code; u2 count) closed by a u1 0xFF terminator}: each segment renders the
    // "%04d - %02d : %03d" line through misc id 3334 and pops event 42.
    pub const MASTER_CONN_COUNT_INFO: i16 = 9938;
    // Item capsule-to-item {u4 count; per row {u1 slot; u2 code; u4 param; u4
    // next_count} while next_count nonzero}: each row inserts into the +0x9DC list
    // through event 11, and a scene item 901107/901108 then triggers the 901109
    // cast.
    pub const ITEM_CAPSULE_TO_ITEM: i16 = 3937;
    // Item timeout {u1 kind_cc; u2 code; u4 row_key; u1 kind_c8}: resolves the
    // CItemTable row; rows of kind 3 reaching a special +0x18 value drop the
    // matching slot from the +0x7664 list, while other rows stop the 0x4E3D effect,
    // open the timed-removal window filled with the two kinds plus the code, and
    // pop event 687.
    pub const ITEM_TIMEOUT: i16 = 3933;
    // Item party-crop {u2 code; u4 param; u2 sub_out; u1 mode; s2 reason; u1 signed
    // member count; count x {u1 kind; s2 name}}: each member row becomes a list
    // node consumed by the delete-list handler, and event 197 pops the code
    // afterwards.
    pub const ITEM_PARTY_CROP: i16 = 3939;
    // Skill around-skill animation {u1 flag; u4 seconds}: nonzero flags apply the
    // around-skill cooldown animation, converting the value to float seconds
    // against the frame rate and popping the warning popup. A zero flag delegates
    // to an embedded animation-state parser first (u4 pair plus a chained u4
    // handler list, unresolved tail) and skips the cooldown path.
    pub const SKILL_AROUND_SKILL_ANI: i16 = 1111;
    // Digimon data-change {u4 tag; u1 a; u1 b; u4 magic; tag-body}: the magic must
    // read 30803 (0x7853) or the client pops the reject line through event 82. Tag
    // 0 - window payload (u1; u2; u1; u2) through event 79; tag 1 - rows payload
    // {u2; 14-byte record; 14-byte record; u2; 14-byte record; 14-byte record}
    // through event 80; tag 2 - blobs payload {u1 count; count x 28-byte record; u1
    // count; count x 28-byte record} through event 81.
    pub const DIGIMON_DATA_CHANGE: i16 = 3242;
    // Game macro-question {u2 a; u2 b; u1 c; u4 d; s2 prompt; u2 count; count x {u4
    // id; s2 text}; u4; u4; u4; u4}: builds the GS2C_RES_MACRO_IMAGE_QUESTION
    // dialog (each answer row is an nsMacroProtector::stAnswerForImageQuestion) and
    // pops event 357 (+).
    pub const GAME_MACRO_QUESTION: i16 = 1064;
    // Master money set {u8 money}: stores the amount into the local wallet
    // singleton slot (+60) and pings event 38.
    pub const MASTER_SET_MONEY: i16 = 9907;
    // Master guild-skill reset with an empty payload: the parser performs no cursor
    // reads and only pings event 284.
    pub const MASTER_GM_GUILD_SKILL_RESET: i16 = 9990;
    // Event masters-matching ping with an empty payload: no cursor reads, the
    // parser fires event 572 carrying {3125, 0}.
    pub const EVENT_MASTERS_MATCHING: i16 = 3125;
    // Cash shop results: BuyRequest {u4 a; u4 b} through event 433; Gift {u2 a; u4
    // b; u4 c; s2 name; u4 d} through event 430; Cart a skipped 48-byte blob
    // through event 426.
    pub const CASH_SHOP_BUY_REQUEST: i16 = 3401;
    pub const CASH_SHOP_GIFT: i16 = 3403;
    pub const CASH_SHOP_CART: i16 = 3405;
    // Channel-dispatch satellites: DungeonFail {u4 skipped} flags the map singleton
    // and settles without an event line ; SelectPortalFailure {u4 skipped} fires
    // event 124 only when the portal-error flag is up ; NatualExp {u1 a; u1 b; u2
    // c} forwards the fields through event 576 carrying the opcode id.
    pub const MONSTER_NATUAL_EXP: i16 = 1602;
    // Raid ranking roster {u4 count; count x {u4 a; s2 name1; s2 name2; u4 b; u4
    // c}}; each row becomes a 72-byte list node consumed by the ranking view and
    // event 629 pops when the list settles.
    pub const MONSTER_MSG_RAID_RANKING_LIST: i16 = 1604;
    pub const SVR_SELECT_PORTAL_FAILURE: i16 = 1710;
    pub const INSMAP_DUNGEON_FAIL: i16 = 4121;
    // Event satellites: HotTimeEvent {u1; u1; u1; u1; u4; u4} forwards the fields
    // through the 453 event (the accumulator starts as the opcode id with both low
    // op bytes zero) ; EventType {s2 name; u4 a; u2 b} through event 528. Registry
    // name "Event::HotTimeEvent"; the older notice above is 3135.
    pub const EVENT_HOT_TIME_EVENT: i16 = 3134;
    pub const EVENT_ITEM_TYPE: i16 = 3140;
    // Pinochi item delete {r4 key; x3 {u2 count; count x {u2 slot; r4 value}}}:
    // block 1 decrements digimon skill-table rows (zeroing the 69-byte row on
    // underflow), blocks 2/3 are limit-checked and popped to ev59/ev60, and the
    // tail walks the special-event NPC list.
    pub const EVENT_PINOCHI_ITEM_DELETE: i16 = 3128;
    // Master user list {u2 count; count x {s2; s2}}: rows render into uList.txt and
    // ping ev42 per line; the tail opens the file in notepad.
    pub const MASTER_USER_LIST: i16 = 9922;
    pub const REPURCHASE_ITEM: i16 = 3978;
    pub const LOAD_NPC_REPURCHASE_LIST: i16 = 3979;
    pub const DIGI_SUMMON_SYNC_REQUEST: i16 = 3652;
    pub const DIGI_SUMMON_SYNC_RESPONSE: i16 = 3652;
    pub const EXITEM_MOVE: i16 = 0x0ED9;
    pub const EXITEM_BATCH_MOVE: i16 = 0x0EDA;
    pub const EXITEM_SORT: i16 = 0x0EDB;
    pub const EXITEM_USE: i16 = 0x0EDC;
    pub const EMOTICON: i16 = 1076;
    pub const WHISPER_MESSAGE: i16 = 1009;
    /// Op 1056 in the modern build: the reader at 0x00DA6F50 takes one dword and,
    /// only for the value 1, opens a menu dialog (event 42). It never reads text,
    /// so it cannot be the shout request; that opcode stays unresolved.
    pub const UNKNOWN_1056: i16 = 1056;
    pub const MEGAPHONE_MESSAGE: i16 = 1074;
    pub const TAMER_REACTION: i16 = 1058;
    pub const PARTNER_STOP: i16 = 1033;
    pub const PARTNER_STOP_RESPONSE: i16 = 1033;
    pub const PARTNER_ATTACK: i16 = 1013;
    pub const PARTNER_ATTACK_RESPONSE: i16 = 1013;
    pub const ATTACK_MISS: i16 = 1014;
    pub const PARTNER_SKILL: i16 = 1015;
    pub const PARTNER_SKILL_RESPONSE: i16 = 1015;
    pub const KILL_ON_HIT: i16 = 1020;
    pub const KILL_ON_SKILL: i16 = 1021;
    pub const PARTNER_SKILL_ERROR: i16 = 1105;
    pub const EVOLUTION: i16 = 1028;
    pub const PARTNER_EVOLUTION: i16 = 1028;
    pub const EVOLUTION_FAILURE: i16 = 1029;
    pub const PARTNER_SWITCH: i16 = 1041;
    pub const PARTNER_SWITCH_RESPONSE: i16 = 1041;
    pub const PARTNER_DELETE: i16 = 1042;
    pub const PARTNER_DELETE_RESPONSE: i16 = 1042;
    pub const CAPSULE_EVOLUTION_SLOT_RESULT: i16 = 3238;
    pub const EVOLUTION_ARMOR_UNLOCKED: i16 = CAPSULE_EVOLUTION_SLOT_RESULT;
    pub const EVOLUTION_UNLOCK: i16 = 1055;
    pub const EVOLUTION_UNLOCK_RESPONSE: i16 = 1055;
    pub const RIDE_MODE_START: i16 = 1325;
    pub const RIDE_MODE_STOP: i16 = 1326;
    pub const OPEN_RIDE_MODE: i16 = 1063;
    pub const SET_TARGET: i16 = 1016;
    pub const STAT_UP: i16 = 1030;
    pub const REFRESH_SCREEN: i16 = 1046;
    pub const AWAY_TIME: i16 = 1069;
    pub const DIGIMON_CHANGE_NAME: i16 = 3206;
    pub const HATCH_INSERT_EGG: i16 = 1036;
    pub const HATCH_INCREASE: i16 = 1037;
    pub const HATCH_FINISH: i16 = 1038;
    pub const HATCH_REMOVE_EGG: i16 = 1039;
    pub const HATCH_FAILURE: i16 = 1040;
    pub const HATCH_BACKUP_INSERT: i16 = 3946;
    pub const HATCH_BACKUP_CANCEL: i16 = 3947;
    pub const INCUBATOR_CLOSE: i16 = 3948;
    pub const DIGIMON_ARCHIVE_MOVE: i16 = 3201;
    pub const DIGIMON_ARCHIVE_LIST: i16 = 3204;
    pub const DIGIMON_ARCHIVE_SWAP: i16 = 3243;
    pub const INVENTORY_SORT: i16 = 3980;
    pub const ITEM_IDENTIFY: i16 = 3968;
    pub const ITEM_CRAFT: i16 = 3982;
    pub const ITEM_REROLL: i16 = 3969;
    pub const ITEM_SOCKET_IN: i16 = 3926;
    pub const ITEM_SOCKET_OUT: i16 = 3927;
    pub const ITEM_SOCKET_IDENTIFY: i16 = 3929;
    pub const ITEM_RETURN: i16 = 3923;
    pub const ITEM_SCAN: i16 = 3987;
    pub const LOAD_GIFT_STORAGE: i16 = 3935;
    pub const GIFT_STORAGE_RETRIEVE: i16 = 3936;
    pub const LOAD_REWARD_STORAGE: i16 = 16001;
    pub const RECOMPENSE_GAIN: i16 = 16002;
    pub const TAMER_SHOP_OPEN: i16 = 1511;
    pub const TAMER_SHOP_CLOSE: i16 = 1512;
    pub const TAMER_SHOP_BUY: i16 = 1513;
    pub const CONSIGNSHOP_OPEN: i16 = 1516;
    pub const CONSIGNSHOP_VIEW: i16 = 1520;
    pub const CONSIGNSHOP_PURCHASE: i16 = 1518;
    pub const CONSIGNSHOP_RETRIEVE: i16 = 1517;
    pub const CASHSHOP_BUY: i16 = 3413;
    pub const CASHSHOP_RELOAD: i16 = 3404;
    pub const QUEST_AVAILABLE_LIST: i16 = 11009;
    pub const QUEST_ACCEPT: i16 = 11002;
    pub const QUEST_DELIVER: i16 = 11004;
    pub const QUEST_GIVE_UP: i16 = 11003;
    pub const QUEST_UPDATE: i16 = 11001;
    pub const QUEST_GOAL_UPDATE: i16 = 11001;
    pub const QUEST_DAILY_UPDATE: i16 = 11006;
    // Quest cancellation receipt {u2 count; count x {u4 kind; u2 slot; u4 a; u4
    // b}}: rows with kind 2 or 9 take the direct-cancel path, the rest land on the
    // secondary list and pop events 59/60 with the collected lists.
    pub const QUEST_CANCELED: i16 = 11008;
    pub const DIE_CONFIRM: i16 = 1022;
    // Buff family (0xD8D7xx/0xDCxxxx): fixed dispatch envelopes on resolved target
    // entities. CreateBuff {u4 target; u2 a; u2 b; u4 c; u4 d}; BuffChange adds a
    // third u2 and a second u4; BuffClear carries {u4 target; u2 count; count x
    // u2}; ReturnFail is {u1 kind; u2 code; u4 upgrade when kind==1};
    // AbnormalDamage is a 20-byte envelope {u4 handler; u4 actor; u2 target; u1
    // flag; u4 pair; u1 kind}; BuffSkill is {u4 target; u2 slot; u1 flag; u4 a; u4
    // b}; Stun is {u4 actor; u4 skill; u2 duration; u4 value}.
    pub const BUFF_CREATE: i16 = 4000;
    pub const BUFF_BUFF_CHANGE: i16 = 4001;
    pub const BUFF_BUFF_CLEAR: i16 = 4002;
    pub const BUFF_RETURN_FAIL: i16 = 4009;
    pub const BUFF_ABNORMAL_DAMAGE: i16 = 4011;
    pub const BUFF_BUFF_SKILL: i16 = 4012;
    pub const BUFF_STUN: i16 = 4013;
    pub const REMOVE_BUFF: i16 = 4005;
    pub const DAMAGE_SKIN_CHANGE: i16 = 3153;
    pub const SEAL_OPEN: i16 = 3971;
    pub const SEAL_CLOSE: i16 = 3972;
    // Seal leader assignment: request carries a u2 card code;
    // confirmation returns {u4 tamer_id, u2 leader_id}.
    pub const SEAL_SET_LEADER: i16 = 3232;
    // Seal leader removal: request carries a u2 card code; server confirms
    // with {u4 tamer_id, u2 card_id}. Opcode 3233 is shared: an empty
    // payload is the encyclopedia open request instead.
    pub const SEAL_REMOVE_LEADER: i16 = 3233;
    pub const SEAL_SET_FAVORITE: i16 = 1334;
    // Encyclopedia open request: opcode 3233 with an empty payload.
    pub const ENCYCLOPEDIA_LOAD: i16 = 3233;
    // Encyclopedia list: a 6-byte payload {u4, u2} is a level update;
    // otherwise {u4 count, count × record{u4 id, u2 level, u8 slot_mask,
    // 5×u2 enchant(AT,BL,CT,EV,HP), s2 size, u1 rewarded}}, 27-byte records.
    pub const ENCYCLOPEDIA_LOAD_RESPONSE: i16 = 3234;
    // Encyclopedia reward claim {u4 digimon_id};
    // response {u4 item_id, u2 item_count}.
    pub const ENCYCLOPEDIA_GET_REWARD: i16 = 3235;
    // Encyclopedia deck use {u4 deck_id} (1001..).
    pub const ENCYCLOPEDIA_DECK_USE: i16 = 3236;
    // Encyclopedia deck buff {u2 option_index}, ENCY_MAX_OPTION == 4.
    pub const ENCYCLOPEDIA_DECK_BUFF: i16 = 3237;
    // Care slot size notice {u2 slot_count} -> event 407.
    pub const DIGIMON_CARE_SLOT_SIZE: i16 = 3205;
    // Battle tag result notices; both payloadless.
    pub const DIGIMON_BATTLE_TAG_SUCCESS: i16 = 3209;
    pub const DIGIMON_BATTLE_TAG_FAILURE: i16 = 3210;
    // Skill point init {u1 count, count × 28-byte record} -> event 477.
    pub const DIGIMON_SKILL_POINT_INIT: i16 = 3213;
    // Incubator facilitation reply {u1 kind, u4 arg} -> event 412.
    pub const DIGIMON_FACILITATION: i16 = 3227;
    // Incubator error notice; parser consumes a u4 and drops it. Field meaning
    // unresolved.
    pub const DIGIMON_INCUBATER_ERROR: i16 = 3228;
    // Incubator list rows: exactly 3 rows of {u4; u4; u4}, each row handed to the
    // slot builder, then an empty event-408 dispatch.
    pub const DIGIMON_INCUBATER_LIST: i16 = 3226;
    // Incubator extraction result {u1} -> event 413.
    pub const DIGIMON_EXTRACTION: i16 = 3229;
    // Incubator slot change {u4 slot; u4 value} -> event 411; slot -1 makes the
    // client close the incubator view instead.
    pub const DIGIMON_INCUBATER_SLOT_CHANGE: i16 = 3231;
    pub const OTHER_TAMER_DETAIL_INFO_REQUEST: i16 = 16070;
    pub const OTHER_TAMER_DETAIL_INFO_RESPONSE: i16 = 16071;
    pub const ARENA_DAILY_POINTS: i16 = 4131;
    pub const ARENA_DAILY_RANKING: i16 = 4130;
    // Ranking requests and their responses share one id each. The client sends
    // `ArenaRequestRank`/`ArenaRequestOldRank` with a rank-type byte (week,
    // month, season), `ArenaRequestFame` with a season number (0 = latest) and
    // `ArenaRequestFameList` with no payload.
    pub const ARENA_REQUEST_RANK: i16 = 16023;
    pub const ARENA_REQUEST_OLD_RANK: i16 = 16024;
    pub const ARENA_REQUEST_FAME: i16 = 16025;
    pub const ARENA_REQUEST_FAME_LIST: i16 = 16026;
    pub const DUNGEON_NEXT_STAGE: i16 = 4126;
    pub const DUNGEON_SURRENDER: i16 = 4127;
    pub const BURNING_EVENT: i16 = 3132;
    pub const DAILY_CHECK_EVENT: i16 = 3136;
    pub const DAILY_CHECK_EVENT_REQUEST: i16 = 3137;
    pub const JOIN_EVENT_QUEUE: i16 = 3124;
    pub const REGION_UNLOCK: i16 = 1051;
    pub const SET_TITLE: i16 = 15;
    pub const CHANGE_TAMER_MODEL: i16 = 1314;
    pub const TAMER_NAME_CHANGE: i16 = 1311;
    pub const RARE_MACHINE_OPEN: i16 = 3956;
    pub const RARE_MACHINE_RUN: i16 = 3957;
    pub const PARTY_INVITE: i16 = 2301;
    pub const PARTY_INVITE_RESPONSE: i16 = 2302;
    pub const PARTY_CHAT: i16 = 2304;
    pub const PARTY_JOIN: i16 = 2305;
    pub const PARTY_KICK: i16 = 2306;
    pub const PARTY_LEAVE: i16 = 2307;
    pub const PARTY_CHANGE_MASTER: i16 = 2308;
    pub const PARTY_CHANGE_LOOT: i16 = 2309;
    pub const PARTY_MEMBER_LIST: i16 = 2310;
    pub const PARTY_MEMBER_DISCONNECTED: i16 = 2312;
    pub const PARTY_MEMBER_INFO: i16 = 2313;
    pub const PARTY_MEMBER_MAP_CHANGE: i16 = 2315;
    pub const PARTY_MEMBER_DIGIMON_CHANGE: i16 = 2316;
    pub const PARTY_DISMISS: i16 = 2317;
    pub const PARTY_CREATED: i16 = 2319;
    pub const PARTY_MEMBER_POSITION: i16 = 2320;
    pub const PARTY_MEMBER_BUFF_CHANGE: i16 = 2323;
    pub const GUILD_CREATE: i16 = 2101;
    pub const GUILD_DELETE: i16 = 2102;
    pub const GUILD_INVITE: i16 = 2109;
    pub const GUILD_INVITE_ACCEPT: i16 = 2103;
    pub const GUILD_INVITE_RESPONSE_ACCEPT: i16 = 2108;
    pub const GUILD_INVITE_DENY: i16 = 2105;
    pub const GUILD_INVITE_FAIL: i16 = 2110;
    pub const GUILD_MEMBER_CONNECT: i16 = 2111;
    pub const GUILD_MEMBER_DISCONNECT_NOTIFY: i16 = 2112;
    pub const GUILD_KICK: i16 = 2106;
    pub const GUILD_LEAVE: i16 = 2107;
    pub const GUILD_MESSAGE: i16 = 2114;
    pub const GUILD_NOTICE: i16 = 2126;
    pub const GUILD_HISTORY: i16 = 2128;
    pub const GUILD_SET_TITLE: i16 = 2129;
    pub const TRADE_REQUEST: i16 = 1501;
    pub const TRADE_REQUEST_SUCCESS: i16 = 1501;
    pub const TRADE_REQUEST_ERROR: i16 = 1507;
    pub const TRADE_ACCEPT: i16 = 1502;
    pub const TRADE_CANCEL: i16 = 1506;
    pub const TRADE_ADD_ITEM: i16 = 1508;
    pub const TRADE_REMOVE_ITEM: i16 = 1531;
    pub const TRADE_REMOVE_ITEM_RESPONSE: i16 = 1519;
    pub const TRADE_ADD_MONEY: i16 = 1509;
    pub const TRADE_CONFIRM: i16 = 1503;
    pub const TRADE_FINAL_CONFIRMATION: i16 = 1504;
    pub const TRADE_LOCK: i16 = 1532;
    pub const TRADE_UNLOCK: i16 = 1505;
    pub const SEASON_PASS_DETAILS: i16 = 641;
    pub const SEASON_PASS_PURCHASE_EXP: i16 = 643;
    pub const SEASON_PASS_MISSION_REWARD: i16 = 644;
    pub const SEASON_PASS_SEASON_REWARD: i16 = 645;
    pub const CHANGE_CHANNEL: i16 = 1050;
    pub const CHANNEL_SWITCH_CONFIRM: i16 = 1703;
    pub const TAMER_SHOP_LIST: i16 = 1515;
    pub const CONSIGNSHOP_WAREHOUSE: i16 = 1523;
    pub const CONSIGNSHOP_WAREHOUSE_RETRIEVE: i16 = 1521;
    pub const CASHSHOP_BUY_HISTORY: i16 = 3412;
    pub const ADD_FRIEND: i16 = 2401;
    pub const GUILD_AUTHORITY_MASTER: i16 = 2119;
    pub const GUILD_AUTHORITY_SUBMASTER: i16 = 2118;
    pub const GUILD_AUTHORITY_MEMBER: i16 = 2116;
    pub const GUILD_AUTHORITY_NEW_MEMBER: i16 = 2115;
    pub const GUILD_AUTHORITY_DATS: i16 = 2117;
    pub const SPIRIT_TO_DIGIMON: i16 = 3239;
    pub const DIGIMON_TO_SPIRIT: i16 = 3240;
    pub const UNKNOWN_DIGIMON_3241: i16 = 3241;
    pub const DIGI_SUMMON_PURCHASE: i16 = 3651;
    pub const DIGI_SUMMON_PURCHASE_RESPONSE: i16 = 3651;
    // Digivice combine and union combine flows, plus the random box draw.
    pub const DIGI_COMBINE_SYNC: i16 = 3661;
    pub const DIGI_COMBINE: i16 = 3662;
    pub const DIGI_COMBINE_REWARD: i16 = 3663;
    pub const UNION_COMBINE_SYNC: i16 = 4301;
    pub const UNION_COMBINE: i16 = 4302;
    pub const UNION_COMBINE_REWARD: i16 = 4303;
    // Modern D-Unit / Union hacking and init flows.
    pub const UNION_HACK_OPEN_REQUEST: i16 = 4311;
    pub const UNION_HACK_OPEN_RESPONSE: i16 = 4311;
    pub const UNION_HACK_MODIFY_REQUEST: i16 = 4312;
    pub const UNION_HACK_MODIFY_RESPONSE: i16 = 4312;
    pub const UNION_INIT_DATA: i16 = 4313;
    pub const RANDOM_BOX_LIST: i16 = 16067;
    pub const RANDOM_BOX_PURCHASE: i16 = 16068;
    pub const LOAD_ACCOUNT_WAREHOUSE: i16 = 3930;
    pub const RETRIEVE_ACCOUNT_WAREHOUSE: i16 = 3931;
    pub const PARTY_MEMBER_DISCONNECT: i16 = 2312;
    pub const MONSTER_RESPAWN_TIMER: i16 = 16064;
    pub const JUMP_BOOSTER: i16 = 1057;
    pub const SKILL_LEVEL_UP: i16 = 1104;
    pub const TAMER_CHARGE_XCRYSTAL: i16 = 16030;
    pub const TAMER_CONSUME_XCRYSTAL: i16 = 16031;
    // GameTwo satellites: DoorObjectOpen {u4 monster id; u1 opened} drives the
    // door-actor spawn and pings event 42 with a formatted log string plus event
    // 538 with the spawned node ; GameGuardCheck {u4 a; u4 b; u2 count; count x {u1
    // kind; u4 c; u4 d; u1 e}} matches the leading id against the selected/main
    // monster and pings event 666 when the guard path settles ; SkillAction {u4
    // kind} triggers a skill action only for kind 4 ; XGuageUpdate {u2 a; s2 name;
    // u4 b; u4 c} through event 423.
    // AllMsg {u2 case; case body}; the case selects the announcement family and
    // each family carries its own trailing fields: 1 = {s2 tamer; u4 item; u4
    // level}, 1605 = {u4 monster; s2 tamer; u4 digimon; u4 item}, 1606 and 1609 =
    // {u4 monster; u4 skipped}. Cases outside that set consume the case word and
    // nothing else.
    pub const GAMETWO_ALL_MSG: i16 = 16006;
    // MonsterAreaAttack {u4 caster; u4 monster id; per-kind body}; the body is
    // selected by the monster row kind: 0 = header only, 1 = {u4 count; count x
    // u4 target}, 2/4/7 = {u4 count; count x {u4 a; u4 b}}.
    pub const GAMETWO_MONSTER_AREA_ATTACK: i16 = 16010;
    pub const GAMETWO_HACK_SHIELD_CHECK: i16 = 16011;
    pub const GAMETWO_DOOR_OBJECT_OPEN: i16 = 16007;
    pub const GAMETWO_GAME_GUARD_CHECK: i16 = 16011;
    pub const GAMETWO_SKILL_ACTION: i16 = 16012;
    pub const GAMETWO_X_GUAGE_UPDATE: i16 = 16034;
    // RaidSkillChain {u4 handler; u4 skill; u2 count; count x {u4 value; u4 next;
    // i32 x; i32 y}}; the last link closes the walk with next == 0.
    pub const GAMETWO_RAID_SKILL_CHAIN: i16 = 16027;
    // RegionBuff {u4 a; u8 blob; u1 flag; u2 count; count x 69-byte rows};
    // the flag selects which buffer view receives the rows.
    pub const GAMETWO_REGION_BUFF: i16 = 16038;
    // TimeCharge family. ItemList {u4 a; u2 b; u4 c; u4 d; u8 e; 69-byte charge
    // row; u4 f} through event 510 ; Normal {u4 a; u4 b; u4 c; 69-byte charge row;
    // u8 d; u2 count; count x {u2 e; u2 f}} building a slot list through event 511
    // ; Immediate {u4 a; u2 b; u4 c; 69-byte charge row} through event 513 ; Cancel
    // with the same body through event 515.
    pub const GAMETWO_TIME_CHARGE_ITEM_LIST: i16 = 16043;
    pub const GAMETWO_TIME_CHARGE_NORMAL: i16 = 16044;
    pub const GAMETWO_TIME_CHARGE_IMMEDIATE: i16 = 16045;
    pub const GAMETWO_TIME_CHARGE_CANCEL: i16 = 16046;
    pub const GAMETWO_TIME_CHARGE_WITHDRAW: i16 = 16047;
    // Satellite pins: PiemonGameStart {u8 a; u4 b; u4 c} through event 75 ;
    // PiemonGameEnd {u4 value} forwarded raw through event 76 ; PlayTimeSet {u4
    // skipped} with the cursor advanced past the u4 and nothing notified.
    pub const MINIEVENT_PIEMON_GAME_START: i16 = 5001;
    pub const MINIEVENT_PIEMON_GAME_END: i16 = 5002;
    pub const MGR_PLAY_TIME_SET: i16 = 9715;
    // Debug notices: InsufficientRange {u4 first; u4 second} with the two fields
    // feeding the #Distance# and #Range# chat substitutions and event 42 popping
    // the formatted chat string ; LoseTarget shares the bare {u4 skipped} advance
    // used by Scanner::Error with no store and no event.
    pub const DEBUG_INSUFFICIENT_RANGE: i16 = 3002;
    pub const DEBUG_LOSE_TARGET: i16 = 3001;
    // Scanner microscope flow: Read {u2 anchor; u4 mode} extends with {u4 value}
    // once mode is neither 0 nor -1 and one more {u4 value2} when mode is 30001 ;
    // Release {u4 flag; u4 a} pops event 591 through the world handler when flag is
    // nonzero.
    pub const SCANNER_READ: i16 = 2204;
    pub const SCANNER_RELEASE: i16 = 2206;
    pub const SCANNER_ERROR: i16 = 2207;
    // Unnamed modern op 1342: no payload at all; the parser drives the singleton
    // label setter at param 35 plus the idle-action tick. 1343 is the same driver
    // at param 36.
    pub const UNKNOWN_1342: i16 = 1342;
    pub const UNKNOWN_1343: i16 = 1343;
    // Notice-family strings, each parsed by its own reader: 2140 {u4 a; u8[71]
    // blob; s2 name} @0x00D95910 event 564 ; 2141 {u4 a; s2 name} @0x00D956F0
    // event 565 ; 2143 {u1 count; count x u4 value; s2 name} @0x00D95A50 event 566.
    pub const MODERN_2140: i16 = 2140;
    pub const MODERN_2141: i16 = 2141;
    pub const MODERN_2143: i16 = 2143;
    // Unnamed skill-table pairing with 3248: 3247 {u4 handler; u2 a; u4 b; u2
    // skipped; u1 c; u4 d} drives the item-table lookup and either the linked
    // notice or the value toggle, both closing with an empty event 476 ; 3248 {u4
    // handler; u4 value} runs the same toggler on the matched digimon and pings
    // empty event 476 when the value path settles on the main monster.
    pub const UNKNOWN_3247: i16 = 3247;
    pub const UNKNOWN_3248: i16 = 3248;
    // Unnamed quad {u4 a; u4 b; u1 c; u1 d; u1 e; u1 f} driving the evolution-list
    // mutation and popping empty event 479.
    pub const UNKNOWN_3249: i16 = 3249;
    // Cash-list snapshots: 3415 {u4 a; u4 b; u2 count; count x {u4 c; u2 d}}
    // through event 438 ; 3417 {u4 a; u4 b; u2 count; count x {u4 c; u4 d; u4 e; u1
    // f; u2 g; u2 h; u2 i; u1 j; u1 k; u1 l; u1 m; u1 n}} 24-byte rows through
    // event 439 ; 3418 {u2 count; count x {u4 a; u2 b}} through event 440 ; 3419
    // {u4 a; u2 count; count x {u4 b}} through event 441.
    pub const MODERN_3415: i16 = 3415;
    pub const MODERN_3417: i16 = 3417;
    pub const MODERN_3418: i16 = 3418;
    pub const MODERN_3419: i16 = 3419;
    // 3420 shares the 3417 24-byte row shape but drops the second header
    // u4: {u4 a; u2 count; count x rows}.
    pub const MODERN_3420: i16 = 3420;
    // Bare flag payload: {u1}.
    pub const UNKNOWN_3990: i16 = 3990;
    pub const TAMER_SUMMON: i16 = 1114;
    pub const TAMER_SKILL_REQUEST: i16 = 1327;
    pub const TRANSCENDENCE_RECEIVE_EXP: i16 = 16039;
    pub const TRANSCENDENCE_SUCCESS: i16 = 16040;
    pub const TIME_CHARGE_RESULT: i16 = 16042;
    pub const WARP_GATE_DUNGEON: i16 = 4119;
    pub const SKILL_UPDATE_COOLDOWN: i16 = 3246;
    // Skill-limit open notice {u4 flag; u4 digimon_row; 28-byte skill blob; u4
    // skill_index; u4 expected} — direct digimon-table mutation plus an empty
    // event-477 ping.
    pub const SKILL_LIMIT_OPEN: i16 = 3245;
    // Skill application broadcast {u1 tag; tag body}. Tag 0 is a melee cast with
    // a caster block, a stat chain and an 18-slot target table; tag 2 is a
    // damage-over-time tick list; tag 3 is a scene cast. Tag 1 carries a body
    // whose trailing field meanings are still unbound, so the encoder refuses it
    // rather than guessing a layout.
    pub const SKILL_APPLY_AROUND: i16 = 1102;
    // Enchant/tactics decrease notice applied to the local digimon: {u2 anchor
    // (client stores value - 1); u2 x14 in two runs of seven}; the loop pairs run
    // one with run two into seven info-list entries, decrements a 69-byte
    // skill-table row and pings event 718 plus an empty event-476.
    pub const DIGIMON_DECREASE_INCHANT: i16 = 1083;
    // Enchant notice {u4 digimon handler; u2 anchor (client stores value - 1); u2
    // x14 in two runs of seven}; same pairing loop as the decrease variant, then a
    // full stat recompute through the base-stats/evolution/digimon tables and
    // events 41, 167 and 476.
    pub const DIGIMON_INCHANT: i16 = 1075;
    // Coat/vanity slot notices: 2201 all {u4 handler} (entity type 14 calls a
    // vanity slot setter), 2202 savers {u4 value; u4 consumed}, 2203 adventure {u4
    // value; u4 handler} with type 13 routing through the tamer vanity table and
    // type 14 through the digimon one.
    pub const ITEM_COAT_ALL: i16 = 2201;
    pub const ITEM_COAT_SAVERS: i16 = 2202;
    pub const ITEM_COAT_ADVENTURE: i16 = 2203;
    // Slot-size updates: 3924 inventory {u2 size} -> ev458 + generic ev353 node
    // {103, size}; 3925 warehouse {u2 size} -> ev61 + ev353 node {104, size}.
    pub const ITEM_INVEN_SLOT_SIZE: i16 = 3924;
    pub const ITEM_WAREHOUSE_SLOT_SIZE: i16 = 3925;
    // Use-failure notice {u2 code; u4 item serial; u4 failure kind}: kind 1 opens a
    // failure message box driven by the CItemTable row, kind 13 pings ev154, kind
    // 19 beeps, other kinds just reset the client.
    pub const ITEM_USE_FAILURE: i16 = 3902;
    // Portable-item failure {u4 code}: switch 7701 pings ev212, 7702 and 7703 gate
    // on prior portable-item usage.
    pub const ITEM_PORTABLE_FAIL: i16 = 3943;
    // Crop-money flow {u4; u4; u4; u4}: the first two u4s form one i64 and the
    // trailing pair feeds the coin delta sums; closes three fixed UI handlers first
    // (23100/22900/23000), plays the coin sound and pings ev671.
    pub const ITEM_CROP_MONEY: i16 = 3911;
    // Limited-attribute change {u2; u1} -> event 585 with an u1 gate slot.
    pub const ITEM_CHANGE_LIMITED_ATTRIBUTE: i16 = 3934;
    // Enchant-info notice: {u4 digimon handler; u2 base; five u2 values} — the five
    // u2s insert into fixed positions 0, 1, 2, 4 and 6 of the digimon's
    // enchant-info list, then the client refreshes that digimon.
    pub const DIGIMON_INCHANT_INFO: i16 = 3214;
    // Macro-result family: 1066 failure {u4} -> ev359, 1067 success with no payload
    // (local zeroed struct -> ev359), 1068 result {u4} -> ev360.
    pub const MACRO_FAILURE: i16 = 1066;
    pub const MACRO_SUCCESS: i16 = 1067;
    pub const MACRO_RESULT: i16 = 1068;
    // Experience-gain notice: five i64 quantity pairs plus two u4 fields, the
    // fourth read being consumed as u16 and the last one feeding a float exp path;
    // every quantity enters the same x100 fixed-point conversion.
    pub const GAME_GET_EXP: i16 = 1018;
    // Target-tracking notice {u4 target handler; u1 flag; u4; u4} resolved through
    // the entity lookup and applied only to non-filtered types.
    pub const GAME_TARGET_INFO: i16 = 1017;
    // Stat-change notice {u4 handler; u2 count; count x {u2 stat; u4 value}}; stat
    // selector 1.10 and 12 dispatch into distinct setters, then an empty
    // event-590/476 ping depending on entity type.
    pub const GAME_CHANGE_STAT: i16 = 1084;
    // Silent-state notice {u2; u4}: both fields are consumed with no dispatch.
    pub const GAME_EVENT: i16 = 1062;
    // Message-all failure and incubator error share one parser: a u4 is consumed and
    // dropped, no dispatch.
    pub const MSG_ALL_FAILURE: i16 = 1071;
    pub const INCUBATER_ERROR: i16 = 3228;
    pub const FRIENDLY_INFO: i16 = 3129;
    // Msg-scope notices with an empty payload: 14005 and 14015 both notify
    // without reading a single byte, so the frame is header only.
    pub const MSG_DUNGEON_MATCH_END: i16 = 14005;
    pub const MSG_DUNGEON_MATCH_CANCEL: i16 = 14015;
    // Dungeon re-entry result {u1 result}; the client stores it and notifies
    // once per frame.
    pub const MSG_DUNGEON_RE_ENTER_RESULT: i16 = 14016;
    // Match state notices: 14007 {u4; u1; u1; u4; u2}, 14008 {u4; u1; u4} and
    // 14009 {u4; u8; u8}. Each ends with one notification carrying the whole
    // envelope.
    pub const MSG_DUNGEON_MATCH_STATE: i16 = 14007;
    pub const MSG_DUNGEON_MATCH_RESULT: i16 = 14008;
    pub const MSG_DUNGEON_MATCH_COUNTER: i16 = 14009;
    // Dungeon stage info {u4; u4; u2; u4; u8}; all five fields are unbound.
    pub const MSG_DUNGEON_STAGE_INFO: i16 = 14001;
    // Dungeon clear info {u4; u1 clear count; count x {u1 flag; u4 value};
    // u1 reward count; count x {u4 value; u2 item}}. Both counts are signed.
    pub const MSG_DUNGEON_CLEAR_INFO: i16 = 14006;
    // Dungeon schedule list {u4 id; u8; u8; clears; schedules; 69B rows; gifts}.
    pub const MSG_DUNGEON_SCHEDULE_LIST: i16 = 14010;
    // Item-make limit data {u2 count; count x {u4 value; u4 value}}.
    pub const ITEM_MAKE_LIMIT_DATA: i16 = 5301;
    // Make-limit reset {u2 count; count x u4}.
    pub const MAKE_LIMIT_RESET: i16 = 5302;
    // Macro protector refresh: the client only clears a local flag and reads no
    // payload byte, so the frame is header only.
    pub const REFRESH_MACRO_PROTECTOR: i16 = 4145;
    // User setting data {u2 count; count x {u2 prefix; modern string}}.
    // The opcode exceeds i16 positive range, so it rides the wire as its
    // two-complement negative form, which is what the client compares against.
    pub const USER_SETTING_DATA: i16 = -5535;
    // Other-tamer detail info, same negative encoding.
    pub const OTHER_TAMER_DETAIL_INFO: i16 = -25535;
    // Cash-shop balance notice {u4 value}; the client reacts only to value 1.
    pub const CASH_BALANCE_NOTICE: i16 = 1056;
    // Progress notice {u4 value; u8 value}; both fields are unbound.
    pub const PROGRESS_NOTICE_905: i16 = 905;
    // Pair of single-word notices; each dispatches a different local event
    // depending on whether the value is zero.
    pub const NOTICE_1309: i16 = 1309;
    pub const NOTICE_1315: i16 = 1315;
    // Notices the client registers with a handler that consumes nothing: the
    // frame is header only and the payload must stay empty. Sending a body would
    // leave those bytes to be read as the next frame's header. The numbers are
    // kept in the names because no capability is bound to them yet.
    pub const NO_PAYLOAD_1031: i16 = 1031;
    pub const NO_PAYLOAD_1079: i16 = 1079;
    pub const NO_PAYLOAD_1080: i16 = 1080;
    pub const NO_PAYLOAD_1089: i16 = 1089;
    pub const NO_PAYLOAD_1103: i16 = 1103;
    pub const NO_PAYLOAD_1112: i16 = 1112;
    pub const NO_PAYLOAD_2121: i16 = 2121;
    pub const NO_PAYLOAD_2205: i16 = 2205;
    pub const NO_PAYLOAD_3003: i16 = 3003;
    pub const NO_PAYLOAD_3101: i16 = 3101;
    pub const NO_PAYLOAD_3906: i16 = 3906;
    pub const NO_PAYLOAD_3908: i16 = 3908;
    pub const NO_PAYLOAD_3919: i16 = 3919;
    pub const NO_PAYLOAD_3920: i16 = 3920;
    pub const NO_PAYLOAD_3921: i16 = 3921;
    pub const NO_PAYLOAD_3928: i16 = 3928;
    pub const NO_PAYLOAD_9904: i16 = 9904;
    // Further notices whose handler consumes nothing: the body is empty and the
    // frame stays header only. 2104 fires a local event and returns.
    pub const NO_PAYLOAD_2104: i16 = 2104;
    pub const NO_PAYLOAD_4128: i16 = 4128;
    pub const NO_PAYLOAD_4139: i16 = 4139;
    pub const NO_PAYLOAD_4142: i16 = 4142;
    pub const NO_PAYLOAD_4601: i16 = 4601;
    pub const NO_PAYLOAD_5358: i16 = 5358;
    // Single-byte notice; the handler consumes one byte and drops it.
    pub const SINGLE_BYTE_NOTICE_2318: i16 = 2318;
    // Macro protector refresh result {u4 value; u2 count; count x u1 bytes;
    // u1 tail}. The byte run is stored as a blob, not as text.
    pub const RESULT_REFRESH_MACRO_PROTECTOR: i16 = 5206;
    // Small item notices: 5005 and 5009 read {u1; u2}; 5006 reads {u1; u2; u2};
    // 5007 reads {u2}; 5008 reads {u1}.
    pub const ITEM_NOTICE_5005: i16 = 5005;
    pub const ITEM_NOTICE_5006: i16 = 5006;
    pub const ITEM_NOTICE_5007: i16 = 5007;
    pub const ITEM_NOTICE_5008: i16 = 5008;
    pub const ITEM_NOTICE_5009: i16 = 5009;
    // Notices 9801, 9804 and 9806 each read one word; 10003 reads one half word.
    // No capability is bound to them yet, so the names stay keyed to the number.
    pub const NOTICE_9801: i16 = 9801;
    pub const NOTICE_9804: i16 = 9804;
    pub const NOTICE_9806: i16 = 9806;
    pub const NOTICE_10003: i16 = 10003;
    // Word notices: 3305, 5354, 3172 and 14058 each read one u4. 16055 reads one
    // u8; 9851 reads a fixed 32-byte run.
    pub const NOTICE_3305: i16 = 3305;
    pub const NOTICE_5354: i16 = 5354;
    pub const NOTICE_3172: i16 = 3172;
    pub const NOTICE_14058: i16 = 14058;
    pub const NOTICE_16055: i16 = 16055;
    pub const NOTICE_9851: i16 = 9851;
    // Party recruiter replies, each carrying a single result word.
    pub const PARTY_RECRUIT_MAKE: i16 = 2501;
    pub const PARTY_RECRUIT_APPLY: i16 = 2503;
    pub const PARTY_RECRUIT_KICKOUT: i16 = 2511;
    pub const PARTY_RECRUIT_WITHDRAW: i16 = 2510;
    pub const PARTY_RECRUIT_CHAT: i16 = 2508;
    pub const PARTY_RECRUIT_CREATE_PARTY: i16 = 2514;
    // Recruiter leave notice {u1 type; u4 member}.
    pub const PARTY_RECRUIT_LEAVE_MEMBER: i16 = 2512;
    // Recruiter member notices, each reading exactly one member row: 2506 is the
    // new-member notice and 2517 the sync-member notice.
    pub const PARTY_RECRUIT_NEW_MEMBER: i16 = 2506;
    pub const PARTY_RECRUIT_SYNC_MEMBER: i16 = 2517;
    // Change-skin result {u1 result}.
    pub const RES_CHANGE_SKIN_RESULT: i16 = 3154;
    // Skin acquisition notice {u1 flag; u4 skin}.
    pub const NTF_ACQUIRE_SKIN: i16 = 3152;
    // Two more replies: 2513 reads one word; 3603 reads two words.
    pub const REPLY_2513: i16 = 2513;
    pub const RESPONSE_3603: i16 = 3603;
    // Item notices: 4138 reads {u1; u4}; 4137 reads {u1; u4; u4}; 3501 and 3514
    // each read {u4; u4}. 16035 reads a fixed 1024-byte run then one word.
    pub const NOTICE_4138: i16 = 4138;
    pub const NOTICE_4137: i16 = 4137;
    pub const RESPONSE_3501: i16 = 3501;
    pub const RESPONSE_3514: i16 = 3514;
    pub const NOTICE_16035: i16 = 16035;
    // Ex-item move result {u4 result}.
    pub const RES_EXITEM_MOVE: i16 = 3801;
    // Mixed-width notices: 4125 and 4136 share {u4; u1; u4; u2; u2; u4};
    // 4132 reads {u1; u8; u4}.
    pub const NOTICE_4125: i16 = 4125;
    pub const NOTICE_4136: i16 = 4136;
    pub const NOTICE_4132: i16 = 4132;
    // {u4 value; u2 first short; u4 second value; u4 third value; u4 fourth
    // value}, the shape opcode 3601 reads.
    pub const NOTICE_3601: i16 = 3601;
    // {u4 value; u8 value; u2 first short; u8 second value}.
    pub const NOTICE_3512: i16 = 3512;
    // Recruiter query list {u4 value; u4 count; u4 value; count x view row;
    // u4 value; u4 value; u4 value}, where a view row is
    // {u8 values; u4 value; u4 value; name; u1 flag; note; u4 value}.
    pub const PARTY_RECRUIT_QUERY_LIST: i16 = 2518;
    // Skin inventory snapshot {u2 group count; count x {u1 flag;
    // u2 item count; count x u4 item}; u4 value}.
    pub const NTF_INIT_SKIN_DATA: i16 = 3151;
    // Ranking stage clear notice {u2 stage; u1 flag; u4 value; u4 value;
    // u2 value}.
    pub const NTF_RANKING_STAGE_CLEAR: i16 = 14059;
    // Mail delete result: two counted runs, each {u2 count; count x {u4; u4}}.
    pub const RES_MAIL_DELETE: i16 = 16054;
    // Sync verification result {u2 count; count x u4 value}.
    pub const RES_SYNC_VERIFICATION: i16 = 16072;
    // Breakthrough: 3672 reads {u4; u4; {u2 count; count x u4}}; 3675 reads the
    // same body after its own leading words.
    pub const RES_CREATE_BREAKTHROUGH_DATA: i16 = 3672;
    // Circuit module operations: 4504 reads {u1 flag; u4 slot; u1 flag;
    // counted byte run}; 4505 the same without the trailing run; 4506 adds a
    // leading double word before the run.
    pub const RES_DS_CIRCUIT_MODULE_EQUIP: i16 = 4504;
    pub const RES_DS_CIRCUIT_MODULE_MOVE: i16 = 4505;
    pub const RES_DS_CIRCUIT_MODULE_UNEQUIP: i16 = 4506;
    // Seasonal notices 3507 and 3508 share {u4 value; u8 value; u4 value;
    // u2 value; u8 value; u2 count; count x {u8 value; u2 value}}.
    pub const NTF_SEASON_3507: i16 = 3507;
    pub const NTF_SEASON_3508: i16 = 3508;
    // Inventory results that report a word and then the updated item list, each
    // item row being the 71-byte serialized form.
    pub const RES_START_DIGIMON_EXPLORE: i16 = 5353;
    pub const RES_OPEN_DIGIMON_EXPLORE_SLOT: i16 = 5356;
    // Explore slot snapshot: {u2 count; count x slot row}, each slot row
    // carrying its own counted pair run.
    pub const NTF_INIT_DIGIMON_EXPLORE_DATA: i16 = 5351;
    // Season pass text row: {u4 value; u4 value; u8 value; modern string}.
    pub const NTF_SEASON_3503: i16 = 3503;
    pub const NTF_SEASON_3506: i16 = 3506;
    pub const NTF_SEASON_3513: i16 = 3513;
    // Login-stage notice {u4 value; modern string}.
    pub const NOTICE_3308: i16 = 3308;
    // Season mission alert {u2 count; count x u4 value}.
    pub const NTF_SEASON_MISSION_ALERT: i16 = 4401;
    // Season rewards claim: {u1 flag; {u2 count; count x {u2; u1; u1; u1}}}.
    pub const RES_SEASON_PASS_GET_SEASON_REWARDS: i16 = 3555;
    // Circuit module extract: {u1 flag; u4 value; counted item list}.
    pub const RES_CIRCUIT_MODULE_EXTRACT: i16 = 4502;
    // Circuit module sync: {u1 flag; {u2 count; count x {u4; u1; item row}};
    // {u2 count; count x u1}}.
    pub const RES_CIRCUIT_MODULE_SYNC: i16 = 4503;
    // Login-stage notice {u4 value; u4 value; modern string; u4 value}.
    pub const NOTICE_901: i16 = 901;
    // Union hack chip results: 3173 reads {u4 x5; u2; u1; item list; u8},
    // 3174 reads {u4 x4; u2; u1; u4; item list; u8}.
    pub const RES_UNION_HACK_OPEN: i16 = 3173;
    pub const RES_UNION_HACK_MODIFY: i16 = 3174;
    // Breakthrough results that report a word, the updated item list, then
    // trailing words: 3673 ends with two, 3674 with one.
    pub const RES_TRY_BREAKTHROUGH: i16 = 3673;
    pub const RES_RESET_BREAKTHROUGH: i16 = 3674;
    // Body is a bare modern string.
    pub const NTF_STRING_3309: i16 = 3309;
    // Preset data save result: {u1 flag; u1 flag; counted item list}.
    pub const RES_PRESET_DATA_SAVE: i16 = 4204;
    // {u4 value; u8 value; u1 flag; u2 count; count x 69-byte row}.
    pub const NOTICE_16038: i16 = 16038;
    pub const RES_REDUCE_DIGIMON_EXPLORE_TIME: i16 = 5357;
    pub const RES_ITEM_EXPLORE_COMPLETE_5359: i16 = 5359;
    pub const RES_ITEM_UPGRADE_DATA_TRANSFER: i16 = 5401;
    pub const RES_ITEM_5402: i16 = 5402;
    // Incubator level-up notice {u4 digimon; u4 level}.
    pub const LEVELUP_DIGIMON_IN_INCUBATER: i16 = 3250;
    // Extra inventory slot expansion notice {u1 flag; u2 slots}.
    pub const NTF_EXTRAINVEN_SLOTEXPANSION: i16 = 3702;
    // Ranking play progress {u2 dungeon; u4 value; u1 flag; u4 value; u2 value;
    // u4 value; u2 value; u4 value; u2 count; count x {u2 stage; u2 wave;
    // u4 value}}.
    pub const MSG_RANKING_PLAY_PROGRESS: i16 = 14057;
}
