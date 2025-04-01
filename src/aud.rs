use anyhow::{Context, Error, Result};
use hound::{SampleFormat, WavReader};
use minimp3::{Decoder, Frame};
use optivorbis::{Remuxer, VorbisOptimizerSettings, remuxer::ogg_to_ogg::Settings};
use std::io::Cursor;
use vorbis_rs::VorbisDecoder;

#[derive(Debug)]
pub enum AudioKind {
	Mp3,
	Ogg,
	Wav,
}

pub fn optimise_vorbis(bytes: &[u8]) -> Result<Vec<u8>> {
	let mut settings = VorbisOptimizerSettings::default();
	settings.comment_fields_action = optivorbis::VorbisCommentFieldsAction::Delete;
	settings.vendor_string_action = optivorbis::VorbisVendorStringAction::Empty;
	let mut options = Settings::default();
	options.error_on_no_vorbis_streams = false;
	options.ignore_start_sample_offset = true;
	let encoder = optivorbis::OggToOgg::new(options, settings);
	let mut output = Vec::new();
	encoder.remux(Cursor::new(bytes), &mut output)?;
	Ok(output)
}

pub fn convert_to_vorbis(bytes: &[u8], kind: &AudioKind) -> Result<Option<Vec<u8>>> {
	if bytes.is_empty() {
		return Ok(None);
	}

	let (pcm_data, sample_rate, channels) = match kind {
		AudioKind::Mp3 => mp3_to_pcm(bytes)?,
		AudioKind::Wav => wav_to_pcm(bytes)?,
		AudioKind::Ogg => vorbis_to_pcm(bytes)?,
	};

	if sample_rate == 0 || pcm_data.is_empty() || is_silent(&pcm_data) {
		return Ok(None);
	}

	if matches!(kind, AudioKind::Ogg) {
		return Ok(Some(bytes.to_vec()));
	}

	pcm_to_vorbis(pcm_data, sample_rate, channels).map(Some)
}

fn is_silent(pcm_data: &[f32]) -> bool {
	pcm_data.iter().all(|&sample| sample == 0.0)
}

fn mp3_to_pcm(bytes: &[u8]) -> Result<(Vec<f32>, u64, u32)> {
	let mut decoder = Decoder::new(bytes);
	let mut pcm_data = Vec::<f32>::new();
	let (mut sample_rate, mut channels) = (None, None);

	while let Ok(Frame {
		data,
		sample_rate: sr,
		channels: ch,
		..
	}) = decoder.next_frame()
	{
		if sample_rate.is_none() {
			sample_rate = Some(sr);
			channels = Some(ch);
		}

		if sample_rate != Some(sr) || channels != Some(ch) {
			anyhow::bail!("mp3 stream parameters changed");
		}

		pcm_data.extend(data.iter().map(|&s| s as f32 / 32768.0));
	}

	Ok((
		pcm_data,
		sample_rate.context("no mp3 frames found")? as u64,
		channels.context("no channel info")? as u32,
	))
}

fn wav_to_pcm(bytes: &[u8]) -> Result<(Vec<f32>, u64, u32)> {
	let reader = WavReader::new(bytes)?;
	let spec = reader.spec();

	let pcm_data = match spec.sample_format {
		SampleFormat::Int => reader
			.into_samples::<i32>()
			.map(|s| Ok::<f32, Error>(s? as f32 / (1i32 << (spec.bits_per_sample - 1)) as f32))
			.collect::<Result<Vec<_>, _>>()?,
		SampleFormat::Float => reader
			.into_samples::<f32>()
			.map(|s| Ok::<f32, Error>(s?.clamp(-1.0, 1.0)))
			.collect::<Result<Vec<_>, _>>()?,
	};

	Ok((pcm_data, spec.sample_rate as u64, spec.channels as u32))
}

fn pcm_to_vorbis(pcm_data: Vec<f32>, sample_rate: u64, channels: u32) -> Result<Vec<u8>> {
	use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

	let channels = channels as usize;
	if channels == 0 || pcm_data.len() % channels != 0 {
		anyhow::bail!("Invalid PCM data for channel count {channels}");
	}

	let mut bytes = Vec::new();
	let mut encoder = VorbisEncoderBuilder::new(
		(sample_rate as u32).try_into()?,
		(channels as u8).try_into()?,
		&mut bytes,
	)?
	.bitrate_management_strategy(VorbisBitrateManagementStrategy::QualityVbr {
		target_quality: 0.3,
	})
	.build()?;

	let channels_data: Vec<_> = (0..channels)
		.map(|i| {
			pcm_data
				.iter()
				.skip(i)
				.step_by(channels)
				.copied()
				.collect::<Vec<_>>()
		})
		.collect();

	encoder.encode_audio_block(
		&channels_data
			.iter()
			.map(|v| v.as_slice())
			.collect::<Vec<_>>(),
	)?;
	encoder.finish()?;

	Ok(bytes)
}

fn vorbis_to_pcm(vorbis_data: &[u8]) -> Result<(Vec<f32>, u64, u32)> {
	let mut decoder = VorbisDecoder::new(std::io::Cursor::new(vorbis_data))?;

	let sample_rate = decoder.sampling_frequency().get() as u64;
	let channel_count = decoder.channels().get() as u32;

	let mut pcm_data = Vec::new();

	while let Some(block) = decoder.decode_audio_block()? {
		let samples_per_channel = block.samples()[0].len();

		for sample_idx in 0..samples_per_channel {
			for channel_idx in 0..channel_count as usize {
				pcm_data.push(block.samples()[channel_idx][sample_idx]);
			}
		}
	}

	Ok((pcm_data, sample_rate, channel_count))
}
