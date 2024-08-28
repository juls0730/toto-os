# CappuccinOS

<!--
    Use Tokei instead of a custom loc count, tokei and my custom loc count seem to disagree by 30-100 lines but I suspect tokei to be more accurate than cloc
    ![LOC](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/juls0730/c16f26c4c5ab7f613fe758c913f9e71f/raw/cappuccinos-loc.json)
-->

[![](https://tokei.rs/b1/github/juls0730/CappuccinOS?category=code&type=Rust)](https://github.com/juls0730/CappuccinOS).

CappuccinOS is a small _next generation_ x86-64 operating system written from scratch in Rust. This README will guide you through the process of building and running CappuccinOS.

> [!WARNING]
> This project is in early development. Things will change.

## Features

- [x] Serial output
- [x] Hardware interrupts
- [x] Heap allocation
- [ ] Externalized kernel modules
  - [x] Initramfs
    - [x] Squashfs driver
      - [x] Programmatic reads
      - [x] Decompression
- [ ] PS/2 Keyboard support
- [ ] ANSI color codes in console
- [ ] SMP
  - [x] Use APIC instead of PIC
- [ ] Pre-emptive multitasking
  - [ ] Scheduling
- [ ] File system
  - [x] FAT file system (read-only rn)
  - [ ] Ext2 file system
- [ ] Block Device support
  - [x] IDE device support
  - [ ] SATA device support
  - [ ] MMC/Nand device support
  - [ ] M.2 NVME device support
- [ ] Basic shell
  - [ ] Basic I/O
    - [ ] Executing Programs from disk
- [ ] Lua interpreter
- [x] Memory management
- [ ] Network support
- [ ] GUI
- [ ] Device drivers
  - [ ] Native intel graphics
- [ ] User authentication
- [ ] Power management
- [ ] Paging
- [ ] RTC Clock

## Setup

Before building CappuccinOS, make sure you have the following installed on your machine:

- rust
- python
- sgdisk
- dosfstools
- squashfs-tools
- qemu (optional)

Clone the repo:

```BASH
git clone https://github.com/juls0730/CappuccinOS.git
cd CappuccinOS
```

Install rust, if you haven't already:

```BASH
curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain none
```

Install the dependencies:

<details>
    <summary>Arch</summary>

    sudo pacman -S gptfdisk dosfstools squashfs-tools python
    # Optionally
    sudo pacman -S qemu-system-x86

</details>

<details>
    <summary>Ubuntu</summary>
    # Python should be installed by default, and if it's not, make an issue or a PR and I'll fix it

    sudo apt install gdisk dosfstools squashfs-tools
    # Optionally
    sudo apt install qemu

</details>

## Usage

Run CappuccinOS with QEMU:

```BASH
make run
```

If you would like to just build CappuccinOS but not run it:

```BASH
make build
```

If you would like to target another architecture other than x86_64, set the `ARCH` variable to the a supported architecture. CappuccinOS is also built in release mode by default, if you would like to build CappuccinOS in debug mode, set the `MODE` variable to `debug`.

Run on a bare metal machine by flashing to a USB stick or hard drive:

```
sudo dd if=bin/CappuccinOS.iso of=/dev/sdX bs=1M && sync
```

**Be careful not to overwrite your hard drive when using `dd`!**

## Supported Architectures

- x86_64
- ~~aarch64~~ not in scope **might** not build
- ~~RISC-V64~~ not in scope **might** not build

## Credits an attributions

Inspiration was mainly from [JDH's Tetris OS](https://www.youtube.com/watch?v=FaILnmUYS_U), mixed with a growing interest in low level in general and an interest in learning rust (yeah, I started this project with not that much rust experience, maybe a CLI app or two, and trust me it shows).

Some Resources I used over the creation of CappuccinOS:

- [OSDev wiki](https://wiki.osdev.org)
- Wikipedia on various random things
- [Squashfs Binary Format](https://dr-emann.github.io/squashfs/squashfs.html)
- [GRUB](https://www.gnu.org/software/grub/grub-download.html) Mainly for Squashfs things, even though I later learned it does things incorrectly

And mostly for examples of how people did stuff I used these (projects made by people who might actually have a clue what they're doing):

- This is missing some entries somehow
- [MOROS](https://github.com/vinc/moros)
- [Felix](https://github.com/mrgian/felix)
- [mOS](https://github.com/Moldytzu/mOS)
- [rust_os](https://github.com/thepowersgang/rust_os/tree/master)
- [Lyre](https://github.com/Lyre-OS/klyre)
- [Limine](https://github.com/limine-bootloader/limine) as my paging implementation is largely a rust translation of its

```
Copyright (C) 2019-2024 mintsuki and contributors.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

## License

CappuccinOS is license under the MIT License. Feel free to modify and distribute in accordance with the license.
