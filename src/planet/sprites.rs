use std::io::Cursor;

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, Frame, RgbaImage};

use super::types::PlanetKind;

pub struct PlanetAnimation {
    pub frames: Vec<RgbaImage>,
}

impl PlanetAnimation {
    pub fn load(kind: PlanetKind) -> Self {
        let bytes: &[u8] = match kind {
            PlanetKind::Earth => include_bytes!("../../assets/planets/earth.gif"),
            PlanetKind::Mars => include_bytes!("../../assets/planets/lava.gif"),
            PlanetKind::Venus => include_bytes!("../../assets/planets/rocky.gif"),
            PlanetKind::Neptune => include_bytes!("../../assets/planets/ice.gif"),
            PlanetKind::Jupiter => include_bytes!("../../assets/planets/gas.gif"),
            PlanetKind::Pluto => include_bytes!("../../assets/planets/water.gif"),
        };

        let cursor = Cursor::new(bytes);
        let decoder = GifDecoder::new(cursor).expect("failed to decode planet GIF");
        let frames: Vec<RgbaImage> = decoder
            .into_frames()
            .filter_map(|f: Result<Frame, _>| f.ok())
            .map(|f| f.into_buffer())
            .collect();

        assert!(!frames.is_empty(), "planet GIF has no frames");

        PlanetAnimation { frames }
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn frame_at(&self, tick: usize) -> &RgbaImage {
        &self.frames[tick % self.frames.len()]
    }
}
