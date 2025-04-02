mod cli;
use cli::Args;

use clap::Parser;
use eyre::Result;
use image::{GenericImageView, Pixel};
use memmap2::Mmap;
use mtzip::ZipArchive as MtZipArchive;
use piz::ZipArchive;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{collections::HashMap, fs::File, io::Read, time::Instant};

use optiosk::*;

const EMPTY_PNG: &[u8; 67] = include_bytes!("1x1.png");

fn main() -> Result<()> {
	let mut args = Args::parse();

	if args.output_path.as_os_str() == "OUTPUT_PATH" {
		args.output_path = args.skin_path.with_extension("opt.osk");
	}

	rayon::ThreadPoolBuilder::new()
		.num_threads(args.threads)
		.build_global()?;

	let timer = Instant::now();

	let input_file = File::open(&args.skin_path)?;
	let input_len = input_file.metadata()?.len();
	let memmap = unsafe { Mmap::map(&input_file)? };
	let input = ZipArchive::new(&memmap)?;

	let (media, others): (Vec<_>, Vec<_>) = input
		.entries()
		.par_iter()
		.filter_map(|file| {
			|| -> Result<Option<_>> {
				let mut reader = input.read(file)?;
				let mut bytes = Vec::with_capacity(file.size);
				reader.read_to_end(&mut bytes)?;
				let mut file_data: FileData = file.path.as_std_path().into();

				match file_data.kind {
					FileKind::Image(mut img_kind) => {
						let guess = image::guess_format(&bytes)?;

						match (guess, img_kind) {
							(a @ image::ImageFormat::Jpeg, ImageKind::Png)
							| (a @ image::ImageFormat::Png, ImageKind::Jpeg) => {
								if a == image::ImageFormat::Jpeg {
									img_kind = ImageKind::Jpeg;
									file_data.path.set_extension("jpg");
								} else {
									img_kind = ImageKind::Png;
									file_data.path.set_extension("png");
								}
							}
							_ => (),
						}

						let the_img = image::load_from_memory_with_format(&bytes, guess)?;

						// empty file optimisation step. pointless for jpeg files because
						// they have no transparency
						if the_img.pixels().all(|p| p.2.to_luma_alpha()[1] == 0)
							&& img_kind == ImageKind::Png
						{
							bytes.clear();
							bytes.extend_from_slice(EMPTY_PNG);
							file_data.path.set_extension("png");
							file_data.kind = FileKind::Image(ImageKind::Png);
						} else {
							if let Some(optimised) = img::optimise(&bytes, img_kind)? {
								bytes = optimised;
							}
						}
					}
					FileKind::Audio(aud_kind) => {
						// if this fails, stream wasnt valid to begin with. let it die.
						if let Ok(ogg) = aud::convert_to_vorbis(&bytes, aud_kind) {
							// if this is nothing, we have a worse file
							if let Some(optimised) = aud::optimise(&ogg, aud_kind)? {
								bytes = optimised;
							} else {
								bytes = ogg;
							}
							file_data.path.set_extension("ogg");
							file_data.kind = FileKind::Audio(AudioKind::Vorbis);
						} else {
							// i see all the dead files use .wav, so im using it here
							bytes.clear();
							file_data.path.set_extension("wav");
							file_data.kind = FileKind::Audio(AudioKind::Wave);
						}
					}
					FileKind::Config(_) => {}
					FileKind::Unknown => return Ok(None),
				}
				Ok(Some((file_data, bytes)))
			}()
			.transpose()
		})
		// todo: this is gross
		.collect::<Result<Vec<_>>>()?
		.into_iter()
		.partition(|(file, _)| matches!(file.kind, FileKind::Image(_) | FileKind::Audio(_)));

	// i dont like this whole grouping section very much either...
	let mut groups = HashMap::new();
	for (file, data) in media {
		let stem = file.path.with_extension("");
		let stem_str = stem.to_str().unwrap().to_string();
		let category = match file.kind {
			FileKind::Image(_) => 0,
			FileKind::Audio(_) => 1,
			_ => unreachable!(),
		};
		groups
			.entry((stem_str, category))
			.or_insert_with(Vec::new)
			.push((file, data));
	}

	let selected_media: Vec<_> = groups
		.into_iter()
		.filter_map(|((_, category), group)| {
			group.into_iter().min_by(|a, b| {
				let a_prio = get_priority(&a.0, category);
				let b_prio = get_priority(&b.0, category);
				a_prio.cmp(&b_prio).then_with(|| a.1.len().cmp(&b.1.len()))
			})
		})
		.collect();

	let mut output = MtZipArchive::new();

	selected_media
		.into_iter()
		.chain(others)
		.for_each(|(file, data)| {
			output
				.add_file_from_memory(data, file.path.to_str().unwrap().to_string())
				.done();
		});

	let mut output_file = File::create(&args.output_path)?;
	output.write_with_rayon(&mut output_file)?;
	let output_len = output_file.metadata()?.len();

	println!(
		"finished in {:.2?}. size: {:.2}MiB => {:.2}MiB (change: {:.2}MiB)",
		timer.elapsed(),
		input_len as f32 / 1048576.0,
		output_len as f32 / 1048576.0,
		(output_len as f32 - input_len as f32) / 1048576.0,
	);

	Ok(())
}

fn get_priority(file: &FileData, category: u8) -> u8 {
	match category {
		0 => match file.kind {
			FileKind::Image(ImageKind::Png) => 0,
			FileKind::Image(ImageKind::Jpeg) => 1,
			_ => 2,
		},
		1 => match file.kind {
			FileKind::Audio(AudioKind::Vorbis) => 0,
			FileKind::Audio(AudioKind::Wave) => 1,
			_ => 2,
		},
		_ => 2,
	}
}
