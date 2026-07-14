#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedVlessRealityClientProfile {
    pub public_key: String,
    pub short_id: String,
    pub server_name: Option<String>,
    pub cipher_suites: Vec<String>,
}

impl OwnedVlessRealityClientProfile {
    pub fn new(
        public_key: impl Into<String>,
        short_id: impl Into<String>,
        server_name: Option<String>,
        cipher_suites: Vec<String>,
    ) -> Self {
        Self {
            public_key: public_key.into(),
            short_id: short_id.into(),
            server_name,
            cipher_suites,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedVlessQuicClientProfile {
    pub server_name: Option<String>,
    pub insecure: bool,
    pub ca_cert_path: Option<String>,
}

impl OwnedVlessQuicClientProfile {
    pub fn new(server_name: Option<String>, insecure: bool, ca_cert_path: Option<String>) -> Self {
        Self {
            server_name,
            insecure,
            ca_cert_path,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnedVlessQuicBindProfile {
    pub cert_path: Option<String>,
    pub key_path: Option<String>,
}

impl OwnedVlessQuicBindProfile {
    pub fn new(cert_path: Option<String>, key_path: Option<String>) -> Self {
        Self {
            cert_path,
            key_path,
        }
    }
}
