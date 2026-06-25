//! B20 golden vectors from Base Sepolia `getB20Address` (factory `0xB20f…`).
//!
//! Hash input is `abi.encode(deployer, salt)` — 32-byte left-padded address + 32-byte salt
//! (64 bytes total), **not** `abi.encodePacked`.

use alloy_primitives::hex::decode;
use createxcrunch::b20::{compute_b20_address, B20Variant};

const TEST_DEPLOYER: &str = "0x34A50a7A272E86EE30b7A74E36f3f02AF18B1eB5";
const TEST_SALT_ASSET_HEX: &str =
    "0xd966e0f1e6f392cefea9931df3ab3130fcc4e2456e1a7fd3412815a023cba94f";
const TEST_ADDR_ASSET: &str = "0xb200000000000000000000d2E0c57D4924D1Ee6B";
const TEST_SALT_STABLE_HEX: &str =
    "0xe3c70aa5c359a8a4e01a887cfc0f547895d5f4fe9a034c9b20dea29a74476a18";
const TEST_ADDR_STABLE: &str = "0xb200000000000000000001E0e960E1525cE46C46";

pub(crate) fn parse_addr(s: &str) -> [u8; 20] {
    let mut out = [0u8; 20];
    out.copy_from_slice(&decode(s.trim_start_matches("0x")).unwrap());
    out
}

pub(crate) fn parse_salt(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&decode(s.trim_start_matches("0x")).unwrap());
    out
}

#[test]
fn compute_b20_address_asset_matches_factory() {
    let deployer = parse_addr(TEST_DEPLOYER);
    let salt = parse_salt(TEST_SALT_ASSET_HEX);
    let got = compute_b20_address(deployer, salt, B20Variant::Asset);
    assert_eq!(got, parse_addr(TEST_ADDR_ASSET));
}

#[test]
fn compute_b20_address_stablecoin_matches_factory() {
    let deployer = parse_addr(TEST_DEPLOYER);
    let salt = parse_salt(TEST_SALT_STABLE_HEX);
    let got = compute_b20_address(deployer, salt, B20Variant::Stablecoin);
    assert_eq!(got, parse_addr(TEST_ADDR_STABLE));
}

#[test]
fn b20_config_expands_suffix_pattern() {
    use createxcrunch::b20::B20Config;
    use createxcrunch::RewardVariant;

    let cfg = B20Config::new(
        0,
        "0x34A50a7A272E86EE30b7A74E36f3f02AF18B1eB5",
        B20Variant::Asset,
        RewardVariant::Matching {
            pattern: "666666666XXXXXXXXX".into(),
        },
        false,
        "output.txt",
    )
    .unwrap();
    assert_eq!(
        cfg.full_pattern(),
        "b200000000000000000000666666666XXXXXXXXX"
    );
}

#[test]
fn b20_config_rejects_leading_threshold_over_9() {
    use createxcrunch::b20::B20Config;
    use createxcrunch::RewardVariant;

    let result = B20Config::new(
        0,
        "0x34A50a7A272E86EE30b7A74E36f3f02AF18B1eB5",
        B20Variant::Asset,
        RewardVariant::LeadingZeros {
            zeros_threshold: 10,
        },
        false,
        "output.txt",
    );
    let err = match result {
        Err(e) => e,
        Ok(_) => panic!("expected validation error"),
    };
    assert!(err.contains("9"));
}
