# rust-mov-vm
## VM for Mov architecture, PGR-Prm implementation

This VM introduces a new processor architecture, called Mov.
It really consists of one command: MOV [src] [dst].
All calculations are done between moves by registers themselves.

Each command is 32-bit: 16-bit source and 16-bit destination (both little-endian).
- source starts with **1 bit - register/constant value**
- - if not set, **15 bits - source register index**
- - if set, **1 bit - sign** and then **14 bit - value**
- destination is 16-bit register index.

## Available registers
```
{'add0': 0, 'add1': 1, 'add': 2,
 'sub0': 3, 'sub1': 4, 'sub': 5,
 'mul0': 6, 'mul1': 7, 'mul': 8,
 'div0': 9, 'div1': 10, 'div': 11, 'mod': 12,
 'tlt0': 13, 'tlt1': 14, 'tlt': 15,
 'cio': 16,
 'io0': 17, 'io1': 18, 'io': 19,
 'atz0': 20, 'atz1': 21, 'atz2': 22, 'atz': 23,
 'memory': 24, 'rand': 25, 'maddr': 26,
 'addr': 27,
 'reg0': 28, 'reg1': 29, 'reg2': 30, 'reg3': 31, 'reg4': 32, 'reg5': 33, 'reg6': 34, 'reg7': 35}
```

### Auto calculations, registers meaning
```
add = add0 + add1
sub = sub0 - sub1
mul = mul0 * mul1
div = div0 / div1 (div1 = 0 -> div = div0)
mod = div0 % div1 (div1 = 0 -> mod = 0)
tlt = 1 if tlt0 < tlt1 else 0
atz = atz1 if atz0 == 0 else atz2
rand = either 32-bit non-deterministic value or zero
  (if VM implementation does not support getting secure random numbers or there is not enough entropy)
addr = current instruction
reg0-7 = general-purpose registers
```

### I/O registers
cio (Curses I/O)
- read from cio: -1 if no keys were pressed on the keyboard, otherwise char code
- write to cio:
- - 256 - clear screen
- - 257 - return terminal to its original state
- - other keys - print that char to screen

io (standard I/O)
- io0 - index of device
- io1 (write-only) - send 64-bit value to device
- io  (read-only)  - receive 64-bit value from device
