use alloc::vec::Vec;


use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::tables::load_tss;
use x86_64::registers::segmentation::{Segment};
use x86_64::structures::gdt::{
    Descriptor, DescriptorFlags, GlobalDescriptorTable, SegmentSelector,
};

use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

use crate::memory::allocator::PAGE_SIZE;

use super::cpu::cpu_apic_id;

pub const INTERRUPT_STACK_SIZE_PAGES: usize = 4;
pub const INTERRUPT_STACK_SIZE: usize = PAGE_SIZE * INTERRUPT_STACK_SIZE_PAGES;
pub const MAX_CPU_COUNT: usize = 256;

pub fn init() {
    load_gdt(cpu_apic_id());
}

pub fn load_gdt(cpu: usize) {
    get_gdt(cpu).init();
}

pub fn get_gdt(cpu: usize) -> &'static GdtInformation {
    &GDTS[cpu]
}

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const CONTEXT_SWITCH_IST_INDEX: u16 = 1;
static mut TSS_STACKS: [[[u8; INTERRUPT_STACK_SIZE]; 10]; MAX_CPU_COUNT] =
    [[[0; INTERRUPT_STACK_SIZE]; 10]; MAX_CPU_COUNT];

fn get_tss_stacks_for_cpu(cpu_id: u16) -> &'static [[u8; INTERRUPT_STACK_SIZE]; 10] {
    unsafe { &TSS_STACKS[cpu_id as usize] }
}

pub struct GdtInformation {
    gdt: GlobalDescriptorTable,
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    task_state_segment_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
}

impl GdtInformation {
    pub fn new(tss: &'static TaskStateSegment) -> GdtInformation {
        let mut gdt = GlobalDescriptorTable::new();
        let kernel_data_flags =
            DescriptorFlags::USER_SEGMENT | DescriptorFlags::PRESENT | DescriptorFlags::WRITABLE;
        let code_sel = gdt.add_entry(Descriptor::kernel_code_segment()); // kernel code segment
        let data_sel = gdt.add_entry(Descriptor::UserSegment(kernel_data_flags.bits())); // kernel data segment
        let tss_sel = gdt.add_entry(Descriptor::tss_segment(tss)); // task state segment
        let user_data_sel = gdt.add_entry(Descriptor::user_data_segment()); // user data segment
        let user_code_sel = gdt.add_entry(Descriptor::user_code_segment()); // user code segment
        GdtInformation {
            gdt: gdt,
            kernel_code_selector: code_sel,
            kernel_data_selector: data_sel,
            task_state_segment_selector: tss_sel,
            user_data_selector: user_data_sel,
            user_code_selector: user_code_sel,
        }
    }

    pub fn init(self: &'static Self) {
        use x86_64::instructions::segmentation::{CS, DS, SS};

        self.gdt.load();

        unsafe {
            CS::set_reg(self.kernel_code_selector);
            DS::set_reg(self.kernel_data_selector);
            SS::set_reg(self.kernel_data_selector);
            load_tss(self.task_state_segment_selector);
        }
    }

    #[inline]
    pub(crate) fn get_kernel_code_segment(&self) -> SegmentSelector {
        self.kernel_code_selector
    }

    pub(crate) fn get_user_code_segment(&self) -> SegmentSelector {
        self.user_code_selector
    }

    pub(crate) fn get_user_data_segment(&self) -> SegmentSelector {
        self.user_data_selector
    }

    pub(crate) fn get_task_state_segment(&self) -> SegmentSelector {
        self.task_state_segment_selector
    }
}

lazy_static! {
    pub(crate) static ref GDTS: Vec<GdtInformation> = {
        let mut gdts = Vec::new();
        for i in 0..MAX_CPU_COUNT {
            gdts.push(GdtInformation::new(&TASK_STATE_SEGMENTS[i]));
        }

        gdts
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct GdtPointer(usize);

impl GdtPointer {
    pub const fn null() -> Self {
        GdtPointer(0)
    }
    pub fn new<T>(ptr: &T) -> GdtPointer {
        GdtPointer(ptr as *const T as usize)
    }
    pub fn as_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    pub fn as_mut<T>(&self) -> &'static mut T {
        unsafe { (self.0 as *mut T).as_mut().unwrap() }
    }

    pub fn as_ref<T>(&self) -> &'static T {
        self.as_mut::<T>()
    }

    pub fn is_null(&self) -> bool {
        self.0 == 0
    }
}

lazy_static! {
    pub(crate) static ref TASK_STATE_SEGMENTS: Vec<TaskStateSegment> = {
        let mut segments = Vec::new();
        for i in 0..MAX_CPU_COUNT {
            let mut tss = TaskStateSegment::new();
            let stacks = get_tss_stacks_for_cpu(i as u16);
            for x in 0..stacks.len() {
                let stack_address = 
                (VirtAddr::from_ptr(&stacks[x]) + (INTERRUPT_STACK_SIZE - 256)).align_down(16 as u64);
                if x < 7 {
                    tss.interrupt_stack_table[x] = stack_address;
                }

                if x >= 7 && x < 10 {
                    tss.privilege_stack_table[x - 7] = stack_address;
                }
            }
            segments.push(tss);
        }
        segments
    };
}
lazy_static! {
    pub(crate) static ref SMP_TSS_POINTERS: Mutex<[GdtPointer; 512]> =
        Mutex::new([GdtPointer::null(); 512]);
}
lazy_static! {
    pub(crate) static ref SMP_GDT_POINTERS: Mutex<[GdtPointer; 512]> =
        Mutex::new([GdtPointer::null(); 512]);
}
