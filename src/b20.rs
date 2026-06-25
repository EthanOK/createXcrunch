use alloy_primitives::{hex, Address, FixedBytes};
use byteorder::{BigEndian, ByteOrder, LittleEndian};
use console::Term;
use fs4::FileExt;
use itertools::chain;
use ocl::{Buffer, Context, Device, MemFlags, Platform, ProQue, Program, Queue};
use rand::{thread_rng, Rng};
use separator::Separatable;
use std::{
    fmt::Write as _,
    fs::{File, OpenOptions},
    io::prelude::*,
    time::{SystemTime, UNIX_EPOCH},
};
use terminal_size::{terminal_size, Height};
use tiny_keccak::{Hasher, Keccak};

use crate::RewardVariant;

pub const B20_PREFIX: [u8; 10] = [0xB2, 0, 0, 0, 0, 0, 0, 0, 0, 0];

static B20_KERNEL_SRC: &str = include_str!("./kernels/b20.cl");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum B20Variant {
    Asset = 0,
    Stablecoin = 1,
}

impl B20Variant {
    pub fn prefix_hex(&self) -> &'static str {
        match self {
            B20Variant::Asset => "b200000000000000000000",
            B20Variant::Stablecoin => "b200000000000000000001",
        }
    }
}

/// B20 address = prefix || variant || keccak256(abi.encode(deployer, salt))[0..9]
pub fn compute_b20_address(deployer: [u8; 20], salt: [u8; 32], variant: B20Variant) -> [u8; 20] {
    let mut hasher = Keccak::v256();
    let mut padded_deployer = [0u8; 32];
    padded_deployer[12..32].copy_from_slice(&deployer);
    hasher.update(&padded_deployer);
    hasher.update(&salt);
    let mut hash = [0u8; 32];
    hasher.finalize(&mut hash);

    let mut addr = [0u8; 20];
    addr[..10].copy_from_slice(&B20_PREFIX);
    addr[10] = variant as u8;
    addr[11..20].copy_from_slice(&hash[..9]);
    addr
}

pub struct B20Config<'a> {
    pub gpu_device: u8,
    pub deployer: [u8; 20],
    pub variant: B20Variant,
    pub reward: RewardVariant,
    pub full_pattern: Option<Box<str>>,
    pub output: &'a str,
}

impl<'a> B20Config<'a> {
    pub fn new(
        gpu_device: u8,
        caller: &str,
        variant: B20Variant,
        reward: RewardVariant,
        full_pattern_mode: bool,
        output: &'a str,
    ) -> Result<Self, &'static str> {
        let caller = caller.strip_prefix("0x").unwrap_or(caller);
        let deployer_vec =
            hex::decode(caller).map_err(|_| "could not decode caller address argument")?;
        let deployer: [u8; 20] = deployer_vec
            .try_into()
            .map_err(|_| "invalid length for caller address argument")?;

        validate_caller_checksum(caller)?;
        validate_b20_reward(&reward, full_pattern_mode, variant)?;

        let full_pattern = match &reward {
            RewardVariant::Matching { pattern } => {
                Some(expand_pattern(pattern, variant, full_pattern_mode)?)
            }
            _ => None,
        };

        Ok(Self {
            gpu_device,
            deployer,
            variant,
            reward,
            full_pattern,
            output,
        })
    }

    pub fn full_pattern(&self) -> &str {
        self.full_pattern.as_deref().unwrap_or("")
    }
}

fn validate_caller_checksum(caller: &str) -> Result<(), &'static str> {
    if !caller.chars().any(|c| c.is_uppercase()) {
        return Ok(());
    }
    let checksummed = match caller.strip_prefix("0x") {
        Some(_) => caller.to_string(),
        None => format!("0x{}", caller),
    };
    match Address::parse_checksummed(checksummed, None) {
        Ok(_) => Ok(()),
        Err(_) => Err("caller address uses invalid checksum"),
    }
}

fn validate_b20_reward(
    reward: &RewardVariant,
    full_pattern_mode: bool,
    variant: B20Variant,
) -> Result<(), &'static str> {
    match reward {
        RewardVariant::LeadingZeros { zeros_threshold }
        | RewardVariant::TotalZeros { zeros_threshold } => {
            validate_suffix_threshold(zeros_threshold)
        }
        RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        }
        | RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => {
            validate_suffix_threshold(leading_zeros_threshold)?;
            validate_suffix_threshold(total_zeros_threshold)
        }
        RewardVariant::Matching { pattern } => {
            validate_matching_pattern(pattern, variant, full_pattern_mode)
        }
    }
}

fn validate_suffix_threshold(t: &u8) -> Result<(), &'static str> {
    if *t == 0 {
        return Err("threshold must be greater than 0");
    }
    if *t > 9 {
        return Err("threshold must be less than or equal to 9");
    }
    Ok(())
}

fn validate_matching_pattern(
    pattern: &str,
    variant: B20Variant,
    full_pattern_mode: bool,
) -> Result<(), &'static str> {
    let p = pattern.strip_prefix("0x").unwrap_or(pattern);
    if !p.chars().all(|c| c == 'X' || c.is_ascii_hexdigit()) {
        return Err("matching pattern must only contain 'X' or hex characters");
    }
    if full_pattern_mode {
        if p.len() != 40 {
            return Err("matching pattern must be 40 characters long");
        }
        if !p.starts_with(variant.prefix_hex()) {
            return Err("pattern prefix incompatible with --variant");
        }
    } else if p.len() != 18 {
        return Err("matching pattern must be 18 characters (suffix mode)");
    }
    Ok(())
}

fn expand_pattern(
    pattern: &str,
    variant: B20Variant,
    full_pattern_mode: bool,
) -> Result<Box<str>, &'static str> {
    let p = pattern.strip_prefix("0x").unwrap_or(pattern);
    if full_pattern_mode {
        Ok(p.to_lowercase().into_boxed_str())
    } else {
        let mut full = String::with_capacity(40);
        full.push_str(variant.prefix_hex());
        full.push_str(p);
        Ok(full.into_boxed_str())
    }
}

const WORK_SIZE: u32 = 0x4000000;
const WORK_FACTOR: u128 = (WORK_SIZE as u128) / 1_000_000;

pub fn gpu_b20(config: B20Config<'_>) -> ocl::Result<()> {
    println!(
        "Setting up OpenCL B20 miner using device {}...",
        config.gpu_device
    );

    let file = b20_output_file(&config);
    let mut found: u64 = 0;
    let mut found_list: Vec<String> = vec![];
    let term = Term::stdout();

    let platform = Platform::new(ocl::core::default_platform()?);
    let device = Device::by_idx_wrap(platform, config.gpu_device as usize)?;
    let context = Context::builder()
        .platform(platform)
        .devices(device)
        .build()?;
    let program = Program::builder()
        .devices(device)
        .src(mk_b20_kernel_src(&config))
        .build(&context)?;
    let queue = Queue::new(&context, device, None)?;
    let ocl_pq = ProQue::new(context, queue, program, Some(WORK_SIZE));

    let mut rng = thread_rng();
    let start_time: f64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64();
    let mut rate: f64 = 0.0;
    let mut cumulative_nonce: u64 = 0;
    let mut previous_time: f64 = 0.0;
    let mut work_duration_millis: u64 = 0;

    loop {
        let salt_prefix = FixedBytes::<4>::random();
        let message_buffer = Buffer::builder()
            .queue(ocl_pq.queue().clone())
            .flags(MemFlags::new().read_only())
            .len(4)
            .copy_host_slice(&salt_prefix[..])
            .build()?;

        let mut nonce: [u32; 1] = rng.gen();
        let mut view_buf = [0; 8];
        let mut nonce_buffer = Buffer::builder()
            .queue(ocl_pq.queue().clone())
            .flags(MemFlags::new().read_only())
            .len(1)
            .copy_host_slice(&nonce)
            .build()?;

        let mut solutions: Vec<u64> = vec![0; 4];
        let solutions_buffer = Buffer::builder()
            .queue(ocl_pq.queue().clone())
            .flags(MemFlags::new().write_only())
            .len(4)
            .copy_host_slice(&solutions)
            .build()?;

        loop {
            let kern = ocl_pq
                .kernel_builder("hashB20Salt")
                .arg_named("message", None::<&Buffer<u8>>)
                .arg_named("nonce", None::<&Buffer<u32>>)
                .arg_named("solutions", None::<&Buffer<u64>>)
                .build()?;

            kern.set_arg("message", Some(&message_buffer))?;
            kern.set_arg("nonce", Some(&nonce_buffer))?;
            kern.set_arg("solutions", &solutions_buffer)?;

            unsafe { kern.enq()? };

            let mut now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            let current_time = now.as_secs() as f64;
            let print_output = current_time - previous_time > 0.99;
            previous_time = current_time;

            if print_output {
                term.clear_screen()?;

                let total_runtime = current_time - start_time;
                let total_runtime_hrs = total_runtime as u64 / 3600;
                let total_runtime_mins = (total_runtime as u64 - total_runtime_hrs * 3600) / 60;
                let total_runtime_secs = total_runtime
                    - (total_runtime_hrs * 3600) as f64
                    - (total_runtime_mins * 60) as f64;

                let work_rate: u128 = WORK_FACTOR * cumulative_nonce as u128;
                if total_runtime > 0.0 {
                    rate = 1.0 / total_runtime;
                }

                LittleEndian::write_u64(&mut view_buf, (nonce[0] as u64) << 32);

                let height = terminal_size().map(|(_w, Height(h))| h).unwrap_or(10);

                term.write_line(&format!(
                    "total runtime: {}:{:02}:{:02} ({} cycles)\t\t\t\
                     work size per cycle: {}",
                    total_runtime_hrs,
                    total_runtime_mins,
                    total_runtime_secs,
                    cumulative_nonce,
                    WORK_SIZE.separated_string(),
                ))?;

                term.write_line(&format!(
                    "rate: {:.2} million attempts per second\t\t\t\
                     total found this run: {}",
                    work_rate as f64 * rate,
                    found
                ))?;

                let threshold_string = b20_threshold_string(&config.reward);
                term.write_line(&format!(
                    "current search space: {}xxxxxxxx{:06x}\t\t\
                     threshold: mining for B20 {} address {}",
                    hex::encode(salt_prefix),
                    BigEndian::read_u64(&view_buf) >> 8,
                    match config.variant {
                        B20Variant::Asset => "ASSET",
                        B20Variant::Stablecoin => "STABLECOIN",
                    },
                    threshold_string
                ))?;

                let rows = if height < 5 { 1 } else { height as usize - 4 };
                let last_rows: Vec<String> = found_list.iter().cloned().rev().take(rows).collect();
                let ordered: Vec<String> = last_rows.iter().cloned().rev().collect();
                term.write_line(&ordered.join("\n"))?;
            }

            cumulative_nonce += 1;
            let work_start_time_millis =
                now.as_secs() * 1000 + now.subsec_nanos() as u64 / 1_000_000;

            if work_duration_millis != 0 {
                std::thread::sleep(std::time::Duration::from_millis(
                    work_duration_millis * 980 / 1000,
                ));
            }

            solutions_buffer.read(&mut solutions).enq()?;
            now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
            work_duration_millis = (now.as_secs() * 1000 + now.subsec_nanos() as u64 / 1_000_000)
                - work_start_time_millis;

            if solutions[0] != 0 {
                break;
            }

            nonce[0] += 1;
            nonce_buffer = Buffer::builder()
                .queue(ocl_pq.queue().clone())
                .flags(MemFlags::new().read_write())
                .len(1)
                .copy_host_slice(&nonce)
                .build()?;
        }

        let solution = solutions[0].to_le_bytes();
        let mined_salt = chain!(salt_prefix, solution[..7].iter().copied());
        let mut salt = [0u8; 32];
        for (i, b) in mined_salt.enumerate() {
            salt[i] = b;
        }

        let address: Vec<u8> = solutions[1]
            .to_be_bytes()
            .into_iter()
            .chain(solutions[2].to_be_bytes())
            .chain(solutions[3].to_be_bytes()[..4].to_vec())
            .collect();

        let suffix = &address[11..20];
        let mut total = 0u8;
        let mut leading = 0u8;
        for (i, &b) in suffix.iter().enumerate() {
            if b != 0 {
                continue;
            }
            if leading == i as u8 {
                leading = i as u8 + 1;
            }
            total += 1;
        }

        let output = format!("0x{} => 0x{}", hex::encode(salt), hex::encode(address));
        let show = format!("{output} ({leading} / {total})");
        match config.reward {
            RewardVariant::Matching { .. } => found_list.push(output.clone()),
            _ => found_list.push(show),
        }

        file.lock_exclusive().expect("Couldn't lock file.");
        writeln!(&file, "{output}").expect("Couldn't write to output file.");
        file.unlock().expect("Couldn't unlock file.");
        found += 1;
    }
}

fn b20_threshold_string(reward: &RewardVariant) -> String {
    match reward {
        RewardVariant::LeadingZeros { zeros_threshold } => {
            format!("with {} leading zero byte(s) in suffix", zeros_threshold)
        }
        RewardVariant::TotalZeros { zeros_threshold } => {
            format!("with {} total zero byte(s) in suffix", zeros_threshold)
        }
        RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => format!(
            "with {} leading and {} total zero byte(s) in suffix",
            leading_zeros_threshold, total_zeros_threshold
        ),
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => format!(
            "with {} leading or {} total zero byte(s) in suffix",
            leading_zeros_threshold, total_zeros_threshold
        ),
        RewardVariant::Matching { pattern } => format!("matching pattern 0x{}", pattern),
    }
}

#[track_caller]
fn b20_output_file(config: &B20Config<'_>) -> File {
    OpenOptions::new()
        .append(true)
        .create(true)
        .read(true)
        .open(config.output)
        .unwrap_or_else(|_| panic!("Could not create or open {} file.", config.output))
}

pub fn mk_b20_kernel_src(config: &B20Config<'_>) -> String {
    let mut src = String::with_capacity(2048 + B20_KERNEL_SRC.len());

    match &config.reward {
        RewardVariant::LeadingZeros { zeros_threshold } => {
            writeln!(src, "#define PATTERN() \"\"").unwrap();
            writeln!(src, "#define LEADING_ZEROES {zeros_threshold}").unwrap();
            writeln!(src, "#define TOTAL_ZEROES 0").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() hasSuffixLeading(digest)").unwrap();
        }
        RewardVariant::TotalZeros { zeros_threshold } => {
            writeln!(src, "#define PATTERN() \"\"").unwrap();
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {zeros_threshold}").unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() hasSuffixTotal(digest)").unwrap();
        }
        RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => {
            writeln!(src, "#define PATTERN() \"\"").unwrap();
            writeln!(src, "#define LEADING_ZEROES {leading_zeros_threshold}").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {total_zeros_threshold}").unwrap();
            writeln!(
                src,
                "#define SUCCESS_CONDITION() hasSuffixLeading(digest) && hasSuffixTotal(digest)"
            )
            .unwrap();
        }
        RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold,
            total_zeros_threshold,
        } => {
            writeln!(src, "#define PATTERN() \"\"").unwrap();
            writeln!(src, "#define LEADING_ZEROES {leading_zeros_threshold}").unwrap();
            writeln!(src, "#define TOTAL_ZEROES {total_zeros_threshold}").unwrap();
            writeln!(
                src,
                "#define SUCCESS_CONDITION() hasSuffixLeading(digest) || hasSuffixTotal(digest)"
            )
            .unwrap();
        }
        RewardVariant::Matching { .. } => {
            writeln!(src, "#define LEADING_ZEROES 0").unwrap();
            writeln!(src, "#define TOTAL_ZEROES 0").unwrap();
            writeln!(src, "#define PATTERN() \"{}\"", config.full_pattern()).unwrap();
            writeln!(src, "#define SUCCESS_CONDITION() isMatching(addr)").unwrap();
        }
    };

    writeln!(src, "#define VARIANT_BYTE {}u", config.variant as u8).unwrap();
    for (i, b) in config.deployer.iter().enumerate() {
        writeln!(src, "#define DEPLOY_{} {}u", i, b).unwrap();
    }

    src.push_str(B20_KERNEL_SRC);
    src
}
