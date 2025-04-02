use anyhow::Result;
use clap::Parser;
use image::{ExtendedColorType, GenericImageView, LumaA, Pixel};
use mtzip::ZipArchive as MtZipArchive;
use piz::ZipArchive;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{
	cmp::Ordering,
	collections::HashMap,
	fs::{File, read},
	io::Cursor,
};

mod cli;
use cli::*;
use optiosk::*;

fn main() -> Result<()> {
	let args = Args::parse();
	rayon::ThreadPoolBuilder::new()
		.num_threads(args.threads)
		.build_global()?;

	// pre-generate this empty image
	let empty_png = {
		let mut buf = Vec::with_capacity(67);
		let img = image::ImageBuffer::<LumaA<u8>, Vec<u8>>::new(1, 1);
		image::write_buffer_with_format(
			&mut Cursor::new(&mut buf),
			&img,
			1,
			1,
			ExtendedColorType::La8,
			image::ImageFormat::Png,
		)?;
		optimise_png(&buf)?
	};

	let timer = std::time::Instant::now();
	let input_bytes = read(&args.skin_path)?;
	let mut output_file = File::create(&args.output_path)?;
	let mut output = MtZipArchive::new();

	let input_zip = ZipArchive::new(&input_bytes)?;
	let input_size = input_bytes.len() as u64;

	let files = input_zip
		.entries()
		.par_iter()
		.filter_map(|metadata| {
			let kind = FileKind::from(metadata.path.as_std_path());

			if matches!(kind, FileKind::Unknown) {
				return None;
			}

			let mut bytes: Vec<u8> = match input_zip.read(&metadata).and_then(|mut r| {
				let mut buf = Vec::with_capacity(metadata.size);
				r.read_to_end(&mut buf).map(|_| buf).map_err(Into::into)
			}) {
				Ok(b) => b,
				Err(_) => return None,
			};

			let mut path = metadata.path.to_path_buf();

			match &kind {
				FileKind::Image(ImageKind::Png) => {
					if let Ok(optimized) = optimise_png(&bytes) {
						bytes = optimized;
					}
					if is_empty_image(&bytes) {
						bytes = empty_png.clone();
					}
				}
				FileKind::Image(img_kind) => {
					if let Ok(png) = convert_to_png(&bytes, img_kind) {
						if should_convert(&args, FileType::Image) {
							bytes = optimise_png(&png).unwrap_or(png);
							path.set_extension("png");
						}
					} else if is_empty_image(&bytes) {
						bytes = empty_png.clone();
						path.set_extension("png");
					}
				}
				FileKind::Audio(aud_kind) => {
					if let Ok(Some(ogg)) = convert_to_vorbis(&bytes, aud_kind) {
						if should_convert(&args, FileType::Audio) {
							bytes = optimise_vorbis(&ogg).unwrap_or(ogg);
							path.set_extension("ogg");
						}
					} else {
						bytes.clear();
					}
				}
				_ => {}
			}

			Some(SkinFile::new(kind, path.into(), bytes))
		})
		.collect::<Vec<_>>();

	let (media, others): (Vec<_>, Vec<_>) = files.into_iter().partition(SkinFile::is_media);
	let mut groups = HashMap::new();

	for file in media {
		let stem = file.path.with_extension("");
		groups
			.entry((stem, file.media_category()))
			.or_insert_with(Vec::new)
			.push(file);
	}

	groups
		.into_iter()
		.filter_map(|((_, category), mut group)| {
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
			group.into_iter().next()
		})
		.chain(others)
		.for_each(|file| {
			output
				.add_file_from_memory(file.bytes, file.path.to_str().unwrap().to_string())
				.done();
		});

	output.write_with_rayon(&mut output_file)?;
	let output_size = output_file.metadata()?.len();

	println!(
		"finished in {:.2?}. size: {:.2}MiB → {:.2}MiB (change: {:.2}MiB)",
		timer.elapsed(),
		input_size as f32 / 1048576.0,
		output_size as f32 / 1048576.0,
		(input_size - output_size) as f32 / 1048576.0,
	);

	Ok(())
}

#[inline]
fn should_convert(args: &Args, file_type: FileType) -> bool {
	!args.preserve_filetypes.contains(&file_type)
		&& !args.preserve_filetypes.contains(&FileType::All)
}

#[inline]
fn is_empty_image(bytes: &[u8]) -> bool {
	image::load_from_memory(bytes).ok().map_or(false, |img| {
		img.dimensions() == (1, 1) || img.pixels().all(|p| p.2.channels()[3] == 0)
	})
}
