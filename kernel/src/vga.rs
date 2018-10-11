use core::ptr::write_volatile;

use terminal;

const VGA_MEMORY: *mut u16 = 0xb8000 as *mut u16;
const HEIGHT: usize = 25;
const WIDTH: usize = 80;

const COLOR: u8 = 7;

pub struct VgaTerminal {
    mem: *mut u16,
}

impl VgaTerminal {
    fn make_elem(ch: u8, color: u8) -> u16 {
        let ch16 = ch as u16;
        let color16 = color as u16;
        ch16 | (color16 << 8)
    }
}

unsafe impl Send for VgaTerminal {}

impl terminal::Terminal for VgaTerminal {
    fn size(&self) -> terminal::Vector {
        terminal::Vector {
            x: WIDTH,
            y: HEIGHT,
        }
    }
    fn blank_elem(&self) -> u8 {
        0
    }

    fn write_at(&mut self, c: u8, pos: terminal::Vector) {
        let e = Self::make_elem(c, COLOR);
        unsafe {
            let p = self.mem.offset((pos.y * self.size().x + pos.x) as isize);
            write_volatile(p, e);
        }
    }
}

pub static VGA_TERMINAL: spin::Mutex<VgaTerminal> =
    spin::Mutex::new(VgaTerminal { mem: VGA_MEMORY });
