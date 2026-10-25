//! Package versions with an rpmvercmp-compatible ordering.

use std::cmp::Ordering;
use std::fmt;

use serde::{Deserialize, Serialize};

/// A package version string, e.g. `1.2.3`, `2:1.0-3.fc45` or `1.0~rc1`.
///
/// Ordering follows rpm: optional `epoch:`, then version, then `-release`,
/// each compared with `rpmvercmp`. Equality is defined by that ordering, so
/// `1.0` equals `1_0`. (No `Hash` impl for that reason.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Version(String);

impl Version {
    pub fn new(raw: impl Into<String>) -> Self {
        Self(raw.into().trim().to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Version {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Version {}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        let (e1, v1, r1) = split_evr(&self.0);
        let (e2, v2, r2) = split_evr(&other.0);
        e1.cmp(&e2)
            .then_with(|| rpmvercmp(v1, v2))
            .then_with(|| match (r1, r2) {
                // rpm: a missing release matches anything.
                (Some(a), Some(b)) => rpmvercmp(a, b),
                _ => Ordering::Equal,
            })
    }
}

/// Split `[epoch:]version[-release]`. A non-numeric epoch is treated as part of the version.
fn split_evr(s: &str) -> (u64, &str, Option<&str>) {
    let (epoch, rest) = match s.split_once(':') {
        Some((e, rest)) if !e.is_empty() && e.bytes().all(|b| b.is_ascii_digit()) => {
            (e.parse().unwrap_or(u64::MAX), rest)
        }
        _ => (0, s),
    };
    match rest.rsplit_once('-') {
        Some((v, r)) => (epoch, v, Some(r)),
        None => (epoch, rest, None),
    }
}

fn is_sep(b: u8) -> bool {
    !b.is_ascii_alphanumeric() && b != b'~' && b != b'^'
}

/// Port of rpm's `rpmvercmp`.
pub fn rpmvercmp(a: &str, b: &str) -> Ordering {
    if a == b {
        return Ordering::Equal;
    }
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        while i < a.len() && is_sep(a[i]) {
            i += 1;
        }
        while j < b.len() && is_sep(b[j]) {
            j += 1;
        }
        let (ca, cb) = (a.get(i).copied(), b.get(j).copied());

        // Tilde sorts before everything, even the end of the string.
        if ca == Some(b'~') || cb == Some(b'~') {
            if ca != Some(b'~') {
                return Ordering::Greater;
            }
            if cb != Some(b'~') {
                return Ordering::Less;
            }
            i += 1;
            j += 1;
            continue;
        }
        // Caret sorts after the end of the string but before anything else.
        if ca == Some(b'^') || cb == Some(b'^') {
            if ca.is_none() {
                return Ordering::Less;
            }
            if cb.is_none() {
                return Ordering::Greater;
            }
            if ca != Some(b'^') {
                return Ordering::Greater;
            }
            if cb != Some(b'^') {
                return Ordering::Less;
            }
            i += 1;
            j += 1;
            continue;
        }
        let (Some(ca), Some(_)) = (ca, cb) else { break };

        let numeric = ca.is_ascii_digit();
        let pred: fn(&u8) -> bool = if numeric {
            |c| c.is_ascii_digit()
        } else {
            |c| c.is_ascii_alphabetic()
        };
        let si = i;
        while i < a.len() && pred(&a[i]) {
            i += 1;
        }
        let sj = j;
        while j < b.len() && pred(&b[j]) {
            j += 1;
        }
        let (mut seg_a, mut seg_b) = (&a[si..i], &b[sj..j]);
        if seg_b.is_empty() {
            // Segments of different kinds: numbers beat letters.
            return if numeric {
                Ordering::Greater
            } else {
                Ordering::Less
            };
        }
        if numeric {
            while seg_a.first() == Some(&b'0') {
                seg_a = &seg_a[1..];
            }
            while seg_b.first() == Some(&b'0') {
                seg_b = &seg_b[1..];
            }
            if seg_a.len() != seg_b.len() {
                return seg_a.len().cmp(&seg_b.len());
            }
        }
        match seg_a.cmp(seg_b) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    match (i < a.len(), j < b.len()) {
        (false, false) => Ordering::Equal,
        (true, _) => Ordering::Greater,
        (false, true) => Ordering::Less,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Ordering::{Equal, Greater, Less};

    fn v(s: &str) -> Version {
        Version::new(s)
    }

    #[test]
    fn rpmvercmp_table() {
        // (left, right, left.cmp(right)) mirrors rpm's own test-suite rows.
        let rows: &[(&str, &str, Ordering)] = &[
            ("1.0", "1.0", Equal),
            ("1.0", "2.0", Less),
            ("2.0", "1.0", Greater),
            ("2.0.1", "2.0.1", Equal),
            ("2.0", "2.0.1", Less),
            ("2.0.1", "2.0", Greater),
            ("5.5p1", "5.5p1", Equal),
            ("5.5p1", "5.5p2", Less),
            ("5.5p2", "5.5p1", Greater),
            ("5.5p10", "5.5p10", Equal),
            ("5.5p1", "5.5p10", Less),
            ("5.5p10", "5.5p1", Greater),
            ("10xyz", "10.1xyz", Less),
            ("10.1xyz", "10xyz", Greater),
            ("xyz10", "xyz10", Equal),
            ("xyz10", "xyz10.1", Less),
            ("xyz.4", "xyz.4", Equal),
            ("xyz.4", "8", Less),
            ("8", "xyz.4", Greater),
            ("xyz.4", "2", Less),
            ("5.5p2", "5.6p1", Less),
            ("5.6p1", "5.5p2", Greater),
            ("5.6p1", "6.5p1", Less),
            ("6.5p1", "5.6p1", Greater),
            ("6.0.rc1", "6.0", Greater),
            ("6.0", "6.0.rc1", Less),
            ("10b2", "10a1", Greater),
            ("10a2", "10b2", Less),
            ("1.0aa", "1.0aa", Equal),
            ("1.0a", "1.0aa", Less),
            ("1.0aa", "1.0a", Greater),
            ("10.0001", "10.0001", Equal),
            ("10.0001", "10.1", Equal),
            ("10.1", "10.0001", Equal),
            ("10.0001", "10.0039", Less),
            ("10.0039", "10.0001", Greater),
            ("4.999.9", "5.0", Less),
            ("5.0", "4.999.9", Greater),
            ("20101121", "20101121", Equal),
            ("20101121", "20101122", Less),
            ("20101122", "20101121", Greater),
            ("2_0", "2_0", Equal),
            ("2.0", "2_0", Equal),
            ("2_0", "2.0", Equal),
            ("a", "a", Equal),
            ("a+", "a+", Equal),
            ("a+", "a_", Equal),
            ("a_", "a+", Equal),
            ("+a", "+a", Equal),
            ("+a", "_a", Equal),
            ("+_", "+_", Equal),
            ("_+", "+_", Equal),
            ("+", "_", Equal),
            ("1.0~rc1", "1.0~rc1", Equal),
            ("1.0~rc1", "1.0", Less),
            ("1.0", "1.0~rc1", Greater),
            ("1.0~rc1", "1.0~rc2", Less),
            ("1.0~rc2", "1.0~rc1", Greater),
            ("1.0~rc1~git123", "1.0~rc1~git123", Equal),
            ("1.0~rc1~git123", "1.0~rc1", Less),
            ("1.0~rc1", "1.0~rc1~git123", Greater),
            ("1.0^", "1.0^", Equal),
            ("1.0^", "1.0", Greater),
            ("1.0", "1.0^", Less),
            ("1.0^git1", "1.0^git1", Equal),
            ("1.0^git1", "1.0", Greater),
            ("1.0", "1.0^git1", Less),
            ("1.0^git1", "1.0^git2", Less),
            ("1.0^git2", "1.0^git1", Greater),
            ("1.0^git1", "1.01", Less),
            ("1.01", "1.0^git1", Greater),
            ("1.0^20160101", "1.0^20160101", Equal),
            ("1.0^20160101", "1.0.1", Less),
            ("1.0.1", "1.0^20160101", Greater),
            ("1.0^20160101^git1", "1.0^20160101^git1", Equal),
            ("1.0^20160102", "1.0^20160101^git1", Greater),
            ("1.0~rc1^git1", "1.0~rc1", Greater),
            ("1.0~rc1", "1.0~rc1^git1", Less),
            ("1.0^git1~pre", "1.0^git1", Less),
            ("1.0^git1", "1.0^git1~pre", Greater),
        ];
        for (a, b, want) in rows {
            assert_eq!(rpmvercmp(a, b), *want, "rpmvercmp({a:?}, {b:?})");
            assert_eq!(rpmvercmp(b, a), want.reverse(), "rpmvercmp({b:?}, {a:?})");
        }
    }

    #[test]
    fn epoch_beats_version() {
        assert!(v("1:1.0") > v("9.9"));
        assert!(v("2:0.1") > v("1:9.9"));
        assert_eq!(v("0:1.0"), v("1.0"));
    }

    #[test]
    fn release_compared_when_both_present() {
        assert!(v("1.0-2.fc45") > v("1.0-1.fc45"));
        assert!(v("1.0-10") > v("1.0-9"));
        assert_eq!(v("1.0"), v("1.0-3"), "missing release matches any");
        assert!(v("1.1") > v("1.0-9"));
    }

    #[test]
    fn eq_is_consistent_with_ord() {
        assert_eq!(v("1.0"), v("1_0"));
        assert_eq!(v("1.0").cmp(&v("1_0")), Equal);
        assert_eq!(v(" 1.0 "), v("1.0"), "trimmed");
    }

    #[test]
    fn serde_is_plain_string() {
        let s = serde_json::to_string(&v("1.2.3")).unwrap();
        assert_eq!(s, "\"1.2.3\"");
        let back: Version = serde_json::from_str(&s).unwrap();
        assert_eq!(back.as_str(), "1.2.3");
    }
}

#[cfg(test)]
mod prop {
    use super::*;
    use proptest::prelude::*;

    fn any_version() -> impl Strategy<Value = String> {
        "[0-9a-z.~^_+-]{0,12}"
    }

    fn evr_part() -> impl Strategy<Value = String> {
        "[0-9a-z.~^_-]{1,12}"
    }

    proptest! {
        #[test]
        fn prop_antisymmetric(a in any_version(), b in any_version()) {
            let (va, vb) = (Version::new(a), Version::new(b));
            prop_assert_eq!(va.cmp(&vb), vb.cmp(&va).reverse());
        }

        #[test]
        fn prop_reflexive(a in any_version()) {
            let va = Version::new(a);
            prop_assert_eq!(&va, &va);
            prop_assert_eq!(va.cmp(&va), Ordering::Equal);
        }

        #[test]
        fn prop_transitive(a in any_version(), b in any_version(), c in any_version()) {
            let (va, vb, vc) = (Version::new(a), Version::new(b), Version::new(c));
            if va <= vb && vb <= vc {
                prop_assert!(va <= vc);
            }
        }

        #[test]
        fn prop_epoch_dominates(x in evr_part(), y in evr_part()) {
            let newer = Version::new(format!("1:{x}"));
            let older = Version::new(format!("0:{y}"));
            prop_assert!(newer > older, "{newer} should beat {older}");
        }

        #[test]
        fn prop_appending_dot_one_is_greater(parts in proptest::collection::vec(0u32..1_000_000, 1..=4)) {
            let base = parts.iter().map(u32::to_string).collect::<Vec<_>>().join(".");
            let longer = format!("{base}.1");
            prop_assert!(Version::new(longer.clone()) > Version::new(base.clone()), "{longer} vs {base}");
        }
    }
}
