//! Fabric extended semantic versions and dependency predicates. No filename
//! interpretation. Space-separated terms are AND; metadata arrays are OR.
use std::cmp::Ordering;

#[derive(Debug)]
struct Version {
    components: Vec<u32>,
    prerelease: Option<String>,
}

impl Version {
    fn parse(raw: &str) -> Option<Self> {
        let raw = raw.split('+').next()?;
        let (numbers, pre) = raw
            .split_once('-')
            .map_or((raw, None), |(n, p)| (n, Some(p)));
        let components: Vec<u32> = numbers
            .split('.')
            .map(|part| {
                if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                    None
                } else {
                    part.parse::<u32>().ok().filter(|n| *n <= i32::MAX as u32)
                }
            })
            .collect::<Option<_>>()?;
        if components.is_empty()
            || pre.is_some_and(|p| {
                !p.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            })
        {
            return None;
        }
        Some(Self {
            components,
            prerelease: pre.map(str::to_owned),
        })
    }
    fn compare(&self, other: &Self) -> Ordering {
        for i in 0..self.components.len().max(other.components.len()) {
            let order = self
                .components
                .get(i)
                .unwrap_or(&0)
                .cmp(other.components.get(i).unwrap_or(&0));
            if order != Ordering::Equal {
                return order;
            }
        }
        match (&self.prerelease, &other.prerelease) {
            (None, None) => Ordering::Equal,
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(a), Some(b)) => {
                if a.is_empty() || b.is_empty() {
                    return a.is_empty().cmp(&b.is_empty()).reverse();
                }
                let mut aa = a.split('.');
                let mut bb = b.split('.');
                loop {
                    match (aa.next(), bb.next()) {
                        (None, None) => return Ordering::Equal,
                        (None, Some(_)) => return Ordering::Less,
                        (Some(_), None) => return Ordering::Greater,
                        (Some(a), Some(b)) => {
                            let an = a.bytes().all(|b| b.is_ascii_digit());
                            let bn = b.bytes().all(|b| b.is_ascii_digit());
                            let order = match (an, bn) {
                                (true, true) => a
                                    .trim_start_matches('0')
                                    .len()
                                    .cmp(&b.trim_start_matches('0').len())
                                    .then_with(|| {
                                        a.trim_start_matches('0').cmp(b.trim_start_matches('0'))
                                    }),
                                (true, false) => Ordering::Less,
                                (false, true) => Ordering::Greater,
                                _ => a.cmp(b),
                            };
                            if order != Ordering::Equal {
                                return order;
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Unsupported/malformed expressions return Err; callers must not interpret
/// uncertainty as satisfaction. The scanner's display OR delimiter is used
/// only for normalized metadata arrays, never a shell or executable expression.
pub fn satisfies(version: &str, requirement: &str) -> Result<bool, &'static str> {
    if version.is_empty() || version.len() > 256 || requirement.len() > 4096 {
        return Err("invalid version predicate");
    }
    let mut any = false;
    for alternative in requirement.split(" or ") {
        let mut all = true;
        for term in alternative.split(' ').filter(|v| !v.is_empty()) {
            if term == "*" {
                continue;
            }
            let (operator, raw) = [">=", "<=", ">", "<", "=", "~", "^"]
                .into_iter()
                .find_map(|op| term.strip_prefix(op).map(|s| (op, s)))
                .unwrap_or(("=", term));
            if raw.is_empty() {
                return Err("empty version predicate");
            }
            if raw.split('.').any(|v| matches!(v, "x" | "X" | "*")) {
                if operator != "=" || raw.contains(['-', '+']) {
                    return Err("invalid wildcard predicate");
                }
                let parts: Vec<_> = raw.split('.').collect();
                let first = parts
                    .iter()
                    .position(|v| matches!(*v, "x" | "X" | "*"))
                    .unwrap();
                if parts[first..]
                    .iter()
                    .any(|v| !matches!(*v, "x" | "X" | "*"))
                {
                    return Err("invalid wildcard predicate");
                }
                let prefix: Vec<u32> = parts[..first]
                    .iter()
                    .map(|v| v.parse().map_err(|_| "invalid wildcard predicate"))
                    .collect::<Result<_, _>>()?;
                let actual = Version::parse(version).ok_or("non-semantic wildcard version")?;
                all &= prefix
                    .iter()
                    .enumerate()
                    .all(|(i, n)| actual.components.get(i).unwrap_or(&0) == n);
                continue;
            }
            let matches = match (Version::parse(version), Version::parse(raw)) {
                (Some(actual), Some(expected)) => {
                    let order = actual.compare(&expected);
                    match operator {
                        "=" => order == Ordering::Equal,
                        ">=" => order != Ordering::Less,
                        "<=" => order != Ordering::Greater,
                        ">" => order == Ordering::Greater,
                        "<" => order == Ordering::Less,
                        "~" | "^" => {
                            let mut upper = Version {
                                components: expected.components.clone(),
                                prerelease: Some(String::new()),
                            };
                            let index = if operator == "^" { 0 } else { 1 };
                            upper
                                .components
                                .resize(upper.components.len().max(index + 1), 0);
                            upper.components[index] = upper.components[index]
                                .checked_add(1)
                                .ok_or("version overflow")?;
                            upper.components.truncate(index + 1);
                            order != Ordering::Less && actual.compare(&upper) == Ordering::Less
                        }
                        _ => return Err("unsupported predicate"),
                    }
                }
                (_, None) if matches!(operator, "=" | ">=" | "<=" | "~" | "^") => version == raw,
                _ => return Err("non-semantic version range"),
            };
            all &= matches;
        }
        any |= all;
    }
    Ok(any)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fabric_predicates() {
        for (v, p, wanted) in [
            ("2.8.2+1.21.10", ">=2.8.0", true),
            ("2.7.9", ">=2.8.0", false),
            ("1.21.11", ">=1.21.11- <1.21.12-", true),
            ("1.21.12-beta.1", ">=1.21.11- <1.21.12-", false),
            ("0.8.14+mc1.21.11", "=0.8.14+ignored", true),
            ("1.21.11", "1.21.x", true),
            ("1.21.11-beta.1", "1.21.x", true),
            ("1.22", "1.21.x", false),
            ("1.2.3.4", "1.2.3.x", true),
            ("1.2.4", "1.2.3.x", false),
            ("1.3", "~1.2", false),
            ("1.2.9", "~1.2", true),
            ("0.9", "^0.2", true),
            ("1.2.0-beta.2", ">1.2.0-beta.1 <1.2", true),
            ("1.2", ">1.2.0-beta.1 <1.2", false),
            ("1.2", "1.1 or >=1.2", true),
            ("release-build", "release-build", true),
            ("1.2.0", "1.2", true),
            ("1.2.0-alpha.10", ">1.2.0-alpha.2", true),
        ] {
            assert_eq!(satisfies(v, p), Ok(wanted), "{v} {p}");
        }
        assert!(satisfies("1.2", ">=1.x").is_err());
        assert!(satisfies("build", ">other").is_err());
    }
}
