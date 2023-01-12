use core::{arch::asm, fmt::Debug, fmt::Display};

use bitvec::macros::internal::funty::Numeric;
use lazy_static::lazy_static;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

use crate::{arch::get_timer_ticks, debug, warn};

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
        if let Some(fast_cal)= fast_calibrate_with_pit() {
            debug!("Fast calibration sucessful, TSC frequency: {}", fast_cal);
            // KHZ, so convert to MHZ, since we want microsecond timer resolution (Approximately anyway)
            Self::unsafe_new(fast_cal / 1000);
        }
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

const NMI_AND_STATUS_CONTROL_REGISTER: u16 = 0x61;
const PIT_CHANNEL_2: u16 = 0x42;
const PIT_COMMAND_REGISTER: u16 = 0x43;
const SPEAKER_GATE: u8 = 1 << 1;
const SPEAKER_STATE: u8 = 1 << 0;
// Inspired by, and largely ported from: https://github.com/torvalds/linux/blob/631aa744423173bf921191ba695bbc7c1aabd9e0/arch/x86/kernel/tsc.c#L518-L616
// Not yet complete.
/*
    #define CAL_MS		10
    #define CAL_LATCH	(PIT_TICK_RATE / (1000 / CAL_MS))
    #define CAL_PIT_LOOPS	1000

    #define CAL2_MS		50
    #define CAL2_LATCH	(PIT_TICK_RATE / (1000 / CAL2_MS))
    #define CAL2_PIT_LOOPS	5000
    #define MAX_QUICK_PIT_MS 50
    #define MAX_QUICK_PIT_ITERATIONS (MAX_QUICK_PIT_MS * PIT_TICK_RATE / 1000 / 256)
    /* The clock frequency of the i8253/i8254 PIT */
    #define PIT_TICK_RATE 1193182ul
*/

const PIT_TICK_RATE: u64 = 1193182;
const MAX_QUICK_PIT_MS: u64 = 50;
const MAX_QUICK_PIT_ITERATIONS: u64 = MAX_QUICK_PIT_MS * PIT_TICK_RATE / 1000 / 256;

fn write_bytes(port: &mut Port<u8>, data: &[u8]) {
    for byte in data {
        unsafe {
            port.write(*byte);
        }
    }
}

fn wait_for_msb(port: &mut Port<u8>, expected: u8) -> (bool, u64, u64) {
    let mut count = 0;
    let mut tsc = 0u64;
    let mut previous_tsc = 0u64;
    for i in 0..5000 {
        count = i;
        unsafe {
            port.read();

            if port.read() != expected {
                break;
            }
        }
        previous_tsc = tsc;
        tsc = read_tsc();
    }

    let delta = previous_tsc.wrapping_sub(tsc);
    return (count > 5, tsc, delta);
}


fn fast_calibrate_with_pit() -> Option<u64> {
    let mut control_port = Port::new(NMI_AND_STATUS_CONTROL_REGISTER);
    let mut pit_channel_2_port = Port::new(PIT_CHANNEL_2);
    let mut pit_command_port = PortWriteOnly::new(PIT_COMMAND_REGISTER);

    unsafe {
        /* Set the Gate high, disable speaker */
        let mut control_port_value: u8 = control_port.read();
        control_port_value &= SPEAKER_GATE ^ 0xFF;
        control_port_value |= SPEAKER_STATE;
        control_port.write(control_port_value);

        /*
         * Setup CTC channel 2* for mode 0, (interrupt on terminal
         * count mode), binary count. Set the latch register to 50ms
         * (LSB then MSB) to begin countdown.
         */
        pit_command_port.write(0xb0u8);
        let le_bytes = 0xffffu16.to_le_bytes();
        // Start at 0xffff
        write_bytes(&mut pit_channel_2_port, &le_bytes);

        // Delay by roughly 1ms, by reading the 16 bit value from channel 2
        pit_channel_2_port.read();
        pit_channel_2_port.read();
        let (result, tsc, delta1) = wait_for_msb(&mut pit_channel_2_port, 0xff);
        if result {
            for i in 0..MAX_QUICK_PIT_ITERATIONS as u8 {
                let (result, looptsc, delta2) =
                    wait_for_msb(&mut pit_channel_2_port, 0xffu8.wrapping_sub(1 + i));
                if !result {
                    debug!("PIT fast calibration failed: unable to compute accurate tsc per PIT tick");
                    return None;
                }

                let delta = looptsc.wrapping_sub(tsc);

                if i == 0
                    && (delta1.wrapping_add(delta2))
                        > (delta.wrapping_mul(MAX_QUICK_PIT_ITERATIONS) >> 11)
                {
                    debug!("PIT fast calibration failed: Error is too high");
                    return None;
                }

                if delta1.wrapping_add(delta2) >= (delta >> 11) {
                    continue;
                }
                pit_channel_2_port.read();
                if pit_channel_2_port.read() != (0xffu8.wrapping_sub(1 + i).wrapping_sub(1)) {
                    debug!("PIT fast calibration failed: PIT counter changed when we didn't expect it to.");
                    return None;
                }
                /*
                 * Ok, if we get here, then we've seen the
                 * MSB of the PIT decrement 'i' times, and the
                 * error has shrunk to less than 500 ppm.
                 *
                 * As a result, we can depend on there not being
                 * any odd delays anywhere, and the TSC reads are
                 * reliable (within the error).
                 *
                 * kHz = ticks / time-in-seconds / 1000;
                 * kHz = (t2 - t1) / (I * 256 / PIT_TICK_RATE) / 1000
                 * kHz = ((t2 - t1) * PIT_TICK_RATE) / (I * 256 * 1000)
                 */
                let delta = delta * PIT_TICK_RATE;
                let delta = delta / ((i as u64 + 1) * 256 * 1000);
                debug!("Calculated CPU khz as {}", delta);
                return Some(delta);
            }
        } else {
            debug!("Unable to get a stable PIT/TSC state for calibration.");
        }
    }
    None
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
