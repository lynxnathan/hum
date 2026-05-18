mod error;
pub mod types;

pub use error::HumParseError;
pub use types::{DoesField, Piece, ThingDef, ThingType};

/// Parse a .hum file content string into a Piece (IndexMap of thing names to ThingDefs).
pub fn parse_hum(content: &str) -> Result<Piece, HumParseError> {
    serde_saphyr::from_str(content).map_err(|e| HumParseError::InvalidSchema(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_thing_with_at_and_like() {
        let input = "space-crackle:\n  at: \"0s\"\n  like: warm pad";
        let piece = parse_hum(input).expect("should parse valid .hum");
        let thing = piece.get("space-crackle").expect("should have space-crackle");
        assert_eq!(thing.at.as_deref(), Some("0s"));
        assert_eq!(thing.like.as_deref(), Some("warm pad"));
    }

    #[test]
    fn reject_unknown_field() {
        let input = "space-crackle:\n  unknown_field: val";
        let err = parse_hum(input).expect_err("should reject unknown field");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("unknown field"),
            "error should mention 'unknown field', got: {msg}"
        );
    }

    #[test]
    fn does_single_string() {
        let input = "bass:\n  does: builds from silence";
        let piece = parse_hum(input).expect("should parse");
        let thing = piece.get("bass").expect("should have bass");
        match thing.does.as_ref().expect("does should be present") {
            DoesField::Single(s) => assert_eq!(s, "builds from silence"),
            DoesField::Multi(_) => panic!("expected Single, got Multi"),
        }
    }

    #[test]
    fn does_multi_list() {
        let input = "bass:\n  does:\n    - builds from silence\n    - fades out";
        let piece = parse_hum(input).expect("should parse");
        let thing = piece.get("bass").expect("should have bass");
        match thing.does.as_ref().expect("does should be present") {
            DoesField::Multi(v) => {
                assert_eq!(v.len(), 2);
                assert_eq!(v[0], "builds from silence");
                assert_eq!(v[1], "fades out");
            }
            DoesField::Single(_) => panic!("expected Multi, got Single"),
        }
    }

    #[test]
    fn ref_keyword_rename() {
        let input = "bass:\n  ref: some-ref";
        let piece = parse_hum(input).expect("should parse");
        let thing = piece.get("bass").expect("should have bass");
        assert_eq!(thing.reference.as_deref(), Some("some-ref"));
    }

    #[test]
    fn where_keyword_rename() {
        let input = "bass:\n  where: center";
        let piece = parse_hum(input).expect("should parse");
        let thing = piece.get("bass").expect("should have bass");
        assert_eq!(thing.location.as_deref(), Some("center"));
    }

    #[test]
    fn all_fields_present() {
        let input = r#"
guitar:
  at: "10s"
  until: "30s"
  does:
    - volume from low to high
    - wah starts slow
  where: left
  within: main-mix
  every: "2s"
  like: wah-wah guitar
  ref: hendrix
  mood: psychedelic
  has:
    sparkle:
      like: bright sparkle
      where: center
"#;
        let piece = parse_hum(input).expect("should parse all fields");
        let thing = piece.get("guitar").expect("should have guitar");
        assert_eq!(thing.at.as_deref(), Some("10s"));
        assert_eq!(thing.until.as_deref(), Some("30s"));
        assert_eq!(thing.location.as_deref(), Some("left"));
        assert_eq!(thing.within.as_deref(), Some("main-mix"));
        assert_eq!(thing.every.as_deref(), Some("2s"));
        assert_eq!(thing.like.as_deref(), Some("wah-wah guitar"));
        assert_eq!(thing.reference.as_deref(), Some("hendrix"));
        assert_eq!(thing.mood.as_deref(), Some("psychedelic"));

        // Check does (Multi)
        let does = thing.does.as_ref().expect("does present");
        assert_eq!(does.as_vec().len(), 2);

        // Check nested has
        let has = thing.has.as_ref().expect("has present");
        let sparkle = has.get("sparkle").expect("sparkle sub-thing");
        assert_eq!(sparkle.like.as_deref(), Some("bright sparkle"));
        assert_eq!(sparkle.location.as_deref(), Some("center"));
    }

    #[test]
    fn does_as_vec_helper() {
        let single = DoesField::Single("test".to_string());
        assert_eq!(single.as_vec(), vec!["test"]);

        let multi = DoesField::Multi(vec!["a".to_string(), "b".to_string()]);
        assert_eq!(multi.as_vec(), vec!["a", "b"]);
    }

    #[test]
    fn empty_thing_is_valid() {
        // A thing with no fields should be valid (all fields are Option)
        let input = "silence:\n  {}";
        let piece = parse_hum(input).expect("empty thing should parse");
        let thing = piece.get("silence").expect("should have silence");
        assert!(thing.at.is_none());
        assert!(thing.like.is_none());
    }

    #[test]
    fn style_field_parses_as_some() {
        let input = "glass:\n  style: laser\n  synth:\n    osc: sine\n";
        let piece = parse_hum(input).expect("should parse style: field");
        let thing = piece.get("glass").expect("should have glass");
        assert_eq!(thing.style.as_deref(), Some("laser"));
    }

    #[test]
    fn style_field_absent_is_none() {
        let input = "glass:\n  synth:\n    osc: sine\n";
        let piece = parse_hum(input).expect("should parse without style");
        let thing = piece.get("glass").expect("should have glass");
        assert!(thing.style.is_none());
    }

    #[test]
    fn unknown_field_still_rejected_with_style() {
        let input = "glass:\n  style: laser\n  bogus: nope\n";
        let err = parse_hum(input).expect_err("should reject unknown field");
        let msg = err.to_string().to_lowercase();
        assert!(
            msg.contains("unknown field"),
            "error should mention 'unknown field', got: {msg}"
        );
    }
}
