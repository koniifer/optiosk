use bitflags::bitflags;
use clap::Parser;
use std::path::PathBuf;

bitflags! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct SkinItems: u16 {
	  const STANDARD = 1 << 0;
	  const CATCH = 1 << 1;
	  const TAIKO = 1 << 2;
	  const MANIA = 1 << 3;
	  const ANIMATIONS = 1 << 4;
	  const IMAGES = 1 << 5;
	  const AUDIO = 1 << 6;
	  const LAZER_FILES = 1 << 7;
	  const PREFIXES = 1 << 8;
  }
}

bitflags! {
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct FileTypes: u8 {
	  const IMAGE = 1 << 0;
	  const AUDIO = 1 << 1;
  }
}

#[derive(Parser, Debug)]
pub struct Args {
	#[clap(
        short,
        long,
        value_parser = parse_skin_items,
        default_value = "all",
        help = "specify which skin items to keep (comma-separated or multiple -k flags)"
    )]
	pub keep: SkinItems,
	pub skin_path: PathBuf,
	#[clap(default_value = "OUTPUT_PATH")]
	pub output_path: PathBuf,
	#[clap(short, long, default_value = "4")]
	pub threads: usize,
}

fn parse_skin_items(s: &str) -> Result<SkinItems, String> {
	let mut items = SkinItems::empty();
	for part in s.split(',') {
		match part.trim() {
			"standard" | "std" => items.insert(SkinItems::STANDARD),
			"catch" | "ctb" | "fruits" => items.insert(SkinItems::CATCH),
			"taiko" => items.insert(SkinItems::TAIKO),
			"mania" => items.insert(SkinItems::MANIA),
			"animations" => items.insert(SkinItems::ANIMATIONS),
			"images" | "img" => items.insert(SkinItems::IMAGES),
			"audio" | "aud" => items.insert(SkinItems::AUDIO),
			"lazerfiles" | "lazer" => items.insert(SkinItems::LAZER_FILES),
			"prefixes" => items.insert(SkinItems::PREFIXES),
			"all" => return Ok(SkinItems::all()),
			"none" => return Ok(SkinItems::empty()),
			_ => return Err(format!("invalid skin item: {}", part)),
		}
	}
	Ok(items)
}
