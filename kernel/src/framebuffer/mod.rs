extern crate alloc;

use alloc::boxed::Box;
use core::cmp::min;

use bootloader_api::info::*;
use lazy_static::*;
use spin::Mutex;

use kernel_shared::memory::*;

#[derive(Debug, Clone, Copy)]
pub(crate) struct Point(pub usize, pub usize);

impl Point {
    pub(crate) fn x(self: &Self) -> usize {
        self.0
    }

    pub(crate) fn y(self: &Self) -> usize {
        self.1
    }
}

struct Pixel(Color, Point);

impl Pixel {
    fn color(self: &Self) -> Color {
        self.0
    }

    fn position(self: &Self) -> Point {
        self.1
    }
}

pub(crate) trait Drawable {
    fn draw(
        self: &Self,
        x: usize,
        y: usize,
        frame_buffer: &mut super::KernelFramebuffer,
        foreground: &Color,
        background: &Color,
    );
}

static mut FRAME_BUFFER_INTERNAL: KernelFramebuffer = KernelFramebuffer { frame_buffer: None };
pub struct FrameBufferWrapper {}
impl FrameBufferWrapper {
    pub(crate) fn get_framebuffer(self: &Self) -> Option<&mut KernelFramebuffer> {
        unsafe { Some(&mut FRAME_BUFFER_INTERNAL) }
    }

    pub(crate) fn set_framebuffer(self: &Self, frame_buffer: Option<&'static mut FrameBuffer>) {
        unsafe {
            FRAME_BUFFER_INTERNAL.frame_buffer = frame_buffer;
        }
    }
}

lazy_static! {
    pub static ref FRAME_BUFFER: Mutex<FrameBufferWrapper> = Mutex::new(FrameBufferWrapper {});
}

pub fn init_framebuffer(frame_buffer: Option<&'static mut FrameBuffer>) {
    FRAME_BUFFER.lock().set_framebuffer(frame_buffer);
}

pub(crate) struct KernelFramebuffer {
    pub(crate) frame_buffer: Option<&'static mut FrameBuffer>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

fn to_percent(num: u8) -> f32 {
    (num as f32) / 255.0 as f32
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Color {
        Color { r: r, g: g, b: b }
    }
    pub fn red() -> Color {
        Color { r: 255, g: 0, b: 0 }
    }
    pub fn green() -> Color {
        Color { r: 0, g: 255, b: 0 }
    }
    pub fn blue() -> Color {
        Color { r: 0, g: 0, b: 0 }
    }
    pub fn white() -> Color {
        Color {
            r: 255,
            g: 255,
            b: 255,
        }
    }
    pub fn black() -> Color {
        Color { r: 0, g: 0, b: 0 }
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
        let x_start = x % frame_buffer_info.width;
        (x_start + y_start) * frame_buffer_info.bytes_per_pixel
    }
    pub fn clear(self: &mut Self, color: &Color) {
        if self.frame_buffer.is_none() {
            return;
        }
        let info = self.frame_buffer.as_ref().unwrap().info();
        self.draw_rect(0, 0, info.width, info.height, color)
    }
    fn set_pixel_raw(self: &mut Self, x: usize, y: usize, color: &[u8]) {
        if self.frame_buffer.is_none() {
            return;
        }
        let fb = self.frame_buffer.as_deref_mut().unwrap();
        let fbi = fb.info();

        if x >= fbi.width || y >= fbi.height {
            return;
        }

        let fb_buffer = fb.buffer_mut();
        let start = Self::get_buffer_start_offset(x as usize, y as usize, fbi);

        let count = min(fbi.bytes_per_pixel, color.len());
        Self::copy_range(fb_buffer, color, 0, start, count);
    }
    #[inline]
    fn to_framebuffer_color(self: &mut Self, color: &Color) -> Option<Box<[u8]>> {
        if self.frame_buffer.is_none() {
            return None;
        }
        let fb = self.frame_buffer.as_deref_mut().unwrap();
        let fbi = fb.info();
        if !Self::is_supported(fbi.pixel_format) {
            return None;
        }
        let mut raw_color = [0 as u8; 3];
        color.to_framebuffer_color(fbi.pixel_format, &mut raw_color);
        return Some(Box::new(raw_color));
    }
    pub fn draw_rect(
        self: &mut Self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        color: &Color,
    ) {
        if self.frame_buffer.is_none() {
            return;
        }
        let fb_color = self.to_framebuffer_color(color);
        if fb_color.is_none() {
            return;
        }

        let raw_color = fb_color.unwrap();
        for y_offset in 0..height {
            for x_offset in 0..width {
                self.set_pixel_raw(x + x_offset, y + y_offset, &raw_color);
            }
        }
    }
    pub fn set_pixel(self: &mut Self, x: usize, y: usize, color: &Color) {
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
        self.set_pixel_raw(x, y, &buf);
    }

    pub(crate) fn shift_up(self: &mut Self, lines: usize) {
        if self.frame_buffer.is_none() {
            return;
        }
        let fb = self.frame_buffer.as_mut().unwrap();
        let info = fb.info();
        let mut_framebuffer = fb.buffer_mut();
        let start_y = lines;
        let start_offset = Self::get_buffer_start_offset(0, start_y, info);
        let end_offset = Self::get_buffer_start_offset(info.width - 1, info.height - 1, info);
        let copy_length = end_offset - start_offset;
        Self::copy_range_self(mut_framebuffer, start_offset, 0, copy_length);
        let clear_color = &Color::black();
        self.draw_rect(0, info.height - lines, info.width, lines, clear_color);
    }

    #[inline]
    fn copy_range(dst: &mut [u8], src: &[u8], src_offset: usize, dst_offset: usize, count: usize) {
        unsafe {
            memcpy(
                dst[dst_offset..].as_mut_ptr(),
                src[src_offset..].as_ptr(),
                count,
            );
        }
    }

    #[inline]
    fn copy_range_self(dst: &mut [u8], src_offset: usize, dst_offset: usize, count: usize) {
        unsafe {
            memcpy(
                dst[dst_offset..].as_mut_ptr(),
                dst[src_offset..].as_ptr(),
                count,
            );
        }
    }
}
