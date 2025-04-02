use std::path::{Path, PathBuf};

pub mod aud;
pub mod img;
pub use aud::AudioKind;
pub use img::ImageKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
	Image(ImageKind),
	Audio(AudioKind),
	Config(ConfigKind),
	Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigKind {
	LazerJson,
	SkinIni,
}

impl From<&Path> for FileData {
	fn from(path: &Path) -> Self {
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

		let kind = match ext.as_str() {
			"png" => FileKind::Image(ImageKind::Png),
			"jpg" | "jpeg" => FileKind::Image(img::ImageKind::Jpeg),
			"mp3" => FileKind::Audio(AudioKind::Mpeg3),
			"ogg" => FileKind::Audio(AudioKind::Vorbis),
			"wav" => FileKind::Audio(AudioKind::Wave),
			"ini" if stem == "skin" => FileKind::Config(ConfigKind::SkinIni),
			"json" => FileKind::Config(ConfigKind::LazerJson),
			_ => FileKind::Unknown,
		};

		Self {
			kind,
			path: path.to_path_buf(),
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileData {
	pub kind: FileKind,
	pub path: PathBuf,
}
