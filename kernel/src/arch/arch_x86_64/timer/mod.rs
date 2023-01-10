use core::{arch::asm, fmt::Debug, fmt::Display};

use lazy_static::lazy_static;
use x86_64::instructions::port::Port;

use crate::{debug, warn};

const CMOS_FLAGS_REGISTER: u8 = 0x0A;
const CMOS_SECONDS_REGISTER: u8 = 0x00;
const CMOS_MINUTES_REGISTER: u8 = 0x02;
const CMOS_HOUR_REGISTER: u8 = 0x04;
const CMOS_NMI_DISABLE_REGISTER_FLAG: u8 = 0x80;

lazy_static! {
    pub(crate) static ref SPIN_TIMER: TscSpinTimer = TscSpinTimer::new();
}
pub(crate) struct TscSpinTimer {
    ticks_per_micro: u64,
}

impl TscSpinTimer {
    pub fn new() -> Self {
        Self::unsafe_new(calibrate())
    }

    fn unsafe_new(ticks_per_micro: u64) -> Self {
        Self {
            ticks_per_micro: ticks_per_micro,
        }
    }

    #[inline(always)]
    pub fn wait_one_micros(&self) -> u64 {
        let mut start = read_tsc();
        loop {
            unsafe {
                asm!("nop", "lfence", "mfence");
            }
            let next = read_tsc();
            if next < start {
                start = next;
                continue;
            }

            if next - start >= self.ticks_per_micro {
                return read_tsc() - start;
            }
        }
    }

    #[inline(always)]
    pub fn ticks(&self, ticks: u64) -> u64 {
        let mut total_ticks = 0;
        while total_ticks < ticks {
            total_ticks += self.wait_one_micros();
        }
        total_ticks
    }

    #[inline(always)]
    pub fn micros(&self, microseconds: u64) {
        let rtc = Cmos::new();
        let ticks_to_sleep = (microseconds * self.ticks_per_micro) + (self.ticks_per_micro * 20);
        rtc.get_time();
        let total_ticks = self.ticks(ticks_to_sleep);

        if total_ticks == 0 {
            if microseconds == 0 {
                return;
            }
            panic!(
                "Slept for 0 ticks, but wanted to sleep for {}",
                ticks_to_sleep
            );
        }

        let sleep_time = total_ticks / self.ticks_per_micro;
        if sleep_time < microseconds {
            panic!(
                "Reported sleep time {} was less than {} microseconds?",
                sleep_time, microseconds
            );
        }
    }

    #[inline(always)]
    pub fn millis(&self, milliseconds: u64) {
        let us = milliseconds * 1000;
        self.micros(us)
    }

    #[inline(always)]
    pub fn seconds(&self, seconds: u64) {
        let ms = seconds * 1000;
        self.millis(ms);
    }
}

pub(crate) fn init() {
    SPIN_TIMER.micros(0);
}

#[inline(always)]
fn sample(mut tick_delay: u64, accuracy: u8) -> u64 {
    loop {
        let rtc = Cmos::new();
        let mut start = rtc.get_time();
        loop {
            if tick_delay == 0 {
                tick_delay = 1;
            }

            let mut end = rtc.get_time();
            let timer = TscSpinTimer::unsafe_new(tick_delay);
            if start != end {
                start = rtc.get_time();
            }
            timer.seconds(1);
            end = rtc.get_time();
            if end == start {
                tick_delay += 41 * 2;
            } else {
                if end.minute == start.minute {
                    if end.second - start.second != 1 {
                        let seconds = end.second - start.second;
                        if seconds > 60 {
                            tick_delay /= 2;
                        } else {
                            tick_delay *= 60 - (seconds as u64);
                            tick_delay /= 60;
                        }
                    } else {
                        let expected_seconds = (accuracy as usize) + 2;
                        start = rtc.get_time();
                        // wait for clock to tick over
                        while start == end {
                            start = rtc.get_time();
                        }
                        let start_sec = ((start.hour as usize * 60) + (start.minute as usize * 60))
                            + start.second as usize;
                        let mut end_sec: usize;
                        for i in 0..expected_seconds {
                            timer.seconds(1);
                            end = rtc.get_time();
                            end_sec = ((end.hour as usize * 60) + (end.minute as usize * 60))
                                + end.second as usize;

                            let spread = match end_sec > start_sec {
                                true => end_sec - start_sec,
                                false => 0,
                            };
                            if spread != i + 1 {
                                let expected_seconds = i + 1;
                                let suggested = match spread {
                                    0 => (tick_delay / 2) * 3,
                                    _ => (tick_delay / spread as u64) * expected_seconds as u64,
                                };
                                debug!(
                                    "Tick delay {} slept for {} instead of {}, trying {}",
                                    tick_delay, spread, expected_seconds, suggested
                                );
                                tick_delay = suggested;
                                break;
                            }
                        }

                        let end_sec = ((end.hour as usize * 60) + (end.minute as usize * 60))
                            + end.second as usize;
                        let spread = end_sec - start_sec;
                        if spread == expected_seconds {
                            debug!(
                                "Start: {}, end: {}, spread:{}, expected: {}, ticks/us: {}",
                                start, end, spread, expected_seconds, tick_delay
                            );
                            debug!("Tick delay {} looks good!", tick_delay);
                            return tick_delay;
                        }
                    }
                }
            }
        }
    }
}
#[inline(always)]
fn calibrate() -> u64 {
    debug!("Calibrating early timer using RTC, this will take at least 20 seconds.");
    sample(4500, 10)
}

struct Cmos {
    cmos_register_port: Port<u8>,
    cmos_value_port: Port<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct CmosTime {
    second: u8,
    minute: u8,
    hour: u8,
}

impl Display for CmosTime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
}

impl Cmos {
    pub fn new() -> Self {
        Self {
            cmos_register_port: Port::new(0x70),
            cmos_value_port: Port::new(0x71),
        }
    }
    #[inline(always)]
    fn get_register(&self, register: u8) -> u8 {
        let state = unsafe { x86_64::instructions::interrupts::are_enabled() };

        if state {
            x86_64::instructions::interrupts::disable();
        }
        unsafe {
            let mut register_port = self.cmos_register_port.clone();
            let mut value_port = self.cmos_value_port.clone();
            register_port.write(register);
            value_port.read()
        }
    }
    #[inline(always)]
    pub fn busy(&self) -> bool {
        self.get_register(CMOS_FLAGS_REGISTER) & 0x80 != 0
    }
    #[inline(always)]
    fn bcd_to_u8(bcd: u8) -> u8 {
        (bcd & 0x0f) + (((bcd >> 4) & 0x0f) * 10)
    }
    #[inline(always)]
    pub fn get_time(&self) -> CmosTime {
        let second = Self::bcd_to_u8(self.get_register(CMOS_SECONDS_REGISTER));
        let minute = Self::bcd_to_u8(self.get_register(CMOS_MINUTES_REGISTER));
        let hour = Self::bcd_to_u8(self.get_register(CMOS_HOUR_REGISTER));

        CmosTime {
            second,
            minute,
            hour,
        }
    }
}
#[inline(always)]
fn read_tsc() -> u64 {
    let mut stamp: u64;
    let interrupt_state = x86_64::instructions::interrupts::are_enabled();
    if interrupt_state {
        x86_64::instructions::interrupts::disable();
    }
    unsafe {
        asm!(
            "push rdx",
            "mfence",
            "lfence",
            "rdtsc",
            "lfence",
            "shl rdx, 32",
            "or rax, rdx",
            "pop rdx",
            out("rax") stamp
        );
    }
    if interrupt_state {
        x86_64::instructions::interrupts::enable();
    }
    stamp
}
