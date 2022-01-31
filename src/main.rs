use std::cmp::min;
use std::collections::HashMap;
use std::io::Read;
use std::io::Write;
use std::thread;
use std::time::Duration;

use termion;
use termion::raw::IntoRawMode;
use termion::raw::RawTerminal;

const MEMORY_SIZE: usize = 1048576;

// big array cannot be allocated on stack
// https://github.com/rust-lang/rust/issues/53827#issuecomment-576450631
macro_rules! box_array {
    ($val:expr ; $len:expr) => {{
        // Use a generic function so that the pointer cast remains type-safe
        fn vec_to_boxed_array<T>(vec: Vec<T>) -> Box<[T; $len]> {
            let boxed_slice = vec.into_boxed_slice();

            let ptr = ::std::boxed::Box::into_raw(boxed_slice) as *mut [T; $len];

            unsafe { Box::from_raw(ptr) }
        }

        vec_to_boxed_array(vec![$val; $len])
    }};
}

// https://www.stackfinder.ru/questions/26321592/how-can-i-read-one-character-from-stdin-without-having-to-hit-enter
// implementing non-blocking read
fn read_noblocking(cin: &mut std::io::Bytes<termion::AsyncReader>) -> Option<u8> {
    match cin.next() {
        Some(Ok(0)) => None,
        Some(Ok(n)) => Some(n),
        Some(Err(_)) => None,
        None => None,
    }
}

struct Memory {
    buffer: Box<[u32; MEMORY_SIZE]>,
    io: (
        std::io::Bytes<termion::AsyncReader>,
        RawTerminal<std::io::Stdout>,
    ),
}
impl Memory {
    fn load32(&self, address: usize) -> u32 {
        self.buffer[address]
    }

    fn load_opcode(&self, address: usize) -> (u16, u16) {
        let i32 = self.load32(address);
        (
            (i32 >> 16).try_into().unwrap(),
            (i32 & 0xFFFF).try_into().unwrap(),
        )
    }

    fn load64(&self, address: usize) -> u64 {
        self.buffer[address * 2] as u64 * 4294967296 + self.buffer[address * 2 + 1] as u64
    }

    fn store64(&mut self, address: usize, value: u64) {
        self.buffer[address * 2] = (value / 4294967296).try_into().unwrap();
        self.buffer[address * 2 + 1] = (value % 4294967296).try_into().unwrap();
    }

    fn store(&mut self, data: &[u8], base: usize) {
        let data_end = min(base + data.len() / 4, MEMORY_SIZE) - 1;
        for i in base..data_end {
            print!("Storing at position {}: ", i);
            self.buffer[i] = 0;
            for j in 0..4 {
                print!("{} ", (i - base) * 4 + j);
                self.buffer[i] *= 256;
                self.buffer[i] += data[(i - base) * 4 + j] as u32;
            }
            println!();
        }
    }
}

struct Registers {
    buffer: [i64; 36],
    triggers: HashMap<
        usize,
        (
            Vec<fn(usize, &mut [i64; 36], &mut Memory)>,
            Vec<fn(usize, &mut [i64; 36], &mut Memory)>,
        ),
    >,
}
impl Registers {
    fn get_triggers_pair(
        &mut self,
        index: usize,
    ) -> &mut (
        Vec<fn(usize, &mut [i64; 36], &mut Memory)>,
        Vec<fn(usize, &mut [i64; 36], &mut Memory)>,
    ) {
        self.triggers
            .entry(index)
            .or_insert((Vec::new(), Vec::new()))
    }

    fn init_triggers(&mut self) {
        fn add_trig(_trig: usize, buffer: &mut [i64; 36], _memory: &mut Memory) {
            buffer[2] = buffer[0] + buffer[1];
        }
        self.get_triggers_pair(0).1.push(add_trig);
        self.get_triggers_pair(1).1.push(add_trig);
        self.get_triggers_pair(2).0.push(add_trig);

        fn sub_trig(_trig: usize, buffer: &mut [i64; 36], _memory: &mut Memory) {
            buffer[5] = buffer[3] - buffer[4];
        }
        self.get_triggers_pair(3).1.push(sub_trig);
        self.get_triggers_pair(4).1.push(sub_trig);
        self.get_triggers_pair(5).0.push(sub_trig);

        fn mul_trig(_trig: usize, buffer: &mut [i64; 36], _memory: &mut Memory) {
            buffer[8] = buffer[6] * buffer[7];
        }
        self.get_triggers_pair(6).1.push(mul_trig);
        self.get_triggers_pair(7).1.push(mul_trig);
        self.get_triggers_pair(8).0.push(mul_trig);

        fn div_trig(_trig: usize, buffer: &mut [i64; 36], _memory: &mut Memory) {
            let div0 = buffer[9];
            let div1 = buffer[10];
            buffer[11] = if div1 != 0 { div0 / div1 } else { div0 };
            buffer[12] = if div1 != 0 { div0 % div1 } else { 0 };
        }
        self.get_triggers_pair(9).1.push(div_trig);
        self.get_triggers_pair(10).1.push(div_trig);
        self.get_triggers_pair(11).0.push(div_trig);
        self.get_triggers_pair(12).0.push(div_trig);

        fn tlt_trig(_trig: usize, buffer: &mut [i64; 36], _memory: &mut Memory) {
            buffer[15] = if buffer[13] < buffer[14] { 1 } else { 0 };
        }
        self.get_triggers_pair(13).1.push(tlt_trig);
        self.get_triggers_pair(14).1.push(tlt_trig);
        self.get_triggers_pair(15).0.push(tlt_trig);

        fn cio_trig(trig: usize, buffer: &mut [i64; 36], memory: &mut Memory) {
            if trig == 1 {
                if buffer[16] == 256 {
                    memory.io.1.lock().flush().unwrap();
                    thread::sleep(Duration::from_millis(50));

                    print!("\x1B[1;1H\x1B[J");

                    memory.io.1.flush().unwrap();

                    return;
                }
                print!(
                    "{}",
                    char::from_u32(buffer[16].try_into().unwrap()).unwrap()
                );
            } else {
                buffer[16] = match read_noblocking(&mut memory.io.0) {
                    Some(v) => v.into(),
                    None => -1,
                };
            }
        }
        self.get_triggers_pair(16).0.push(cio_trig);
        self.get_triggers_pair(16).1.push(cio_trig);

        fn io_trig(trig: usize, buffer: &mut [i64; 36], _memory: &mut Memory) {
            if trig == 1 {
                print!(
                    "{}",
                    char::from_u32(buffer[18].try_into().unwrap()).unwrap()
                )
            } else {
                buffer[19] = 10;
            }
        }
        self.get_triggers_pair(18).1.push(io_trig);
        self.get_triggers_pair(19).0.push(io_trig);

        fn atz_trig(_trig: usize, buffer: &mut [i64; 36], _memory: &mut Memory) {
            buffer[23] = if buffer[20] == 0 {
                buffer[21]
            } else {
                buffer[22]
            };
        }
        self.get_triggers_pair(20).1.push(atz_trig);
        self.get_triggers_pair(21).1.push(atz_trig);
        self.get_triggers_pair(22).1.push(atz_trig);
        self.get_triggers_pair(23).0.push(atz_trig);

        fn mem_trig(trig: usize, buffer: &mut [i64; 36], memory: &mut Memory) {
            if trig == 1 {
                memory.store64(
                    buffer[26].try_into().unwrap(),
                    buffer[24].try_into().unwrap(),
                );
            } else {
                buffer[24] = memory
                    .load64(buffer[26].try_into().unwrap())
                    .try_into()
                    .unwrap();
            }
        }
        self.get_triggers_pair(24).0.push(mem_trig);
        self.get_triggers_pair(24).1.push(mem_trig);
        self.get_triggers_pair(26).1.push(mem_trig);
    }

    fn set(&mut self, index: usize, value: i64, memory: &mut Memory) {
        self.buffer[index] = value;

        match &self.triggers.get(&index) {
            Some(trigs) => {
                let buf = &mut (self.buffer);
                for callback in trigs.1.iter() {
                    callback(1, buf, memory);
                }
            }
            None => {}
        }
    }

    fn get(&mut self, index: usize, memory: &mut Memory) -> i64 {
        match &self.triggers.get(&index) {
            Some(trigs) => {
                let buf = &mut (self.buffer);
                for callback in trigs.0.iter() {
                    callback(0, buf, memory);
                }
            }
            None => {}
        }
        self.buffer[index]
    }
}

fn main() {
    let stdout = std::io::stdout().into_raw_mode().unwrap();
    let stdin = termion::async_stdin().bytes();

    let mut regs: Registers = Registers {
        buffer: [0; 36],
        triggers: HashMap::new(),
    };
    let mut mem: Memory = Memory {
        buffer: box_array![0; MEMORY_SIZE],
        io: (stdin, stdout),
    };

    regs.init_triggers();

    regs.set(20, 2, &mut mem);
    regs.set(21, 5, &mut mem);
    regs.set(22, 6, &mut mem);
    assert_eq!(regs.get(23, &mut mem), 6);

    mem.store(b"\x80<\x00\x1c\x80\n\x00\x1b\x80\x01\x00\x04\x00\x03\x00\x14\x00\x1d\x00\x15\x80\x07\x00\x16\x00\x17\x00\x1b\x00\x1e\x00\x10\x00\x05\x00\x03\x80\x03\x00\x1b\x00\x10\x00\x03\x80 \x00\x04\x00\x05\x00\x14\x80N\x00\x15\x80\x10\x00\x16\x00\x17\x00\x1b\xa5T\x00\x10\x00\x1c\x00\x03\x80\x02\x00\x04\x00\x05\x00\x03\xa5P\x00\x1e\x80\x17\x00\x1d\x80\x02\x00\x1b\xa5W\x00\x10\x80\n\x00\x10\xa5Q\x00\x10\x80 \x00\x10\x80 \x00\x10\x80T\x00\x10\x80i\x00\x10\x80g\x00\x10\x80e\x00\x10\x80r\x00\x10\x80O\x00\x10\x80S\x00\x10\x80 \x00\x10\x80v\x00\x10\x800\x00\x10\x80.\x00\x10\x800\x00\x10\x80.\x00\x10\x801\x00\x10\x80 \x00\x10\x80|\x00\x10\x80 \x00\x10\x80n\x00\x10\x80o\x00\x10\x80t\x00\x10\x80 \x00\x10\x80l\x00\x10\x80i\x00\x10\x80c\x00\x10\x80e\x00\x10\x80n\x00\x10\x80s\x00\x10\x80e\x00\x10\x80d\x00\x10\x80!\x00\x10\x80!\x00\x10\x80!\x00\x10\x00\x1c\x00\x03\x80$\x00\x04\x00\x05\x00\x03\x80 \x00\x1e\x80B\x00\x1d\x80\x02\x00\x1b\xa5Q\x00\x10\x80\n\x00\x10\xa5Z\x00\x10\x00\x1c\x00\x03\x80\x02\x00\x04\x00\x05\x00\x03\xa5P\x00\x1e\x80K\x00\x1d\x80\x02\x00\x1b\xa5]\x00\x10\x81\x00\x00\x10\x80\n\x00\x1b\x81\x01\x00\x10", 0);
    println!("First 64: {}", mem.load64(0));
    println!("First 32: {}", mem.load32(0));

    let mut i: u64 = 0;
    print!(
        " ... stopped: tick={}",
        loop {
            let addr = regs.buffer[27] as usize;

            if addr >= MEMORY_SIZE {
                break i;
            }

            let (src, dst) = mem.load_opcode(addr);

            let val = if src & 0x8000 != 0 {
                (src & 0x7FFF) as i64
            } else {
                regs.get(src.try_into().unwrap(), &mut mem)
            };
            regs.buffer[27] = (addr + 1).try_into().unwrap();
            regs.set(dst.try_into().unwrap(), val, &mut mem);

            i = i + 1;
        }
    );
    println!(", addr={}", regs.buffer[27]);
}
