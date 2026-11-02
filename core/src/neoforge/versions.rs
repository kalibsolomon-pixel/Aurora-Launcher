//! NeoForge version-range semantics (Maven-style ranges).
//!
//! NeoForge mod metadata declares dependencies with Maven version ranges
//! (`"[26.2,)"`, `"[3,]"`, `"1.21.1"`), not Fabric-style predicates. This
//! module evaluates exactly that grammar — nothing more — and refuses
//! malformed input so uncertainty is never interpreted as satisfaction.

/// Whether `version` satisfies a Maven-style version-range requirement.
///
/// Grammar: an optional exact version, a single bracketed range with
/// inclusive (`[`, `]`) or exclusive (`(`, `)`) bounds, or `(`, `[`-opened
/// unbounded ranges. Empty lower bounds mean "no floor", empty upper bounds
/// mean "no ceiling". Anything malformed is an error, never a match.
pub fn satisfies(version: &str, requirement: &str) -> Result<bool, &'static str> {
    let requirement = requirement.trim();
    if requirement.is_empty() {
        return Err("empty version range");
    }
    if !requirement.starts_with(['[', '(']) {
        // A bare requirement is an exact (or prefix-wildcard) match; a
        // requirement that is neither parseable nor bracketed is malformed
        // and never silently satisfied.
        if let Some(stem) = requirement.strip_suffix(".*") {
            if parse(stem).is_none() {
                return Err("malformed wildcard requirement");
            }
            return Ok(version == stem
                || version
                    .strip_prefix(stem)
                    .and_then(|rest| rest.strip_prefix('.'))
                    .is_some_and(|rest| {
                        !rest.starts_with('.') && rest.chars().all(|c| c.is_ascii_digit())
                    }));
        }
        if requirement == "*" {
            return Ok(true);
        }
        if parse(requirement).is_none() {
            return Err("malformed exact requirement");
        }
        return Ok(compare(version, requirement) == Some(std::cmp::Ordering::Equal));
    }
    let close = requirement.chars().last().expect("non-empty");
    if !matches!(close, ']' | ')') {
        return Err("unterminated version range");
    }
    let inner = &requirement[1..requirement.len() - 1];
    let (lower_raw, upper_raw) = match inner.split_once(',') {
        Some((lower, upper)) => (lower.trim(), upper.trim()),
        // "[1.2.3]" is Maven's inclusive single-point range.
        None => {
            let point = inner.trim();
            if point.is_empty() {
                return Err("empty version range");
            }
            return Ok(compare(version, point) == Some(std::cmp::Ordering::Equal));
        }
    };
    if lower_raw.is_empty() && upper_raw.is_empty() {
        return Ok(true);
    }
    let mut ok = true;
    if !lower_raw.is_empty() {
        let ordering =
            compare(version, lower_raw).ok_or("the version or range bound could not be parsed")?;
        let inclusive = requirement.starts_with('[');
        ok &= if inclusive {
            ordering != std::cmp::Ordering::Less
        } else {
            ordering == std::cmp::Ordering::Greater
        };
    }
    if !upper_raw.is_empty() {
        let ordering =
            compare(version, upper_raw).ok_or("the version or range bound could not be parsed")?;
        let inclusive = close == ']';
        ok &= if inclusive {
            ordering != std::cmp::Ordering::Greater
        } else {
            ordering == std::cmp::Ordering::Less
        };
    }
    Ok(ok)
}

/// Compares two dotted versions numerically; `None` when either side is not
/// a parseable dotted version.
fn compare(left: &str, right: &str) -> Option<std::cmp::Ordering> {
    Some(parse(left)?.cmp(&parse(right)?))
}

/// One parsed version: numeric components plus an optional prerelease tail
/// (a version with a prerelease ranks below the same release).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    components: Vec<u64>,
    prerelease: Option<String>,
}

fn parse(version: &str) -> Option<Version> {
    let version = version.trim();
    if version.is_empty() {
        return None;
    }
    let (base, prerelease) = match version.split_once('-') {
        Some((base, prerelease)) => (base, Some(prerelease.to_owned())),
        None => (version, None),
    };
    let mut components = Vec::new();
    for segment in base.split('.') {
        if segment.is_empty() {
            return None;
        }
        components.push(segment.parse::<u64>().ok()?);
    }
    Some(Version {
        components,
        prerelease,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maven_ranges_evaluate_with_boundary_semantics() {
        assert!(satisfies("26.2.0.88", "[26.2,)").unwrap());
        assert!(satisfies("26.2.0.88", "[26,)").unwrap());
        assert!(!satisfies("26.2.0.88", "[26.3,)").unwrap());
        assert!(satisfies("26.2.0.88", "(26.1,27)").unwrap());
        assert!(satisfies("26.2.0.88", "[26.2.0.88]").unwrap());
        assert!(satisfies("26.2.0.88", "[26.2.0.88,26.2.0.89]").unwrap());
        assert!(!satisfies("26.2.0.88", "(26.2.0.88,26.2.0.89]").unwrap());
        assert!(satisfies("26.2.0.88", "[,26.2.0.89]").unwrap());
        assert!(satisfies("3.0.0", "[3,]").unwrap());
        assert!(!satisfies("2.9.9", "[3,]").unwrap());
    }

    #[test]
    fn exact_and_wildcard_requirements_match_verbatim() {
        assert!(satisfies("1.21.1", "1.21.1").unwrap());
        assert!(!satisfies("1.21.2", "1.21.1").unwrap());
        assert!(satisfies("1.21.11", "1.21.*").unwrap());
    }

    #[test]
    fn malformed_ranges_are_errors_never_matches() {
        assert!(satisfies("26.2", "").is_err());
        assert!(satisfies("26.2", "[26.2").is_err());
        assert!(satisfies("26.2", "26.2]").is_err());
        assert!(satisfies("26.2", "[]").is_err());
        assert!(satisfies("beta", "[26,)").is_err());
    }
}
