use core::cmp::min;

use bootloader_api::{info::*, *};

mod console;

#[derive(Debug, Clone, Copy)]
struct Point(usize, usize);

impl Point {
    fn x(self: &Self) -> usize {
        self.0
    }

    fn y(self: &Self) -> usize {
        self.1
    }
}

struct Pixel(Color, Point);

impl Pixel{
    fn color(self: &Self) -> Color {
        self.0
    }

    fn position(self: &Self) -> Point {
        self.1
    }
}

trait Drawable {
    fn draw(self: &Self, x: usize, y:usize, frame_buffer: &mut KernelFramebuffer, foreground: Color, background: Color);
}

struct KernelFramebuffer {
    frame_buffer: Option<&'static mut FrameBuffer>,
}

#[derive(Debug, Clone, Copy)]
struct Color {
    r: u8,
    g: u8,
    b: u8,
}

fn to_percent(num: u8) -> f32 {
    (num as f32) / 255.0 as f32
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Color {
        Color { r: r, g: g, b: b }
    }
    pub fn to_framebuffer_color(self: &Self, pixel_format: PixelFormat, buffer: &mut [u8]) {
        if pixel_format == PixelFormat::Bgr {
            buffer[0] = self.b;
            buffer[1] = self.g;
            buffer[2] = self.r;
        } else if pixel_format == PixelFormat::Rgb {
            buffer[0] = self.r;
            buffer[1] = self.g;
            buffer[2] = self.b;
        } else if pixel_format == PixelFormat::U8 {
            let r_percent = to_percent(self.r) * 0.3;
            let g_percent = to_percent(self.g) * 0.59;
            let b_percent = to_percent(self.b) * 0.11;
            let mut greyscale_percent = r_percent + g_percent + b_percent;
            if greyscale_percent > 1.0 {
                greyscale_percent = 1.0;
            } else if greyscale_percent < 0.0 {
                greyscale_percent = 0.0;
            }
            let final_greyscale_value = (255.0 * greyscale_percent) as u8;
            buffer[0] = final_greyscale_value;
        } else {
            buffer.fill(0);
        }
    }
}

impl KernelFramebuffer {
    fn is_supported(pixel_format: PixelFormat) -> bool {
        match pixel_format {
            PixelFormat::Rgb => true,
            PixelFormat::Bgr => true,
            PixelFormat::U8 => true,
            _ => false,
        }
    }

    fn get_buffer_start_offset(x: usize, y: usize, frame_buffer_info: FrameBufferInfo) -> usize {
        let y_start = (y % frame_buffer_info.height) * frame_buffer_info.stride;
        let x_start = (x % frame_buffer_info.width) * frame_buffer_info.bytes_per_pixel;
        x_start + y_start
    }
    fn set_pixel(self: &mut Self, x: usize, y: usize, color: Color) {
        if self.frame_buffer.is_none() {
            return;
        }
        let fb = self.frame_buffer.as_deref_mut().unwrap();
        let fbi = fb.info();
        if Self::is_supported(fbi.pixel_format) == false {
            return;
        }
        let mut buf = [0 as u8; 3];

        color.to_framebuffer_color(fbi.pixel_format, &mut buf);
        let fb_buffer = fb.buffer_mut();
        let start = Self::get_buffer_start_offset(x as usize, y as usize, fbi);
        let loop_count = min(fbi.bytes_per_pixel, buf.len());
        for i in 0..loop_count {
            let offset = start + i;
            fb_buffer[offset] = buf[i];
        }
    }
}
