use core::fmt;

use crate::framebuffer::*;
use lazy_static::*;
use spin::Mutex;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub(crate) struct Glyph {
    header: FontHeader,
    bytes: [u8; 32],
}

pub(crate) struct Console {
    font: Font,
}

static mut CONSOLE_X_POSITION: usize = 0;

lazy_static! {
    static ref CONSOLE: Mutex<Console> = Mutex::new(Console { font: Font::new() });
}

pub(crate) fn _print(args: fmt::Arguments) {
    for c in args.as_str().unwrap().chars() {
        match c {
            '\n' => CONSOLE.lock().new_line(),
            _ => CONSOLE.lock().put_char(c)
        };
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}


impl Console {
    pub fn new_line(self: &Self) {
        let locked = FRAME_BUFFER.lock();
        let frame_buffer = locked.get_framebuffer().unwrap();
        self.new_line_internal(frame_buffer);
    }
    fn new_line_internal(self: &Self, frame_buffer: &mut KernelFramebuffer) {
        let glyph = self.font.glyph(b' ');
        frame_buffer.shift_up(glyph.height());
        unsafe { CONSOLE_X_POSITION = 0 };
    }
    pub fn put_char(self: &Self, c: char) {
        let glyph = self.font.glyph(c as u8);
        let mut x_offset: usize = unsafe { CONSOLE_X_POSITION };
        let locked = FRAME_BUFFER.lock();
        let frame_buffer = locked.get_framebuffer().unwrap();
        let platform_framebuffer = frame_buffer.frame_buffer.as_ref().unwrap();
        
        let info = platform_framebuffer.info();
        let y_offset = info.height - glyph.header.charsize as usize;

        if x_offset >= (info.width - glyph.width()) {
            self.new_line_internal(frame_buffer);
            x_offset = 0;
        }
        {
            glyph.draw(
                x_offset,
                y_offset,
                frame_buffer,
                &Color::green(),
                &Color::black(),
            );
            x_offset += 8;
        }
        unsafe { CONSOLE_X_POSITION = x_offset };
    }
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
        let index = y;
        let mask = 0b10000000 as u8 >> x;
        let masked = self.bytes[index] & mask;
        masked != 0
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

pub(crate) struct Font {
    header: FontHeader,
    glyphs: [Glyph; 256],
}

impl Font {
    pub(crate) fn new() -> Font {
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

    pub fn glyph(self: &Self, c: u8) -> Glyph {
        self.glyphs[c as usize]
    }
}

impl Drawable for Glyph {
    fn draw(
        self: &Self,
        x_offset: usize,
        y_offset: usize,
        frame_buffer: &mut super::KernelFramebuffer,
        foreground: &Color,
        background: &Color,
    ) {
        let width = self.width();
        let height = self.height();
        for y in 0..height {
            for x in 0..width {
                let mut color = background;
                if self.pixel(x, y) {
                    color = foreground;
                }
                frame_buffer.set_pixel(x_offset + x, y_offset + y, color);
            }
        }
    }
}
