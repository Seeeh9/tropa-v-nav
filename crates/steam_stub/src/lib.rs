//! Stands in for the `steamworks` crate where Steam does not run. `Client::init_app` always fails,
//! so the game takes its no-Steam paths and no other call here is ever reached.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct SteamId(u64);
impl SteamId {
    pub fn from_raw(id: u64) -> SteamId {
        SteamId(id)
    }
    pub fn raw(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct LobbyId(u64);
impl LobbyId {
    pub fn from_raw(id: u64) -> LobbyId {
        LobbyId(id)
    }
    pub fn raw(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone)]
pub struct SteamError;
impl fmt::Display for SteamError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("Steam does not run on this system")
    }
}
impl std::error::Error for SteamError {}

#[derive(Clone)]
pub struct Client(());

impl Client {
    pub fn init_app(_app_id: u32) -> Result<Client, SteamError> {
        Err(SteamError)
    }
    pub fn run_callbacks(&self) {}
    pub fn register_callback<C, F: FnMut(C) + Send + 'static>(&self, _f: F) -> CallbackHandle {
        CallbackHandle
    }
    pub fn utils(&self) -> Utils {
        Utils
    }
    pub fn user(&self) -> User {
        User
    }
    pub fn friends(&self) -> Friends {
        Friends
    }
    pub fn matchmaking(&self) -> Matchmaking {
        Matchmaking
    }
    pub fn networking_messages(&self) -> networking_messages::NetworkingMessages {
        networking_messages::NetworkingMessages
    }
}

pub struct CallbackHandle;
pub struct Utils;
impl Utils {
    pub fn ui_language(&self) -> String {
        String::new()
    }
    pub fn is_overlay_enabled(&self) -> bool {
        false
    }
}
pub struct User;
impl User {
    pub fn steam_id(&self) -> SteamId {
        SteamId(0)
    }
}
pub struct Friends;
impl Friends {
    pub fn name(&self) -> String {
        String::new()
    }
    pub fn set_rich_presence(&self, _key: &str, _value: Option<&str>) -> bool {
        false
    }
    pub fn activate_invite_dialog(&self, _lobby: LobbyId) {}
}

#[derive(Debug, Clone, Copy)]
pub enum LobbyType {
    Private,
    FriendsOnly,
    Public,
    Invisible,
}
pub struct LobbyKey(String);
impl LobbyKey {
    pub fn new(key: &str) -> LobbyKey {
        LobbyKey(key.to_string())
    }
}
#[derive(Debug, Clone, Copy)]
pub enum StringFilterKind {
    EqualToOrLessThan,
    LessThan,
    Equal,
    GreaterThan,
    EqualToOrGreaterThan,
    NotEqual,
}
pub struct StringFilter<'a>(pub LobbyKey, pub &'a str, pub StringFilterKind);
#[derive(Default)]
pub struct LobbyListFilter<'a> {
    pub string: Option<Vec<StringFilter<'a>>>,
}

pub struct Matchmaking;
impl Matchmaking {
    pub fn create_lobby<F: FnOnce(Result<LobbyId, SteamError>) + Send + 'static>(&self, _ty: LobbyType, _max: u32, _cb: F) {}
    pub fn set_lobby_list_filter(&self, _filter: LobbyListFilter<'_>) -> &Self {
        self
    }
    pub fn request_lobby_list<F: FnOnce(Result<Vec<LobbyId>, SteamError>) + Send + 'static>(&self, _cb: F) {}
    pub fn join_lobby<F: FnOnce(Result<LobbyId, ()>) + Send + 'static>(&self, _lobby: LobbyId, _cb: F) {}
    pub fn leave_lobby(&self, _lobby: LobbyId) {}
    pub fn set_lobby_data(&self, _lobby: LobbyId, _key: &str, _value: &str) -> bool {
        false
    }
    pub fn lobby_data(&self, _lobby: LobbyId, _key: &str) -> Option<String> {
        None
    }
    pub fn lobby_owner(&self, _lobby: LobbyId) -> SteamId {
        SteamId(0)
    }
    pub fn lobby_members(&self, _lobby: LobbyId) -> Vec<SteamId> {
        Vec::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ChatMemberStateChange {
    Entered,
    Left,
    Disconnected,
    Kicked,
    Banned,
}
pub struct LobbyChatUpdate {
    pub lobby: LobbyId,
    pub user_changed: SteamId,
    pub making_change: SteamId,
    pub member_state_change: ChatMemberStateChange,
}
pub struct GameLobbyJoinRequested {
    pub lobby_steam_id: LobbyId,
    pub friend_steam_id: SteamId,
}

pub mod networking_types {
    use super::SteamId;

    pub struct NetworkingIdentity(Option<SteamId>);
    impl NetworkingIdentity {
        pub fn new_steam_id(id: SteamId) -> NetworkingIdentity {
            NetworkingIdentity(Some(id))
        }
        pub fn steam_id(&self) -> Option<SteamId> {
            self.0
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SendFlags(u32);
    impl SendFlags {
        pub const RELIABLE_NO_NAGLE: SendFlags = SendFlags(9);
        pub const AUTO_RESTART_BROKEN_SESSION: SendFlags = SendFlags(32);
    }
    impl std::ops::BitOr for SendFlags {
        type Output = SendFlags;
        fn bitor(self, o: SendFlags) -> SendFlags {
            SendFlags(self.0 | o.0)
        }
    }

    pub struct NetworkingMessage(());
    impl NetworkingMessage {
        pub fn identity_peer(&self) -> NetworkingIdentity {
            NetworkingIdentity(None)
        }
        pub fn data(&self) -> &[u8] {
            &[]
        }
    }
}

pub mod networking_messages {
    use super::networking_types::{NetworkingIdentity, NetworkingMessage, SendFlags};
    use super::SteamError;

    pub struct SessionRequest(NetworkingIdentity);
    impl SessionRequest {
        pub fn remote(&self) -> &NetworkingIdentity {
            &self.0
        }
        pub fn accept(self) -> bool {
            false
        }
        pub fn reject(self) {}
    }

    pub struct NetworkingMessages;
    impl NetworkingMessages {
        pub fn session_request_callback<F: FnMut(SessionRequest) + Send + 'static>(&self, _f: F) {}
        pub fn receive_messages_on_channel(&self, _channel: u32, _max: usize) -> Vec<NetworkingMessage> {
            Vec::new()
        }
        pub fn send_message_to_user(&self, _to: NetworkingIdentity, _flags: SendFlags, _data: &[u8], _channel: u32) -> Result<(), SteamError> {
            Err(SteamError)
        }
    }
}
