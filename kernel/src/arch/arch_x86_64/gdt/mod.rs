use lazy_static::lazy_static;
use x86_64::structures::gdt::{
    Descriptor, DescriptorFlags, GlobalDescriptorTable, SegmentSelector,
};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

use crate::memory::allocator::PAGE_SIZE;
pub const INTERRUPT_STACK_SIZE_PAGES: usize = 5;
pub const INTERRUPT_STACK_SIZE: usize = PAGE_SIZE * INTERRUPT_STACK_SIZE_PAGES;

pub fn init() {
    GDT.init();
}

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const CONTEXT_SWITCH_IST_INDEX: u16 = 1;

lazy_static! {
    pub(crate) static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            static mut STACK: [u8; INTERRUPT_STACK_SIZE] = [0; INTERRUPT_STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + INTERRUPT_STACK_SIZE;
            stack_end
        };
        tss.interrupt_stack_table[CONTEXT_SWITCH_IST_INDEX as usize] = {
            static mut STACK: [u8; INTERRUPT_STACK_SIZE] = [0; INTERRUPT_STACK_SIZE];

            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            let stack_end = stack_start + INTERRUPT_STACK_SIZE;
            stack_end
        };
        tss
    };
}

pub(crate) struct GdtInformation {
    gdt: GlobalDescriptorTable,
    kernel_code_selector: SegmentSelector,
    kernel_data_selector: SegmentSelector,
    task_state_segment_selector: SegmentSelector,
    user_data_selector: SegmentSelector,
    user_code_selector: SegmentSelector,
}

impl GdtInformation {
    pub fn new() -> GdtInformation {
        let mut gdt = GlobalDescriptorTable::new();
        let kernel_data_flags =
            DescriptorFlags::USER_SEGMENT | DescriptorFlags::PRESENT | DescriptorFlags::WRITABLE;
        let code_sel = gdt.add_entry(Descriptor::kernel_code_segment()); // kernel code segment
        let data_sel = gdt.add_entry(Descriptor::UserSegment(kernel_data_flags.bits())); // kernel data segment
        let tss_sel = gdt.add_entry(Descriptor::tss_segment(&TSS)); // task state segment
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
        use x86_64::instructions::segmentation::{Segment, CS, DS};
        use x86_64::instructions::tables::load_tss;

        self.gdt.load();

        unsafe {
            CS::set_reg(self.kernel_code_selector);
            DS::set_reg(self.kernel_data_selector);
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
}

lazy_static! {
    pub(crate) static ref GDT: GdtInformation = GdtInformation::new();
}
