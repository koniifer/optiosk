use eyre::Result;
use std::panic::catch_unwind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImageKind {
	Png,
	Jpeg,
}

pub fn optimise(bytes: &[u8], kind: ImageKind, quality: u8) -> Result<Option<Vec<u8>>> {
	use ImageKind::*;
	match kind {
		Png => {
			let optimised = oxipng::optimize_from_memory(
				bytes,
				&oxipng::Options {
					fix_errors: true,
					optimize_alpha: true,
					strip: oxipng::StripChunks::Safe,
					scale_16: if quality != 100 { true } else { false },
					deflate: oxipng::Deflaters::Libdeflater { compression: 12 },
					fast_evaluation: true,
					..oxipng::Options::from_preset(3)
				},
			)?;
			if optimised.is_empty() || optimised.len() >= bytes.len() {
				Ok(None)
			} else {
				Ok(Some(optimised))
			}
		}
		Jpeg => catch_unwind(|| {
			let decomp = mozjpeg::Decompress::new_mem(bytes)?;
			let colour_space = decomp.color_space();

			let mut comp = mozjpeg::Compress::new(colour_space);

			comp.set_size(decomp.width(), decomp.height());
			comp.set_quality(quality as f32);
			comp.set_optimize_coding(true);
			comp.set_optimize_scans(true);
			comp.set_smoothing_factor(0);
			comp.set_progressive_mode();

			let mut optimised = Vec::new();

			let mut started = comp.start_compress(&mut optimised)?;

			let pixels = decomp.to_colorspace(colour_space)?.read_scanlines()?;
			started.write_scanlines(&pixels)?;

			started.finish()?;

			if optimised.is_empty() || optimised.len() >= bytes.len() {
				Ok(None)
			} else {
				Ok(Some(optimised))
			}
		})
		.unwrap_or_else(|_| eyre::bail!("jpeg decode/encode panic")),
	}
}
