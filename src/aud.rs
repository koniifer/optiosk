use eyre::{ContextCompat, Result};
use hound::{SampleFormat, WavReader};
use minimp3::{Decoder, Frame};
use optivorbis::{Remuxer, VorbisOptimizerSettings, remuxer::ogg_to_ogg::Settings};
use std::{io::Cursor, panic::catch_unwind};
use vorbis_rs::VorbisDecoder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioKind {
	Vorbis,
	Wave,
	Mpeg3,
}

pub fn optimise(bytes: &[u8], kind: AudioKind) -> Result<Option<Vec<u8>>> {
	match kind {
		AudioKind::Vorbis => {
			let mut settings = VorbisOptimizerSettings::default();
			settings.comment_fields_action = optivorbis::VorbisCommentFieldsAction::Delete;
			settings.vendor_string_action = optivorbis::VorbisVendorStringAction::Empty;

			let options = Settings {
				error_on_no_vorbis_streams: false,
				// not sure how safe this is for osu skins
				ignore_start_sample_offset: true,
				..Default::default()
			};

			let encoder = optivorbis::OggToOgg::new(options, settings);
			let mut output = Vec::with_capacity(bytes.len());
			encoder.remux(Cursor::new(bytes), &mut output)?;
			if output.len() >= bytes.len() {
				Ok(None)
			} else {
				Ok(Some(output))
			}
		}
		_ => Ok(None),
	}
}

pub fn convert_to_vorbis(bytes: &[u8], kind: AudioKind) -> Result<Vec<u8>> {
	if bytes.is_empty() {
		eyre::bail!("this is an empty file");
	}

	let (pcm_data, sample_rate, channels) = match kind {
		AudioKind::Mpeg3 => mp3_to_pcm(bytes)?,
		AudioKind::Wave => wav_to_pcm(bytes)?,
		AudioKind::Vorbis => vorbis_to_pcm(bytes)?,
	};

	if sample_rate == 0 || pcm_data.is_empty() || is_silent(&pcm_data) {
		eyre::bail!("this is an empty file");
	}

	if matches!(kind, AudioKind::Vorbis) {
		return Ok(bytes.to_vec());
	}

	pcm_to_vorbis(pcm_data, sample_rate, channels)
}

fn is_silent(pcm_data: &[f32]) -> bool {
	pcm_data.iter().all(|&sample| sample.abs() < f32::EPSILON)
}

fn mp3_to_pcm(bytes: &[u8]) -> Result<(Vec<f32>, u64, u32)> {
	// this library sometimes panics. dunno why. lol.
	catch_unwind(|| {
		let mut decoder = Decoder::new(bytes);
		let mut pcm = Vec::new();
		let (mut rate, mut ch) = (None, None);

		while let Ok(Frame {
			data,
			sample_rate,
			channels,
			..
		}) = decoder.next_frame()
		{
			rate.get_or_insert(sample_rate);
			ch.get_or_insert(channels);

			if rate != Some(sample_rate) || ch != Some(channels) {
				eyre::bail!("mp3 stream parameters changed");
			}

			pcm.extend(data.iter().map(|&s| s as f32 / 32768.0));
		}

		Ok((
			pcm,
			rate.context("no mp3 frames")? as u64,
			ch.context("no channels")? as u32,
		))
	})
	.unwrap_or_else(|_| eyre::bail!("mp3 decoder panic"))
}

fn wav_to_pcm(bytes: &[u8]) -> Result<(Vec<f32>, u64, u32)> {
	let reader = WavReader::new(bytes)?;
	let spec = reader.spec();

	let pcm = match spec.sample_format {
		SampleFormat::Int => reader
			.into_samples::<i32>()
			.map(|s| Ok(s? as f32 / (1i32 << (spec.bits_per_sample - 1)) as f32))
			.collect::<Result<Vec<_>>>()?,
		SampleFormat::Float => reader
			.into_samples::<f32>()
			.map(|s| Ok(s?.clamp(-1.0, 1.0)))
			.collect::<Result<Vec<_>>>()?,
	};

	Ok((pcm, spec.sample_rate as u64, spec.channels as u32))
}

fn pcm_to_vorbis(pcm: Vec<f32>, rate: u64, channels: u32) -> Result<Vec<u8>> {
	use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

	let channels = channels as usize;
	let mut bytes = Vec::with_capacity(pcm.len() * 4);

	VorbisEncoderBuilder::new(
		(rate as u32).try_into()?,
		(channels as u8).try_into()?,
		&mut bytes,
	)?
	.bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
		// maybe i will make this configurable
		target_quality: 0.4,
	})
	.build()?
	.encode_audio_block(
		&(0..channels)
			.map(|i| {
				pcm.iter()
					.skip(i)
					.step_by(channels)
					.copied()
					.collect::<Vec<_>>()
			})
			.collect::<Vec<_>>()
			.iter()
			.map(|v| v.as_slice())
			.collect::<Vec<_>>(),
	)?;

	Ok(bytes)
}

fn vorbis_to_pcm(data: &[u8]) -> Result<(Vec<f32>, u64, u32)> {
	let mut decoder = VorbisDecoder::new(Cursor::new(data))?;
	let mut pcm = Vec::new();
	let rate = decoder.sampling_frequency().get() as u64;
	let ch = decoder.channels().get() as u32;

	while let Some(block) = decoder.decode_audio_block()? {
		for i in 0..block.samples()[0].len() {
			for c in 0..ch as usize {
				pcm.push(block.samples()[c][i]);
			}
		}
	}

	Ok((pcm, rate, ch))
}
