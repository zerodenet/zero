use zero_core::Error;

pub fn validate_credential_part(value: &str, field: &'static str) -> Result<(), Error> {
    let len = value.len();
    if len == 0 {
        return Err(Error::Config(match field {
            "username" => "SOCKS5 username must not be empty",
            "password" => "SOCKS5 password must not be empty",
            _ => "SOCKS5 credential must not be empty",
        }));
    }
    if len > u8::MAX as usize {
        return Err(Error::Config(match field {
            "username" => "SOCKS5 username is too long",
            "password" => "SOCKS5 password is too long",
            _ => "SOCKS5 credential is too long",
        }));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_credential_part;

    #[test]
    fn rejects_empty_credentials() {
        assert!(validate_credential_part("", "username").is_err());
        assert!(validate_credential_part("", "password").is_err());
    }

    #[test]
    fn rejects_oversized_credentials() {
        let oversized = "a".repeat(256);
        assert!(validate_credential_part(&oversized, "username").is_err());
        assert!(validate_credential_part(&oversized, "password").is_err());
    }

    #[test]
    fn accepts_u8_sized_credentials() {
        let max_len = "a".repeat(255);
        assert!(validate_credential_part(&max_len, "username").is_ok());
        assert!(validate_credential_part(&max_len, "password").is_ok());
    }
}
