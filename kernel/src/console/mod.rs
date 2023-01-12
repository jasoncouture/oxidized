use alloc::string::ToString;
use core::fmt;

use lazy_static::*;
use spin::Mutex;

use crate::framebuffer::*;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub(crate) struct Glyph {
    height: usize,
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
    let locked_console = CONSOLE.lock();
    for c in args.to_string().chars() {
        if !c.is_ascii() {
            continue;
        }
        match c {
            '\n' => locked_console.new_line(),
            _ => locked_console.put_char(c),
        };
    }
}

#[macro_export]
macro_rules! console_print {
    ($($arg:tt)*) => ($crate::console::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! console_println {
    () => ($crate::console_print!("\n"));
    ($($arg:tt)*) => ($crate::console_print!("{}\n", format_args!($($arg)*)));
}

impl Console {
    pub fn new_line(self: &Self) {
        let locked = FRAME_BUFFER.lock();
        let frame_buffer = locked.get_framebuffer().unwrap();
        let glyph = self.font.glyph(b' ');
        self.new_line_internal(frame_buffer, &glyph);
    }
    fn new_line_internal(self: &Self, frame_buffer: &mut KernelFramebuffer, glyph: &Glyph) {
        frame_buffer.shift_up(glyph.height());
        unsafe { CONSOLE_X_POSITION = 0 };
    }
    pub fn put_char(self: &Self, c: char) {
        let glyph = self.font.glyph(c as u8);
        let mut x_offset: usize = unsafe { CONSOLE_X_POSITION };
        let locked = FRAME_BUFFER.lock();
        let frame_buffer_option = locked.get_framebuffer();
        if frame_buffer_option.is_none() {
            return;
        }
        let frame_buffer = frame_buffer_option.unwrap();
        let platform_framebuffer_option = frame_buffer.frame_buffer.as_ref();
        if platform_framebuffer_option.is_none() {
            return;
        }
        let platform_framebuffer = platform_framebuffer_option.unwrap();

        let info = platform_framebuffer.info();
        let y_offset = info.height - glyph.height();

        if x_offset >= (info.width - glyph.width()) {
            self.new_line_internal(frame_buffer, &glyph);
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
    #[inline]
    fn width(self: &Self) -> usize {
        8
    }
    #[inline]
    fn height(self: &Self) -> usize {
        self.height
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
    glyphs: [Glyph; 256],
}

impl Font {
    pub(crate) fn new() -> Font {
        let bytes = include_bytes!("console_font.psf");
        assert!(bytes[0] == 0x36);
        assert!(bytes[1] == 0x04);
        let header = FontHeader::create(bytes);
        let mut glyphs = [Glyph {
            height: header.charsize as usize,
            bytes: [0 as u8; 32],
        }; 256];
        for i in 0..256 as usize {
            let mut glyph_bytes = [0 as u8; 32];
            let base_index = 4 + (i * header.charsize as usize);
            for x in 0..header.charsize as usize {
                glyph_bytes[x] = bytes[base_index + x as usize];
            }
            glyphs[i] = Glyph {
                height: header.charsize as usize,
                bytes: glyph_bytes,
            };
        }
        Font { glyphs: glyphs }
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
