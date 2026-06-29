use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use zero_core::{Address, Error, Network, ProtocolType, Session};
use zero_traits::AsyncSocket;

use crate::shared::{
    read_address, read_exact, write_reply, write_reply_with_address, Socks5Reply, CMD_CONNECT,
    CMD_UDP_ASSOCIATE, METHOD_NOT_ACCEPTABLE, METHOD_NO_AUTH, METHOD_USERNAME_PASSWORD,
    SOCKS5_VERSION, USERPASS_STATUS_FAILURE, USERPASS_STATUS_SUCCESS, USERPASS_VERSION,
};
use crate::udp::Socks5InboundUdpSession;

#[derive(Debug, Default, Clone, Copy)]
pub struct Socks5Inbound;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Socks5Request {
    Connect(Box<Session>),
    UdpAssociate(Socks5UdpAssociateRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Socks5UdpAssociateRequest {
    pub client_hint: Address,
    pub client_port: u16,
}

pub trait Socks5PasswordAuth {
    fn required(&self) -> bool;
    fn verify(&self, username: &str, password: &str) -> bool;
    /// Returns the principal_key for a successfully authenticated user.
    /// Defaults to the username if not overridden.
    fn principal_key_for(&self, username: &str) -> Option<String> {
        Some(String::from(username))
    }
    /// Returns `(up_bps, down_bps)` for the authenticated user.
    fn rate_limit_for(&self, _username: &str) -> (Option<u64>, Option<u64>) {
        (None, None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredSocks5User {
    username: String,
    password: String,
    principal_key: Option<String>,
    up_bps: Option<u64>,
    down_bps: Option<u64>,
}

impl ConfiguredSocks5User {
    pub fn new(
        username: String,
        password: String,
        principal_key: Option<String>,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Self {
        Self {
            username,
            password,
            principal_key,
            up_bps,
            down_bps,
        }
    }

    pub fn from_config_parts(
        username: String,
        password: String,
        principal_key: Option<String>,
        up_bps: Option<u64>,
        down_bps: Option<u64>,
    ) -> Self {
        Self::new(username, password, principal_key, up_bps, down_bps)
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfiguredSocks5PasswordAuth {
    users: Vec<ConfiguredSocks5User>,
}

impl ConfiguredSocks5PasswordAuth {
    pub fn from_users(users: Vec<ConfiguredSocks5User>) -> Self {
        Self { users }
    }

    pub fn from_config_parts<I>(users: I) -> Self
    where
        I: IntoIterator<Item = (String, String, Option<String>, Option<u64>, Option<u64>)>,
    {
        Self::from_users(
            users
                .into_iter()
                .map(|(username, password, principal_key, up_bps, down_bps)| {
                    ConfiguredSocks5User::from_config_parts(
                        username,
                        password,
                        principal_key,
                        up_bps,
                        down_bps,
                    )
                })
                .collect(),
        )
    }

    pub fn from_config_users<I, U>(users: I) -> Self
    where
        I: IntoIterator<Item = U>,
        U: IntoSocks5AuthUserConfig,
    {
        Self::from_config_parts(users.into_iter().map(U::into_socks5_auth_user_config))
    }
}

impl Socks5PasswordAuth for ConfiguredSocks5PasswordAuth {
    fn required(&self) -> bool {
        !self.users.is_empty()
    }

    fn verify(&self, username: &str, password: &str) -> bool {
        self.users
            .iter()
            .any(|user| user.username == username && user.password == password)
    }

    fn principal_key_for(&self, username: &str) -> Option<String> {
        self.users
            .iter()
            .find(|user| user.username == username)
            .and_then(|user| user.principal_key.clone())
    }

    fn rate_limit_for(&self, username: &str) -> (Option<u64>, Option<u64>) {
        self.users
            .iter()
            .find(|user| user.username == username)
            .map(|user| (user.up_bps, user.down_bps))
            .unwrap_or((None, None))
    }
}

pub trait IntoSocks5AuthUserConfig {
    fn into_socks5_auth_user_config(
        self,
    ) -> (String, String, Option<String>, Option<u64>, Option<u64>);
}

impl IntoSocks5AuthUserConfig for (String, String, Option<String>, Option<u64>, Option<u64>) {
    fn into_socks5_auth_user_config(
        self,
    ) -> (String, String, Option<String>, Option<u64>, Option<u64>) {
        self
    }
}

impl IntoSocks5AuthUserConfig for (&str, &str, Option<&str>, Option<u64>, Option<u64>) {
    fn into_socks5_auth_user_config(
        self,
    ) -> (String, String, Option<String>, Option<u64>, Option<u64>) {
        (
            self.0.to_owned(),
            self.1.to_owned(),
            self.2.map(ToOwned::to_owned),
            self.3,
            self.4,
        )
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct NoSocks5PasswordAuth;

impl Socks5PasswordAuth for NoSocks5PasswordAuth {
    fn required(&self) -> bool {
        false
    }

    fn verify(&self, _username: &str, _password: &str) -> bool {
        false
    }
}

impl Socks5Inbound {
    pub fn protocol(&self) -> ProtocolType {
        ProtocolType::Socks5
    }

    pub fn udp_session(&self) -> Socks5InboundUdpSession {
        Socks5InboundUdpSession::new()
    }

    pub async fn accept_request<S>(&self, stream: &mut S) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        match self.accept_command(stream).await? {
            Socks5Request::Connect(session) => Ok(*session),
            Socks5Request::UdpAssociate(_) => {
                write_reply(stream, Socks5Reply::CommandNotSupported).await?;
                Err(Error::Unsupported("SOCKS5 command is not supported"))
            }
        }
    }

    pub async fn accept_command<S>(&self, stream: &mut S) -> Result<Socks5Request, Error>
    where
        S: AsyncSocket,
    {
        self.accept_command_with_auth(stream, &NoSocks5PasswordAuth)
            .await
    }

    pub async fn accept_command_with_auth<S, A>(
        &self,
        stream: &mut S,
        auth: &A,
    ) -> Result<Socks5Request, Error>
    where
        S: AsyncSocket,
        A: Socks5PasswordAuth,
    {
        let username = negotiate_method(stream, auth).await?;
        let mut request = read_request(stream).await?;
        if let (Some(name), Socks5Request::Connect(ref mut session)) =
            (username.as_ref(), &mut request)
        {
            let pk = auth.principal_key_for(name);
            let (up, down) = auth.rate_limit_for(name);
            let mut sa = zero_core::SessionAuth::new("socks5");
            sa.principal_key = pk;
            sa.up_bps = up;
            sa.down_bps = down;
            session.apply_auth(sa);
        }
        Ok(request)
    }

    pub async fn send_response<S>(&self, stream: &mut S, reply: Socks5Reply) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        write_reply(stream, reply).await
    }

    pub async fn send_success_response<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_response(stream, Socks5Reply::Succeeded).await
    }

    pub async fn send_blocked_response<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_response(stream, Socks5Reply::ConnectionNotAllowed)
            .await
    }

    pub async fn send_upstream_failure_response<S>(&self, stream: &mut S) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_response(stream, Socks5Reply::HostUnreachable)
            .await
    }

    pub async fn send_response_with_bound<S>(
        &self,
        stream: &mut S,
        reply: Socks5Reply,
        address: &Address,
        port: u16,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        write_reply_with_address(stream, reply, address, port).await
    }

    pub async fn send_success_response_with_bound<S>(
        &self,
        stream: &mut S,
        address: &Address,
        port: u16,
    ) -> Result<(), Error>
    where
        S: AsyncSocket,
    {
        self.send_response_with_bound(stream, Socks5Reply::Succeeded, address, port)
            .await
    }

    pub async fn handshake<S>(&self, stream: &mut S) -> Result<Session, Error>
    where
        S: AsyncSocket,
    {
        let session = self
            .handshake_with_auth(stream, &NoSocks5PasswordAuth)
            .await?;

        Ok(session)
    }

    pub async fn handshake_with_auth<S, A>(
        &self,
        stream: &mut S,
        auth: &A,
    ) -> Result<Session, Error>
    where
        S: AsyncSocket,
        A: Socks5PasswordAuth,
    {
        let session = match self.accept_command_with_auth(stream, auth).await? {
            Socks5Request::Connect(session) => *session,
            Socks5Request::UdpAssociate(_) => {
                write_reply(stream, Socks5Reply::CommandNotSupported).await?;
                return Err(Error::Unsupported("SOCKS5 command is not supported"));
            }
        };
        self.send_response(stream, Socks5Reply::Succeeded).await?;

        Ok(session)
    }
}

async fn negotiate_method<S, A>(stream: &mut S, auth: &A) -> Result<Option<String>, Error>
where
    S: AsyncSocket,
    A: Socks5PasswordAuth,
{
    let mut header = [0_u8; 2];
    read_exact(stream, &mut header).await?;

    if header[0] != SOCKS5_VERSION {
        return Err(Error::Protocol("invalid SOCKS5 version"));
    }

    let method_count = header[1] as usize;
    if method_count == 0 {
        return Err(Error::Protocol("SOCKS5 method list is empty"));
    }

    let mut methods = vec![0_u8; method_count];
    read_exact(stream, &mut methods).await?;

    let selected_method = select_method(&methods, auth.required());

    if selected_method == METHOD_NOT_ACCEPTABLE {
        stream
            .write_all(&[SOCKS5_VERSION, METHOD_NOT_ACCEPTABLE])
            .await
            .map_err(|_| Error::Io("failed to write SOCKS5 auth negotiation response"))?;
        return Err(Error::Unsupported("SOCKS5 auth method is not supported"));
    }

    stream
        .write_all(&[SOCKS5_VERSION, selected_method])
        .await
        .map_err(|_| Error::Io("failed to write SOCKS5 auth negotiation response"))?;

    if selected_method == METHOD_USERNAME_PASSWORD {
        return authenticate_username_password(stream, auth).await;
    }

    Ok(None)
}

fn select_method(methods: &[u8], password_required: bool) -> u8 {
    if password_required {
        return if methods.contains(&METHOD_USERNAME_PASSWORD) {
            METHOD_USERNAME_PASSWORD
        } else {
            METHOD_NOT_ACCEPTABLE
        };
    }

    if methods.contains(&METHOD_NO_AUTH) {
        METHOD_NO_AUTH
    } else {
        METHOD_NOT_ACCEPTABLE
    }
}

async fn authenticate_username_password<S, A>(
    stream: &mut S,
    auth: &A,
) -> Result<Option<String>, Error>
where
    S: AsyncSocket,
    A: Socks5PasswordAuth,
{
    let mut header = [0_u8; 2];
    read_exact(stream, &mut header).await?;

    if header[0] != USERPASS_VERSION {
        return Err(Error::Protocol(
            "invalid SOCKS5 username/password auth version",
        ));
    }

    let username_len = header[1] as usize;
    if username_len == 0 {
        return Err(Error::Protocol("SOCKS5 username must not be empty"));
    }

    let mut username = vec![0_u8; username_len];
    read_exact(stream, &mut username).await?;

    let mut password_len = [0_u8; 1];
    read_exact(stream, &mut password_len).await?;
    let password_len = password_len[0] as usize;
    if password_len == 0 {
        return Err(Error::Protocol("SOCKS5 password must not be empty"));
    }

    let mut password = vec![0_u8; password_len];
    read_exact(stream, &mut password).await?;

    let username = String::from_utf8(username)
        .map_err(|_| Error::Protocol("SOCKS5 username is not valid UTF-8"))?;
    let password = String::from_utf8(password)
        .map_err(|_| Error::Protocol("SOCKS5 password is not valid UTF-8"))?;

    let accepted = auth.verify(&username, &password);
    let status = if accepted {
        USERPASS_STATUS_SUCCESS
    } else {
        USERPASS_STATUS_FAILURE
    };
    stream
        .write_all(&[USERPASS_VERSION, status])
        .await
        .map_err(|_| Error::Io("failed to write SOCKS5 username/password auth response"))?;

    if accepted {
        Ok(Some(username))
    } else {
        Err(Error::Unsupported(
            "SOCKS5 username/password authentication failed",
        ))
    }
}

async fn read_request<S>(stream: &mut S) -> Result<Socks5Request, Error>
where
    S: AsyncSocket,
{
    let mut header = [0_u8; 4];
    read_exact(stream, &mut header).await?;

    if header[0] != SOCKS5_VERSION {
        return Err(Error::Protocol("invalid SOCKS5 request version"));
    }

    let address = match read_address(stream, header[3]).await {
        Ok(address) => address,
        Err(Error::Unsupported(_)) => {
            write_reply(stream, Socks5Reply::AddressTypeNotSupported).await?;
            return Err(Error::Unsupported("SOCKS5 address type is not supported"));
        }
        Err(error) => return Err(error),
    };

    let mut port = [0_u8; 2];
    read_exact(stream, &mut port).await?;

    let port = u16::from_be_bytes(port);

    match header[1] {
        CMD_CONNECT => Ok(Socks5Request::Connect(Box::new(Session::new(
            0,
            address,
            port,
            Network::Tcp,
            ProtocolType::Socks5,
        )))),
        CMD_UDP_ASSOCIATE => Ok(Socks5Request::UdpAssociate(Socks5UdpAssociateRequest {
            client_hint: address,
            client_port: port,
        })),
        _ => {
            write_reply(stream, Socks5Reply::CommandNotSupported).await?;
            Err(Error::Unsupported("SOCKS5 command is not supported"))
        }
    }
}
