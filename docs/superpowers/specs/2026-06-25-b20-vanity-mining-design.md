# B20 Vanity Address Mining — Design Spec

**Date:** 2026-06-25  
**Status:** Approved (brainstorming)  
**Reference:** [B20 Native Token Standard — Address Derivation](https://docs.base.org/base-chain/specs/upgrades/beryl/b20#address-derivation)

## Summary

Add a new `b20` subcommand to `createXcrunch` that GPU-mines `bytes32` salts for vanity Base B20 token addresses. B20 address derivation requires a single `keccak256(deployer, salt)` hash, making it significantly faster than the existing CreateX create2/create3 paths.

## Requirements

| Requirement | Decision |
|-------------|----------|
| Primary capability | GPU vanity salt search (same UX as create2/create3) |
| Variants | ASSET (`0x00`) and STABLECOIN (`0x01`) via `--variant` |
| Deployer input | Required `--caller` (maps to `createB20` msg.sender) |
| Search criteria | Reuse `--leading`, `--total`, `--either`, `--matching` |
| Pattern input | Default: 18-char suffix; `--full-pattern` for full 40-char |
| Zero counting | `--leading` / `--total` apply to 9-byte hash suffix only |
| Performance | B20 throughput must exceed create3 on the same GPU |

## CLI

New subcommand parallel to `create2` / `create3`:

```bash
createxcrunch b20 \
  --caller 0xYourDeployerAddress \
  --variant asset \
  --matching 666666666XXXXXXXXX \
  --gpu-device-id 0 \
  --output output.txt
```

### Arguments

| Flag | Required | Default | Description |
|------|----------|---------|-------------|
| `--caller` | yes | — | Deployer address (`msg.sender` for `createB20`) |
| `--variant` | no | `asset` | `asset` or `stablecoin` |
| `--leading` | no* | — | Min leading zero bytes in hash suffix (max 9) |
| `--total` | no* | — | Min total zero bytes in hash suffix (max 9) |
| `--either` | no | false | Match leading OR total threshold |
| `--matching` | no* | — | Pattern match (18-char suffix or 40-char with `--full-pattern`) |
| `--full-pattern` | no | false | Treat `--matching` as full 40-char address pattern |
| `--gpu-device-id` | no | `0` | OpenCL device index |
| `--output` | no | `output.txt` | Output file for found salts |

*At least one search criterion (`--leading`, `--total`, or `--matching`) is required, same as existing commands.

### Pattern modes

**Suffix mode (default):** user supplies 18 hex chars (+ `X` wildcards). Program prepends fixed prefix:

| Variant | Auto-prepended prefix (22 hex chars) |
|---------|--------------------------------------|
| ASSET | `b200000000000000000000` |
| STABLECOIN | `b200000000000000000001` |

Example: `--matching 666666666XXXXXXXXX` → full address `0xb200000000000000000000666666666...`

**Full pattern mode (`--full-pattern`):** user supplies all 40 hex chars. Program validates that bytes 0–10 match the selected variant's expected prefix and variant byte; errors if incompatible.

### Output format

Same as existing commands:

```text
0x<SALT_BYTES32> => 0x<B20_ADDRESS>
```

For zero-based rewards, append `(leading / total)` counts for the suffix.

## Address Algorithm

Per [B20 spec](https://docs.base.org/base-chain/specs/upgrades/beryl/b20#address-derivation):

```text
address = [10-byte B20 prefix] [1-byte variant] [9-byte hash]
```

| Field | Value |
|-------|-------|
| 10-byte prefix | `0xB2000000000000000000` (fixed; tokens start with `0xB200…`) |
| 1-byte variant | `0x00` = ASSET, `0x01` = STABLECOIN (address byte index 10) |
| 9-byte hash | First 9 bytes of `keccak256(deployer, salt)` |

### Hash input encoding

```text
input  = abi.encodePacked(deployer, salt)   // 52 bytes: 20 + 32
hash   = keccak256(input)
suffix = hash[0..9]
```

**Verification gate:** Before merging, confirm encoding against `IB20Factory.getB20Address(variant, sender, salt)` on Base Sepolia or `base-anvil` via:

```bash
cast call 0xB20f000000000000000000000000000000000000 \
  "getB20Address(uint8,address,bytes32)(address)" \
  <variant> <deployer> <salt> --rpc-url <RPC>
```

Factory precompile: `0xB20f000000000000000000000000000000000000`

## Salt Mining Strategy

Reuse createXcrunch's random-prefix + nonce iteration pattern, adapted for `bytes32` salt:

```text
salt = [4-byte random] [7-byte from nonce] [21-byte zeros]
```

- **Fixed:** deployer (20 bytes) injected as kernel constants
- **Searched:** first 11 bytes of salt (4 random + 7 nonce-driven)
- **Hash input:** 52 bytes total — much smaller than CreateX's 85+ byte sponge

## GPU Kernel

### File layout

```
src/kernels/b20.cl    # Self-contained B20 OpenCL kernel
src/b20.rs            # B20Config, mk_b20_kernel_src(), gpu_b20()
```

Existing `keccak256.cl` and CreateX paths are **not modified**.

### Performance rationale

| Aspect | CreateX (create3) | B20 |
|--------|-------------------|-----|
| Keccak rounds | 2–3 full/partial rounds | 1 partial round |
| Sponge input size | ~85–135 bytes | 52 bytes |
| Post-hash assembly | CREATE2 prefix + optional CREATE3 proxy | Fixed prefix + variant byte |

Expected: **2–4× higher attempts/sec** on the same GPU vs create3.

### Kernel flow (`hashB20Salt`)

1. Assemble salt: `[4-byte message][7-byte nonce][21 zero bytes]`
2. Build sponge: `[20-byte deployer][32-byte salt]` + Keccak padding (52-byte input)
3. Run `partial_keccakf` (only first 9 bytes of digest needed)
4. Assemble address: `prefix[10] + variant[1] + hash[9]`
5. Evaluate `SUCCESS_CONDITION()` (matching / suffix leading / suffix total)
6. Write solutions buffer: `[nonce, addr_word0, addr_word1, addr_word2]`

Reuse `keccakf`, `partial_keccakf`, and reward-check helpers copied from `keccak256.cl` (OpenCL has no `#include`; keep `b20.cl` self-contained). Adapt leading/total checks to scan only the 9-byte suffix.

`WORK_SIZE` remains `0x4000000`.

## Rust Architecture

```
src/
  lib.rs          # pub mod b20; re-export compute_b20_address
  b20.rs          # B20Variant, B20Config, compute_b20_address, gpu_b20, mk_b20_kernel_src
  cli.rs          # Commands::B20(B20Args)
  main.rs         # Dispatch b20 subcommand
  kernels/b20.cl
tests/
  b20_test.rs     # CPU reference + GPU kernel tests
```

### CPU reference function

```rust
pub fn compute_b20_address(
    deployer: [u8; 20],
    salt: [u8; 32],
    variant: B20Variant,
) -> [u8; 20]
```

Uses existing `tiny-keccak` dependency. Golden test vectors verified against `getB20Address`.

## Testing

| Layer | Scope | GPU required |
|-------|-------|--------------|
| Unit | `compute_b20_address` vs known vectors | No |
| Kernel | Fixed nonce → expected address (rstest, mirrors existing tests) | Yes |
| Optional integration | `getB20Address` RPC cross-check | No (skipped in CI) |

### Acceptance criteria

1. CPU reference matches `getB20Address` for ≥3 vectors (ASSET, STABLECOIN, multiple deployers)
2. GPU kernel tests match CPU reference
3. B20 `--matching` attempts/sec > create3 on same hardware (document in README or benchmark note)

## Error Handling

| Condition | Error |
|-----------|-------|
| Missing `--caller` | clap required-field error |
| Invalid `--variant` | clap enum validation |
| Suffix pattern length ≠ 18 | `"matching pattern must be 18 characters (suffix mode)"` |
| Full pattern length ≠ 40 | `"matching pattern must be 40 characters long"` |
| Full pattern prefix ≠ variant | `"pattern prefix incompatible with --variant …"` |
| Invalid checksum on `--caller` | Same as existing: `"caller address uses invalid checksum"` |
| `--leading` > 9 or `--total` > 9 | `"threshold must be less than or equal to 9"` (suffix max) |
| `--leading` / `--total` == 0 | Same as existing: `"threshold must be greater than 0"` |

## Documentation

- Update `README.md` with `b20` usage example
- Add `.superpowers/` to `.gitignore`

## Out of Scope

- CPU-only address computation subcommand (user chose GPU mining only)
- Cross-chain salt variants (not applicable to B20)
- B20 token deployment scripting (`createB20` call construction)

## Next Step

After spec approval → invoke `writing-plans` skill for implementation plan.
