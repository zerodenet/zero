use std::io::{self};
use zero_protocol_vless::reality::common::*;
use zero_protocol_vless::reality::reality_aead::AeadKey;
use zero_protocol_vless::reality::reality_cipher_suite::CipherSuite;
use zero_protocol_vless::reality::reality_records::*;

const CS: CipherSuite = CipherSuite::AES_128_GCM_SHA256;

fn encrypt_plaintext_to_records(
    cipher_suite: CipherSuite,
    plaintext: &mut Vec<u8>,
    key: &[u8],
    iv: &[u8],
    write_seq: &mut u64,
    ciphertext_buf: &mut Vec<u8>,
) -> io::Result<()> {
    let aead_key = AeadKey::new(cipher_suite, key)?;
    let mut encryptor = RecordEncryptor::new(&aead_key, iv, write_seq);
    encryptor.encrypt_app_data(plaintext, ciphertext_buf)
}

fn encrypt_handshake_to_records(
    cipher_suite: CipherSuite,
    handshake_data: &[u8],
    key: &[u8],
    iv: &[u8],
    write_seq: &mut u64,
    ciphertext_buf: &mut Vec<u8>,
) -> io::Result<()> {
    let aead_key = AeadKey::new(cipher_suite, key)?;
    let mut encryptor = RecordEncryptor::new(&aead_key, iv, write_seq);
    encryptor.encrypt_handshake(handshake_data, ciphertext_buf)
}

/// Test helper: decrypt a record by allocating and calling decrypt_record_in_place
fn decrypt_record(
    decryptor: &mut RecordDecryptor<'_>,
    ciphertext: &[u8],
    record_len: u16,
) -> io::Result<(u8, Vec<u8>)> {
    let mut buf = ciphertext.to_vec();
    let (content_type, plaintext) = decryptor.decrypt_record_in_place(&mut buf, record_len)?;
    Ok((content_type, plaintext.to_vec()))
}

#[test]
fn test_encrypt_empty_plaintext() {
    let mut plaintext = Vec::new();
    let key = [0u8; 16];
    let iv = [0u8; 12];
    let mut seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    let result =
        encrypt_plaintext_to_records(CS, &mut plaintext, &key, &iv, &mut seq, &mut ciphertext_buf);
    assert!(result.is_ok());
    assert!(ciphertext_buf.is_empty());
    assert_eq!(seq, 0); // No records encrypted
}

#[test]
fn test_encrypt_small_plaintext_single_record() {
    let mut plaintext = vec![0x41u8; 100]; // 100 bytes of 'A'
    let key = [0x01u8; 16];
    let iv = [0x02u8; 12];
    let mut seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    let result =
        encrypt_plaintext_to_records(CS, &mut plaintext, &key, &iv, &mut seq, &mut ciphertext_buf);
    assert!(result.is_ok());
    assert!(plaintext.is_empty()); // Should be cleared

    // Should have one record: header(5) + plaintext(100) + content_type(1) + tag(16)
    let expected_len = TLS_RECORD_HEADER_SIZE + 100 + 1 + 16;
    assert_eq!(ciphertext_buf.len(), expected_len);
    assert_eq!(seq, 1); // One record encrypted

    // Verify TLS header
    assert_eq!(ciphertext_buf[0], CONTENT_TYPE_APPLICATION_DATA);
    assert_eq!(ciphertext_buf[1], 0x03);
    assert_eq!(ciphertext_buf[2], 0x03);
    let record_len = u16::from_be_bytes([ciphertext_buf[3], ciphertext_buf[4]]) as usize;
    assert_eq!(record_len, 100 + 1 + 16);
}

#[test]
fn test_encrypt_max_single_record() {
    // Test exactly at the boundary - should still be single record
    let mut plaintext = vec![0x42u8; MAX_TLS_PLAINTEXT_LEN];
    let key = [0x03u8; 16];
    let iv = [0x04u8; 12];
    let mut seq = 5u64;
    let mut ciphertext_buf = Vec::new();

    let result =
        encrypt_plaintext_to_records(CS, &mut plaintext, &key, &iv, &mut seq, &mut ciphertext_buf);
    assert!(result.is_ok());
    assert!(plaintext.is_empty());

    // Should have one record
    let expected_len = TLS_RECORD_HEADER_SIZE + MAX_TLS_PLAINTEXT_LEN + 1 + 16;
    assert_eq!(ciphertext_buf.len(), expected_len);
    assert_eq!(seq, 6); // One record encrypted
}

#[test]
fn test_encrypt_fragmentation_two_records() {
    // One byte over the limit - should produce two records
    let mut plaintext = vec![0x43u8; MAX_TLS_PLAINTEXT_LEN + 1];
    let key = [0x05u8; 16];
    let iv = [0x06u8; 12];
    let mut seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    let result =
        encrypt_plaintext_to_records(CS, &mut plaintext, &key, &iv, &mut seq, &mut ciphertext_buf);
    assert!(result.is_ok());
    assert!(plaintext.is_empty());
    assert_eq!(seq, 2); // Two records encrypted

    // First record: full MAX_TLS_PLAINTEXT_LEN
    let first_record_len = TLS_RECORD_HEADER_SIZE + MAX_TLS_PLAINTEXT_LEN + 1 + 16;
    // Second record: 1 byte
    let second_record_len = TLS_RECORD_HEADER_SIZE + 1 + 1 + 16;
    assert_eq!(ciphertext_buf.len(), first_record_len + second_record_len);

    // Verify first record header
    assert_eq!(ciphertext_buf[0], CONTENT_TYPE_APPLICATION_DATA);
    let first_payload_len = u16::from_be_bytes([ciphertext_buf[3], ciphertext_buf[4]]) as usize;
    assert_eq!(first_payload_len, MAX_TLS_PLAINTEXT_LEN + 1 + 16);

    // Verify second record header
    let second_header_offset = first_record_len;
    assert_eq!(
        ciphertext_buf[second_header_offset],
        CONTENT_TYPE_APPLICATION_DATA
    );
    let second_payload_len = u16::from_be_bytes([
        ciphertext_buf[second_header_offset + 3],
        ciphertext_buf[second_header_offset + 4],
    ]) as usize;
    assert_eq!(second_payload_len, 1 + 1 + 16);
}

#[test]
fn test_encrypt_fragmentation_multiple_records() {
    // Test 3x the max size - should produce 3 records
    let size = MAX_TLS_PLAINTEXT_LEN * 3;
    let mut plaintext = vec![0x44u8; size];
    let key = [0x07u8; 16];
    let iv = [0x08u8; 12];
    let mut seq = 10u64;
    let mut ciphertext_buf = Vec::new();

    let result =
        encrypt_plaintext_to_records(CS, &mut plaintext, &key, &iv, &mut seq, &mut ciphertext_buf);
    assert!(result.is_ok());
    assert!(plaintext.is_empty());
    assert_eq!(seq, 13); // Three records encrypted

    // Each record: header(5) + MAX_TLS_PLAINTEXT_LEN + content_type(1) + tag(16)
    let record_len = TLS_RECORD_HEADER_SIZE + MAX_TLS_PLAINTEXT_LEN + 1 + 16;
    assert_eq!(ciphertext_buf.len(), record_len * 3);
}

#[test]
fn test_sequence_number_increments() {
    let key = [0x0Bu8; 16];
    let iv = [0x0Cu8; 12];
    let mut seq = 100u64;
    let mut ciphertext_buf = Vec::new();

    // First call with small data
    let mut plaintext1 = vec![0x46u8; 50];
    encrypt_plaintext_to_records(
        CS,
        &mut plaintext1,
        &key,
        &iv,
        &mut seq,
        &mut ciphertext_buf,
    )
    .unwrap();
    assert_eq!(seq, 101);

    // Second call with data requiring fragmentation
    let mut plaintext2 = vec![0x47u8; MAX_TLS_PLAINTEXT_LEN + 500];
    encrypt_plaintext_to_records(
        CS,
        &mut plaintext2,
        &key,
        &iv,
        &mut seq,
        &mut ciphertext_buf,
    )
    .unwrap();
    assert_eq!(seq, 103); // Two more records
}

#[test]
fn test_ciphertext_buf_appends() {
    let key = [0x0Du8; 16];
    let iv = [0x0Eu8; 12];
    let mut seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    // Pre-populate buffer
    ciphertext_buf.extend_from_slice(b"existing data");
    let initial_len = ciphertext_buf.len();

    let mut plaintext = vec![0x48u8; 100];
    encrypt_plaintext_to_records(CS, &mut plaintext, &key, &iv, &mut seq, &mut ciphertext_buf)
        .unwrap();

    // Should append, not overwrite
    assert!(ciphertext_buf.len() > initial_len);
    assert_eq!(&ciphertext_buf[..initial_len], b"existing data");
}

#[test]
fn test_encrypt_single_byte() {
    // Minimum non-empty plaintext
    let mut plaintext = vec![0x42u8; 1];
    let key = [0x0Fu8; 16];
    let iv = [0x10u8; 12];
    let mut seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    let result =
        encrypt_plaintext_to_records(CS, &mut plaintext, &key, &iv, &mut seq, &mut ciphertext_buf);
    assert!(result.is_ok());
    assert!(plaintext.is_empty());
    assert_eq!(seq, 1);

    // header(5) + plaintext(1) + content_type(1) + tag(16) = 23
    assert_eq!(ciphertext_buf.len(), 23);
}

#[test]
fn test_handshake_encrypt_empty() {
    let handshake_data: &[u8] = &[];
    let key = [0u8; 16];
    let iv = [0u8; 12];
    let mut seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    let result =
        encrypt_handshake_to_records(CS, handshake_data, &key, &iv, &mut seq, &mut ciphertext_buf);
    assert!(result.is_ok());
    assert!(ciphertext_buf.is_empty());
    assert_eq!(seq, 0); // No records encrypted
}

#[test]
fn test_handshake_encrypt_small_single_record() {
    // Typical small handshake (EncryptedExtensions + small cert + CV + Finished)
    let handshake_data = vec![0x16u8; 500];
    let key = [0x01u8; 16];
    let iv = [0x02u8; 12];
    let mut seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    let result = encrypt_handshake_to_records(
        CS,
        &handshake_data,
        &key,
        &iv,
        &mut seq,
        &mut ciphertext_buf,
    );
    assert!(result.is_ok());
    assert_eq!(seq, 1); // One record encrypted

    // Should have one record: header(5) + handshake(500) + content_type(1) + tag(16)
    let expected_len = TLS_RECORD_HEADER_SIZE + 500 + 1 + 16;
    assert_eq!(ciphertext_buf.len(), expected_len);

    // Verify TLS header - outer type is ApplicationData (0x17)
    assert_eq!(ciphertext_buf[0], CONTENT_TYPE_APPLICATION_DATA);
    assert_eq!(ciphertext_buf[1], 0x03);
    assert_eq!(ciphertext_buf[2], 0x03);
    let record_len = u16::from_be_bytes([ciphertext_buf[3], ciphertext_buf[4]]) as usize;
    assert_eq!(record_len, 500 + 1 + 16);
}

#[test]
fn test_handshake_encrypt_max_single_record() {
    // Exactly at the boundary - should still be single record
    let handshake_data = vec![0x16u8; MAX_TLS_PLAINTEXT_LEN];
    let key = [0x03u8; 16];
    let iv = [0x04u8; 12];
    let mut seq = 5u64;
    let mut ciphertext_buf = Vec::new();

    let result = encrypt_handshake_to_records(
        CS,
        &handshake_data,
        &key,
        &iv,
        &mut seq,
        &mut ciphertext_buf,
    );
    assert!(result.is_ok());
    assert_eq!(seq, 6); // One record encrypted

    let expected_len = TLS_RECORD_HEADER_SIZE + MAX_TLS_PLAINTEXT_LEN + 1 + 16;
    assert_eq!(ciphertext_buf.len(), expected_len);
}

#[test]
fn test_handshake_encrypt_fragmentation_two_records() {
    // One byte over the limit - should produce two records
    let handshake_data = vec![0x16u8; MAX_TLS_PLAINTEXT_LEN + 1];
    let key = [0x05u8; 16];
    let iv = [0x06u8; 12];
    let mut seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    let result = encrypt_handshake_to_records(
        CS,
        &handshake_data,
        &key,
        &iv,
        &mut seq,
        &mut ciphertext_buf,
    );
    assert!(result.is_ok());
    assert_eq!(seq, 2); // Two records encrypted

    // First record: full MAX_TLS_PLAINTEXT_LEN
    let first_record_len = TLS_RECORD_HEADER_SIZE + MAX_TLS_PLAINTEXT_LEN + 1 + 16;
    // Second record: 1 byte
    let second_record_len = TLS_RECORD_HEADER_SIZE + 1 + 1 + 16;
    assert_eq!(ciphertext_buf.len(), first_record_len + second_record_len);
}

#[test]
fn test_record_encryptor_app_data() {
    let aead_key = AeadKey::new(CS, &[0x42u8; 16]).unwrap();
    let iv = [0x99u8; 12];
    let mut seq = 0u64;
    let mut out = Vec::new();

    let mut encryptor = RecordEncryptor::new(&aead_key, &iv, &mut seq);

    let mut plaintext = vec![0xAAu8; 100];
    encryptor
        .encrypt_app_data(&mut plaintext, &mut out)
        .unwrap();

    assert!(plaintext.is_empty());
    assert_eq!(seq, 1);
    assert_eq!(out.len(), 5 + 100 + 1 + 16);
}

#[test]
fn test_record_encryptor_handshake() {
    let aead_key = AeadKey::new(CS, &[0x42u8; 16]).unwrap();
    let iv = [0x99u8; 12];
    let mut seq = 0u64;
    let mut out = Vec::new();

    let mut encryptor = RecordEncryptor::new(&aead_key, &iv, &mut seq);

    let handshake = vec![0xBBu8; 200];
    encryptor.encrypt_handshake(&handshake, &mut out).unwrap();

    assert_eq!(seq, 1);
    assert_eq!(out.len(), 5 + 200 + 1 + 16);
}

#[test]
fn test_record_encryptor_close_notify() {
    let aead_key = AeadKey::new(CS, &[0x42u8; 16]).unwrap();
    let iv = [0x99u8; 12];
    let mut seq = 0u64;
    let mut out = Vec::new();

    let mut encryptor = RecordEncryptor::new(&aead_key, &iv, &mut seq);
    encryptor.encrypt_close_notify(&mut out).unwrap();

    assert_eq!(seq, 1);
    // header(5) + alert(2) + content_type(1) + tag(16) = 24
    assert_eq!(out.len(), 24);
}

#[test]
fn test_record_decryptor_roundtrip() {
    let aead_key = AeadKey::new(CS, &[0x42u8; 16]).unwrap();
    let iv = [0x99u8; 12];
    let mut enc_seq = 0u64;
    let mut dec_seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    // Encrypt
    let mut encryptor = RecordEncryptor::new(&aead_key, &iv, &mut enc_seq);
    let mut plaintext = vec![0xCCu8; 100];
    let original = plaintext.clone();
    encryptor
        .encrypt_app_data(&mut plaintext, &mut ciphertext_buf)
        .unwrap();

    // Decrypt - extract ciphertext from record
    let record_len = u16::from_be_bytes([ciphertext_buf[3], ciphertext_buf[4]]);
    let ciphertext = &ciphertext_buf[5..];

    let mut decryptor = RecordDecryptor::new(&aead_key, &iv, &mut dec_seq);
    let (content_type, decrypted) = decrypt_record(&mut decryptor, ciphertext, record_len).unwrap();

    assert_eq!(content_type, CONTENT_TYPE_APPLICATION_DATA);
    assert_eq!(decrypted, original);
    assert_eq!(dec_seq, 1);
}

#[test]
fn test_record_decryptor_handshake_roundtrip() {
    let aead_key = AeadKey::new(CS, &[0x42u8; 16]).unwrap();
    let iv = [0x99u8; 12];
    let mut enc_seq = 0u64;
    let mut dec_seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    // Encrypt handshake
    let mut encryptor = RecordEncryptor::new(&aead_key, &iv, &mut enc_seq);
    let handshake = vec![0xDDu8; 150];
    encryptor
        .encrypt_handshake(&handshake, &mut ciphertext_buf)
        .unwrap();

    // Decrypt
    let record_len = u16::from_be_bytes([ciphertext_buf[3], ciphertext_buf[4]]);
    let ciphertext = &ciphertext_buf[5..];

    let mut decryptor = RecordDecryptor::new(&aead_key, &iv, &mut dec_seq);
    let (content_type, decrypted) = decrypt_record(&mut decryptor, ciphertext, record_len).unwrap();

    assert_eq!(content_type, CONTENT_TYPE_HANDSHAKE);
    assert_eq!(decrypted, handshake);
}

#[test]
fn test_roundtrip_single_byte() {
    let key_bytes = [0x11u8; 16];
    let iv = [0x22u8; 12];
    let mut enc_seq = 0u64;
    let mut dec_seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    let aead_key = AeadKey::new(CS, &key_bytes).unwrap();

    // Encrypt single byte
    let mut plaintext = vec![0x42u8];
    let original = plaintext.clone();
    {
        let mut enc = RecordEncryptor::new(&aead_key, &iv, &mut enc_seq);
        enc.encrypt_app_data(&mut plaintext, &mut ciphertext_buf)
            .unwrap();
    }

    // Decrypt
    let record_len = u16::from_be_bytes([ciphertext_buf[3], ciphertext_buf[4]]);
    let ciphertext = &ciphertext_buf[5..];

    let mut dec = RecordDecryptor::new(&aead_key, &iv, &mut dec_seq);
    let (ct, decrypted) = decrypt_record(&mut dec, ciphertext, record_len).unwrap();

    assert_eq!(ct, CONTENT_TYPE_APPLICATION_DATA);
    assert_eq!(decrypted, original);
}

#[test]
fn test_roundtrip_large_64kb() {
    let key_bytes = [0x33u8; 16];
    let iv = [0x44u8; 12];
    let mut enc_seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    let aead_key = AeadKey::new(CS, &key_bytes).unwrap();

    // 64KB will be fragmented into 4 records
    let mut plaintext = vec![0x55u8; 65536];
    let original = plaintext.clone();
    {
        let mut enc = RecordEncryptor::new(&aead_key, &iv, &mut enc_seq);
        enc.encrypt_app_data(&mut plaintext, &mut ciphertext_buf)
            .unwrap();
    }

    assert_eq!(enc_seq, 4); // 64KB / 16KB = 4 records

    // Decrypt all records and reassemble
    let mut dec_seq = 0u64;
    let mut dec = RecordDecryptor::new(&aead_key, &iv, &mut dec_seq);
    let mut reassembled = Vec::new();
    let mut offset = 0;

    while offset < ciphertext_buf.len() {
        let record_len =
            u16::from_be_bytes([ciphertext_buf[offset + 3], ciphertext_buf[offset + 4]]);
        let ciphertext = &ciphertext_buf[offset + 5..offset + 5 + record_len as usize];

        let (ct, decrypted) = decrypt_record(&mut dec, ciphertext, record_len).unwrap();
        assert_eq!(ct, CONTENT_TYPE_APPLICATION_DATA);
        reassembled.extend_from_slice(&decrypted);

        offset += 5 + record_len as usize;
    }

    assert_eq!(reassembled, original);
}

#[test]
fn test_encryptor_sequence_exhaustion() {
    let aead_key = AeadKey::new(CS, &[0x42u8; 16]).unwrap();
    let iv = [0x99u8; 12];
    let mut seq = u64::MAX; // Start at max
    let mut out = Vec::new();

    let mut encryptor = RecordEncryptor::new(&aead_key, &iv, &mut seq);
    let mut plaintext = vec![0xAAu8; 10];

    // First encryption should succeed (uses seq = MAX)
    let result = encryptor.encrypt_app_data(&mut plaintext, &mut out);
    assert!(result.is_err()); // Should fail because MAX + 1 would overflow
    assert!(result.unwrap_err().to_string().contains("exhausted"));
}

#[test]
fn test_decryptor_sequence_exhaustion() {
    let aead_key = AeadKey::new(CS, &[0x42u8; 16]).unwrap();
    let iv = [0x99u8; 12];

    // First encrypt with seq=0 to get valid ciphertext
    let mut enc_seq = 0u64;
    let mut ciphertext_buf = Vec::new();
    {
        let mut encryptor = RecordEncryptor::new(&aead_key, &iv, &mut enc_seq);
        let mut plaintext = vec![0xBBu8; 10];
        encryptor
            .encrypt_app_data(&mut plaintext, &mut ciphertext_buf)
            .unwrap();
    }

    // Try to decrypt with seq at MAX - decryption will fail due to wrong nonce,
    // but even if it succeeded, seq increment would fail
    let mut dec_seq = u64::MAX;
    let record_len = u16::from_be_bytes([ciphertext_buf[3], ciphertext_buf[4]]);
    let ciphertext = &ciphertext_buf[5..];

    let mut decryptor = RecordDecryptor::new(&aead_key, &iv, &mut dec_seq);
    let result = decrypt_record(&mut decryptor, ciphertext, record_len);
    // Will fail either due to decryption (wrong nonce) or seq exhaustion
    assert!(result.is_err());
}

#[test]
fn test_alert_roundtrip() {
    let aead_key = AeadKey::new(CS, &[0x42u8; 16]).unwrap();
    let iv = [0x99u8; 12];
    let mut enc_seq = 0u64;
    let mut dec_seq = 0u64;
    let mut ciphertext_buf = Vec::new();

    // Encrypt close_notify alert
    {
        let mut encryptor = RecordEncryptor::new(&aead_key, &iv, &mut enc_seq);
        encryptor.encrypt_close_notify(&mut ciphertext_buf).unwrap();
    }

    assert_eq!(enc_seq, 1);

    // Decrypt and verify
    let record_len = u16::from_be_bytes([ciphertext_buf[3], ciphertext_buf[4]]);
    let ciphertext = &ciphertext_buf[5..];

    let mut decryptor = RecordDecryptor::new(&aead_key, &iv, &mut dec_seq);
    let (content_type, plaintext) = decrypt_record(&mut decryptor, ciphertext, record_len).unwrap();

    assert_eq!(content_type, CONTENT_TYPE_ALERT);
    assert_eq!(plaintext.len(), 2);
    assert_eq!(plaintext[0], 0x01); // level = warning
    assert_eq!(plaintext[1], 0x00); // desc = close_notify
}
