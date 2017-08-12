use core::iter;
use core::str;

pub const WIDTH: usize = 80;
pub const HEIGHT: usize = 1024;

pub struct BufferLine(pub [u8; WIDTH]);

impl Clone for BufferLine {
    fn clone(&self) -> BufferLine {
        let BufferLine(a) = *self;
        BufferLine(a)
    }
}

impl Copy for BufferLine { }

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

impl Copy for Buffer { }

impl Buffer {
    pub fn new() -> Buffer {
        Buffer {
            bottom_line: 25,
            data: [BufferLine([0; WIDTH]); HEIGHT]
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
