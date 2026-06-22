//! Verify ring::pbkdf2 output for mieru-style key derivation.
//! Can be compared against Go's golang.org/x/crypto/pbkdf2.

#![cfg(feature = "crypto")]

/// Verify that our specific mieru key derivation matches Go expectation.
/// Matches upstream cipher.HashPassword + pbkdf2Gen:
///   hashedPassword = SHA-256(password || 0x00 || username)
///   salt           = SHA-256(uint64_be(timestamp))
///   key            = PBKDF2-HMAC-SHA256(hashedPassword, salt, 64, 32)
/// Run the Go snippet below to verify the output matches.
#[test]
fn test_mieru_style_key() {
    use sha2::Digest;

    let username = "testuser";
    let password = "testpass";
    let timestamp: u64 = 0x0000000012345678;

    // Step 1: hashedPassword = SHA-256(password || 0x00 || username)
    let mut pw = sha2::Sha256::new();
    pw.update(password.as_bytes());
    pw.update([0x00]);
    pw.update(username.as_bytes());
    let hashed_password = pw.finalize();

    // Step 2: salt = SHA-256(uint64_be(timestamp))
    let mut hasher = sha2::Sha256::new();
    hasher.update(timestamp.to_be_bytes());
    let salt = hasher.finalize();

    eprintln!("username: {username}");
    eprintln!("password: {password}");
    eprintln!("timestamp: {timestamp} (hex: {timestamp:016x})");
    eprint!("hashedPassword: ");
    for b in hashed_password.iter() {
        eprint!("{b:02x}");
    }
    eprintln!();
    eprint!("salt: ");
    for b in salt.iter() {
        eprint!("{b:02x}");
    }
    eprintln!();

    // Step 3: key = PBKDF2-HMAC-SHA256(hashedPassword, salt, 64, 32)
    let mut key = [0u8; 32];
    ring::pbkdf2::derive(
        ring::pbkdf2::PBKDF2_HMAC_SHA256,
        std::num::NonZeroU32::new(64).unwrap(),
        &salt,
        &hashed_password,
        &mut key,
    );

    eprint!("key:  ");
    for b in key.iter() {
        eprint!("{b:02x}");
    }
    eprintln!();
    eprintln!();
    eprintln!("Compare with Go output:");
    eprintln!("  go run -mod=mod - <<'EOF'");
    eprintln!("  package main");
    eprintln!("  import (\"crypto/sha256\"; \"encoding/binary\"; \"encoding/hex\"; \"fmt\"; \"golang.org/x/crypto/pbkdf2\")");
    eprintln!("  func main() {{");
    eprintln!("    username := []byte(\"testuser\")");
    eprintln!("    password := []byte(\"testpass\")");
    eprintln!("    hashedPassword := sha256.Sum256(append(append(password, 0x00), username...))");
    eprintln!("    var ts uint64 = 0x0000000012345678");
    eprintln!("    var b [8]byte");
    eprintln!("    binary.BigEndian.PutUint64(b[:], ts)");
    eprintln!("    salt := sha256.Sum256(b[:])");
    eprintln!("    key := pbkdf2.Key(hashedPassword[:], salt[:], 64, 32, sha256.New)");
    eprintln!("    fmt.Printf(\"key: %x\\n\", key)");
    eprintln!("  }}");
    eprintln!("  EOF");
}
