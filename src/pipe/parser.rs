/// Pipe block parser — parses `pipe:` multiline strings into PipeExpr AST.
///
/// Syntax:
///   source_thing
///   |> transform(args)
///   |> transform(args)
///
/// Source can be a bare thing name or thing.field accessor.

use anyhow::{bail, Result};
use super::types::{PipeExpr, PipeSource, Transform};

/// Parse a pipe block string into a PipeExpr.
///
/// Input is the multiline string from a `pipe:` YAML field.
/// Lines are trimmed and filtered; the first non-empty line is the source,
/// subsequent lines starting with `|>` are transforms.
pub fn parse_pipe_block(input: &str) -> Result<PipeExpr> {
    let lines: Vec<&str> = input
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();

    if lines.is_empty() {
        bail!("pipe block is empty");
    }

    // First line: source
    let source = parse_source(lines[0])?;

    // Remaining lines: transforms (each must start with |>)
    let mut transforms = Vec::new();
    for &line in &lines[1..] {
        let stripped = line.strip_prefix("|>").ok_or_else(|| {
            anyhow::anyhow!("pipe transform line must start with '|>': {}", line)
        })?;
        let transform = parse_transform(stripped.trim())?;
        transforms.push(transform);
    }

    Ok(PipeExpr { source, transforms })
}

/// Parse a source: either "thing" or "thing.field"
fn parse_source(s: &str) -> Result<PipeSource> {
    if let Some(dot_pos) = s.find('.') {
        let thing = &s[..dot_pos];
        let field = &s[dot_pos + 1..];
        if thing.is_empty() || field.is_empty() {
            bail!("invalid pipe source: '{}'", s);
        }
        Ok(PipeSource::Field(thing.to_string(), field.to_string()))
    } else {
        if s.is_empty() {
            bail!("pipe source is empty");
        }
        Ok(PipeSource::Thing(s.to_string()))
    }
}

/// Parse a single transform call like "replicate(3)" or "shift(semitones: 4)"
fn parse_transform(s: &str) -> Result<Transform> {
    let (name, args_str) = if let Some(paren_pos) = s.find('(') {
        let name = s[..paren_pos].trim();
        let args = s[paren_pos + 1..].trim_end_matches(')').trim();
        (name, args)
    } else {
        (s, "")
    };

    match name {
        "replicate" => {
            let n: usize = args_str.parse().map_err(|_| {
                anyhow::anyhow!("replicate expects a number, got: '{}'", args_str)
            })?;
            Ok(Transform::Replicate { n })
        }
        "shift" => {
            let semitones = parse_kv_i32(args_str, "semitones")?;
            Ok(Transform::Shift { semitones })
        }
        "spread" => {
            // spread(pan: -0.8~0.8)
            let pan_str = parse_kv_str(args_str, "pan")?;
            let (lo, hi) = parse_range(&pan_str)?;
            Ok(Transform::Spread { lo, hi })
        }
        "tempo" => {
            // tempo(0.35s/note)
            let secs = parse_tempo_arg(args_str)?;
            Ok(Transform::Tempo { seconds_per_note: secs })
        }
        "take" => {
            let n: usize = args_str.parse().map_err(|_| {
                anyhow::anyhow!("take expects a number, got: '{}'", args_str)
            })?;
            Ok(Transform::Take { n })
        }
        "repeat" => {
            let n: usize = args_str.parse().map_err(|_| {
                anyhow::anyhow!("repeat expects a number, got: '{}'", args_str)
            })?;
            Ok(Transform::Repeat { n })
        }
        "each" => {
            // each(i => shift(semitones: i * 4))
            Ok(Transform::Each { expr: args_str.to_string() })
        }
        "map" => {
            Ok(Transform::Map { expr: args_str.to_string() })
        }
        _ => bail!("unknown pipe transform: '{}'", name),
    }
}

/// Parse "key: value" from a string, returning the value for the given key.
fn parse_kv_str(s: &str, key: &str) -> Result<String> {
    let prefix = format!("{}:", key);
    let stripped = s.strip_prefix(&prefix).or_else(|| {
        // Try with space after colon
        let prefix2 = format!("{}: ", key);
        s.strip_prefix(&prefix2)
    });
    match stripped {
        Some(val) => Ok(val.trim().to_string()),
        None => bail!("expected '{}:' in '{}'", key, s),
    }
}

/// Parse "key: N" as i32.
fn parse_kv_i32(s: &str, key: &str) -> Result<i32> {
    let val_str = parse_kv_str(s, key)?;
    val_str.parse::<i32>().map_err(|_| {
        anyhow::anyhow!("expected integer for '{}', got: '{}'", key, val_str)
    })
}

/// Parse range syntax "lo~hi" into (f32, f32).
fn parse_range(s: &str) -> Result<(f32, f32)> {
    let parts: Vec<&str> = s.split('~').collect();
    if parts.len() != 2 {
        bail!("invalid range syntax: '{}' (expected 'lo~hi')", s);
    }
    let lo: f32 = parts[0].parse().map_err(|_| {
        anyhow::anyhow!("invalid range lo: '{}'", parts[0])
    })?;
    let hi: f32 = parts[1].parse().map_err(|_| {
        anyhow::anyhow!("invalid range hi: '{}'", parts[1])
    })?;
    Ok((lo, hi))
}

/// Parse tempo argument: "0.35s/note" or "0.35"
fn parse_tempo_arg(s: &str) -> Result<f32> {
    let stripped = s.trim().trim_end_matches("s/note").trim_end_matches("s");
    stripped.parse::<f32>().map_err(|_| {
        anyhow::anyhow!("invalid tempo value: '{}'", s)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_source() {
        let expr = parse_pipe_block("glass\n|> replicate(3)").unwrap();
        assert_eq!(expr.source, PipeSource::Thing("glass".to_string()));
        assert_eq!(expr.transforms.len(), 1);
        assert_eq!(expr.transforms[0], Transform::Replicate { n: 3 });
    }

    #[test]
    fn parse_field_source() {
        let expr = parse_pipe_block("glass.notes\n|> take(4)").unwrap();
        assert_eq!(expr.source, PipeSource::Field("glass".to_string(), "notes".to_string()));
    }

    #[test]
    fn parse_full_chain() {
        let input = r#"
            glass
            |> replicate(3)
            |> each(i => shift(semitones: i * 4))
            |> spread(pan: -0.8~0.8)
        "#;
        let expr = parse_pipe_block(input).unwrap();
        assert_eq!(expr.source, PipeSource::Thing("glass".to_string()));
        assert_eq!(expr.transforms.len(), 3);
        assert_eq!(expr.transforms[0], Transform::Replicate { n: 3 });
        assert!(matches!(expr.transforms[1], Transform::Each { .. }));
        assert_eq!(expr.transforms[2], Transform::Spread { lo: -0.8, hi: 0.8 });
    }

    #[test]
    fn parse_shift() {
        let expr = parse_pipe_block("glass\n|> shift(semitones: 4)").unwrap();
        assert_eq!(expr.transforms[0], Transform::Shift { semitones: 4 });
    }

    #[test]
    fn parse_tempo() {
        let expr = parse_pipe_block("glass\n|> tempo(0.35s/note)").unwrap();
        assert_eq!(expr.transforms[0], Transform::Tempo { seconds_per_note: 0.35 });
    }

    #[test]
    fn parse_take_repeat() {
        let expr = parse_pipe_block("glass\n|> take(4)\n|> repeat(8)").unwrap();
        assert_eq!(expr.transforms[0], Transform::Take { n: 4 });
        assert_eq!(expr.transforms[1], Transform::Repeat { n: 8 });
    }

    #[test]
    fn empty_input_fails() {
        assert!(parse_pipe_block("").is_err());
    }

    #[test]
    fn bad_transform_name_fails() {
        assert!(parse_pipe_block("glass\n|> unknown(3)").is_err());
    }
}
