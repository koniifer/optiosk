mod cli;
use cli::{Args, SkinItems};

use eyre::Result;
use file_format::FileFormat;
use memmap2::Mmap;
use piz::ZipArchive;
use rayon::{
	iter::{IntoParallelIterator, IntoParallelRefMutIterator, ParallelIterator},
	slice::ParallelSliceMut,
};
use std::{
	borrow::Cow,
	fs::File,
	io::{Read, Write},
	panic::catch_unwind,
	time::Instant,
};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use optiosk::*;

const EMPTY_PNG: &[u8; 67] = include_bytes!("1x1.png");

fn main() -> Result<()> {
	let args = Args::parse_and_validate()?;

	rayon::ThreadPoolBuilder::new()
		.num_threads(args.threads)
		.build_global()?;

	#[cfg(feature = "debug")]
	let mut timer = Instant::now();
	let begin = Instant::now();

	let input_file = File::open(&args.skin_path)?;
	let memmap = unsafe { Mmap::map(&input_file)? };

	let input = match catch_unwind(|| ZipArchive::new(&memmap)) {
		Ok(i) => i?,
		_ => eyre::bail!("encountered zip decode panic (probably malformed zip file). exiting."),
	};

	// file (elimination & classification) step
	let (mut media, mut entries): (Vec<_>, Vec<_>) = input
		.entries()
		.into_par_iter()
		.filter_map(|entry| {
			|| -> Result<_> {
				let mut reader = input.read(entry)?;
				let mut bytes = Vec::with_capacity(entry.size);
				reader.read_to_end(&mut bytes)?;
				let mut file_data: FileData = entry.path.as_ref().into();

				// some skins fail to use the correct extensions for their files
				file_data.kind = match FileFormat::from_bytes(&bytes).into() {
					FileKind::Unknown => file_data.kind,
					kind => kind,
				};

				let mut p = file_data.path.with_extension("");
				// saving some headaches
				if let Some("Skin") = p.file_name() {
					p.set_file_name("skin");
				}
				file_data.path = Cow::Owned(p);

				match file_data.kind {
					FileKind::Image(_) => {
						let is_hd = file_data
							.path
							.file_name()
							.is_some_and(|n| n.contains("@2x"));

						let keep = match (is_hd, args.keep) {
							(true, k) => k.contains(SkinItems::HD_IMAGES),
							(false, k) => k.contains(SkinItems::SD_IMAGES),
						};
						if !keep {
							return Ok(None);
						}
					}
					FileKind::Audio(_) if !args.keep.contains(SkinItems::AUDIO) => return Ok(None),
					FileKind::Config(ConfigKind::LazerJson)
						if !args.keep.contains(SkinItems::LAZER_FILES) =>
					{
						return Ok(None);
					}
					FileKind::Unknown => return Ok(None),
					_ => (),
				}
				Ok(Some((file_data, bytes)))
			}()
			.transpose()
		})
		.collect::<Result<Vec<_>>>()?
		.into_par_iter()
		.partition(|(data, _)| matches!(data.kind, FileKind::Image(_) | FileKind::Audio(_)));

	#[cfg(feature = "debug")]
	{
		println!("finished file elimination in {:.3?}.", timer.elapsed());
		timer = Instant::now();
	}

	// sort by highest internal priority to avoid using the wrong assets
	media.par_sort_by(|a, b| {
		a.0
			.path
			.cmp(&b.0.path)
			.then_with(|| match (&a.0.kind, &b.0.kind) {
				(FileKind::Image(_), FileKind::Audio(_)) => std::cmp::Ordering::Less,
				(FileKind::Audio(_), FileKind::Image(_)) => std::cmp::Ordering::Greater,
				_ => std::cmp::Ordering::Equal,
			})
			.then_with(|| get_priority(&a.0).cmp(&get_priority(&b.0)))
	});

	// deduplication step
	let mut prev_path = None;
	entries.extend(media.into_iter().filter_map(|(data, bytes)| {
		let current_path = &data.path;
		if prev_path.as_ref() != Some(current_path) {
			prev_path = Some(data.path.clone());
			Some((data, bytes))
		} else {
			None
		}
	}));

	#[cfg(feature = "debug")]
	{
		println!("finished file deduplication in {:.3?}.", timer.elapsed());
		timer = Instant::now();
	}

	// todo: skin editing step.
	// entries
	// 	.par_iter_mut()
	// 	.try_for_each(|(data, bytes)| -> Result<()> { Ok(()) })?;

	#[cfg(feature = "debug")]
	{
		println!("finished skin editing (no-op) in {:.3?}", timer.elapsed());
		timer = Instant::now();
	}

	// file (optimisation & conversion) step
	entries
		.par_iter_mut()
		.try_for_each(|(data, bytes)| -> Result<_> {
			match data.kind {
				FileKind::Image(kind) => match img::is_empty(bytes, kind) {
					// not sure if on error here is a good idea.
					Ok(true) | Err(_) => *bytes = EMPTY_PNG.to_vec(),
					Ok(false) => {
						if let Some(optimised) = img::optimise(bytes, kind, args.image_quality)? {
							*bytes = optimised;
						}
					}
				},
				FileKind::Audio(kind) => {
					if let Ok(vorbis) = aud::convert_to_vorbis(bytes, kind, args.audio_quality) {
						// if `vorbis` was None, then `bytes` was already a vorbis stream
						let vorbis = vorbis.as_ref().unwrap_or(bytes);
						*bytes = aud::optimise(&vorbis, AudioKind::Vorbis)?.unwrap_or(vorbis.to_owned());
						data.kind = FileKind::Audio(AudioKind::Vorbis);
					} else {
						bytes.clear();
						data.kind = FileKind::Audio(AudioKind::Wave);
					}
				}
				_ => (),
			}
			Ok(())
		})?;

	#[cfg(feature = "debug")]
	{
		println!("finished file optimisation in {:.3?}", timer.elapsed());
	}

	let entries_data = entries
		.into_par_iter()
		.map(|(data, bytes)| {
			let len = bytes.len();
			let path = data.path.with_extension(data.kind.extension());

			// arbitrary number that seems to produce the smallest size on average.
			const BZIP_THRESHOLD: usize = 700 * 1024;

			let options = SimpleFileOptions::default()
				.compression_method(if len < 2 {
					CompressionMethod::Stored
				} else if len < BZIP_THRESHOLD {
					CompressionMethod::Deflated
				} else {
					CompressionMethod::Bzip2
				})
				.compression_level(if len == 0 { None } else { Some(9) });
			Ok((bytes, path.into_string(), options))
		})
		.collect::<Result<Vec<_>>>()?;

	let mut output_file = File::create(&args.output_path)?;
	let mut output = ZipWriter::new(&mut output_file);

	entries_data
		.into_iter()
		.try_for_each(|(bytes, path, options)| -> Result<_> {
			output.start_file_from_path(path, options)?;
			output.write_all(&bytes)?;
			Ok(())
		})?;

	output.finish()?;

	let output_len = output_file.metadata()?.len() as f32;
	let input_len = input_file.metadata()?.len() as f32;
	let delta = output_len - input_len;

	println!(
		"finished in {:.3?} (total). size: {:.3}MiB => {:.3}MiB (change: {:.3}MiB)",
		begin.elapsed(),
		input_len / 1048576.0,
		output_len / 1048576.0,
		delta / 1048576.0
	);

	Ok(())
}

// after testing this was faster by like 100 microseconds. nice.
fn get_priority(file: &FileData) -> u8 {
	match &file.kind {
		FileKind::Image(ImageKind::Png) => 0,
		FileKind::Image(ImageKind::Jpeg) => 1,
		FileKind::Audio(AudioKind::Vorbis) => 0,
		FileKind::Audio(AudioKind::Wave) => 1,
		_ => 2,
	}
}
