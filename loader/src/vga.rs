use core::ptr::write_volatile;

use terminal;

const VGA_MEMORY: *mut u16 = 0xb8000 as *mut u16;
const HEIGHT: usize = 25;
const WIDTH: usize = 80;

const COLOR: u8 = 7;

fn make_elem(ch: u8, color: u8) -> u16 {
    let ch16 = ch as u16;
    let color16 = color as u16;
    ch16 | (color16 << 8)
}

fn set_at(x: usize, y: usize, ch: u8) {
    let e = make_elem(ch, COLOR);
    unsafe {
        let p = VGA_MEMORY.offset((y * WIDTH + x) as isize);
        write_volatile(p, e);
    }
}

pub fn clear() {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            set_at(x, y, 0);
        }
    }
}

pub fn display_terminal(term: &terminal::Buffer) {
    clear();

    let top_line = if term.bottom_line >= 25 {
        term.bottom_line - 25
    } else {
        terminal::HEIGHT - 25 + term.bottom_line
    };

    for y in 0..HEIGHT {
        let term_line = (y + top_line) % terminal::HEIGHT;
        let terminal::BufferLine(ref line) = term.data[term_line];
        for x in 0..WIDTH {
            set_at(x, y, line[x]);
        }
    }
}
