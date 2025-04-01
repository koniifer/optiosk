use anyhow::Result;
use image::{ImageEncoder, codecs::png::PngEncoder};
use oxipng::{Options, optimize_from_memory};

#[derive(Debug)]
pub enum ImageKind {
	Png,
	Jpeg,
}

pub fn optimise_png(bytes: &[u8]) -> Result<Vec<u8>> {
	Ok(optimize_from_memory(
		bytes,
		&Options {
			fix_errors: true,
			optimize_alpha: true,
			..Default::default()
		},
	)?)
}

pub fn convert_to_png(bytes: &[u8], kind: &ImageKind) -> Result<Vec<u8>> {
	match kind {
		ImageKind::Jpeg => {
			let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg)?;
			let mut buffer = Vec::new();
			PngEncoder::new_with_quality(
				&mut buffer,
				image::codecs::png::CompressionType::Best,
				image::codecs::png::FilterType::Adaptive,
			)
			.write_image(
				img.as_bytes(),
				img.width(),
				img.height(),
				img.color().into(),
			)?;
			Ok(buffer)
		}
		ImageKind::Png => Ok(bytes.to_vec()),
	}
}
