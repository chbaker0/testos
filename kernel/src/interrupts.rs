use x86_64::structures::idt;

lazy_static! {
    static ref IDT: idt::Idt = {
        let mut idt = idt::Idt::new();
        idt
    };
}

pub fn init() {
    IDT.load();
}
