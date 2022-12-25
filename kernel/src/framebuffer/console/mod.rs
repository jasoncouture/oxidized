use super::{Color, Drawable};

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct Glyph {
    header: FontHeader,
    bytes: [u8; 32],
}

impl Glyph {
    fn width(self: &Self) -> usize {
        8
    }

    fn height(self: &Self) -> usize {
        self.header.charsize as usize
    }

    fn pixel(self: &Self, mut x: usize, mut y: usize) -> bool {
        x = x % self.width();
        y = y % self.height();
        let index = y * self.width();
        let mask = 0b10000000 as u8 >> x;
        (self.bytes[index] & mask) > 0
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
#[derive(Debug)]
struct FontHeader {
    magic: [u8; 2],
    mode: u8,
    charsize: u8,
}

impl FontHeader {
    fn create(font_bytes: &[u8]) -> FontHeader {
        assert!(font_bytes.len() >= 4);
        let font_bytes = include_bytes!("console_font.psf");
        let magic: [u8; 2] = [font_bytes[0], font_bytes[1]];
        let mode = font_bytes[2];
        let char_size = font_bytes[3];

        FontHeader {
            magic: magic,
            mode: mode,
            charsize: char_size,
        }
    }
}

struct Font {
    header: FontHeader,
    glyphs: [Glyph; 256],
}

impl Font {
    fn new() -> Font {
        let bytes = include_bytes!("console_font.psf");
        assert!(bytes[0] == 0x36);
        assert!(bytes[1] == 0x04);
        let header = FontHeader::create(bytes);
        let mut glyphs = [Glyph {
            header: header,
            bytes: [0 as u8; 32],
        }; 256];
        for i in 0..256 as usize {
            let mut glyph_bytes = [0 as u8; 32];
            let base_index = 4 + (i * header.charsize as usize);
            for x in 0..header.charsize as usize {
                glyph_bytes[x] = bytes[base_index + x as usize];
            }
            glyphs[i] = Glyph {
                header,
                bytes: glyph_bytes,
            };
        }
        Font {
            header: header,
            glyphs: glyphs,
        }
    }

    fn glyph(self: &Self, char: u8) -> Glyph {
        self.glyphs[char as usize]
    }
}

impl Drawable for Glyph {
    fn draw(
        self: &Self,
        x_offset: usize,
        y_offset: usize,
        frame_buffer: &mut super::KernelFramebuffer,
        foreground: Color,
        background: Color,
    ) {
        let width = self.width();
        let height = self.height();
        for y in 0..height {
            for x in 0..width {
                if self.pixel(x, y) {
                    frame_buffer.set_pixel(x_offset + x, y_offset + y, foreground);
                } else {
                    frame_buffer.set_pixel(x_offset + x, y_offset + y, background);
                }
            }
        }
    }
}
