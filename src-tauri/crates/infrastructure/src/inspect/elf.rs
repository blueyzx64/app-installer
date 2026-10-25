//! Minimal ELF header reader for AppImage runtimes (F-04).
//!
//! An AppImage type 2 file is an ELF runtime with a SquashFS image glued to its
//! end. The image starts right after the ELF section header table:
//! `e_shoff + e_shentsize * e_shnum`. We only read bytes here; nothing is run.

use installer_domain::{Arch, InspectError, UnsupportedReason};

/// Bytes needed to read every field we use (64-bit header is 64 bytes).
pub const ELF_HEADER_LEN: usize = 64;

const PT_INTERP: u32 = 3;
/// Cap on program headers we walk, so a hostile header cannot make us loop long.
const MAX_PHDRS: usize = 256;

/// SquashFS magic ("hsqs", little endian).
pub const SQUASHFS_MAGIC: &[u8; 4] = b"hsqs";
/// DwarFS images start with "DWARFS".
pub const DWARFS_MAGIC: &[u8; 6] = b"DWARFS";

/// What AppImage type the `AI` marker at byte 8 announces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppImageType {
    One,
    Two,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfHeader {
    pub is_64: bool,
    pub big_endian: bool,
    pub machine: u16,
    pub phoff: u64,
    pub phentsize: u16,
    pub phnum: u16,
    pub shoff: u64,
    pub shentsize: u16,
    pub shnum: u16,
    pub appimage_type: Option<AppImageType>,
}

impl ElfHeader {
    /// Parse the first bytes of a file. `Corrupt` if it is not a sane ELF header.
    pub fn parse(b: &[u8]) -> Result<ElfHeader, InspectError> {
        if b.len() < 52 || &b[..4] != b"\x7fELF" {
            return Err(InspectError::Corrupt);
        }
        let is_64 = match b[4] {
            1 => false,
            2 => true,
            _ => return Err(InspectError::Corrupt),
        };
        let big_endian = match b[5] {
            1 => false,
            2 => true,
            _ => return Err(InspectError::Corrupt),
        };
        if is_64 && b.len() < ELF_HEADER_LEN {
            return Err(InspectError::Corrupt);
        }
        let rd = Reader { b, big_endian };
        let appimage_type = match (b.get(8..10), b.get(10)) {
            (Some(b"AI"), Some(1)) => Some(AppImageType::One),
            (Some(b"AI"), Some(2)) => Some(AppImageType::Two),
            _ => None,
        };
        let machine = rd.u16(18);
        // Field offsets differ between ELF32 and ELF64.
        let (phoff, shoff, phentsize, phnum, shentsize, shnum) = if is_64 {
            (
                rd.u64(32),
                rd.u64(40),
                rd.u16(54),
                rd.u16(56),
                rd.u16(58),
                rd.u16(60),
            )
        } else {
            (
                u64::from(rd.u32(28)),
                u64::from(rd.u32(32)),
                rd.u16(42),
                rd.u16(44),
                rd.u16(46),
                rd.u16(48),
            )
        };
        Ok(ElfHeader {
            is_64,
            big_endian,
            machine,
            phoff,
            phentsize,
            phnum,
            shoff,
            shentsize,
            shnum,
            appimage_type,
        })
    }

    /// End of the section header table = start of the SquashFS image.
    /// `None` on overflow or when there is no section header table.
    pub fn squashfs_offset(&self) -> Option<u64> {
        if self.shoff == 0 || self.shentsize == 0 || self.shnum == 0 {
            return None;
        }
        self.shoff
            .checked_add(u64::from(self.shentsize).checked_mul(u64::from(self.shnum))?)
    }

    /// Architecture from `e_machine`.
    pub fn arch(&self) -> Arch {
        match self.machine {
            62 => Arch::X86_64,
            183 => Arch::Aarch64,
            40 => Arch::Armv7,
            3 => Arch::I686,
            other => Arch::Other(format!("elf-machine-{other}")),
        }
    }

    /// Whether the runtime has a program interpreter (`PT_INTERP`), i.e. is dynamically
    /// linked. `runtime` must hold the bytes from file start to the SquashFS offset.
    pub fn is_dynamic(&self, runtime: &[u8]) -> bool {
        let entsize = usize::from(self.phentsize);
        let min = if self.is_64 { 56 } else { 32 };
        if entsize < min {
            return false;
        }
        let rd = Reader {
            b: runtime,
            big_endian: self.big_endian,
        };
        let Ok(base) = usize::try_from(self.phoff) else {
            return false;
        };
        for i in 0..usize::from(self.phnum).min(MAX_PHDRS) {
            let Some(at) = i.checked_mul(entsize).and_then(|o| o.checked_add(base)) else {
                return false;
            };
            if at.checked_add(4).is_none_or(|end| end > runtime.len()) {
                return false;
            }
            if rd.u32(at) == PT_INTERP {
                return true;
            }
        }
        false
    }
}

/// Bounds-safe reads; out of range reads give 0 instead of panicking.
struct Reader<'a> {
    b: &'a [u8],
    big_endian: bool,
}

impl Reader<'_> {
    fn arr<const N: usize>(&self, at: usize) -> Option<[u8; N]> {
        self.b.get(at..at.checked_add(N)?)?.try_into().ok()
    }
    fn u16(&self, at: usize) -> u16 {
        self.arr(at).map_or(0, |a| {
            if self.big_endian {
                u16::from_be_bytes(a)
            } else {
                u16::from_le_bytes(a)
            }
        })
    }
    fn u32(&self, at: usize) -> u32 {
        self.arr(at).map_or(0, |a| {
            if self.big_endian {
                u32::from_be_bytes(a)
            } else {
                u32::from_le_bytes(a)
            }
        })
    }
    fn u64(&self, at: usize) -> u64 {
        self.arr(at).map_or(0, |a| {
            if self.big_endian {
                u64::from_be_bytes(a)
            } else {
                u64::from_le_bytes(a)
            }
        })
    }
}

/// What sits at the SquashFS offset.
pub fn classify_image(magic: &[u8]) -> Result<(), InspectError> {
    if magic.starts_with(SQUASHFS_MAGIC) {
        Ok(())
    } else if magic.starts_with(DWARFS_MAGIC) {
        Err(InspectError::Unsupported(UnsupportedReason::Dwarfs))
    } else {
        Err(InspectError::Corrupt)
    }
}

/// Check the AppImage type: 2 is supported, 1 is not, anything else is not an AppImage.
pub fn require_type_two(h: &ElfHeader) -> Result<(), InspectError> {
    match h.appimage_type {
        Some(AppImageType::Two) => Ok(()),
        Some(AppImageType::One) => Err(InspectError::Unsupported(UnsupportedReason::AppImageType1)),
        None => Err(InspectError::Corrupt),
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    /// Build an ELF header (+ optional PT_INTERP phdr + empty section table) as bytes.
    /// Section header table sits at `shoff` and the returned vec ends exactly where it ends.
    pub fn elf_stub(is_64: bool, big: bool, machine: u16, ai: u8, dynamic: bool) -> Vec<u8> {
        let hdr = if is_64 { 64usize } else { 52 };
        let phent = if is_64 { 56usize } else { 32 };
        let shent = if is_64 { 64usize } else { 40 };
        let phnum = usize::from(dynamic);
        let phoff = hdr;
        let shoff = hdr + phnum * phent;
        let shnum = 2usize;
        let total = shoff + shnum * shent;
        let mut b = vec![0u8; total];
        b[..4].copy_from_slice(b"\x7fELF");
        b[4] = if is_64 { 2 } else { 1 };
        b[5] = if big { 2 } else { 1 };
        b[6] = 1;
        b[8] = b'A';
        b[9] = b'I';
        b[10] = ai;
        let p16 = |b: &mut [u8], at: usize, v: u16| {
            let a = if big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            };
            b[at..at + 2].copy_from_slice(&a);
        };
        let p32 = |b: &mut [u8], at: usize, v: u32| {
            let a = if big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            };
            b[at..at + 4].copy_from_slice(&a);
        };
        let p64 = |b: &mut [u8], at: usize, v: u64| {
            let a = if big {
                v.to_be_bytes()
            } else {
                v.to_le_bytes()
            };
            b[at..at + 8].copy_from_slice(&a);
        };
        p16(&mut b, 16, 2);
        p16(&mut b, 18, machine);
        if is_64 {
            if dynamic {
                p64(&mut b, 32, phoff as u64);
            }
            p64(&mut b, 40, shoff as u64);
            p16(&mut b, 52, 64);
            p16(&mut b, 54, phent as u16);
            p16(&mut b, 56, phnum as u16);
            p16(&mut b, 58, shent as u16);
            p16(&mut b, 60, shnum as u16);
        } else {
            if dynamic {
                p32(&mut b, 28, phoff as u32);
            }
            p32(&mut b, 32, shoff as u32);
            p16(&mut b, 40, 52);
            p16(&mut b, 42, phent as u16);
            p16(&mut b, 44, phnum as u16);
            p16(&mut b, 46, shent as u16);
            p16(&mut b, 48, shnum as u16);
        }
        if dynamic {
            p32(&mut b, phoff, 3); // PT_INTERP
        }
        b
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::elf_stub;
    use super::*;

    #[test]
    fn offset_table_32_64_le_be() {
        for (is_64, big) in [(true, false), (true, true), (false, false), (false, true)] {
            let b = elf_stub(is_64, big, 62, 2, false);
            let h = ElfHeader::parse(&b).unwrap();
            assert_eq!(h.is_64, is_64);
            assert_eq!(h.big_endian, big);
            assert_eq!(h.squashfs_offset(), Some(b.len() as u64), "{is_64} {big}");
            assert_eq!(h.appimage_type, Some(AppImageType::Two));
            assert!(require_type_two(&h).is_ok());
        }
    }

    #[test]
    fn machine_to_arch() {
        let rows = [
            (62, Arch::X86_64),
            (183, Arch::Aarch64),
            (40, Arch::Armv7),
            (3, Arch::I686),
            (243, Arch::Other("elf-machine-243".into())),
        ];
        for (m, want) in rows {
            let h = ElfHeader::parse(&elf_stub(true, false, m, 2, false)).unwrap();
            assert_eq!(h.arch(), want);
        }
    }

    #[test]
    fn type_one_and_unmarked() {
        let h = ElfHeader::parse(&elf_stub(true, false, 62, 1, false)).unwrap();
        assert_eq!(
            require_type_two(&h),
            Err(InspectError::Unsupported(UnsupportedReason::AppImageType1))
        );
        let mut b = elf_stub(true, false, 62, 2, false);
        b[8] = 0;
        let h = ElfHeader::parse(&b).unwrap();
        assert_eq!(require_type_two(&h), Err(InspectError::Corrupt));
    }

    #[test]
    fn rejects_garbage_and_short() {
        assert_eq!(ElfHeader::parse(b""), Err(InspectError::Corrupt));
        assert_eq!(ElfHeader::parse(b"\x7fELF"), Err(InspectError::Corrupt));
        let mut b = elf_stub(true, false, 62, 2, false);
        b[4] = 9;
        assert_eq!(ElfHeader::parse(&b), Err(InspectError::Corrupt));
        let b = elf_stub(true, false, 62, 2, false);
        assert_eq!(ElfHeader::parse(&b[..60]), Err(InspectError::Corrupt));
    }

    #[test]
    fn offset_overflow_and_missing_table() {
        let mut b = elf_stub(true, false, 62, 2, false);
        b[40..48].copy_from_slice(&u64::MAX.to_le_bytes());
        let h = ElfHeader::parse(&b).unwrap();
        assert_eq!(h.squashfs_offset(), None);
        let mut b = elf_stub(true, false, 62, 2, false);
        b[60..62].copy_from_slice(&0u16.to_le_bytes());
        assert_eq!(ElfHeader::parse(&b).unwrap().squashfs_offset(), None);
    }

    #[test]
    fn dynamic_detection() {
        for (is_64, big) in [(true, false), (false, true)] {
            let dynb = elf_stub(is_64, big, 62, 2, true);
            let h = ElfHeader::parse(&dynb).unwrap();
            assert!(h.is_dynamic(&dynb));
            let stat = elf_stub(is_64, big, 62, 2, false);
            let h = ElfHeader::parse(&stat).unwrap();
            assert!(!h.is_dynamic(&stat));
        }
        // Truncated runtime: no panic, not dynamic.
        let dynb = elf_stub(true, false, 62, 2, true);
        let h = ElfHeader::parse(&dynb).unwrap();
        assert!(!h.is_dynamic(&dynb[..66]));
    }

    #[test]
    fn image_magic() {
        assert!(classify_image(b"hsqs\0\0").is_ok());
        assert_eq!(
            classify_image(b"DWARFS\x02"),
            Err(InspectError::Unsupported(UnsupportedReason::Dwarfs))
        );
        assert_eq!(classify_image(b"xxxx"), Err(InspectError::Corrupt));
        assert_eq!(classify_image(b""), Err(InspectError::Corrupt));
    }
}
