use camino::Utf8Path;
use file_format::FileFormat;
pub use img::ImageKind;
use std::borrow::Cow;

pub mod aud;
pub mod img;
pub use aud::AudioKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigKind {
	LazerJson,
	SkinIni,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
	Image(ImageKind),
	Audio(AudioKind),
	Config(ConfigKind),
	Unknown,
}

impl FileKind {
	pub fn extension(&self) -> &str {
		match self {
			FileKind::Image(ImageKind::Jpeg) => "jpg",
			FileKind::Image(ImageKind::Png) => "png",
			FileKind::Audio(AudioKind::MP3) => "mp3",
			FileKind::Audio(AudioKind::Wave) => "wav",
			FileKind::Audio(AudioKind::Vorbis) => "ogg",
			FileKind::Config(ConfigKind::LazerJson) => "json",
			FileKind::Config(ConfigKind::SkinIni) => "ini",
			FileKind::Unknown => "",
		}
	}
}

impl From<FileFormat> for FileKind {
	/// returns FileKind::Unknown for config files, because there is no context of path extension.
	fn from(format: FileFormat) -> Self {
		match format {
			FileFormat::JointPhotographicExpertsGroup => FileKind::Image(ImageKind::Jpeg),
			FileFormat::PortableNetworkGraphics => FileKind::Image(ImageKind::Png),
			FileFormat::WaveformAudio => FileKind::Audio(AudioKind::Wave),
			FileFormat::OggVorbis => FileKind::Audio(AudioKind::Vorbis),
			FileFormat::Mpeg12AudioLayer3 => FileKind::Audio(AudioKind::MP3),
			_ => FileKind::Unknown,
		}
	}
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileData<'a> {
	pub kind: FileKind,
	pub path: Cow<'a, Utf8Path>,
}

impl<'a> From<&'a Utf8Path> for FileData<'a> {
	fn from(path: &'a Utf8Path) -> Self {
		let ext = path.extension().unwrap_or_default().to_lowercase();

		let kind = match ext.as_str() {
			"png" => FileKind::Image(ImageKind::Png),
			"jpg" | "jpeg" => FileKind::Image(img::ImageKind::Jpeg),
			"mp3" => FileKind::Audio(AudioKind::MP3),
			"ogg" => FileKind::Audio(AudioKind::Vorbis),
			"wav" => FileKind::Audio(AudioKind::Wave),
			"ini" if path.file_stem().unwrap_or_default().to_lowercase() == "skin" => {
				FileKind::Config(ConfigKind::SkinIni)
			}
			"json" => FileKind::Config(ConfigKind::LazerJson),
			_ => FileKind::Unknown,
		};

		Self {
			kind,
			path: Cow::Borrowed(path),
		}
	}
}
