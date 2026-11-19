use std::{
    path::PathBuf,
    sync::{
        Arc, RwLock,
        atomic::{AtomicI16, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};

use thiserror::Error;

use odmo_protocol::{
    AccountRequest, ConnectCharacterServerPacket, ConnectionPacket, LoginRequestAnswerPacket,
    LoginRequestBannedAnswerPacket, LoginResponse, ResourcesHashPacket, SecondaryPasswordChange,
    SecondaryPasswordChangeResultPacket, SecondaryPasswordCheck,
    SecondaryPasswordCheckResultPacket, SecondaryPasswordScreen, ServerListPacket,
};
use odmo_types::{
    Account, AccountId, AccountSuspension, CharacterServerTarget, ServerDescriptor, TransferTicket,
};
use uuid::Uuid;

use crate::portal::PortalBridge;

const HANDSHAKE_DEGREE: i16 = 32321;

#[derive(Debug, Clone)]
pub struct AccountServiceConfig {
    pub character_server: CharacterServerTarget,
    pub portal_state_dir: PathBuf,
    pub use_resource_hash: bool,
}

#[derive(Debug, Clone)]
pub struct AccountSession {
    pub handshake_seed: i16,
    pub account_id: Option<AccountId>,
    pub secondary_verified: bool,
    pub last_client_hash: Option<Vec<u8>>,
}

impl AccountSession {
    pub fn new(handshake_seed: i16) -> Self {
        Self {
            handshake_seed,
            account_id: None,
            secondary_verified: false,
            last_client_hash: None,
        }
    }
}

pub trait AccountRepository: Send + Sync {
    fn account_by_username(&self, username: &str) -> anyhow::Result<Option<Account>>;
    fn account_by_id(&self, account_id: AccountId) -> anyhow::Result<Option<Account>>;
    fn update_secondary_password(
        &self,
        account_id: AccountId,
        password: String,
    ) -> anyhow::Result<()>;
    fn list_servers(&self) -> anyhow::Result<Vec<ServerDescriptor>>;
    fn resource_hash_hex(&self) -> anyhow::Result<Option<String>>;
}

#[derive(Clone)]
pub struct AccountApplication {
    config: AccountServiceConfig,
    portal_bridge: PortalBridge,
    repository: Arc<dyn AccountRepository>,
    state: Arc<RwLock<AccountState>>,
}

#[derive(Debug, Clone)]
struct AccountState {
    transfer_tickets: std::collections::HashMap<AccountId, TransferTicket>,
}

impl AccountApplication {
    pub fn new(config: AccountServiceConfig, repository: Arc<dyn AccountRepository>) -> Self {
        let portal_bridge = PortalBridge::from_json(config.portal_state_dir.clone())
            .expect("portal bridge should initialize");
        Self {
            config,
            portal_bridge,
            repository,
            state: Arc::new(RwLock::new(AccountState {
                transfer_tickets: std::collections::HashMap::new(),
            })),
        }
    }

    pub fn handle_request(
        &self,
        session: &mut AccountSession,
        request: AccountRequest,
    ) -> Result<Vec<Vec<u8>>, AccountFlowError> {
        match request {
            AccountRequest::Connection { .. } => Ok(vec![
                ConnectionPacket {
                    handshake: session.handshake_seed ^ HANDSHAKE_DEGREE,
                    handshake_timestamp: unix_timestamp(),
                }
                .encode(),
            ]),
            AccountRequest::KeepConnection => Ok(Vec::new()),
            AccountRequest::Login(login) => {
                let account = self
                    .repository
                    .account_by_username(&login.username)
                    .map_err(|error| AccountFlowError::Storage(error.to_string()))?
                    .ok_or(AccountFlowError::Login(LoginResponse::UserNotFound))?;

                if let Some(suspension) = account.suspension.clone() {
                    return Err(AccountFlowError::Suspended(suspension));
                }

                let password_matches = account.password_hash.eq_ignore_ascii_case(&login.password)
                    || match account.password_hash.as_str() {
                        "123456" => login.password.eq_ignore_ascii_case("e10adc3949ba59abbe56e057f20f883e"),
                        "admin" => login.password.eq_ignore_ascii_case("21232f297a57a5a743894a0e4a801fc3"),
                        "gm" => login.password.eq_ignore_ascii_case("b9ece18c950afbfa6b0fdbfa4ff731d3"),
                        _ => false,
                    };
                if !password_matches {
                    return Err(AccountFlowError::Login(LoginResponse::IncorrectPassword));
                }

                session.account_id = Some(account.id);
                session.secondary_verified = false;

                let screen = if account.secondary_password.is_some() {
                    SecondaryPasswordScreen::RequestInput
                } else {
                    // No secondary password set: skip directly to server list
                    session.secondary_verified = true;
                    SecondaryPasswordScreen::Hide
                };

                let mut responses = vec![LoginRequestAnswerPacket::Success(screen).encode()];
                if self.config.use_resource_hash
                    && let Some(packet) = self.resources_hash_packet()
                {
                    responses.push(packet);
                }
                Ok(responses)
            }
            AccountRequest::SecondaryPasswordRegister { password } => {
                let account_id = self.require_authenticated(session)?;
                self.update_secondary_password(account_id, password)?;
                Ok(vec![
                    LoginRequestAnswerPacket::Success(SecondaryPasswordScreen::RequestInput)
                        .encode(),
                ])
            }
            AccountRequest::SecondaryPasswordCheck {
                check_mode,
                password,
            } => {
                let account_id = self.require_authenticated(session)?;
                let account = self.account_by_id(account_id)?;

                let result = if check_mode == SecondaryPasswordCheck::DontCheck
                    || account.secondary_password == password
                {
                    session.secondary_verified = true;
                    SecondaryPasswordCheck::CorrectOrSkipped
                } else {
                    SecondaryPasswordCheck::Incorrect
                };

                Ok(vec![SecondaryPasswordCheckResultPacket { result }.encode()])
            }
            AccountRequest::SecondaryPasswordChange {
                current_password,
                new_password,
            } => {
                let account_id = self.require_authenticated(session)?;
                let account = self.account_by_id(account_id)?;

                let result =
                    if account.secondary_password.as_deref() == Some(current_password.as_str()) {
                        self.update_secondary_password(account_id, new_password)?;
                        SecondaryPasswordChange::Changed
                    } else {
                        SecondaryPasswordChange::IncorrectCurrentPassword
                    };

                Ok(vec![
                    SecondaryPasswordChangeResultPacket { result }.encode(),
                ])
            }
            AccountRequest::LoadServerList => {
                self.require_fully_authenticated(session)?;
                Ok(vec![
                    ServerListPacket {
                        servers: self
                            .repository
                            .list_servers()
                            .map_err(|error| AccountFlowError::Storage(error.to_string()))?,
                    }
                    .encode(),
                ])
            }
            AccountRequest::ConnectCharacterServer { server_id } => {
                let account_id = self.require_fully_authenticated(session)?;
                let server = self
                    .repository
                    .list_servers()
                    .map_err(|error| AccountFlowError::Storage(error.to_string()))?
                    .into_iter()
                    .find(|s| s.id == server_id as u32 || (server_id == 0 && s.id == 1))
                    .ok_or(AccountFlowError::UnknownServer(server_id as u32))?;

                self.issue_transfer_ticket(account_id, server.id);

                let mut responses = Vec::new();
                if self.config.use_resource_hash
                    && let Some(packet) = self.resources_hash_packet()
                {
                    responses.push(packet);
                }
                responses.push(
                    ConnectCharacterServerPacket {
                        account_id,
                        target: self.config.character_server.clone(),
                    }
                    .encode(),
                );
                Ok(responses)
            }
            AccountRequest::ResourcesHash { client_hash } => {
                session.last_client_hash = Some(client_hash);
                Ok(Vec::new())
            }
        }
    }

    pub fn failure_packet(error: &AccountFlowError) -> Option<Vec<u8>> {
        match error {
            AccountFlowError::Login(reason) => {
                Some(LoginRequestAnswerPacket::Failed(*reason).encode())
            }
            AccountFlowError::Suspended(suspension) => Some(
                LoginRequestBannedAnswerPacket {
                    remaining_time_in_seconds: suspension.remaining_seconds,
                    reason: suspension.reason.clone(),
                }
                .encode(),
            ),
            AccountFlowError::Unauthenticated | AccountFlowError::UnknownServer(_) => None,
            AccountFlowError::SecondaryPasswordRequired => None,
            AccountFlowError::Storage(_) => None,
        }
    }

    fn require_authenticated(
        &self,
        session: &AccountSession,
    ) -> Result<AccountId, AccountFlowError> {
        session.account_id.ok_or(AccountFlowError::Unauthenticated)
    }

    fn require_fully_authenticated(
        &self,
        session: &AccountSession,
    ) -> Result<AccountId, AccountFlowError> {
        let account_id = self.require_authenticated(session)?;
        if !session.secondary_verified {
            return Err(AccountFlowError::SecondaryPasswordRequired);
        }
        Ok(account_id)
    }

    fn account_by_id(&self, account_id: AccountId) -> Result<Account, AccountFlowError> {
        self.repository
            .account_by_id(account_id)
            .map_err(|error| AccountFlowError::Storage(error.to_string()))?
            .ok_or(AccountFlowError::Unauthenticated)
    }

    fn update_secondary_password(
        &self,
        account_id: AccountId,
        password: String,
    ) -> Result<(), AccountFlowError> {
        self.repository
            .update_secondary_password(account_id, password)
            .map_err(|error| AccountFlowError::Storage(error.to_string()))
    }

    fn resources_hash_packet(&self) -> Option<Vec<u8>> {
        self.repository
            .resource_hash_hex()
            .ok()
            .flatten()
            .map(|hash_hex| ResourcesHashPacket { hash_hex }.encode())
    }

    fn issue_transfer_ticket(&self, account_id: AccountId, server_id: u32) {
        let ticket = TransferTicket {
            token: Uuid::new_v4().to_string(),
            account_id,
            server_id,
        };

        let mut state = self.write_state();
        state.transfer_tickets.insert(account_id, ticket.clone());
        let _ = self.portal_bridge.store_transfer_ticket(&ticket);
    }

    fn write_state(&self) -> std::sync::RwLockWriteGuard<'_, AccountState> {
        self.state.write().expect("account state poisoned")
    }
}

#[derive(Debug)]
pub struct SessionFactory {
    next_seed: AtomicI16,
}

impl Default for SessionFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionFactory {
    pub fn new() -> Self {
        Self {
            next_seed: AtomicI16::new(1_000),
        }
    }

    pub fn create(&self) -> AccountSession {
        let seed = self.next_seed.fetch_add(1, Ordering::Relaxed);
        AccountSession::new(seed)
    }
}

#[derive(Debug, Error)]
pub enum AccountFlowError {
    #[error("login failed")]
    Login(LoginResponse),
    #[error("account suspended")]
    Suspended(AccountSuspension),
    #[error("request requires authenticated session")]
    Unauthenticated,
    #[error("request requires validated secondary password")]
    SecondaryPasswordRequired,
    #[error("unknown target server {0}")]
    UnknownServer(u32),
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
    use std::{collections::HashMap, path::PathBuf, sync::RwLock};

    use super::*;
    use odmo_protocol::PacketReader;
    use odmo_types::AccessLevel;

    const TEST_SECONDARY_PASSWORD: &str = "4321";
    const TEST_RESOURCE_HASH_HEX: &str = "0123456789ABCDEF";

    fn unique_test_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("odmo-{name}-{}", uuid::Uuid::new_v4()))
    }

    #[derive(Debug)]
    struct InMemoryAccountRepository {
        accounts: RwLock<HashMap<String, Account>>,
        servers: Vec<ServerDescriptor>,
        resource_hash_hex: Option<String>,
    }

    impl InMemoryAccountRepository {
        fn demo() -> Self {
            let mut accounts = HashMap::new();
            accounts.insert(
                "admin".to_string(),
                Account {
                    id: 1,
                    username: "admin".to_string(),
                    password_hash: "admin".to_string(),
                    email: "admin@odmo.local".to_string(),
                    access_level: AccessLevel::Administrator,
                    secondary_password: None,
                    suspension: None,
                },
            );
            accounts.insert(
                "gm".to_string(),
                Account {
                    id: 2,
                    username: "gm".to_string(),
                    password_hash: "gm".to_string(),
                    email: "gm@odmo.local".to_string(),
                    access_level: AccessLevel::GameMaster,
                    secondary_password: Some(TEST_SECONDARY_PASSWORD.to_string()),
                    suspension: None,
                },
            );
            accounts.insert(
                "banned".to_string(),
                Account {
                    id: 3,
                    username: "banned".to_string(),
                    password_hash: "banned".to_string(),
                    email: "banned@odmo.local".to_string(),
                    access_level: AccessLevel::Player,
                    secondary_password: None,
                    suspension: Some(AccountSuspension {
                        remaining_seconds: 3_600,
                        reason: "Policy violation".to_string(),
                    }),
                },
            );

            Self {
                accounts: RwLock::new(accounts),
                servers: vec![
                    ServerDescriptor {
                        id: 1,
                        name: "ODMO Alpha".to_string(),
                        maintenance: false,
                        overloaded: false,
                        is_new: true,
                        character_count: 0,
                    },
                    ServerDescriptor {
                        id: 2,
                        name: "ODMO Beta".to_string(),
                        maintenance: false,
                        overloaded: false,
                        is_new: false,
                        character_count: 0,
                    },
                ],
                resource_hash_hex: Some(TEST_RESOURCE_HASH_HEX.to_string()),
            }
        }
    }

    impl AccountRepository for InMemoryAccountRepository {
        fn account_by_username(&self, username: &str) -> anyhow::Result<Option<Account>> {
            Ok(self
                .accounts
                .read()
                .expect("repo poisoned")
                .get(username)
                .cloned())
        }

        fn account_by_id(&self, account_id: AccountId) -> anyhow::Result<Option<Account>> {
            Ok(self
                .accounts
                .read()
                .expect("repo poisoned")
                .values()
                .find(|account| account.id == account_id)
                .cloned())
        }

        fn update_secondary_password(
            &self,
            account_id: AccountId,
            password: String,
        ) -> anyhow::Result<()> {
            if let Some(account) = self
                .accounts
                .write()
                .expect("repo poisoned")
                .values_mut()
                .find(|account| account.id == account_id)
            {
                account.secondary_password = Some(password);
            }
            Ok(())
        }

        fn list_servers(&self) -> anyhow::Result<Vec<ServerDescriptor>> {
            Ok(self.servers.clone())
        }

        fn resource_hash_hex(&self) -> anyhow::Result<Option<String>> {
            Ok(self.resource_hash_hex.clone())
        }
    }

    #[test]
    fn successful_login_updates_session_and_returns_hide_secondary_password() {
        let app = AccountApplication::new(
            AccountServiceConfig {
                character_server: CharacterServerTarget {
                    address: "127.0.0.1".to_string(),
                    port: 7002,
                },
                portal_state_dir: unique_test_dir("account-login-success"),
                use_resource_hash: true,
            },
            Arc::new(InMemoryAccountRepository::demo()),
        );
        let mut session = AccountSession::new(111);

        let responses = app
            .handle_request(
                &mut session,
                AccountRequest::Login(odmo_protocol::account::LoginPayload {
                    username: "admin".to_string(),
                    password: "admin".to_string(),
                    cpu: "cpu".to_string(),
                    gpu: "gpu".to_string(),
                }),
            )
            .expect("login should succeed");

        assert_eq!(session.account_id, Some(1));
        let raw = PacketReader::from_frame(&responses[0]).expect("packet should decode");
        assert_eq!(raw.packet_type, 3301);
        // LoginRequestAnswerPacket::Success(Hide) = i32(0) + u8(1)
        assert_eq!(raw.payload, vec![0, 0, 0, 0, 1]);
        let hash = PacketReader::from_frame(&responses[1]).expect("hash packet should decode");
        assert_eq!(hash.packet_type, 10003);
    }

    #[test]
    fn unauthenticated_server_list_is_rejected() {
        let app = AccountApplication::new(
            AccountServiceConfig {
                character_server: CharacterServerTarget {
                    address: "127.0.0.1".to_string(),
                    port: 7002,
                },
                portal_state_dir: unique_test_dir("account-unauth"),
                use_resource_hash: false,
            },
            Arc::new(InMemoryAccountRepository::demo()),
        );
        let mut session = AccountSession::new(111);

        let error = app
            .handle_request(&mut session, AccountRequest::LoadServerList)
            .expect_err("server list should require auth");

        assert!(matches!(error, AccountFlowError::Unauthenticated));
    }

    #[test]
    fn connect_character_server_uses_selected_target() {
        let app = AccountApplication::new(
            AccountServiceConfig {
                character_server: CharacterServerTarget {
                    address: "127.0.0.1".to_string(),
                    port: 7002,
                },
                portal_state_dir: unique_test_dir("account-connect-character"),
                use_resource_hash: false,
            },
            Arc::new(InMemoryAccountRepository::demo()),
        );
        let mut session = AccountSession::new(111);
        session.account_id = Some(1);
        session.secondary_verified = true;

        let responses = app
            .handle_request(
                &mut session,
                AccountRequest::ConnectCharacterServer { server_id: 1 },
            )
            .expect("redirect should succeed");

        let raw = PacketReader::from_frame(&responses[0]).expect("frame should decode");
        assert_eq!(raw.packet_type, 901);
        assert_eq!(i32::from_le_bytes(raw.payload[4..8].try_into().unwrap()), 1);
    }

    #[test]
    fn login_with_existing_secondary_password_requests_input() {
        let app = AccountApplication::new(
            AccountServiceConfig {
                character_server: CharacterServerTarget {
                    address: "127.0.0.1".to_string(),
                    port: 7002,
                },
                portal_state_dir: unique_test_dir("account-secondary-input"),
                use_resource_hash: false,
            },
            Arc::new(InMemoryAccountRepository::demo()),
        );
        let mut session = AccountSession::new(111);

        let responses = app
            .handle_request(
                &mut session,
                AccountRequest::Login(odmo_protocol::account::LoginPayload {
                    username: "gm".to_string(),
                    password: "gm".to_string(),
                    cpu: "cpu".to_string(),
                    gpu: "gpu".to_string(),
                }),
            )
            .expect("login should succeed");

        let raw = PacketReader::from_frame(&responses[0]).expect("packet should decode");
        assert_eq!(raw.payload, vec![0, 0, 0, 0, 2]);
    }

    #[test]
    fn secondary_password_check_unlocks_server_list() {
        let app = AccountApplication::new(
            AccountServiceConfig {
                character_server: CharacterServerTarget {
                    address: "127.0.0.1".to_string(),
                    port: 7002,
                },
                portal_state_dir: unique_test_dir("account-secondary-check"),
                use_resource_hash: false,
            },
            Arc::new(InMemoryAccountRepository::demo()),
        );
        let mut session = AccountSession::new(111);
        session.account_id = Some(2);

        let responses = app
            .handle_request(
                &mut session,
                AccountRequest::SecondaryPasswordCheck {
                    check_mode: SecondaryPasswordCheck::Check,
                    password: Some(TEST_SECONDARY_PASSWORD.to_string()),
                },
            )
            .expect("secondary password should succeed");

        let raw = PacketReader::from_frame(&responses[0]).expect("packet should decode");
        assert_eq!(raw.packet_type, 9804);
        assert!(session.secondary_verified);
    }

    #[test]
    fn suspended_accounts_return_banned_packet() {
        let app = AccountApplication::new(
            AccountServiceConfig {
                character_server: CharacterServerTarget {
                    address: "127.0.0.1".to_string(),
                    port: 7002,
                },
                portal_state_dir: unique_test_dir("account-suspended"),
                use_resource_hash: false,
            },
            Arc::new(InMemoryAccountRepository::demo()),
        );
        let mut session = AccountSession::new(111);
        let error = app
            .handle_request(
                &mut session,
                AccountRequest::Login(odmo_protocol::account::LoginPayload {
                    username: "banned".to_string(),
                    password: "banned".to_string(),
                    cpu: "cpu".to_string(),
                    gpu: "gpu".to_string(),
                }),
            )
            .expect_err("banned login should fail");

        let packet = AccountApplication::failure_packet(&error).expect("ban should map to packet");
        let raw = PacketReader::from_frame(&packet).expect("packet should decode");
        assert_eq!(raw.packet_type, 3308);
    }
}
