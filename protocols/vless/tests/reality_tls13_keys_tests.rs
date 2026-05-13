use ring::hmac;
use zero_protocol_vless::reality::reality_cipher_suite::CipherSuite;
    use zero_protocol_vless::reality::reality_tls13_keys::*;

    const CS_SHA256: CipherSuite = CipherSuite::AES_128_GCM_SHA256;

    // Test vectors from RFC 5869 Appendix A
    #[test]
    fn test_hkdf_expand_sha256_rfc_vector() {
        // Test Case 1 from RFC 5869 (simplified - using extracted PRK directly)
        let prk = [
            0x07, 0x77, 0x09, 0x36, 0x2c, 0x2e, 0x32, 0xdf, 0x0d, 0xdc, 0x3f, 0x0d, 0xc4, 0x7b,
            0xba, 0x63, 0x90, 0xb6, 0xc7, 0x3b, 0xb5, 0x0f, 0x9c, 0x31, 0x22, 0xec, 0x84, 0x4a,
            0xd7, 0xc2, 0xb3, 0xe5,
        ];
        let info = [0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9];

        let result = hkdf_expand(hmac::HMAC_SHA256, &prk, &info, 42).unwrap();
        assert_eq!(result.len(), 42);

        // Check first few bytes match expected pattern
        assert_eq!(result[0], 0x3c);
        assert_eq!(result[1], 0xb2);
        assert_eq!(result[2], 0x5f);
    }

    #[test]
    fn test_hkdf_expand_sha256_empty_info() {
        let prk = vec![0x42u8; 32];
        let result = hkdf_expand(hmac::HMAC_SHA256, &prk, &[], 16);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 16);
    }

    #[test]
    fn test_hkdf_expand_sha256_max_length() {
        let prk = vec![0x42u8; 32];
        let info = b"test info";

        // Maximum output length is 255 * hash_len (32 for SHA256) = 8160 bytes
        let result = hkdf_expand(hmac::HMAC_SHA256, &prk, info, 8160);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 8160);

        // Should fail for length > 8160
        let result = hkdf_expand(hmac::HMAC_SHA256, &prk, info, 8161);
        assert!(result.is_err());
    }

    #[test]
    fn test_hkdf_expand_label() {
        let secret = vec![0x42u8; 32];
        let result = hkdf_expand_label_with_algorithm(hmac::HMAC_SHA256, &secret, b"test", b"", 16);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 16);
    }

    #[test]
    fn test_hkdf_expand_label_with_context() {
        let secret = vec![0x42u8; 32];
        let context = vec![0x11u8; 32];
        let result =
            hkdf_expand_label_with_algorithm(hmac::HMAC_SHA256, &secret, b"finished", &context, 32);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.len(), 32);

        // Result should be deterministic
        let result2 =
            hkdf_expand_label_with_algorithm(hmac::HMAC_SHA256, &secret, b"finished", &context, 32)
                .unwrap();
        assert_eq!(output, result2);
    }

    #[test]
    fn test_hkdf_extract() {
        let salt = vec![0x11u8; 32];
        let ikm = vec![0x22u8; 32];

        let result1 = hkdf_extract_with_algorithm(hmac::HMAC_SHA256, &salt, &ikm);
        assert_eq!(result1.len(), 32); // SHA256 output length

        // Should be deterministic
        let result2 = hkdf_extract_with_algorithm(hmac::HMAC_SHA256, &salt, &ikm);
        assert_eq!(result1, result2);

        // Different input should give different output
        let ikm2 = vec![0x33u8; 32];
        let result3 = hkdf_extract_with_algorithm(hmac::HMAC_SHA256, &salt, &ikm2);
        assert_ne!(result1, result3);
    }

    #[test]
    fn test_derive_traffic_keys() {
        let traffic_secret = vec![0x99u8; 32];

        // Test TLS_AES_128_GCM_SHA256
        let result = derive_traffic_keys(&traffic_secret, CS_SHA256);
        assert!(result.is_ok());
        let (key, iv) = result.unwrap();
        assert_eq!(key.len(), 16);
        assert_eq!(iv.len(), 12);
    }

    #[test]
    fn test_compute_finished_verify_data() {
        let base_key = vec![0xAAu8; 32];
        let handshake_hash = vec![0xBBu8; 32];

        let result = compute_finished_verify_data(CS_SHA256, &base_key, &handshake_hash);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 32);
    }
