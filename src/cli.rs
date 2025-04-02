use clap::{ArgAction, Parser, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
pub struct Args {
	#[clap(
        short,
        long,
        value_parser,
        action = ArgAction::Append,
        default_value = "all",
        help = "specify which skin items to keep (comma-separated or multiple -k flags)"
    )]
	pub keep: Vec<SkinItem>,
	pub skin_path: PathBuf,
	pub output_path: PathBuf,
	#[clap(short, long, default_value = "8")]
	pub threads: usize,
	#[clap(
        short,
        long,
        value_parser,
        action = ArgAction::Append,
        default_value = "image",
        help = "file types to preserve. will not preserve garbage files. (comma-separated or multiple -p flags)"
    )]
	pub preserve_filetypes: Vec<FileType>,
}

#[derive(ValueEnum, Default, Debug, Clone, Eq, PartialEq)]
pub enum SkinItem {
	Standard,
	Catch,
	Taiko,
	Mania,
	Animations,
	Audio,
	LazerFiles,
	Prefixes,
	#[default]
	All,
	None,
}

#[derive(ValueEnum, Debug, Default, Clone, Eq, PartialEq)]
pub enum FileType {
	#[default]
	Image,
	Audio,
	All,
	None,
}
