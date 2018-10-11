use core::iter;
use core::str;

pub struct Vector {
    pub x: usize,
    pub y: usize,
}

pub trait Terminal {
    fn size(&self) -> Vector;
    fn blank_elem(&self) -> u8;

    fn write_at(&mut self, c: u8, pos: Vector);

    fn clear(&mut self) {
        let sz = self.size();
        for y in 0..sz.y {
            for x in 0..sz.x {
                let blank_elem = self.blank_elem();
                self.write_at(blank_elem, Vector { x: x, y: y });
            }
        }
    }
}

pub const WIDTH: usize = 80;
pub const HEIGHT: usize = 1024;

pub struct BufferLine(pub [u8; WIDTH]);

impl Clone for BufferLine {
    fn clone(&self) -> BufferLine {
        let BufferLine(a) = *self;
        BufferLine(a)
    }
}

impl Copy for BufferLine {}

pub struct Buffer {
    pub bottom_line: usize,
    pub data: [BufferLine; HEIGHT],
}

impl Clone for Buffer {
    fn clone(&self) -> Buffer {
        Buffer {
            bottom_line: self.bottom_line,
            data: self.data,
        }
    }
}

impl Copy for Buffer {}

impl Buffer {
    pub const fn new() -> Buffer {
        Buffer {
            bottom_line: 25,
            data: [BufferLine([0; WIDTH]); HEIGHT],
        }
    }

    pub fn write_line(&mut self, s: &str) {
        let truncated = s.bytes().take(WIDTH);
        let padded = truncated.chain(iter::repeat(0));

        let BufferLine(ref mut line) = self.data[self.bottom_line];
        for (a, b) in line.iter_mut().zip(padded) {
            *a = b;
        }
        self.bottom_line = (self.bottom_line + 1) % HEIGHT;
    }
}

pub fn display_buffer<T: Terminal>(term: &mut T, buffer: &Buffer) {
    term.clear();

    let top_line = if buffer.bottom_line >= term.size().y {
        buffer.bottom_line - term.size().y
    } else {
        HEIGHT - term.size().y + buffer.bottom_line
    };

    for y in 0..term.size().y {
        let term_line = (y + top_line) % HEIGHT;
        let BufferLine(ref line) = buffer.data[term_line];
        for x in 0..term.size().x {
            term.write_at(line[x], Vector { x: x, y: y });
        }
    }
}
