use zero_traits::{
    ClientTlsProfile, GrpcTransportProfile, H2TransportProfile, HttpUpgradeTransportProfile,
    ServerTlsProfile, SplitHttpTransportProfile, WebSocketTransportProfile,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedServerTlsProfile {
    pub cert_path: String,
    pub key_path: String,
    pub alpn: Vec<String>,
    pub server_fingerprint: Option<String>,
}

impl OwnedServerTlsProfile {
    pub fn from_profile(profile: &(impl ServerTlsProfile + ?Sized)) -> Self {
        Self {
            cert_path: profile.cert_path().to_owned(),
            key_path: profile.key_path().to_owned(),
            alpn: profile.alpn().to_vec(),
            server_fingerprint: profile.server_fingerprint().map(str::to_owned),
        }
    }
}

impl ServerTlsProfile for OwnedServerTlsProfile {
    fn cert_path(&self) -> &str {
        &self.cert_path
    }

    fn key_path(&self) -> &str {
        &self.key_path
    }

    fn alpn(&self) -> &[String] {
        self.alpn.as_slice()
    }

    fn server_fingerprint(&self) -> Option<&str> {
        self.server_fingerprint.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedClientTlsProfile {
    pub server_name: Option<String>,
    pub disable_sni: bool,
    pub ca_cert_path: Option<String>,
    pub insecure: bool,
    pub alpn: Vec<String>,
    pub client_fingerprint: Option<String>,
}

impl OwnedClientTlsProfile {
    pub fn from_profile(profile: &(impl ClientTlsProfile + ?Sized)) -> Self {
        Self {
            server_name: profile.server_name().map(str::to_owned),
            disable_sni: profile.disable_sni(),
            ca_cert_path: profile.ca_cert_path().map(str::to_owned),
            insecure: profile.insecure(),
            alpn: profile.alpn().to_vec(),
            client_fingerprint: profile.client_fingerprint().map(str::to_owned),
        }
    }
}

impl ClientTlsProfile for OwnedClientTlsProfile {
    fn server_name(&self) -> Option<&str> {
        self.server_name.as_deref()
    }

    fn disable_sni(&self) -> bool {
        self.disable_sni
    }

    fn ca_cert_path(&self) -> Option<&str> {
        self.ca_cert_path.as_deref()
    }

    fn insecure(&self) -> bool {
        self.insecure
    }

    fn alpn(&self) -> &[String] {
        self.alpn.as_slice()
    }

    fn client_fingerprint(&self) -> Option<&str> {
        self.client_fingerprint.as_deref()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedWebSocketProfile {
    pub path: String,
    pub headers: Vec<(String, String)>,
}

impl OwnedWebSocketProfile {
    pub fn from_profile(profile: &(impl WebSocketTransportProfile + ?Sized)) -> Self {
        Self {
            path: profile.path().to_owned(),
            headers: profile.header_pairs(),
        }
    }
}

impl WebSocketTransportProfile for OwnedWebSocketProfile {
    fn path(&self) -> &str {
        &self.path
    }

    fn header_pairs(&self) -> Vec<(String, String)> {
        self.headers.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedGrpcProfile {
    pub service_names: Vec<String>,
}

impl OwnedGrpcProfile {
    pub fn from_profile(profile: &(impl GrpcTransportProfile + ?Sized)) -> Self {
        Self {
            service_names: profile.service_names().to_vec(),
        }
    }
}

impl GrpcTransportProfile for OwnedGrpcProfile {
    fn service_names(&self) -> &[String] {
        self.service_names.as_slice()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedH2Profile {
    pub host: Option<String>,
    pub path: String,
}

impl OwnedH2Profile {
    pub fn from_profile(profile: &(impl H2TransportProfile + ?Sized)) -> Self {
        Self {
            host: profile.host().map(str::to_owned),
            path: profile.path().to_owned(),
        }
    }
}

impl H2TransportProfile for OwnedH2Profile {
    fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedHttpUpgradeProfile {
    pub host: Option<String>,
    pub path: String,
}

impl OwnedHttpUpgradeProfile {
    pub fn from_profile(profile: &(impl HttpUpgradeTransportProfile + ?Sized)) -> Self {
        Self {
            host: profile.host().map(str::to_owned),
            path: profile.path().to_owned(),
        }
    }
}

impl HttpUpgradeTransportProfile for OwnedHttpUpgradeProfile {
    fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    fn path(&self) -> &str {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedSplitHttpProfile {
    pub host: Option<String>,
    pub path: String,
    pub mode: String,
}

impl OwnedSplitHttpProfile {
    pub fn from_profile(profile: &(impl SplitHttpTransportProfile + ?Sized)) -> Self {
        Self {
            host: profile.host().map(str::to_owned),
            path: profile.path().to_owned(),
            mode: profile.mode().to_owned(),
        }
    }
}

impl SplitHttpTransportProfile for OwnedSplitHttpProfile {
    fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn mode(&self) -> &str {
        &self.mode
    }
}
