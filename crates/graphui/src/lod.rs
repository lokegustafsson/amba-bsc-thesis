use arrayvec::ArrayVec;

#[derive(Clone, Debug, Default)]
pub struct LodText {
	levels: ArrayVec<(String, u32, u32), 4>,
}

impl LodText {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn coarser(&mut self, text: String) {
		self.levels.push(Self::level(text));
	}

	pub(crate) fn get_given_available_square(&self, width: u32, height: u32) -> (&str, u32, u32) {
		for &(ref content, w, h) in &self.levels {
			if w <= width && h <= height {
				return (content, w, h);
			}
		}
		("", 0, 0)
	}

	pub(crate) fn get_full(&self) -> &str {
		self.levels.first().map_or("", |(content, ..)| content)
	}

	fn level(text: String) -> (String, u32, u32) {
		const MAX_WIDTH: usize = 80;
		let mut width = 0;
		let mut height = 0;
		for line in text.lines() {
			let line_len_codepoints = line.chars().count();
			if line_len_codepoints <= MAX_WIDTH {
				width = width.max(line_len_codepoints);
				height += 1;
			} else {
				width = MAX_WIDTH;
				height += (line_len_codepoints + MAX_WIDTH - 1) / MAX_WIDTH;
			}
		}
		(
			text,
			u32::try_from(width).unwrap(),
			u32::try_from(height).unwrap(),
		)
	}
}
