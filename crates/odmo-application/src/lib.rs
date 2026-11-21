pub mod account;
pub mod character;
pub mod game;
pub mod movement;
pub mod npcs;
pub mod portal;
pub mod portals;

pub fn bootstrap_message() -> &'static str {
    "odmo application layer"
}

/// Trait for broadcasting packets to other connected sessions.
/// Implemented by service binaries using tokio channels.
pub trait BroadcastSink: Send + Sync {
    fn send_to(&self, character_id: u64, packet: &[u8]) -> anyhow::Result<()>;
    fn is_online(&self, character_id: u64) -> bool;
    fn send_to_visible(
        &self,
        map_id: i16,
        channel: u8,
        exclude_character_id: u64,
        packet: &[u8],
    ) -> anyhow::Result<()>;
    /// Deliver to every connected session regardless of map and channel, except
    /// the sender. Channels that span the whole server use this.
    fn send_to_all_except(
        &self,
        exclude_character_id: u64,
        packet: &[u8],
    ) -> anyhow::Result<()>;
    /// Update the character's known map location for broadcast filtering.
    fn update_location(&self, character_id: u64, map_id: i16, channel: u8);
}

/// Shared in-memory game state for cross-session communication.
/// Replaces the file-based PortalBridge for real-time features.
#[derive(Debug, Clone)]
pub struct OnlineMapState {
    /// (map_id, channel) -> set of character_ids present
    map_presence: std::sync::Arc<dashmap::DashMap<(i16, u8), Vec<u64>>>,
    /// character_id -> pending social notifications
    social_inbox: std::sync::Arc<dashmap::DashMap<u64, Vec<portal::SocialNotification>>>,
    /// account_id -> transfer ticket
    transfer_tickets: std::sync::Arc<dashmap::DashMap<u64, odmo_types::TransferTicket>>,
    /// account_id -> game session ticket
    game_session_tickets: std::sync::Arc<dashmap::DashMap<u64, odmo_types::GameSessionTicket>>,
}

impl OnlineMapState {
    pub fn new() -> Self {
        Self {
            map_presence: std::sync::Arc::new(dashmap::DashMap::new()),
            social_inbox: std::sync::Arc::new(dashmap::DashMap::new()),
            transfer_tickets: std::sync::Arc::new(dashmap::DashMap::new()),
            game_session_tickets: std::sync::Arc::new(dashmap::DashMap::new()),
        }
    }

    // --- Map Presence ---

    pub fn register_map_presence(&self, map_id: i16, channel: u8, character_id: u64) {
        let mut entry = self.map_presence.entry((map_id, channel)).or_default();
        if !entry.contains(&character_id) {
            entry.push(character_id);
        }
    }

    pub fn unregister_map_presence(&self, map_id: i16, channel: u8, character_id: u64) {
        if let Some(mut entry) = self.map_presence.get_mut(&(map_id, channel)) {
            entry.value_mut().retain(|id| *id != character_id);
        }
    }

    pub fn characters_on_map(&self, map_id: i16, channel: u8) -> Vec<u64> {
        self.map_presence
            .get(&(map_id, channel))
            .map(|entry| entry.value().clone())
            .unwrap_or_default()
    }

    // --- Social Inbox ---

    pub fn push_notification(&self, character_id: u64, notification: portal::SocialNotification) {
        self.social_inbox
            .entry(character_id)
            .or_default()
            .push(notification);
    }

    pub fn drain_notifications(&self, character_id: u64) -> Vec<portal::SocialNotification> {
        self.social_inbox
            .remove(&character_id)
            .map(|(_, v)| v)
            .unwrap_or_default()
    }

    // --- Transfer Tickets ---

    pub fn store_transfer_ticket(&self, ticket: odmo_types::TransferTicket) {
        self.transfer_tickets.insert(ticket.account_id, ticket);
    }

    pub fn consume_transfer_ticket(&self, account_id: u64) -> Option<odmo_types::TransferTicket> {
        self.transfer_tickets.remove(&account_id).map(|(_, v)| v)
    }

    // --- Game Session Tickets ---

    pub fn store_game_session_ticket(&self, ticket: odmo_types::GameSessionTicket) {
        self.game_session_tickets.insert(ticket.account_id, ticket);
    }

    pub fn consume_game_session_ticket(
        &self,
        account_id: u64,
    ) -> Option<odmo_types::GameSessionTicket> {
        self.game_session_tickets
            .get(&account_id)
            .map(|entry| entry.value().clone())
    }
}

impl Default for OnlineMapState {
    fn default() -> Self {
        Self::new()
    }
}
