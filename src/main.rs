use clap::Parser;
use createxcrunch::{
    b20::{B20Config, B20Variant},
    cli::{B20VariantArg, Cli, Commands},
    gpu, gpu_b20, Config, RewardVariant,
};

fn build_reward(
    zeros: Option<u8>,
    total: Option<u8>,
    either: bool,
    pattern: Option<Box<str>>,
) -> RewardVariant {
    match (zeros, total, either, pattern) {
        (Some(zeros), None, false, None) => RewardVariant::LeadingZeros {
            zeros_threshold: zeros,
        },
        (None, Some(total), false, None) => RewardVariant::TotalZeros {
            zeros_threshold: total,
        },
        (Some(zeros), Some(total), false, None) => RewardVariant::LeadingAndTotalZeros {
            leading_zeros_threshold: zeros,
            total_zeros_threshold: total,
        },
        (Some(zeros), Some(total), true, None) => RewardVariant::LeadingOrTotalZeros {
            leading_zeros_threshold: zeros,
            total_zeros_threshold: total,
        },
        (None, None, false, Some(pattern)) => {
            let pattern = pattern
                .strip_prefix("0x")
                .unwrap_or(&pattern)
                .to_owned()
                .into_boxed_str();
            RewardVariant::Matching { pattern }
        }
        _ => unreachable!(),
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Create2(args) => {
            let reward = build_reward(
                args.cli_args.zeros,
                args.cli_args.total,
                args.cli_args.either,
                args.cli_args.pattern,
            );
            match Config::new(
                args.cli_args.gpu_device_id,
                &args.cli_args.factory,
                args.cli_args.caller.as_deref(),
                args.cli_args.chain_id,
                Some(&args.init_code_hash),
                reward,
                &args.cli_args.output,
            ) {
                Ok(mut config) => {
                    config.count = args.cli_args.count;
                    gpu(config).unwrap_or_else(|e| panic!("{}", e))
                }
                Err(e) => panic!("{}", e),
            }
        }
        Commands::Create3(args) => {
            let reward = build_reward(args.zeros, args.total, args.either, args.pattern);
            match Config::new(
                args.gpu_device_id,
                &args.factory,
                args.caller.as_deref(),
                args.chain_id,
                None,
                reward,
                &args.output,
            ) {
                Ok(mut config) => {
                    config.count = args.count;
                    gpu(config).unwrap_or_else(|e| panic!("{}", e))
                }
                Err(e) => panic!("{}", e),
            }
        }
        Commands::B20(args) => {
            let variant = match args.variant {
                B20VariantArg::Asset => B20Variant::Asset,
                B20VariantArg::Stablecoin => B20Variant::Stablecoin,
            };
            let reward = build_reward(args.zeros, args.total, args.either, args.pattern);
            match B20Config::new(
                args.gpu_device_id,
                &args.caller,
                variant,
                reward,
                args.full_pattern,
                &args.output,
            ) {
                Ok(mut config) => {
                    config.count = args.count;
                    gpu_b20(config).unwrap_or_else(|e| panic!("{}", e))
                }
                Err(e) => panic!("{}", e),
            }
        }
    }
}
