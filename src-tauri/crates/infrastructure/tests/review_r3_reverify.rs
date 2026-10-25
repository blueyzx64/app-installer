//! R3 re-verify (swarm/R3/reverify a478c71): rpm header sizes must not drive allocation (D-039).
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use installer_domain::InspectError;
use installer_infrastructure::inspect::rpm::RpmInspector;

/// Records the largest single allocation request.
struct Peak;
static PEAK: AtomicUsize = AtomicUsize::new(0);
unsafe impl GlobalAlloc for Peak {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        PEAK.fetch_max(l.size(), Ordering::Relaxed);
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        PEAK.fetch_max(l.size(), Ordering::Relaxed);
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static A: Peak = Peak;

/// 96-byte lead + a signature header index that claims `entries` / `data` sizes.
fn tiny_rpm(entries: u32, data: u32) -> Vec<u8> {
    let mut lead = vec![0u8; 96];
    lead[..4].copy_from_slice(&[0xED, 0xAB, 0xEE, 0xDB]);
    lead[4] = 3;
    lead[78..80].copy_from_slice(&1u16.to_be_bytes()); // os
    lead[80..82].copy_from_slice(&5u16.to_be_bytes()); // sig type
    let mut h = vec![0x8e, 0xad, 0xe8, 1, 0, 0, 0, 0];
    h.extend(entries.to_be_bytes());
    h.extend(data.to_be_bytes());
    lead.extend(h);
    lead
}

fn tmpfile(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("r3rv-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

/// HEADER_BYTES_MAX (64 MiB) wraps the reader, but rpm 0.30 does
/// `vec![0; data_section_size + num_entries*16]` from the claimed sizes BEFORE reading.
/// A 112-byte file makes us request ~4 GiB. With no swap / strict overcommit that
/// allocation fails and Rust aborts the whole app.
#[test]
fn review_r3rv_rpm_header_cap_does_not_bound_allocation() {
    let p = tmpfile("alloc.rpm", &tiny_rpm(0, 0xFFFF_FF00));
    assert_eq!(std::fs::metadata(&p).unwrap().len(), 112);
    PEAK.store(0, Ordering::Relaxed);
    let r = RpmInspector::inspect_path(&p, false);
    let peak = PEAK.load(Ordering::Relaxed);
    assert_eq!(r.unwrap_err(), InspectError::Corrupt);
    assert!(
        peak <= 128 << 20,
        "112-byte file caused a {} MiB allocation (cap is 64 MiB)",
        peak >> 20
    );
}

/// num_entries * 16 overflows u32 inside the rpm crate: debug builds panic
/// (spawn_blocking -> JoinError -> InspectError::Io -> ErrorCode::Unknown, not corruptFile).
#[test]
fn review_r3rv_rpm_header_entry_count_overflow() {
    let p = tmpfile("ovf.rpm", &tiny_rpm(0x1000_0001, 0));
    let r = std::panic::catch_unwind(|| RpmInspector::inspect_path(&p, false));
    assert!(r.is_ok(), "inspector panicked on entry-count overflow");
    assert_eq!(r.unwrap().unwrap_err(), InspectError::Corrupt);
}
