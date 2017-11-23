use mm;

extern {
    fn context_init_asm(stack_pointer: u64, entry: extern fn() -> !) -> *mut u8;
    fn context_switch_asm(stack: u64, old_stack: *mut u64);
}

pub struct Context {
    rsp: u64,
}

impl Context {
    pub fn new(stack_pages: u64, entry: extern fn() -> !) -> Context {
        assert!(stack_pages >= 1);

        // Allocate stack.
        let first_page = mm::allocate_address_space(stack_pages).unwrap();
        for i in 0..stack_pages {
            let frame = mm::get_frame_allocator().get_frame() as u64;
            mm::map_to(mm::Page(first_page + i), mm::Frame(frame >> 12), 0b1001,
                       mm::get_frame_allocator());
        }

        let stack_base = (first_page * mm::PAGE_SIZE as u64) as *mut u8;
        let stack_size = stack_pages * mm::PAGE_SIZE as u64;

        // Set up stack.
        let rsp = unsafe {
            context_init_asm(stack_base.offset(stack_size as isize) as u64, entry) as u64
        };

        Context {
            rsp: rsp,
        }
    }

    pub fn new_empty() -> Context {
        Context {
            rsp: 0,
        }
    }

    pub fn switch(&mut self, new_context: &mut Context) {
        unsafe {
            context_switch_asm(new_context.rsp, &mut self.rsp as *mut _);
        }
    }
}