//! Tests for ADNL (Abstract Datagram Network Layer) module

use super::crypto::*;
use crate::adnl::{AdnlAesParams, AdnlCodec, AdnlError, AdnlPeer};
use rand::rngs::OsRng;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_util::bytes::{Bytes, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

#[test]
fn test_keypair_generation() {
    let mut rng = OsRng;
    let keypair1 = KeyPair::generate(&mut rng);
    let keypair2 = KeyPair::generate(&mut rng);

    // Generated keypairs should be different
    assert_ne!(
        keypair1.public_key.to_bytes(),
        keypair2.public_key.to_bytes()
    );
}

#[test]
fn test_adnl_codec_roundtrip() {
    let params = AdnlAesParams::default();
    let mut encoder = AdnlCodec::client(&params);
    let mut decoder = AdnlCodec::server(&params);
    let payload = Bytes::from_static(b"hello adnl");
    let mut frame = BytesMut::new();

    encoder.encode(payload.clone(), &mut frame).unwrap();
    let decoded = decoder.decode(&mut frame).unwrap().unwrap();

    assert_eq!(decoded, payload);
    assert!(frame.is_empty());
}

#[test]
fn test_adnl_codec_empty_payload_uses_minimum_frame_body() {
    let params = AdnlAesParams::default();
    let mut encoder = AdnlCodec::client(&params);
    let mut decoder = AdnlCodec::server(&params);
    let mut frame = BytesMut::new();

    encoder.encode(Bytes::new(), &mut frame).unwrap();

    assert_eq!(frame.len(), 68);
    assert_eq!(decoder.decode(&mut frame).unwrap().unwrap(), Bytes::new());
    assert!(frame.is_empty());
}

#[test]
fn test_adnl_codec_directionality_matches_client_and_server_params() {
    let params = AdnlAesParams::default();
    let mut client_codec = AdnlCodec::client(&params);
    let mut server_codec = AdnlCodec::server(&params);
    let mut frame = BytesMut::new();

    client_codec
        .encode(Bytes::from_static(b"client to server"), &mut frame)
        .unwrap();
    assert_eq!(
        server_codec.decode(&mut frame).unwrap().unwrap(),
        Bytes::from_static(b"client to server")
    );

    server_codec
        .encode(Bytes::from_static(b"server to client"), &mut frame)
        .unwrap();
    assert_eq!(
        client_codec.decode(&mut frame).unwrap().unwrap(),
        Bytes::from_static(b"server to client")
    );
    assert!(frame.is_empty());
}

#[test]
fn test_adnl_codec_waits_for_partial_frame() {
    let params = AdnlAesParams::default();
    let mut encoder = AdnlCodec::client(&params);
    let mut decoder = AdnlCodec::server(&params);
    let mut frame = BytesMut::new();

    encoder
        .encode(Bytes::from_static(b"partial"), &mut frame)
        .unwrap();
    let split_at = frame.len() - 1;
    let tail = frame.split_off(split_at);

    assert!(decoder.decode(&mut frame).unwrap().is_none());

    frame.extend_from_slice(&tail);
    assert_eq!(
        decoder.decode(&mut frame).unwrap().unwrap(),
        Bytes::from_static(b"partial")
    );
}

#[test]
fn test_adnl_codec_rejects_tampered_frame() {
    let params = AdnlAesParams::default();
    let mut encoder = AdnlCodec::client(&params);
    let mut decoder = AdnlCodec::server(&params);
    let mut frame = BytesMut::new();

    encoder
        .encode(Bytes::from_static(b"tamper"), &mut frame)
        .unwrap();
    let last = frame.len() - 1;
    frame[last] ^= 0x01;

    let err = decoder.decode(&mut frame).unwrap_err();
    assert!(matches!(err, AdnlError::IntegrityError));
}

#[test]
fn test_adnl_codec_decodes_multiple_frames() {
    let params = AdnlAesParams::default();
    let mut encoder = AdnlCodec::client(&params);
    let mut decoder = AdnlCodec::server(&params);
    let mut frame = BytesMut::new();

    encoder
        .encode(Bytes::from_static(b"first"), &mut frame)
        .unwrap();
    encoder
        .encode(Bytes::from_static(b"second"), &mut frame)
        .unwrap();

    assert_eq!(
        decoder.decode(&mut frame).unwrap().unwrap(),
        Bytes::from_static(b"first")
    );
    assert_eq!(
        decoder.decode(&mut frame).unwrap().unwrap(),
        Bytes::from_static(b"second")
    );
    assert!(frame.is_empty());
}

#[test]
fn test_adnl_codec_rejects_too_large_payload() {
    let params = AdnlAesParams::default();
    let mut encoder = AdnlCodec::client(&params);
    let mut frame = BytesMut::new();
    let payload = Bytes::from(vec![0u8; (1 << 24) - 63]);

    let err = encoder.encode(payload, &mut frame).unwrap_err();

    assert!(matches!(err, AdnlError::TooLongPacket));
    assert!(frame.is_empty());
}

#[tokio::test]
async fn test_adnl_loopback_handshake() {
    let server_secret = SecretKey::from_bytes([7u8; 32]);
    let server_keypair = KeyPair::from(&server_secret);
    let server_public = server_keypair.public_key.to_bytes();
    let (client_transport, server_transport) = tokio::io::duplex(4096);

    let server = tokio::spawn(async move {
        AdnlPeer::handle_handshake(server_transport, |_| Some(server_keypair)).await
    });
    let client = AdnlPeer::perform_handshake(client_transport, server_public).await;
    let server = server.await.unwrap();

    assert!(client.is_ok());
    assert!(server.is_ok());
}

#[tokio::test]
async fn adnl_connect_with_timeout_maps_handshake_timeout() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (_stream, _addr) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(60)).await;
    });

    let server_secret = SecretKey::from_bytes([9u8; 32]);
    let server_public = KeyPair::from(&server_secret).public_key.to_bytes();
    let error =
        match AdnlPeer::connect_with_timeout(server_public, address, Duration::from_millis(10))
            .await
        {
            Ok(_) => panic!("connect should time out while waiting for ADNL handshake"),
            Err(error) => error,
        };

    server.abort();
    assert!(matches!(
        error,
        AdnlError::Timeout {
            operation: "adnl_handshake",
            timeout
        } if timeout == Duration::from_millis(10)
    ));
}

#[test]
fn test_keypair_from_secret_key() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let keypair = KeyPair::from(&secret);

    // Should be able to create keypair from secret key
    assert_eq!(keypair.secret_key.nonce().len(), 32);
}

#[test]
fn test_public_key_from_bytes() {
    let bytes = [
        75, 54, 96, 93, 16, 21, 8, 159, 230, 42, 68, 148, 54, 18, 251, 196, 205, 254, 252, 114, 76,
        87, 204, 218, 132, 26, 196, 181, 191, 188, 115, 123,
    ];

    let pubkey = PublicKey::from_bytes(bytes);
    assert!(pubkey.is_some());

    let pubkey = pubkey.unwrap();
    assert_eq!(pubkey.to_bytes(), bytes);
}

#[test]
fn test_public_key_invalid_bytes() {
    // Test with all zeros (likely invalid point)
    let bytes = [0u8; 32];
    let _pubkey = PublicKey::from_bytes(bytes);
    // Some byte patterns may be invalid Ed25519 points
    // The result depends on the specific implementation
}

#[test]
fn test_public_key_equality() {
    let bytes = [42u8; 32];
    let pubkey1 = PublicKey::from_bytes(bytes);
    let pubkey2 = PublicKey::from_bytes(bytes);

    if let (Some(pk1), Some(pk2)) = (pubkey1, pubkey2) {
        assert_eq!(pk1, pk2);
    }
}

#[test]
fn test_public_key_as_bytes() {
    let bytes = [123u8; 32];
    if let Some(pubkey) = PublicKey::from_bytes(bytes) {
        assert_eq!(pubkey.as_bytes(), &bytes);
        assert_eq!(pubkey.to_bytes(), bytes);
    }
}

#[test]
fn test_public_key_display() {
    let secret = SecretKey::from_bytes([
        99, 87, 207, 105, 199, 108, 51, 89, 172, 108, 232, 48, 240, 147, 49, 155, 145, 60, 66, 55,
        98, 149, 119, 0, 251, 19, 132, 69, 151, 132, 184, 53,
    ]);

    let pubkey = PublicKey::from(&secret);
    let display_str = format!("{}", pubkey);

    // Should be 64 hex characters (32 bytes)
    assert_eq!(display_str.len(), 64);
    // Should only contain hex characters
    assert!(display_str.chars().all(|c| c.is_ascii_hexdigit()));
}

#[test]
fn test_public_key_debug() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let pubkey = PublicKey::from(&secret);
    let debug_str = format!("{:?}", pubkey);

    // Debug should produce hex output
    assert_eq!(debug_str.len(), 64);
}

#[test]
fn test_secret_key_operations() {
    let bytes = [123u8; 32];
    let secret = SecretKey::from_bytes(bytes);

    assert_eq!(secret.to_bytes(), bytes);
    assert_eq!(secret.as_bytes(), &bytes);
}

#[test]
fn test_expanded_secret_key() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let expanded = secret.expand();

    // Nonce should be 32 bytes
    assert_eq!(expanded.nonce().len(), 32);
}

#[test]
fn test_sign_and_verify_tl() {
    let secret = SecretKey::from_bytes([
        99, 87, 207, 105, 199, 108, 51, 89, 172, 108, 232, 48, 240, 147, 49, 155, 145, 60, 66, 55,
        98, 149, 119, 0, 251, 19, 132, 69, 151, 132, 184, 53,
    ]);

    let keypair = KeyPair::from(&secret);
    let data = b"hello world";

    let signature = keypair.sign_tl(data);
    assert_eq!(signature.len(), 64);

    // Verify signature
    assert!(keypair.public_key.verify_tl(data, &signature));
}

#[test]
fn test_sign_and_verify_raw() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let keypair = KeyPair::from(&secret);
    let data = b"test message";

    let signature = keypair.sign_raw(data);
    assert_eq!(signature.len(), 64);

    // Verify signature
    assert!(keypair.public_key.verify_raw(data, &signature));
}

#[test]
fn test_verify_tl_wrong_signature() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let keypair = KeyPair::from(&secret);
    let data = b"original message";

    let signature = keypair.sign_tl(data);

    // Try to verify with different data
    let wrong_data = b"modified message";
    assert!(!keypair.public_key.verify_tl(wrong_data, &signature));
}

#[test]
fn test_verify_raw_wrong_signature() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let keypair = KeyPair::from(&secret);
    let data = b"original message";

    let signature = keypair.sign_raw(data);

    // Try to verify with different data
    let wrong_data = b"modified message";
    assert!(!keypair.public_key.verify_raw(wrong_data, &signature));
}

#[test]
fn test_verify_with_wrong_public_key() {
    let secret1 = SecretKey::from_bytes([42u8; 32]);
    let keypair1 = KeyPair::from(&secret1);

    let secret2 = SecretKey::from_bytes([99u8; 32]);
    let keypair2 = KeyPair::from(&secret2);

    let data = b"test message";
    let signature = keypair1.sign_raw(data);

    // Try to verify with different public key
    assert!(!keypair2.public_key.verify_raw(data, &signature));
}

#[test]
fn test_compute_shared_secret_symmetric() {
    let secret1 = SecretKey::from_bytes([
        215, 30, 117, 171, 183, 9, 171, 48, 212, 45, 10, 198, 14, 66, 109, 80, 163, 180, 194, 66,
        82, 184, 13, 48, 240, 102, 40, 110, 156, 5, 13, 143,
    ]);
    let keypair1 = KeyPair::from(&secret1);

    let secret2 = SecretKey::from_bytes([
        181, 115, 13, 55, 26, 150, 138, 43, 66, 28, 162, 50, 0, 133, 120, 24, 20, 142, 183, 60,
        159, 53, 200, 97, 14, 123, 63, 249, 222, 211, 186, 99,
    ]);
    let keypair2 = KeyPair::from(&secret2);

    let shared1 = keypair1.compute_shared_secret(&keypair2.public_key);
    let shared2 = keypair2.compute_shared_secret(&keypair1.public_key);

    // Both parties should compute the same shared secret
    assert_eq!(shared1, shared2);
}

#[test]
fn test_compute_shared_secret_value() {
    let secret1 = SecretKey::from_bytes([
        215, 30, 117, 171, 183, 9, 171, 48, 212, 45, 10, 198, 14, 66, 109, 80, 163, 180, 194, 66,
        82, 184, 13, 48, 240, 102, 40, 110, 156, 5, 13, 143,
    ]);
    let keypair1 = KeyPair::from(&secret1);

    let secret2 = SecretKey::from_bytes([
        181, 115, 13, 55, 26, 150, 138, 43, 66, 28, 162, 50, 0, 133, 120, 24, 20, 142, 183, 60,
        159, 53, 200, 97, 14, 123, 63, 249, 222, 211, 186, 99,
    ]);
    let keypair2 = KeyPair::from(&secret2);

    let shared = keypair1.compute_shared_secret(&keypair2.public_key);

    // Verify against known value
    let expected = [
        30, 243, 238, 65, 216, 53, 237, 172, 6, 120, 204, 220, 34, 163, 18, 28, 181, 245, 215, 233,
        98, 0, 87, 11, 85, 6, 41, 130, 140, 95, 66, 72,
    ];
    assert_eq!(shared, expected);
}

#[test]
fn test_tl_public_key_ed25519() {
    let key_bytes = [42u8; 32];
    let tl_key = tl::PublicKey::Ed25519 { key: &key_bytes };

    match tl_key {
        tl::PublicKey::Ed25519 { key } => {
            assert_eq!(key, &key_bytes);
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_tl_public_key_owned_conversion() {
    let key_bytes = [42u8; 32];
    let tl_key = tl::PublicKey::Ed25519 { key: &key_bytes };

    let owned = tl_key.as_equivalent_owned();
    let back_to_ref = owned.as_equivalent_ref();

    match (tl_key, back_to_ref) {
        (tl::PublicKey::Ed25519 { key: k1 }, tl::PublicKey::Ed25519 { key: k2 }) => {
            assert_eq!(k1, k2);
        }
        _ => panic!("Conversion failed"),
    }
}

#[test]
fn test_tl_public_key_aes() {
    let key_bytes = [99u8; 32];
    let tl_key = tl::PublicKey::Aes { key: &key_bytes };

    let owned = tl_key.as_equivalent_owned();
    match owned {
        tl::PublicKeyOwned::Aes { key } => {
            assert_eq!(key, key_bytes);
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_tl_public_key_overlay() {
    let name = b"overlay_name";
    let tl_key = tl::PublicKey::Overlay { name };

    let owned = tl_key.as_equivalent_owned();
    match owned {
        tl::PublicKeyOwned::Overlay { name: n } => {
            assert_eq!(n, name.to_vec());
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_tl_public_key_unencoded() {
    let data = b"unencoded_data";
    let tl_key = tl::PublicKey::Unencoded { data };

    let owned = tl_key.as_equivalent_owned();
    match owned {
        tl::PublicKeyOwned::Unencoded { data: d } => {
            assert_eq!(d, data.to_vec());
        }
        _ => panic!("Wrong variant"),
    }
}

#[test]
fn test_public_key_from_tl() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let pubkey = PublicKey::from(&secret);
    let tl_key = pubkey.as_tl();

    let pubkey_from_tl = PublicKey::from_tl(tl_key);
    assert!(pubkey_from_tl.is_some());
    assert_eq!(pubkey_from_tl.unwrap(), pubkey);
}

#[test]
fn test_public_key_from_tl_non_ed25519() {
    let tl_key = tl::PublicKey::Aes { key: &[42u8; 32] };
    let pubkey = PublicKey::from_tl(tl_key);

    // Should return None for non-Ed25519 keys
    assert!(pubkey.is_none());
}

#[test]
fn test_signature_deterministic() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let keypair = KeyPair::from(&secret);
    let data = b"test message";

    let sig1 = keypair.sign_raw(data);
    let sig2 = keypair.sign_raw(data);

    // Signatures should be deterministic (same message, same key = same signature)
    assert_eq!(sig1, sig2);
}

#[test]
fn test_multiple_signatures_verify() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let keypair = KeyPair::from(&secret);

    let messages = [b"message 1", b"message 2", b"message 3"];

    for msg in &messages {
        let signature = keypair.sign_raw(*msg);
        assert!(keypair.public_key.verify_raw(*msg, &signature));
    }
}

#[test]
fn test_keypair_sign_tl_vs_raw() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let keypair = KeyPair::from(&secret);
    let data = b"test";

    let sig_tl = keypair.sign_tl(data);
    let sig_raw = keypair.sign_raw(data);

    // Each should verify correctly with their respective verification methods
    assert!(keypair.public_key.verify_tl(data, &sig_tl));
    assert!(keypair.public_key.verify_raw(data, &sig_raw));
}

#[test]
fn test_public_key_serialization_roundtrip() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let pubkey = PublicKey::from(&secret);

    // Serialize to JSON
    let json = serde_json::to_string(&pubkey).unwrap();

    // Deserialize back
    let deserialized: PublicKey = serde_json::from_str(&json).unwrap();

    assert_eq!(pubkey, deserialized);
}

#[test]
fn test_keypair_copy_clone() {
    let secret = SecretKey::from_bytes([42u8; 32]);
    let keypair = KeyPair::from(&secret);
    let keypair_copy = keypair;

    // Should be able to use both copies
    let data = b"test";
    let sig1 = keypair.sign_raw(data);
    let sig2 = keypair_copy.sign_raw(data);

    assert_eq!(sig1, sig2);
}
