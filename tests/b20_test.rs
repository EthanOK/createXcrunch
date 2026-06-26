//! B20 golden vectors from Base Sepolia `getB20Address` (factory `0xB20f…`).
//!
//! Hash input is `abi.encode(deployer, salt)` — 32-byte left-padded address + 32-byte salt
//! (64 bytes total), **not** `abi.encodePacked`.

use alloy_primitives::hex::decode;
use createxcrunch::b20::{assemble_b20_salt, compute_b20_address, B20Variant};

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
fn b20_config_full_pattern_preserves_uppercase_wildcards() {
    use createxcrunch::b20::B20Config;
    use createxcrunch::RewardVariant;

    let cfg = B20Config::new(
        0,
        "0x34A50a7A272E86EE30b7A74E36f3f02AF18B1eB5",
        B20Variant::Asset,
        RewardVariant::Matching {
            pattern: "b200000000000000000000666666XXXXXXXXXXXX".into(),
        },
        true,
        "output.txt",
    )
    .unwrap();
    assert_eq!(
        cfg.full_pattern(),
        "b200000000000000000000666666XXXXXXXXXXXX"
    );
}

#[test]
fn assemble_b20_salt_matches_createx_sender_salt() {
    use itertools::chain;

    let caller = parse_addr("0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A");
    let tail = [
        0x37, 0xd8, 0x45, 0x71, 0x20, 0xa3, 0xba, 0x01, 0x14, 0x2e, 0x38,
    ];
    let b20 = assemble_b20_salt(caller, tail);
    let createx: Vec<u8> = chain!(caller, [0u8], tail.iter().copied()).collect();
    assert_eq!(b20.as_slice(), createx.as_slice());
}

#[test]
fn assemble_b20_salt_uses_caller_prefix_like_create2_sender() {
    let deployer = parse_addr("0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A");
    let tail = [
        0x37, 0xd8, 0x45, 0x71, 0x20, 0xa3, 0xba, 0x01, 0x14, 0x2e, 0x38,
    ];
    let salt = assemble_b20_salt(deployer, tail);
    assert_eq!(&salt[..20], &deployer);
    assert_eq!(salt[20], 0);
    assert_eq!(&salt[21..], &tail);
    assert_eq!(
        salt,
        parse_salt(
            "0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A0037d8457120a3ba01142e38"
        )
    );
}

#[test]
fn mined_salt_matches_cpu_reference_for_readme_caller() {
    let deployer = parse_addr("0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A");
    let salt = parse_salt(
        "0x37d8457120a3ba01142e38000000000000000000000000000000000000000000",
    );
    let got = compute_b20_address(deployer, salt, B20Variant::Asset);
    assert_eq!(
        got,
        parse_addr("0xb2000000000000000000006EC62413A36F73CdA7")
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

#[test]
fn b20_config_expands_six_count_suffix_patterns() {
    use createxcrunch::b20::B20Config;
    use createxcrunch::RewardVariant;

    let cases = [
        (1, "6XXXXXXXXXXXXXXXXX", "b2000000000000000000006XXXXXXXXXXXXXXXXX"),
        (2, "66XXXXXXXXXXXXXXXX", "b20000000000000000000066XXXXXXXXXXXXXXXX"),
        (3, "666XXXXXXXXXXXXXXX", "b200000000000000000000666XXXXXXXXXXXXXXX"),
        (4, "6666XXXXXXXXXXXXXX", "b2000000000000000000006666XXXXXXXXXXXXXX"),
        (5, "66666XXXXXXXXXXXXX", "b20000000000000000000066666XXXXXXXXXXXXX"),
        (6, "666666XXXXXXXXXXXX", "b200000000000000000000666666XXXXXXXXXXXX"),
    ];
    for (n, pattern, expected) in cases {
        let cfg = B20Config::new(
            0,
            "0x34A50a7A272E86EE30b7A74E36f3f02AF18B1eB5",
            B20Variant::Asset,
            RewardVariant::Matching {
                pattern: pattern.into(),
            },
            false,
            "output.txt",
        )
        .unwrap_or_else(|e| panic!("pattern with {n} sixes: {e}"));
        assert_eq!(cfg.full_pattern(), expected, "{n} sixes");
    }
}
