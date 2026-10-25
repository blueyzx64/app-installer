//! Minimal ELF header reader for AppImage runtimes (F-04).
//!
//! An AppImage type 2 file is an ELF runtime with a SquashFS image glued to its
//! end. The image starts right after the ELF section header table:
//! `e_shoff + e_shentsize * e_shnum`. We only read bytes here; nothing is run.

use installer_domain::{Arch, InspectError, UnsupportedReason};

/// Bytes needed to read every field we use (64-bit header is 64 bytes).
pub const ELF_HEADER_LEN: usize = 64;

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const DT_NULL: u64 = 0;
const DT_NEEDED: u64 = 1;
const DT_STRTAB: u64 = 5;
/// Caps so a hostile header cannot make us loop or allocate much.
const MAX_DYNAMIC_ENTRIES: usize = 4096;
const MAX_NEEDED: usize = 64;
const MAX_LIB_NAME: usize = 256;
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

impl ElfHeader {
    /// Names listed as `DT_NEEDED` in the runtime's dynamic section (what the dynamic
    /// loader must find). A static runtime has none. `runtime` holds the bytes from file
    /// start to the SquashFS offset. Anything malformed ends the walk quietly.
    pub fn needed_libs(&self, runtime: &[u8]) -> Vec<String> {
        let rd = Reader {
            b: runtime,
            big_endian: self.big_endian,
        };
        let (min, word) = if self.is_64 {
            (56usize, 8usize)
        } else {
            (32, 4)
        };
        let entsize = usize::from(self.phentsize);
        let Ok(base) = usize::try_from(self.phoff) else {
            return Vec::new();
        };
        if entsize < min {
            return Vec::new();
        }
        // (offset, vaddr, filesz) of PT_LOAD segments, and the PT_DYNAMIC (offset, filesz).
        let mut loads: Vec<(u64, u64, u64)> = Vec::new();
        let mut dynamic: Option<(u64, u64)> = None;
        for i in 0..usize::from(self.phnum).min(MAX_PHDRS) {
            let Some(at) = i.checked_mul(entsize).and_then(|o| o.checked_add(base)) else {
                return Vec::new();
            };
            if at.checked_add(min).is_none_or(|end| end > runtime.len()) {
                return Vec::new();
            }
            let kind = rd.u32(at);
            let (off, vaddr, filesz) = if self.is_64 {
                (rd.u64(at + 8), rd.u64(at + 16), rd.u64(at + 32))
            } else {
                (
                    u64::from(rd.u32(at + 4)),
                    u64::from(rd.u32(at + 8)),
                    u64::from(rd.u32(at + 16)),
                )
            };
            match kind {
                PT_LOAD => loads.push((off, vaddr, filesz)),
                PT_DYNAMIC => dynamic = Some((off, filesz)),
                _ => {}
            }
        }
        let Some((dyn_off, dyn_size)) = dynamic else {
            return Vec::new();
        };
        let (Ok(dyn_off), Ok(dyn_size)) = (usize::try_from(dyn_off), usize::try_from(dyn_size))
        else {
            return Vec::new();
        };
        let mut strtab_vaddr: Option<u64> = None;
        let mut needed: Vec<u64> = Vec::new();
        for i in 0..(dyn_size / (2 * word)).min(MAX_DYNAMIC_ENTRIES) {
            let Some(at) = i.checked_mul(2 * word).and_then(|o| o.checked_add(dyn_off)) else {
                break;
            };
            if at
                .checked_add(2 * word)
                .is_none_or(|end| end > runtime.len())
            {
                break;
            }
            let (tag, val) = if self.is_64 {
                (rd.u64(at), rd.u64(at + 8))
            } else {
                (u64::from(rd.u32(at)), u64::from(rd.u32(at + 4)))
            };
            match tag {
                DT_NULL => break,
                DT_NEEDED if needed.len() < MAX_NEEDED => needed.push(val),
                DT_STRTAB => strtab_vaddr = Some(val),
                _ => {}
            }
        }
        let Some(strtab_vaddr) = strtab_vaddr else {
            return Vec::new();
        };
        // Virtual address to file offset through the PT_LOAD that contains it.
        let Some(strtab_off) = loads.iter().find_map(|&(off, vaddr, filesz)| {
            let delta = strtab_vaddr.checked_sub(vaddr)?;
            (delta < filesz).then(|| off.checked_add(delta)).flatten()
        }) else {
            return Vec::new();
        };
        needed
            .into_iter()
            .filter_map(|idx| {
                let start = usize::try_from(strtab_off.checked_add(idx)?).ok()?;
                let tail = runtime.get(start..)?;
                let tail = &tail[..tail.len().min(MAX_LIB_NAME)];
                let end = tail.iter().position(|&b| b == 0)?;
                Some(String::from_utf8_lossy(&tail[..end]).into_owned())
            })
            .collect()
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
    /// Section header table ends the returned vec, so its length is the SquashFS offset.
    pub fn elf_stub(is_64: bool, big: bool, machine: u16, ai: u8, dynamic: bool) -> Vec<u8> {
        build(is_64, big, machine, ai, dynamic.then_some(&[][..]))
    }

    /// Like `elf_stub` with a dynamic runtime whose `DT_NEEDED` entries are `needed`
    /// (real PT_INTERP, PT_LOAD, PT_DYNAMIC, string table and dynamic section).
    pub fn elf_stub_needed(
        is_64: bool,
        big: bool,
        machine: u16,
        ai: u8,
        needed: &[&str],
    ) -> Vec<u8> {
        build(is_64, big, machine, ai, Some(needed))
    }

    const LOAD_BASE: u64 = 0x40_0000;

    fn build(is_64: bool, big: bool, machine: u16, ai: u8, dynamic: Option<&[&str]>) -> Vec<u8> {
        let hdr = if is_64 { 64usize } else { 52 };
        let phent = if is_64 { 56usize } else { 32 };
        let shent = if is_64 { 64usize } else { 40 };
        let word = if is_64 { 8usize } else { 4 };
        let with_dyn = dynamic.is_some_and(|n| !n.is_empty());
        let phnum = match dynamic {
            None => 0usize,
            Some(_) if with_dyn => 3,
            Some(_) => 1,
        };
        let phoff = hdr;
        // dynstr: leading NUL, then each name.
        let mut dynstr = vec![0u8];
        let mut name_idx = Vec::new();
        for n in dynamic.unwrap_or(&[]) {
            name_idx.push(dynstr.len() as u64);
            dynstr.extend_from_slice(n.as_bytes());
            dynstr.push(0);
        }
        let dynstr_off = phoff + phnum * phent;
        let dyn_off = dynstr_off + dynstr.len();
        let dyn_entries = if with_dyn { name_idx.len() + 2 } else { 0 };
        let dyn_size = dyn_entries * 2 * word;
        let shoff = dyn_off + dyn_size;
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
        let put = |b: &mut [u8], at: usize, width: usize, v: u64| {
            let bytes = if big {
                v.to_be_bytes()[8 - width..].to_vec()
            } else {
                v.to_le_bytes()[..width].to_vec()
            };
            b[at..at + width].copy_from_slice(&bytes);
        };
        put(&mut b, 16, 2, 2);
        put(&mut b, 18, 2, u64::from(machine));
        if is_64 {
            if phnum > 0 {
                put(&mut b, 32, 8, phoff as u64);
            }
            put(&mut b, 40, 8, shoff as u64);
            put(&mut b, 52, 2, 64);
            put(&mut b, 54, 2, phent as u64);
            put(&mut b, 56, 2, phnum as u64);
            put(&mut b, 58, 2, shent as u64);
            put(&mut b, 60, 2, shnum as u64);
        } else {
            if phnum > 0 {
                put(&mut b, 28, 4, phoff as u64);
            }
            put(&mut b, 32, 4, shoff as u64);
            put(&mut b, 40, 2, 52);
            put(&mut b, 42, 2, phent as u64);
            put(&mut b, 44, 2, phnum as u64);
            put(&mut b, 46, 2, shent as u64);
            put(&mut b, 48, 2, shnum as u64);
        }
        // Program header writer: (type, offset, vaddr, filesz).
        let phdr = |b: &mut [u8], i: usize, kind: u64, off: u64, vaddr: u64, filesz: u64| {
            let at = phoff + i * phent;
            put(b, at, 4, kind);
            if is_64 {
                put(b, at + 8, 8, off);
                put(b, at + 16, 8, vaddr);
                put(b, at + 32, 8, filesz);
                put(b, at + 40, 8, filesz);
            } else {
                put(b, at + 4, 4, off);
                put(b, at + 8, 4, vaddr);
                put(b, at + 16, 4, filesz);
                put(b, at + 20, 4, filesz);
            }
        };
        if phnum > 0 {
            phdr(&mut b, 0, 3, 0, 0, 0); // PT_INTERP
        }
        if with_dyn {
            phdr(&mut b, 1, 1, 0, LOAD_BASE, total as u64); // PT_LOAD over the whole stub
            phdr(
                &mut b,
                2,
                2,
                dyn_off as u64,
                LOAD_BASE + dyn_off as u64,
                dyn_size as u64,
            );
            b[dynstr_off..dynstr_off + dynstr.len()].copy_from_slice(&dynstr);
            let mut at = dyn_off;
            for idx in &name_idx {
                put(&mut b, at, word, 1); // DT_NEEDED
                put(&mut b, at + word, word, *idx);
                at += 2 * word;
            }
            put(&mut b, at, word, 5); // DT_STRTAB
            put(&mut b, at + word, word, LOAD_BASE + dynstr_off as u64);
            // trailing DT_NULL is already zero
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
    fn needed_libs_all_layouts() {
        for (is_64, big) in [(true, false), (true, true), (false, false), (false, true)] {
            let b = testutil::elf_stub_needed(is_64, big, 62, 2, &["libfuse.so.2", "libc.so.6"]);
            let h = ElfHeader::parse(&b).unwrap();
            assert_eq!(
                h.needed_libs(&b),
                ["libfuse.so.2", "libc.so.6"],
                "{is_64} {big}"
            );
            let none = testutil::elf_stub_needed(is_64, big, 62, 2, &[]);
            let h = ElfHeader::parse(&none).unwrap();
            assert!(h.needed_libs(&none).is_empty());
            assert!(h.is_dynamic(&none));
        }
    }

    #[test]
    fn needed_libs_survives_garbage() {
        let b = testutil::elf_stub_needed(true, false, 62, 2, &["libfuse.so.2"]);
        let h = ElfHeader::parse(&b).unwrap();
        // Truncations never panic.
        for cut in 0..b.len() {
            let _ = h.needed_libs(&b[..cut]);
        }
        // Every single-byte corruption never panics.
        for i in 0..b.len() {
            let mut c = b.clone();
            c[i] = 0xff;
            let _ = h.needed_libs(&c);
        }
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
