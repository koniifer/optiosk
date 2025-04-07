use bitflags::bitflags;
use clap::{Parser, value_parser};
use std::path::PathBuf;

bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub struct SkinItems: u16 {
		const STANDARD = 1 << 0;
		const CATCH = 1 << 1;
		const TAIKO = 1 << 2;
		const MANIA = 1 << 3;
		const ANIMATIONS = 1 << 4;
		const HD_IMAGES = 1 << 5;
		const SD_IMAGES = 1 << 6;
		const AUDIO = 1 << 7;
		const LAZER_FILES = 1 << 8;
		const PREFIXES = 1 << 9;
	}
}

bitflags! {
	#[derive(Debug, Clone, Copy, PartialEq, Eq)]
	pub struct Tweaks: u16 {
		const TRIPLE_STACK = 1 << 0;
		const INSTANT_FADE = 1 << 1;
		const REMOVE_COMBO_COLOURS = 1 << 2;
		const REMOVE_CURSORTRAIL = 1 << 3;
		const REMOVE_FOLLOWPOINTS = 1 << 4;
		const REMOVE_HITCIRCLES = 1 << 5;
	}
}

#[derive(Parser, Debug)]
pub struct Args {
	#[clap(
    short,
    long,
    value_parser = parse_skin_items,
    default_value = "all",
    help = "specify which skin items to keep (comma-separated) [possible values: all, standard, catch, taiko, mania, hd-images, sd-images, audio, lazer-files, prefixes, none]"
  )]
	pub keep: SkinItems,
	#[clap(
    short,
    long,
    value_parser = parse_tweaks,
    hide_default_value = true,
    default_value = "",
    help = "specify which tweaks to use (comma-separated) [possible values: high-ar, triple-stack, insta-fade, remove-combo-colours]"
  )]
	pub tweaks: Tweaks,
	pub skin_path: PathBuf,
	#[clap(
		default_value = "OUTPUT_PATH",
		hide_default_value = true,
		help = "optional. will output to '/path/to/my-skin.opt.osk' if no path specified"
	)]
	pub output_path: PathBuf,
	#[clap(short = 'j', long, default_value = "0")]
	pub threads: usize,
	#[clap(long, value_parser = value_parser!(u8).range(1..=100), default_value = "80", help = "default is 80 because minor jpeg artifacts shouldnt be noticeable. set to 100 for (theoretically) lossless compression")]
	pub image_quality: u8,
	#[clap(long, value_parser = value_parser!(u8).range(1..=100), default_value = "1", help = "default is 1 because i really can't tell a difference... set to 100 for (theoretically) lossless compression")]
	pub audio_quality: u8,
}

impl Args {
	pub fn parse_and_validate() -> eyre::Result<Self> {
		let mut args = Self::parse();

		if args.output_path.as_os_str() == "OUTPUT_PATH" {
			args.output_path = args.skin_path.with_extension("opt.osk");
		}

		if !args.keep.is_all() {
			eprintln!(
				"warning: removing files does not yet update skin.ini, so you may break a skin by doing this."
			)
		}

		if !args.tweaks.is_empty() {
			eyre::bail!("tweaks are unimplemented so far.")
		}

		if args
			.tweaks
			.contains(Tweaks::INSTANT_FADE | Tweaks::TRIPLE_STACK)
			&& !args.tweaks.contains(Tweaks::REMOVE_COMBO_COLOURS)
		{
			eyre::bail!("triple stack with instant fade does not support combo colours.")
		}

		Ok(args)
	}
}

fn parse_skin_items(s: &str) -> Result<SkinItems, String> {
	let mut items = SkinItems::empty();
	for part in s.split(',') {
		match part.trim() {
			"standard" => items.insert(SkinItems::STANDARD),
			"catch" => items.insert(SkinItems::CATCH),
			"taiko" => items.insert(SkinItems::TAIKO),
			"mania" => items.insert(SkinItems::MANIA),
			"animations" => items.insert(SkinItems::ANIMATIONS),
			"hd-images" => items.insert(SkinItems::HD_IMAGES),
			"sd-images" => items.insert(SkinItems::SD_IMAGES),
			"audio" => items.insert(SkinItems::AUDIO),
			"lazer-files" => items.insert(SkinItems::LAZER_FILES),
			"prefixes" => items.insert(SkinItems::PREFIXES),
			"all" => return Ok(SkinItems::all()),
			"none" => return Ok(SkinItems::empty()),
			_ => return Err(format!("invalid skin item: {}", part)),
		}
	}
	Ok(items)
}

fn parse_tweaks(s: &str) -> Result<Tweaks, String> {
	let mut tweaks = Tweaks::empty();
	for part in s.split(',') {
		match part.trim() {
			"high-ar" => {
				tweaks.insert(Tweaks::INSTANT_FADE | Tweaks::TRIPLE_STACK | Tweaks::REMOVE_COMBO_COLOURS)
			}
			"triple-stack" => tweaks.insert(Tweaks::TRIPLE_STACK),
			"insta-fade" => tweaks.insert(Tweaks::INSTANT_FADE),
			"remove-combo-colours" => tweaks.insert(Tweaks::REMOVE_COMBO_COLOURS),
			"remove-followpoints" => tweaks.insert(Tweaks::REMOVE_FOLLOWPOINTS),
			"remove-hitcircles" => tweaks.insert(Tweaks::REMOVE_HITCIRCLES),
			"" => (),
			_ => return Err(format!("invalid skin item: {}", part)),
		}
	}
	Ok(tweaks)
}
