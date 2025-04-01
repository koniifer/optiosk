use std::{
	fmt,
	path::{Path, PathBuf},
};

pub mod aud;
pub mod img;

pub use aud::*;
pub use img::*;

#[derive(Debug)]
pub enum FileKind {
	Image(ImageKind),
	Audio(AudioKind),
	Config(ConfigKind),
	Unknown,
}

#[derive(Debug)]
pub enum ConfigKind {
	SkinIni,
	Lazer,
}

impl<T: AsRef<Path>> From<T> for FileKind {
	fn from(path: T) -> Self {
		let path = path.as_ref();
		let stem = path
			.file_stem()
			.and_then(|s| s.to_str())
			.unwrap_or_default()
			.to_lowercase();
		let ext = path
			.extension()
			.and_then(|s| s.to_str())
			.unwrap_or_default()
			.to_lowercase();

		match ext.as_str() {
			"png" => Self::Image(ImageKind::Png),
			"jpg" | "jpeg" => Self::Image(img::ImageKind::Jpeg),
			"mp3" => Self::Audio(AudioKind::Mp3),
			"ogg" => Self::Audio(AudioKind::Ogg),
			"wav" => Self::Audio(AudioKind::Wav),
			"ini" if stem == "skin" => Self::Config(ConfigKind::SkinIni),
			"json" => Self::Config(ConfigKind::Lazer),
			_ => Self::Unknown,
		}
	}
}

pub struct SkinFile {
	pub kind: FileKind,
	pub path: PathBuf,
	pub bytes: Vec<u8>,
}

impl SkinFile {
	pub fn new(kind: FileKind, path: PathBuf, bytes: Vec<u8>) -> Self {
		Self { kind, path, bytes }
	}

	pub fn is_media(&self) -> bool {
		matches!(self.kind, FileKind::Image(_) | FileKind::Audio(_))
	}

	pub fn media_category(&self) -> u8 {
		match self.kind {
			FileKind::Image(_) => 0,
			FileKind::Audio(_) => 1,
			_ => unreachable!(),
		}
	}

	pub fn png_priority(&self) -> u8 {
		match &self.kind {
			FileKind::Image(ImageKind::Png) => 0,
			FileKind::Image(_) => 1,
			_ => 2,
		}
	}

	pub fn audio_priority(&self) -> u8 {
		match &self.kind {
			FileKind::Audio(AudioKind::Ogg) => 0,
			FileKind::Audio(AudioKind::Wav) => 1,
			FileKind::Audio(_) => 2,
			_ => 3,
		}
	}
}

impl fmt::Debug for SkinFile {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("SkinFile")
			.field("kind", &self.kind)
			.field("path", &self.path)
			.finish()
	}
}
