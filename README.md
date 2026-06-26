# `createXcrunch`

[![👮‍♂️ Sanity checks](https://github.com/HrikB/createXcrunch/actions/workflows/checks.yml/badge.svg)](https://github.com/HrikB/createXcrunch/actions/workflows/checks.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/license/mit)

`createXcrunch` is a [Rust](https://www.rust-lang.org)-based program designed to efficiently find _zero-leading_, _zero-containing_, or _pattern-matching_ addresses for the [CreateX](https://github.com/pcaversaccio/createx) contract factory. Uses [OpenCL](https://www.khronos.org/opencl/) in order to leverage a GPU's mining capabilities.

## Installation

1. **Clone the Repository**

```console
git clone https://github.com/HrikB/createXcrunch.git
cd createXcrunch
```

2. **Build the Project**

```console
cargo build --release
```

> [!NOTE]
> Building on Windows works as long as you have installed the [CUDA Toolkit](https://docs.nvidia.com/cuda/cuda-installation-guide-microsoft-windows/) or the [AMD Radeon Software](https://www.amd.com/en/resources/support-articles/faqs/RS-INSTALL.html). However, the [WSL 2](https://learn.microsoft.com/en-us/windows/wsl/install) installation on Windows `x64` systems with NVIDIA hardware fails, as the current NVIDIA driver does not yet support passing [OpenCL](https://en.wikipedia.org/wiki/OpenCL) to Windows Subsystem for Linux (WSL) (see [here](https://github.com/microsoft/WSL/issues/6951)).

## Example Setup on [Vast.ai](https://vast.ai)

#### Update Linux

```console
sudo apt update && sudo apt upgrade
```

#### Install `build-essential` Packages

> We need the GNU Compiler Collection (GCC) later.

```console
sudo apt install build-essential
```

#### Install CUDA Toolkit

> `createXcrunch` uses [OpenCL](https://en.wikipedia.org/wiki/OpenCL) which is natively supported via the NVIDIA OpenCL extensions.

```console
sudo apt install nvidia-cuda-toolkit
```

#### Install Rust

> Enter `1` to select the default option and press the `Enter` key to continue the installation. Restart the current shell after completing the installation.

```console
curl https://sh.rustup.rs -sSf | sh
```

#### Build `createXcrunch`

```console
git clone https://github.com/HrikB/createXcrunch.git
cd createXcrunch
cargo build --release
```

🎉 Congrats, now you're ready to crunch your salt(s)!

## Usage

`createXcrunch` has three subcommands: `create2`, `create3`, and `b20`.

### Salt layout (mined `bytes32`)

All commands GPU-search the same **11-byte tail**: `[4-byte random][7-byte nonce]`. The full salt depends on the command:

| Command | Salt structure (32 bytes) |
|---------|---------------------------|
| `create2` / `create3` (no flags) | `[4 random][7 nonce][21 × 0x00]` |
| `create2` / `create3` + `--caller` | `[caller (20)][0x00][4 random][7 nonce]` |
| `create2` / `create3` + `--crosschain` | `[20-byte prefix][0x01][chain_id?][4 random][7 nonce]` |
| **`b20`** (always permissioned) | **`[caller (20)][0x00][4 random][7 nonce]`** — same as create2/create3 with `--caller` |

Example B20 salt (caller `0x88c6…134A`):

```text
0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A 00 d883677aab55cd021d2eea
   └────────────── caller ──────────────────┘ ^^ └──── random + nonce ────┘
```

Output format (`output.txt`):

```text
0x<SALT> => 0x<ADDRESS>
```

### Create3

```console
./target/release/createxcrunch create3 \
  --caller 0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A \
  --crosschain 1 \
  --matching ba5edXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXba5ed \
  --count 3
```

`--count` (`-n`) stops after finding the given number of results; `0` (default) runs indefinitely.

### Create2

Requires `--code-hash` (keccak256 of the contract creation bytecode):

```console
./target/release/createxcrunch create2 \
  --code-hash 0x0000000000000000000000000000000000000000000000000000000000000000 \
  --caller 0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A \
  --matching 6XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX \
  --count 3
```

Use your real init code hash for deployment (the all-zero hash above is only for testing the miner).

### B20 (Base Native Token Standard)

Mine a vanity salt for a [B20 Native Token Standard](https://docs.base.org/base-chain/specs/upgrades/beryl/b20) address on Base.

**Address derivation** (verified against factory `0xB20f…` on Base Sepolia):

```text
address = [10-byte B20 prefix][1-byte variant][keccak256(abi.encode(deployer, salt))[0:9]]
```

- Hash input is **`abi.encode(deployer, salt)`** (64 bytes), not `abi.encodePacked`.
- `--caller` is required: deployer for `createB20` / `getB20Address`, and encoded at the start of the mined salt (see table above).
- `--variant asset` (default) or `stablecoin` selects the variant byte in the address.

```console
./target/release/createxcrunch b20 \
  --caller 0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A \
  --variant asset \
  --matching 666666XXXXXXXXXXXX \
  --count 3
```

Suffix mode (default): pass **18 hex characters** for the hash suffix only; the program prepends the B20 prefix + variant, e.g. `666666XXXXXXXXXXXX` → full address `0xb200000000000000000000666666…`.

Use `--full-pattern` for a complete 40-character address pattern (must match the chosen `--variant` prefix).

B20 performs a single `keccak256` and is typically **2–4× faster** than `create3` on the same GPU.

#### Verify a result on-chain

```console
cast call 0xB20f000000000000000000000000000000000000 \
  "getB20Address(uint8,address,bytes32)(address)" \
  0 \
  0x88c6C46EBf353A52Bdbab708c23D0c81dAA8134A \
  0x<SALT_FROM_OUTPUT> \
  --rpc-url https://sepolia.base.org
```

The returned address must match the right-hand side in `output.txt`.

### Help

```console
./target/release/createxcrunch create2 --help
./target/release/createxcrunch create3 --help
./target/release/createxcrunch b20 --help
```

## Local Development

We recommend using [`cargo-nextest`](https://nexte.st) as test runner for this repository. To install it on a Linux `x86_64` machine, invoke:

```console
curl -LsSf https://get.nexte.st/latest/linux | tar zxf - -C ${CARGO_HOME:-~/.cargo}/bin
```

Afterwards you can run the tests via:

```console
cargo nextest run
```

## Contributions

PRs welcome!

## Acknowledgements

- [`create2crunch`](https://github.com/0age/create2crunch)
- [Function Selection Miner](https://github.com/Vectorized/function-selector-miner)
- [`CreateX` – A Trustless, Universal Contract Deployer](https://github.com/pcaversaccio/createx)
