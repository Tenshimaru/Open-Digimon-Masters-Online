use std::{fs, path::PathBuf, sync::Arc};

/// A fresh access code for one login. The account service hands it to the client with
/// the character server address; the client presents it to the character and game
/// services, which accept it only when it matches the ticket issued with it. Never 0.
pub fn new_access_code() -> u32 {
    let bytes = uuid::Uuid::new_v4().into_bytes();
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]).max(1)
}

/// Whether `access_code` is the one a ticket with `token` was issued for.
pub fn access_code_matches(token: &str, access_code: u32) -> bool {
    token == access_code.to_string()
}

use anyhow::Context;
use odmo_types::{AccountId, CharacterId, CharacterSummary, GameSessionTicket, TransferTicket};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SocialNotification {
    pub kind: SocialNotificationKind,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SocialNotificationKind {
    FriendConnect { name: String },
    MapTamerSpawn { character: CharacterSummary },
    MapTamerUnload { character: CharacterSummary },
}

// ---------------------------------------------------------------------------
// Trait — the contract every backend must implement
// ---------------------------------------------------------------------------

pub trait PortalStore: Send + Sync + 'static {
    // Transfer tickets (account → character)
    fn store_transfer_ticket(&self, ticket: &TransferTicket) -> anyhow::Result<()>;
    fn consume_transfer_ticket(
        &self,
        account_id: AccountId,
    ) -> anyhow::Result<Option<TransferTicket>>;

    // Game session tickets (character → game)
    fn store_game_session_ticket(&self, ticket: &GameSessionTicket) -> anyhow::Result<()>;
    fn consume_game_session_ticket(
        &self,
        account_id: AccountId,
    ) -> anyhow::Result<Option<GameSessionTicket>>;

    // Social notifications
    fn enqueue_social_notification(
        &self,
        character_id: CharacterId,
        notification: SocialNotification,
    ) -> anyhow::Result<()>;
    fn consume_social_notifications(
        &self,
        character_id: CharacterId,
    ) -> anyhow::Result<Vec<SocialNotification>>;

    // Map presence
    fn load_map_presence(&self, map_id: i16, channel: u8) -> anyhow::Result<Vec<CharacterSummary>>;
    fn upsert_map_presence(&self, character: &CharacterSummary) -> anyhow::Result<()>;
    fn remove_map_presence(
        &self,
        map_id: i16,
        channel: u8,
        character_id: CharacterId,
    ) -> anyhow::Result<Vec<CharacterSummary>>;
}

// ---------------------------------------------------------------------------
// JSON file-based implementation (dev mode)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct JsonPortalBridge {
    root: PathBuf,
}

impl JsonPortalBridge {
    pub fn new(root: PathBuf) -> anyhow::Result<Self> {
        fs::create_dir_all(&root).with_context(|| {
            format!(
                "failed to create portal bridge directory {}",
                root.display()
            )
        })?;
        Ok(Self { root })
    }

    fn ticket_path(&self, account_id: AccountId) -> PathBuf {
        self.root.join(format!("account-{account_id}.json"))
    }

    fn game_ticket_path(&self, account_id: AccountId) -> PathBuf {
        self.root.join(format!("game-account-{account_id}.json"))
    }

    fn social_inbox_path(&self, character_id: CharacterId) -> PathBuf {
        self.root
            .join(format!("social-character-{character_id}.json"))
    }

    fn map_presence_path(&self, map_id: i16, channel: u8) -> PathBuf {
        self.root
            .join(format!("map-{map_id}-channel-{channel}.json"))
    }

    fn load_social_notifications(
        &self,
        character_id: CharacterId,
    ) -> anyhow::Result<Vec<SocialNotification>> {
        let path = self.social_inbox_path(character_id);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let payload = fs::read(&path)
            .with_context(|| format!("failed to read social inbox {}", path.display()))?;
        let notifications = serde_json::from_slice(&payload)
            .with_context(|| format!("failed to decode social inbox {}", path.display()))?;
        Ok(notifications)
    }
}

impl PortalStore for JsonPortalBridge {
    fn store_transfer_ticket(&self, ticket: &TransferTicket) -> anyhow::Result<()> {
        let path = self.ticket_path(ticket.account_id);
        let payload = serde_json::to_vec_pretty(ticket)?;
        fs::write(&path, payload)
            .with_context(|| format!("failed to write transfer ticket {}", path.display()))?;
        Ok(())
    }

    fn consume_transfer_ticket(
        &self,
        account_id: AccountId,
    ) -> anyhow::Result<Option<TransferTicket>> {
        let path = self.ticket_path(account_id);
        if !path.exists() {
            return Ok(None);
        }
        let payload = fs::read(&path)
            .with_context(|| format!("failed to read transfer ticket {}", path.display()))?;
        let ticket = serde_json::from_slice(&payload)
            .with_context(|| format!("failed to decode transfer ticket {}", path.display()))?;
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove consumed ticket {}", path.display()))?;
        Ok(Some(ticket))
    }

    fn store_game_session_ticket(&self, ticket: &GameSessionTicket) -> anyhow::Result<()> {
        let path = self.game_ticket_path(ticket.account_id);
        let payload = serde_json::to_vec_pretty(ticket)?;
        fs::write(&path, payload)
            .with_context(|| format!("failed to write game session ticket {}", path.display()))?;
        Ok(())
    }

    fn consume_game_session_ticket(
        &self,
        account_id: AccountId,
    ) -> anyhow::Result<Option<GameSessionTicket>> {
        let path = self.game_ticket_path(account_id);
        if !path.exists() {
            return Ok(None);
        }
        let payload = fs::read(&path)
            .with_context(|| format!("failed to read game session ticket {}", path.display()))?;
        let ticket = serde_json::from_slice(&payload)
            .with_context(|| format!("failed to decode game session ticket {}", path.display()))?;
        Ok(Some(ticket))
    }

    fn enqueue_social_notification(
        &self,
        character_id: CharacterId,
        notification: SocialNotification,
    ) -> anyhow::Result<()> {
        let mut notifications = self.load_social_notifications(character_id)?;
        notifications.push(notification);
        let path = self.social_inbox_path(character_id);
        let payload = serde_json::to_vec_pretty(&notifications)?;
        fs::write(&path, payload)
            .with_context(|| format!("failed to write social inbox {}", path.display()))?;
        Ok(())
    }

    fn consume_social_notifications(
        &self,
        character_id: CharacterId,
    ) -> anyhow::Result<Vec<SocialNotification>> {
        let notifications = self.load_social_notifications(character_id)?;
        let path = self.social_inbox_path(character_id);
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("failed to remove social inbox {}", path.display()))?;
        }
        Ok(notifications)
    }

    fn load_map_presence(&self, map_id: i16, channel: u8) -> anyhow::Result<Vec<CharacterSummary>> {
        let path = self.map_presence_path(map_id, channel);
        if !path.exists() {
            return Ok(Vec::new());
        }
        let payload = fs::read(&path)
            .with_context(|| format!("failed to read map presence {}", path.display()))?;
        match serde_json::from_slice(&payload) {
            Ok(entries) => Ok(entries),
            Err(err) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %err,
                    "discarding stale or corrupted map presence file"
                );
                let _ = fs::remove_file(&path);
                Ok(Vec::new())
            }
        }
    }

    fn upsert_map_presence(&self, character: &CharacterSummary) -> anyhow::Result<()> {
        let mut entries = self.load_map_presence(character.map_id, character.channel)?;
        entries.retain(|entry| entry.id != character.id);
        entries.push(character.clone());
        let path = self.map_presence_path(character.map_id, character.channel);
        let payload = serde_json::to_vec_pretty(&entries)?;
        fs::write(&path, payload)
            .with_context(|| format!("failed to write map presence {}", path.display()))?;
        Ok(())
    }

    fn remove_map_presence(
        &self,
        map_id: i16,
        channel: u8,
        character_id: CharacterId,
    ) -> anyhow::Result<Vec<CharacterSummary>> {
        let mut entries = self.load_map_presence(map_id, channel)?;
        let before_len = entries.len();
        entries.retain(|entry| entry.id != character_id);
        let path = self.map_presence_path(map_id, channel);
        if entries.is_empty() {
            if path.exists() {
                fs::remove_file(&path).with_context(|| {
                    format!("failed to remove empty map presence {}", path.display())
                })?;
            }
        } else if entries.len() != before_len {
            let payload = serde_json::to_vec_pretty(&entries)?;
            fs::write(&path, payload)
                .with_context(|| format!("failed to update map presence {}", path.display()))?;
        }
        Ok(entries)
    }
}

// ---------------------------------------------------------------------------
// PortalBridge — thin wrapper that holds an Arc<dyn PortalStore>
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct PortalBridge {
    inner: Arc<dyn PortalStore>,
}

impl std::fmt::Debug for PortalBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PortalBridge").finish_non_exhaustive()
    }
}

impl PortalBridge {
    pub fn from_store(store: impl PortalStore) -> Self {
        Self {
            inner: Arc::new(store),
        }
    }

    pub fn from_json(root: PathBuf) -> anyhow::Result<Self> {
        Ok(Self::from_store(JsonPortalBridge::new(root)?))
    }

    pub fn store_transfer_ticket(&self, ticket: &TransferTicket) -> anyhow::Result<()> {
        self.inner.store_transfer_ticket(ticket)
    }

    pub fn consume_transfer_ticket(
        &self,
        account_id: AccountId,
    ) -> anyhow::Result<Option<TransferTicket>> {
        self.inner.consume_transfer_ticket(account_id)
    }

    pub fn store_game_session_ticket(&self, ticket: &GameSessionTicket) -> anyhow::Result<()> {
        self.inner.store_game_session_ticket(ticket)
    }

    pub fn consume_game_session_ticket(
        &self,
        account_id: AccountId,
    ) -> anyhow::Result<Option<GameSessionTicket>> {
        self.inner.consume_game_session_ticket(account_id)
    }

    pub fn enqueue_social_notification(
        &self,
        character_id: CharacterId,
        notification: SocialNotification,
    ) -> anyhow::Result<()> {
        self.inner
            .enqueue_social_notification(character_id, notification)
    }

    pub fn consume_social_notifications(
        &self,
        character_id: CharacterId,
    ) -> anyhow::Result<Vec<SocialNotification>> {
        self.inner.consume_social_notifications(character_id)
    }

    pub fn load_map_presence(
        &self,
        map_id: i16,
        channel: u8,
    ) -> anyhow::Result<Vec<CharacterSummary>> {
        self.inner.load_map_presence(map_id, channel)
    }

    pub fn upsert_map_presence(&self, character: &CharacterSummary) -> anyhow::Result<()> {
        self.inner.upsert_map_presence(character)
    }

    pub fn remove_map_presence(
        &self,
        map_id: i16,
        channel: u8,
        character_id: CharacterId,
    ) -> anyhow::Result<Vec<CharacterSummary>> {
        self.inner
            .remove_map_presence(map_id, channel, character_id)
    }
}
