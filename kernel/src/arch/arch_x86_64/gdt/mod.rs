use core::alloc::Layout;
use lazy_static::lazy_static;
use spin::Mutex;
use x86_64::instructions::tables::load_tss;
use x86_64::registers::segmentation::{Segment, FS, GS};
use x86_64::structures::gdt::{
    Descriptor, DescriptorFlags, GlobalDescriptorTable, SegmentSelector,
};

use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

use crate::memory::allocator::{kmalloc, PAGE_SIZE};

pub const INTERRUPT_STACK_SIZE_PAGES: usize = 1;
pub const INTERRUPT_STACK_SIZE: usize = PAGE_SIZE * INTERRUPT_STACK_SIZE_PAGES;

pub fn init() {
    BOOT_GDT.init();
}

pub(crate) fn allocate_gdt() -> GdtPointer {
    let gdt_pointer: GdtPointer;
    let gdt_raw_pointer = kmalloc(Layout::new::<GdtInformation>());
    let tss_raw_pointer = kmalloc(Layout::new::<TaskStateSegment>());
    unsafe {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = VirtAddr::new(kmalloc(
            Layout::from_size_align(INTERRUPT_STACK_SIZE, 16)
                .expect("Invalid layout for interrupt stack?"),
        )
            as u64)
            + INTERRUPT_STACK_SIZE;
        (*(tss_raw_pointer as *mut TaskStateSegment)) = tss;
        (*(gdt_raw_pointer as *mut GdtInformation)) = GdtInformation::new(
            (tss_raw_pointer as *const TaskStateSegment)
                .as_ref()
                .unwrap(),
        );
        gdt_pointer = GdtPointer::new((gdt_raw_pointer as *const GdtInformation).as_ref().unwrap());
    }
    gdt_pointer
}

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
pub const CONTEXT_SWITCH_IST_INDEX: u16 = 1;

pub(crate) struct GdtInformation {
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
        use x86_64::instructions::segmentation::{CS, DS};

        self.gdt.load();

        unsafe {
            CS::set_reg(self.kernel_code_selector);
            DS::set_reg(self.kernel_data_selector);
            FS::set_reg(self.user_code_selector);
            GS::set_reg(self.user_data_selector);
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

    pub(crate) fn get_task_state_segment(&self) -> SegmentSelector {
        self.task_state_segment_selector
    }
}

lazy_static! {
    pub(crate) static ref BOOT_GDT: GdtInformation = GdtInformation::new(&BOOT_TSS);
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
    pub(crate) static ref BOOT_TSS: TaskStateSegment = {
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
lazy_static! {
    pub(crate) static ref SMP_TSS_POINTERS: Mutex<[GdtPointer; 512]> =
        Mutex::new([GdtPointer::null(); 512]);
}
lazy_static! {
    pub(crate) static ref SMP_GDT_POINTERS: Mutex<[GdtPointer; 512]> =
        Mutex::new([GdtPointer::null(); 512]);
}
