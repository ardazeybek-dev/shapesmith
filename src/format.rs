//! String format detection.
//!
//! Every detector is deliberately *stricter* than the validators the generated
//! code relies on (Zod v4 and ajv-formats): a format is only claimed when every
//! observed value would pass those validators, so the emitted schema never
//! rejects the samples it was inferred from.

pub const DATE_TIME: u8 = 1 << 0;
pub const DATE: u8 = 1 << 1;
pub const EMAIL: u8 = 1 << 2;
pub const UUID: u8 = 1 << 3;
pub const URL: u8 = 1 << 4;
pub const ALL: u8 = DATE_TIME | DATE | EMAIL | UUID | URL;

/// Returns the set of formats `s` satisfies, as a bit mask.
pub fn detect(s: &str) -> u8 {
    let b = s.as_bytes();
    let mut flags = 0;
    if is_date_time(b) {
        flags |= DATE_TIME;
    }
    if is_date(b) {
        flags |= DATE;
    }
    if is_email(b) {
        flags |= EMAIL;
    }
    if is_uuid(b) {
        flags |= UUID;
    }
    if is_url(b) {
        flags |= URL;
    }
    flags
}

/// Picks the single format named by a mask, if exactly one bit is set.
pub fn single(mask: u8) -> Option<u8> {
    (mask.count_ones() == 1).then_some(mask)
}

fn num(b: &[u8]) -> Option<u32> {
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(b.iter().fold(0, |n, d| n * 10 + u32::from(d - b'0')))
}

fn days_in_month(year: u32, month: u32) -> u32 {
    match month {
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// `YYYY-MM-DD` with a real calendar day.
fn is_date(b: &[u8]) -> bool {
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    match (num(&b[0..4]), num(&b[5..7]), num(&b[8..10])) {
        (Some(y), Some(m), Some(d)) => (1..=12).contains(&m) && d >= 1 && d <= days_in_month(y, m),
        _ => false,
    }
}

/// `YYYY-MM-DDTHH:MM:SS[.fraction](Z|±HH:MM)`
fn is_date_time(b: &[u8]) -> bool {
    if b.len() < 20 || !is_date(&b[..10]) || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return false;
    }
    let time_ok = matches!(
        (num(&b[11..13]), num(&b[14..16]), num(&b[17..19])),
        (Some(h), Some(m), Some(s)) if h < 24 && m < 60 && s < 60
    );
    if !time_ok {
        return false;
    }
    let mut rest = &b[19..];
    if rest.first() == Some(&b'.') {
        let digits = rest[1..].iter().take_while(|c| c.is_ascii_digit()).count();
        if digits == 0 {
            return false;
        }
        rest = &rest[1 + digits..];
    }
    match rest {
        [b'Z'] => true,
        [b'+' | b'-', h1, h2, b':', m1, m2] => matches!(
            (num(&[*h1, *h2]), num(&[*m1, *m2])),
            (Some(h), Some(m)) if h < 24 && m < 60
        ),
        _ => false,
    }
}

fn is_domain(b: &[u8]) -> bool {
    let labels: Vec<&[u8]> = b.split(|&c| c == b'.').collect();
    if labels.len() < 2 {
        return false;
    }
    let label_ok = |l: &[u8]| {
        !l.is_empty()
            && l.len() <= 63
            && l[0].is_ascii_alphanumeric()
            && l[l.len() - 1].is_ascii_alphanumeric()
            && l.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-')
    };
    let tld = labels[labels.len() - 1];
    labels.iter().all(|l| label_ok(l)) && tld.len() >= 2 && tld.iter().all(u8::is_ascii_alphabetic)
}

/// A conservative subset of addresses that Zod and ajv-formats both accept.
fn is_email(b: &[u8]) -> bool {
    let Some(at) = b.iter().position(|&c| c == b'@') else {
        return false;
    };
    let (local, domain) = (&b[..at], &b[at + 1..]);
    !local.is_empty()
        && local.len() <= 64
        && local[0] != b'.'
        && local[local.len() - 1] != b'.'
        && !local.windows(2).any(|w| w == b"..")
        && local.iter().all(|c| c.is_ascii_alphanumeric() || b"._+-".contains(c))
        && is_domain(domain)
}

/// RFC 9562 UUID: version nibble 1-8, variant 10xx.
fn is_uuid(b: &[u8]) -> bool {
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            14 => (b'1'..=b'8').contains(c),
            19 => b"89abAB".contains(c),
            _ => c.is_ascii_hexdigit(),
        })
}

/// `http(s)://host[:port][/path][?query][#fragment]` using only RFC 3986 characters.
fn is_url(b: &[u8]) -> bool {
    let rest = if let Some(r) = b.strip_prefix(b"https://") {
        r
    } else if let Some(r) = b.strip_prefix(b"http://") {
        r
    } else {
        return false;
    };
    let host_end = rest.iter().position(|c| b"/?#".contains(c)).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(host_end);
    let (host, port) = match authority.iter().position(|&c| c == b':') {
        Some(i) => (&authority[..i], Some(&authority[i + 1..])),
        None => (authority, None),
    };
    if port.is_some_and(|p| num(p).is_none_or(|n| n > 65535)) {
        return false;
    }
    let host_ok = host == b"localhost" || is_domain(host);
    host_ok && is_uri_tail(tail)
}

fn is_uri_tail(b: &[u8]) -> bool {
    let mut i = 0;
    let mut seen_hash = false;
    while i < b.len() {
        let c = b[i];
        if c == b'%' {
            if i + 2 >= b.len() || !b[i + 1].is_ascii_hexdigit() || !b[i + 2].is_ascii_hexdigit() {
                return false;
            }
            i += 3;
            continue;
        }
        if c == b'#' {
            if seen_hash {
                return false;
            }
            seen_hash = true;
        } else if !(c.is_ascii_alphanumeric() || b"-._~:/?@!$&'()*+,;=".contains(&c)) {
            return false;
        }
        i += 1;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(detect("2024-02-29"), DATE);
        assert_eq!(detect("2023-02-29"), 0, "not a leap year");
        assert_eq!(detect("2024-13-01"), 0);
        assert_eq!(detect("2024-1-01"), 0);
    }

    #[test]
    fn date_times() {
        for ok in ["2024-05-01T10:00:00Z", "2024-05-01T10:00:00.123Z", "2024-05-01T23:59:59+03:00"] {
            assert_eq!(detect(ok), DATE_TIME, "{ok}");
        }
        for bad in ["2024-05-01 10:00:00Z", "2024-05-01T10:00:00", "2024-05-01T24:00:00Z", "2024-05-01T10:00:00.Z"] {
            assert_eq!(detect(bad), 0, "{bad}");
        }
    }

    #[test]
    fn emails() {
        assert_eq!(detect("ada.lovelace+test@mail.example.com"), EMAIL);
        for bad in ["a@b", "a..b@x.com", ".a@x.com", "a@-x.com", "a b@x.com", "a@x.c0m", "a%b@x.com"] {
            assert_eq!(detect(bad), 0, "{bad}");
        }
    }

    #[test]
    fn uuids() {
        assert_eq!(detect("550e8400-e29b-41d4-a716-446655440000"), UUID);
        assert_eq!(detect("550e8400-e29b-01d4-a716-446655440000"), 0, "version 0");
        assert_eq!(detect("550e8400-e29b-41d4-c716-446655440000"), 0, "bad variant");
    }

    #[test]
    fn urls() {
        for ok in ["https://example.com", "http://localhost:8080/a/b?q=1#top", "https://x.io/%20a"] {
            assert_eq!(detect(ok), URL, "{ok}");
        }
        for bad in ["ftp://x.com", "https://", "https://exa mple.com", "https://x.com/a b", "https://x.com/%zz"] {
            assert_eq!(detect(bad), 0, "{bad}");
        }
    }
}
