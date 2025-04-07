use http::{header::GetAll, HeaderValue};

/// Given a list of header values (which may be comma-separated and may have quality parameters)
/// and a list of supported items (each supported item is a full value or a prefix ending with '*'),
/// return Some(supported_item) for the first supported item that matches any header value, or None.
pub fn find_first_supported<'a, I>(header_values: &[HeaderValue], supported: I) -> Option<&'a str>
where
    I: IntoIterator<Item = &'a str> + Clone,
{
    // best candidate: (quality, supported_index, candidate)
    let mut best: Option<(f32, usize, &'a str)> = None;

    for value in header_values.iter() {
        if let Ok(s) = value.to_str() {
            for token in s.split(',') {
                let token = token.trim();
                if token.is_empty() {
                    continue;
                }
                let mut parts = token.split(';');
                let enc = parts.next()?.trim();
                if enc.is_empty() {
                    continue;
                }
                let quality = parts
                    .find_map(|p| {
                        let p = p.trim();
                        if let Some(q_str) = p.strip_prefix("q=") {
                            q_str.parse::<f32>().ok()
                        } else {
                            None
                        }
                    })
                    .unwrap_or(1.0);

                // For each supported encoding, iterate over a clone of the iterable.
                for (i, supp) in supported.clone().into_iter().enumerate() {
                    let is_match = if supp == "*" {
                        true
                    } else if let Some(prefix) = supp.strip_suffix('*') {
                        enc.starts_with(prefix)
                    } else {
                        enc.eq_ignore_ascii_case(supp)
                    };

                    if is_match {
                        best = match best {
                            Some((best_q, best_idx, _))
                                if quality > best_q || (quality == best_q && i < best_idx) =>
                            {
                                Some((quality, i, supp))
                            }
                            None => Some((quality, i, supp)),
                            _ => best,
                        };
                    }
                }
            }
        }
    }

    best.map(|(_, _, candidate)| candidate)
}

pub fn header_contains(header_values: &GetAll<HeaderValue>, needle: &str) -> bool {
    if needle == "*" {
        return true;
    }
    let mut headers = header_values
        .iter()
        .flat_map(|value| value.to_str().unwrap_or("").split(','))
        .map(|s| s.trim().split(';').next().unwrap_or(""))
        .filter(|s| !s.is_empty());

    let needle_lower = needle;
    if needle.ends_with('*') {
        let prefix = &needle_lower[..needle_lower.len() - 1];
        headers.any(|h| h.starts_with(prefix))
    } else {
        headers.any(|h| h == needle_lower)
    }
}
