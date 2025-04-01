use anyhow::Result;
use clap::Parser;
use mtzip::ZipArchive as MtZipArchive;
use piz::ZipArchive;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
	cmp::Ordering,
	collections::HashMap,
	fs::{File, read},
};

mod cli;
use cli::*;

use optiosk::*;

fn main() -> Result<()> {
	let args = Args::parse();

	rayon::ThreadPoolBuilder::new()
		.num_threads(args.threads)
		.build_global()?;

	let begin = std::time::Instant::now();
	let input_bytes = read(&args.skin_path)?;
	let mut output_file = File::create(&args.output_path)?;
	// let mut output = ZipWriter::new(File::create(&args.output_path)?);
	let mut output = MtZipArchive::new();

	let input_zip = ZipArchive::new(&input_bytes)?;
	let input_len = input_bytes.len() as u64;

	let files = input_zip
		.entries()
		.par_iter()
		.filter_map(|metadata| {
			let kind = FileKind::from(metadata.path.as_std_path());
			(!matches!(kind, FileKind::Unknown)).then_some((metadata, kind))
		})
		.map(|(metadata, kind)| {
			let mut bytes = Vec::with_capacity(metadata.size);
			input_zip.read(&metadata)?.read_to_end(&mut bytes)?;
			let mut path = metadata.path.to_path_buf();

			match &kind {
				FileKind::Image(ImageKind::Png) => {
					if let Ok(optimised) = optimise_png(&bytes) {
						bytes = optimised;
					}
				}
				FileKind::Image(img_kind) if should_convert(&args, FileType::Image) => {
					if let Ok(png) = convert_to_png(&bytes, img_kind) {
						if let Ok(optimised) = optimise_png(&png) {
							bytes = optimised;
							path.set_extension("png");
						}
					}
				}
				FileKind::Audio(aud_kind) if should_convert(&args, FileType::Audio) => {
					if let Ok(Some(ogg)) = convert_to_vorbis(&bytes, aud_kind) {
						bytes = optimise_vorbis(&ogg).unwrap_or(ogg);
						path.set_extension("ogg");
					}
				}
				_ => {}
			}

			Ok(SkinFile::new(kind, path.into(), bytes))
		})
		.collect::<Result<Vec<_>>>()?;

	let (image_audio, others): (Vec<_>, Vec<_>) = files.into_iter().partition(|f| f.is_media());
	let mut groups = HashMap::new();

	for file in image_audio {
		let stem = file.path.with_extension("");
		let category = file.media_category();
		groups
			.entry((stem, category))
			.or_insert_with(Vec::new)
			.push(file);
	}

	let mut selected = Vec::new();
	for ((_, category), mut group) in groups {
		group.sort_by(|a, b| match category {
			0 => a
				.png_priority()
				.cmp(&b.png_priority())
				.then(a.bytes.len().cmp(&b.bytes.len())),
			1 => a
				.audio_priority()
				.cmp(&b.audio_priority())
				.then(a.bytes.len().cmp(&b.bytes.len())),
			_ => Ordering::Equal,
		});
		if let Some(best) = group.into_iter().next() {
			selected.push(best);
		}
	}

	for file in selected.into_iter().chain(others) {
		output
			.add_file_from_memory(file.bytes, file.path.to_str().unwrap().to_string())
			.done();
	}

	output.write_with_rayon(&mut output_file)?;

	let output_size = output_file.metadata()?.len();

	println!(
		"finished in {:.2?}. size: {:.2}MiB → {:.2}MiB",
		begin.elapsed(),
		input_len as f32 / 1048576.0,
		output_size as f32 / 1048576.0
	);

	Ok(())
}

#[inline(always)]
fn should_convert(args: &Args, file_type: FileType) -> bool {
	!args.preserve_filetypes.contains(&file_type)
		&& !args.preserve_filetypes.contains(&FileType::All)
}
