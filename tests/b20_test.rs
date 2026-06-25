//! B20 golden vectors from `IB20Factory.getB20Address` on Base Sepolia.
//!
//! Verified 2026-06-25 via:
//! ```text
//! cast call 0xB20f000000000000000000000000000000000000 \
//!   "getB20Address(uint8,address,bytes32)(address)" \
//!   <variant> 0x34A50a7A272E86EE30b7A74E36f3f02AF18B1eB5 <salt> \
//!   --rpc-url https://sepolia.base.org
//! ```
//!
//! Salts: `cast keccak "b20-test-asset"` / `cast keccak "b20-test-stable"`.
//!
//! Hash encoding note: `getB20Address` matches `keccak256(abi.encode(deployer, salt))`
//! (32-byte padded address + 32-byte salt = 64 bytes), not `encodePacked` (52 bytes).

use alloy_primitives::hex::decode;

const TEST_DEPLOYER: &str = "0x34A50a7A272E86EE30b7A74E36f3f02AF18B1eB5";

const TEST_SALT_ASSET_HEX: &str =
    "0xd966e0f1e6f392cefea9931df3ab3130fcc4e2456e1a7fd3412815a023cba94f";
const TEST_ADDR_ASSET: &str = "0xb200000000000000000000d2E0c57D4924D1Ee6B";

const TEST_SALT_STABLE_HEX: &str =
    "0xe3c70aa5c359a8a4e01a887cfc0f547895d5f4fe9a034c9b20dea29a74476a18";
const TEST_ADDR_STABLE: &str = "0xb200000000000000000001E0e960E1525cE46C46";

fn parse_addr(s: &str) -> [u8; 20] {
    let mut out = [0u8; 20];
    out.copy_from_slice(&decode(s.trim_start_matches("0x")).unwrap());
    out
}

fn parse_salt(s: &str) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&decode(s.trim_start_matches("0x")).unwrap());
    out
}
