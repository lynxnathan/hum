use serde::Deserialize;
use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Value — a parameter that can be fixed or a range (mapped to LFNoise1 modulation)
// ---------------------------------------------------------------------------

/// A synthesis parameter value: either a fixed constant or an LFO-modulated range.
///
/// - `Fixed(x)` compiles to a constant float in the UGen graph.
/// - `Range(lo, hi)` compiles to `LFNoise1.kr(rate).range(lo, hi)`.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Fixed(f32),
    Range(f32, f32),
}

impl Value {
    /// Return the fixed value, or the midpoint for a range (useful for defaults).
    pub fn fixed_or_mid(&self) -> f32 {
        match self {
            Value::Fixed(v) => *v,
            Value::Range(lo, hi) => (lo + hi) / 2.0,
        }
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Fixed(0.0)
    }
}

impl From<f32> for Value {
    fn from(v: f32) -> Self {
        Value::Fixed(v)
    }
}

impl FromStr for Value {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if let Some(tilde_pos) = s.find('~') {
            let lo = s[..tilde_pos]
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("invalid range lo: '{}'", &s[..tilde_pos]))?;
            let hi = s[tilde_pos + 1..]
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("invalid range hi: '{}'", &s[tilde_pos + 1..]))?;
            Ok(Value::Range(lo, hi))
        } else {
            let v = s
                .parse::<f32>()
                .map_err(|_| format!("invalid float: '{}'", s))?;
            Ok(Value::Fixed(v))
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Fixed(v) => write!(f, "{}", v),
            Value::Range(lo, hi) => write!(f, "{}~{}", lo, hi),
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Accept either a number or a string like "0.03~0.08"
        struct ValueVisitor;
        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a number or a range string like '0.03~0.08'")
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(Value::Fixed(v as f32))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(Value::Fixed(v as f32))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(Value::Fixed(v as f32))
            }

            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Self::Value, E> {
                s.parse::<Value>().map_err(serde::de::Error::custom)
            }
        }
        deserializer.deserialize_any(ValueVisitor)
    }
}

// ---------------------------------------------------------------------------
// SynthBlock — the top-level synth: field in a ThingDef
// ---------------------------------------------------------------------------

/// The inline synthesis IR block parsed from a .hum file's `synth:` field.
/// All fields are optional — you can have just `osc: sine` with defaults.
/// `deny_unknown_fields` rejects any unrecognized keys at parse time.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SynthBlock {
    pub notes: Option<Vec<String>>,
    pub osc: Option<OscLayer>,
    pub filter: Option<FilterPrimitive>,
    pub env: Option<EnvPrimitive>,
    pub distort: Option<DistortPrimitive>,
    pub fx: Option<FxPrimitive>,
    pub pan: Option<PanPrimitive>,
    pub amp: Option<Value>,
    pub tempo: Option<String>,
    /// Path to an audio sample file (relative to project root).
    /// When present, the synth uses PlayBuf instead of an oscillator.
    pub sample: Option<String>,
    /// Whether the sample loops continuously. Default false (one-shot).
    #[serde(rename = "loop")]
    pub loop_mode: Option<bool>,
}

// ---------------------------------------------------------------------------
// Helper: parse "name(key: val, key: val)" syntax
// ---------------------------------------------------------------------------

/// Parse a function-call-like string: "name(key: val, key: val)" -> (name, params)
/// Also handles bare names like "sine" -> ("sine", empty map)
fn parse_primitive_call(s: &str) -> (&str, Vec<(&str, &str)>) {
    let s = s.trim();
    if let Some(paren_pos) = s.find('(') {
        let name = s[..paren_pos].trim();
        // Strip trailing ')'
        let args_str = s[paren_pos + 1..].trim_end_matches(')').trim();
        if args_str.is_empty() {
            return (name, vec![]);
        }
        let params: Vec<(&str, &str)> = args_str
            .split(',')
            .filter_map(|pair| {
                let mut parts = pair.splitn(2, ':');
                let key = parts.next()?.trim();
                let val = parts.next()?.trim();
                if key.is_empty() {
                    None
                } else {
                    Some((key, val))
                }
            })
            .collect();
        (name, params)
    } else {
        (s, vec![])
    }
}

/// Look up a parameter value by key, returning the &str if found.
fn get_param<'a>(params: &[(&str, &'a str)], key: &str) -> Option<&'a str> {
    params.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Parse a parameter as f32, returning default if not found.
fn get_param_f32(params: &[(&str, &str)], key: &str, default: f32) -> Result<f32, String> {
    match get_param(params, key) {
        Some(v) => v
            .parse::<f32>()
            .map_err(|_| format!("invalid float for '{}': {}", key, v)),
        None => Ok(default),
    }
}

/// Parse a parameter as Value (supports range syntax `lo~hi`), returning default if not found.
fn get_param_value(params: &[(&str, &str)], key: &str, default: f32) -> Result<Value, String> {
    match get_param(params, key) {
        Some(v) => v.parse::<Value>(),
        None => Ok(Value::Fixed(default)),
    }
}

/// Parse a parameter as u8, returning default if not found.
fn get_param_u8(params: &[(&str, &str)], key: &str, default: u8) -> Result<u8, String> {
    match get_param(params, key) {
        Some(v) => v
            .parse::<u8>()
            .map_err(|_| format!("invalid u8 for '{}': {}", key, v)),
        None => Ok(default),
    }
}

// ---------------------------------------------------------------------------
// OscPrimitive
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum NoiseType {
    White,
    Pink,
    Brown,
}

impl FromStr for NoiseType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_lowercase().as_str() {
            "white" => Ok(NoiseType::White),
            "pink" => Ok(NoiseType::Pink),
            "brown" => Ok(NoiseType::Brown),
            _ => Err(format!("unknown noise type: '{}'", s)),
        }
    }
}

/// Oscillator primitive: a single oscillator voice.
#[derive(Debug, Clone, PartialEq)]
pub enum OscPrimitive {
    Sine { freq: Option<Value> },
    Saw { detune: Option<Value> },
    Pulse { width: Value, detune: Option<Value> },
    Noise { noise_type: NoiseType },
}

/// An oscillator layer: one or more oscillators summed together.
/// Parsed from `osc: "saw + sine(freq: 30)"` or `osc: "pulse(width: 0.5)"`.
#[derive(Debug, Clone, PartialEq)]
pub struct OscLayer(pub Vec<OscPrimitive>);

impl FromStr for OscPrimitive {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, params) = parse_primitive_call(s);
        match name.to_lowercase().as_str() {
            "sine" => {
                let freq = match get_param(&params, "freq") {
                    Some(v) => Some(v.parse::<Value>()?),
                    None => None,
                };
                Ok(OscPrimitive::Sine { freq })
            }
            "saw" => {
                let detune = match get_param(&params, "detune") {
                    Some(v) => Some(v.parse::<Value>()?),
                    None => None,
                };
                Ok(OscPrimitive::Saw { detune })
            }
            "pulse" => {
                let width = get_param_value(&params, "width", 0.5)?;
                let detune = match get_param(&params, "detune") {
                    Some(v) => Some(v.parse::<Value>()?),
                    None => None,
                };
                Ok(OscPrimitive::Pulse { width, detune })
            }
            "noise" => {
                let noise_type = match get_param(&params, "type") {
                    Some(t) => t.parse::<NoiseType>()?,
                    None => NoiseType::White,
                };
                Ok(OscPrimitive::Noise { noise_type })
            }
            _ => Err(format!("unknown osc type: '{}'", name)),
        }
    }
}

impl FromStr for OscLayer {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        // Split on " + " to support multi-osc: "saw + sine(freq: 30)"
        let parts: Vec<&str> = s.split(" + ").collect();
        let mut oscs = Vec::with_capacity(parts.len());
        for part in parts {
            oscs.push(part.trim().parse::<OscPrimitive>()?);
        }
        if oscs.is_empty() {
            return Err("empty osc expression".to_string());
        }
        Ok(OscLayer(oscs))
    }
}

impl<'de> Deserialize<'de> for OscLayer {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse::<OscLayer>().map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// FilterPrimitive
// ---------------------------------------------------------------------------

/// Filter primitive: the `filter:` field in a synth: block.
#[derive(Debug, Clone, PartialEq)]
pub enum FilterPrimitive {
    Lpf { cutoff: Value },
    Hpf { cutoff: Value },
    Bpf { freq: Value, q: Value },
}

impl FromStr for FilterPrimitive {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, params) = parse_primitive_call(s);
        match name.to_lowercase().as_str() {
            "lpf" => {
                let cutoff = get_param_value(&params, "cutoff", 1000.0)?;
                Ok(FilterPrimitive::Lpf { cutoff })
            }
            "hpf" => {
                let cutoff = get_param_value(&params, "cutoff", 1000.0)?;
                Ok(FilterPrimitive::Hpf { cutoff })
            }
            "bpf" => {
                let freq = get_param_value(&params, "freq", 1000.0)?;
                let q = get_param_value(&params, "q", 1.0)?;
                Ok(FilterPrimitive::Bpf { freq, q })
            }
            _ => Err(format!("unknown filter type: '{}'", name)),
        }
    }
}

// ---------------------------------------------------------------------------
// EnvPrimitive
// ---------------------------------------------------------------------------

/// Envelope primitive: the `env:` field in a synth: block.
#[derive(Debug, Clone, PartialEq)]
pub enum EnvPrimitive {
    Perc { attack: f32, release: f32 },
    Adsr { attack: f32, decay: f32, sustain: f32, release: f32 },
}

impl FromStr for EnvPrimitive {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, params) = parse_primitive_call(s);
        match name.to_lowercase().as_str() {
            "perc" => {
                let attack = get_param_f32(&params, "attack", 0.01)?;
                let release = get_param_f32(&params, "release", 0.5)?;
                Ok(EnvPrimitive::Perc { attack, release })
            }
            "adsr" => {
                let attack = get_param_f32(&params, "a", 0.01)?;
                let decay = get_param_f32(&params, "d", 0.1)?;
                let sustain = get_param_f32(&params, "s", 0.8)?;
                let release = get_param_f32(&params, "r", 0.3)?;
                Ok(EnvPrimitive::Adsr { attack, decay, sustain, release })
            }
            _ => Err(format!("unknown env type: '{}'", name)),
        }
    }
}

// ---------------------------------------------------------------------------
// DistortPrimitive
// ---------------------------------------------------------------------------

/// Distortion primitive: the `distort:` field in a synth: block.
#[derive(Debug, Clone, PartialEq)]
pub enum DistortPrimitive {
    Tanh { drive: Value },
    Bitcrush { bits: u8 },
}

impl FromStr for DistortPrimitive {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, params) = parse_primitive_call(s);
        match name.to_lowercase().as_str() {
            "tanh" => {
                let drive = get_param_value(&params, "drive", 1.0)?;
                Ok(DistortPrimitive::Tanh { drive })
            }
            "bitcrush" => {
                let bits = get_param_u8(&params, "bits", 8)?;
                Ok(DistortPrimitive::Bitcrush { bits })
            }
            _ => Err(format!("unknown distort type: '{}'", name)),
        }
    }
}

// ---------------------------------------------------------------------------
// FxPrimitive
// ---------------------------------------------------------------------------

/// Effects primitive: the `fx:` field in a synth: block.
#[derive(Debug, Clone, PartialEq)]
pub enum FxPrimitive {
    Reverb { mix: Value, room: Value },
    Delay { time: Value, feedback: Value },
    Allpass { time: Value, feedback: Value },
}

impl FromStr for FxPrimitive {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, params) = parse_primitive_call(s);
        match name.to_lowercase().as_str() {
            "reverb" => {
                let mix = get_param_value(&params, "mix", 0.5)?;
                let room = get_param_value(&params, "room", 0.5)?;
                Ok(FxPrimitive::Reverb { mix, room })
            }
            "delay" => {
                let time = get_param_value(&params, "time", 0.3)?;
                let feedback = get_param_value(&params, "feedback", 0.5)?;
                Ok(FxPrimitive::Delay { time, feedback })
            }
            "allpass" => {
                let time = get_param_value(&params, "time", 0.3)?;
                let feedback = get_param_value(&params, "feedback", 0.5)?;
                Ok(FxPrimitive::Allpass { time, feedback })
            }
            _ => Err(format!("unknown fx type: '{}'", name)),
        }
    }
}

// ---------------------------------------------------------------------------
// PanPrimitive
// ---------------------------------------------------------------------------

/// Panning primitive: the `pan:` field in a synth: block.
#[derive(Debug, Clone, PartialEq)]
pub enum PanPrimitive {
    Center,
    Noise { rate: Value, range: Value },
    Lfo { rate: Value },
    /// Fixed pan position (-1.0 = hard left, 0.0 = center, 1.0 = hard right).
    /// Used by pipe spread() transform.
    Fixed { value: f32 },
}

impl FromStr for PanPrimitive {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (name, params) = parse_primitive_call(s);
        match name.to_lowercase().as_str() {
            "center" => Ok(PanPrimitive::Center),
            "noise" => {
                let rate = get_param_value(&params, "rate", 0.1)?;
                let range = match get_param(&params, "range") {
                    Some(r) => r.parse::<Value>()?,
                    None => Value::Range(-1.0, 1.0),
                };
                Ok(PanPrimitive::Noise { rate, range })
            }
            "lfo" => {
                let rate = get_param_value(&params, "rate", 0.05)?;
                Ok(PanPrimitive::Lfo { rate })
            }
            "fixed" => {
                let value = get_param_f32(&params, "value", 0.0)?;
                Ok(PanPrimitive::Fixed { value })
            }
            _ => Err(format!("unknown pan type: '{}'", name)),
        }
    }
}

// ---------------------------------------------------------------------------
// Serde: Deserialize via FromStr for all primitives
// ---------------------------------------------------------------------------

/// Macro to implement Deserialize for a primitive enum via its FromStr impl.
/// This allows YAML values like `osc: sine` or `osc: pulse(width: 0.5)` to
/// deserialize as typed enums.
macro_rules! impl_deserialize_from_str {
    ($t:ty) => {
        impl<'de> Deserialize<'de> for $t {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                s.parse::<$t>().map_err(serde::de::Error::custom)
            }
        }
    };
}

impl_deserialize_from_str!(FilterPrimitive);
impl_deserialize_from_str!(EnvPrimitive);
impl_deserialize_from_str!(DistortPrimitive);
impl_deserialize_from_str!(FxPrimitive);
impl_deserialize_from_str!(PanPrimitive);

// ---------------------------------------------------------------------------
// Display impls (useful for debugging and error messages)
// ---------------------------------------------------------------------------

impl fmt::Display for OscPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OscPrimitive::Sine { freq: None } => write!(f, "sine"),
            OscPrimitive::Sine { freq: Some(v) } => write!(f, "sine(freq: {})", v),
            OscPrimitive::Saw { detune: None } => write!(f, "saw"),
            OscPrimitive::Saw { detune: Some(v) } => write!(f, "saw(detune: {})", v),
            OscPrimitive::Pulse { width, detune: None } => write!(f, "pulse(width: {})", width),
            OscPrimitive::Pulse { width, detune: Some(d) } => write!(f, "pulse(width: {}, detune: {})", width, d),
            OscPrimitive::Noise { noise_type } => write!(f, "noise(type: {:?})", noise_type),
        }
    }
}

impl fmt::Display for OscLayer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, osc) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, " + ")?;
            }
            write!(f, "{}", osc)?;
        }
        Ok(())
    }
}

impl fmt::Display for FilterPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterPrimitive::Lpf { cutoff } => write!(f, "lpf(cutoff: {})", cutoff),
            FilterPrimitive::Hpf { cutoff } => write!(f, "hpf(cutoff: {})", cutoff),
            FilterPrimitive::Bpf { freq, q } => write!(f, "bpf(freq: {}, q: {})", freq, q),
        }
    }
}

impl fmt::Display for EnvPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EnvPrimitive::Perc { attack, release } => write!(f, "perc(attack: {}, release: {})", attack, release),
            EnvPrimitive::Adsr { attack, decay, sustain, release } => write!(f, "adsr(a: {}, d: {}, s: {}, r: {})", attack, decay, sustain, release),
        }
    }
}

impl fmt::Display for DistortPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DistortPrimitive::Tanh { drive } => write!(f, "tanh(drive: {})", drive),
            DistortPrimitive::Bitcrush { bits } => write!(f, "bitcrush(bits: {})", bits),
        }
    }
}

impl fmt::Display for FxPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FxPrimitive::Reverb { mix, room } => write!(f, "reverb(mix: {}, room: {})", mix, room),
            FxPrimitive::Delay { time, feedback } => write!(f, "delay(time: {}, feedback: {})", time, feedback),
            FxPrimitive::Allpass { time, feedback } => write!(f, "allpass(time: {}, feedback: {})", time, feedback),
        }
    }
}

impl fmt::Display for PanPrimitive {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PanPrimitive::Center => write!(f, "center"),
            PanPrimitive::Noise { rate, range } => write!(f, "noise(rate: {}, range: {})", rate, range),
            PanPrimitive::Lfo { rate } => write!(f, "lfo(rate: {})", rate),
            PanPrimitive::Fixed { value } => write!(f, "fixed(value: {})", value),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Value --

    #[test]
    fn value_parse_fixed() {
        let v: Value = "0.5".parse().unwrap();
        assert_eq!(v, Value::Fixed(0.5));
    }

    #[test]
    fn value_parse_range() {
        let v: Value = "0.03~0.08".parse().unwrap();
        assert_eq!(v, Value::Range(0.03, 0.08));
    }

    #[test]
    fn value_parse_negative_range() {
        let v: Value = "-1.0~1.0".parse().unwrap();
        assert_eq!(v, Value::Range(-1.0, 1.0));
    }

    #[test]
    fn value_fixed_or_mid() {
        assert_eq!(Value::Fixed(5.0).fixed_or_mid(), 5.0);
        assert_eq!(Value::Range(0.0, 10.0).fixed_or_mid(), 5.0);
    }

    // -- OscPrimitive --

    #[test]
    fn osc_parse_sine() {
        let osc: OscPrimitive = "sine".parse().unwrap();
        assert_eq!(osc, OscPrimitive::Sine { freq: None });
    }

    #[test]
    fn osc_parse_sine_with_freq() {
        let osc: OscPrimitive = "sine(freq: 30)".parse().unwrap();
        assert_eq!(osc, OscPrimitive::Sine { freq: Some(Value::Fixed(30.0)) });
    }

    #[test]
    fn osc_parse_saw() {
        let osc: OscPrimitive = "saw".parse().unwrap();
        assert_eq!(osc, OscPrimitive::Saw { detune: None });
    }

    #[test]
    fn osc_parse_saw_with_detune() {
        let osc: OscPrimitive = "saw(detune: 0.03)".parse().unwrap();
        assert_eq!(osc, OscPrimitive::Saw { detune: Some(Value::Fixed(0.03)) });
    }

    #[test]
    fn osc_parse_pulse_with_width() {
        let osc: OscPrimitive = "pulse(width: 0.5)".parse().unwrap();
        assert_eq!(osc, OscPrimitive::Pulse { width: Value::Fixed(0.5), detune: None });
    }

    #[test]
    fn osc_parse_pulse_with_range_width() {
        let osc: OscPrimitive = "pulse(width: 0.03~0.08)".parse().unwrap();
        assert_eq!(osc, OscPrimitive::Pulse { width: Value::Range(0.03, 0.08), detune: None });
    }

    #[test]
    fn osc_parse_pulse_default_width() {
        let osc: OscPrimitive = "pulse".parse().unwrap();
        assert_eq!(osc, OscPrimitive::Pulse { width: Value::Fixed(0.5), detune: None });
    }

    #[test]
    fn osc_parse_noise_white() {
        let osc: OscPrimitive = "noise(type: white)".parse().unwrap();
        assert_eq!(
            osc,
            OscPrimitive::Noise {
                noise_type: NoiseType::White
            }
        );
    }

    #[test]
    fn osc_parse_noise_pink() {
        let osc: OscPrimitive = "noise(type: pink)".parse().unwrap();
        assert_eq!(
            osc,
            OscPrimitive::Noise {
                noise_type: NoiseType::Pink
            }
        );
    }

    #[test]
    fn osc_parse_noise_default() {
        let osc: OscPrimitive = "noise".parse().unwrap();
        assert_eq!(
            osc,
            OscPrimitive::Noise {
                noise_type: NoiseType::White
            }
        );
    }

    #[test]
    fn osc_parse_unknown_fails() {
        let result = "triangle".parse::<OscPrimitive>();
        assert!(result.is_err());
    }

    // -- OscLayer (multi-osc) --

    #[test]
    fn osclayer_single() {
        let layer: OscLayer = "saw".parse().unwrap();
        assert_eq!(layer.0.len(), 1);
        assert_eq!(layer.0[0], OscPrimitive::Saw { detune: None });
    }

    #[test]
    fn osclayer_multi() {
        let layer: OscLayer = "saw(detune: 0.03) + sine(freq: 30)".parse().unwrap();
        assert_eq!(layer.0.len(), 2);
        assert_eq!(layer.0[0], OscPrimitive::Saw { detune: Some(Value::Fixed(0.03)) });
        assert_eq!(layer.0[1], OscPrimitive::Sine { freq: Some(Value::Fixed(30.0)) });
    }

    #[test]
    fn osclayer_multi_bare() {
        let layer: OscLayer = "saw + sine".parse().unwrap();
        assert_eq!(layer.0.len(), 2);
    }

    // -- FilterPrimitive --

    #[test]
    fn filter_parse_lpf_with_cutoff() {
        let f: FilterPrimitive = "lpf(cutoff: 800)".parse().unwrap();
        assert_eq!(f, FilterPrimitive::Lpf { cutoff: Value::Fixed(800.0) });
    }

    #[test]
    fn filter_parse_lpf_with_range() {
        let f: FilterPrimitive = "lpf(cutoff: 180~900)".parse().unwrap();
        assert_eq!(f, FilterPrimitive::Lpf { cutoff: Value::Range(180.0, 900.0) });
    }

    #[test]
    fn filter_parse_hpf_bare() {
        let f: FilterPrimitive = "hpf".parse().unwrap();
        assert_eq!(f, FilterPrimitive::Hpf { cutoff: Value::Fixed(1000.0) });
    }

    #[test]
    fn filter_parse_bpf_with_params() {
        let f: FilterPrimitive = "bpf(freq: 2000, q: 0.3)".parse().unwrap();
        assert_eq!(
            f,
            FilterPrimitive::Bpf {
                freq: Value::Fixed(2000.0),
                q: Value::Fixed(0.3)
            }
        );
    }

    #[test]
    fn filter_parse_bpf_default_q() {
        let f: FilterPrimitive = "bpf(freq: 2000)".parse().unwrap();
        assert_eq!(
            f,
            FilterPrimitive::Bpf {
                freq: Value::Fixed(2000.0),
                q: Value::Fixed(1.0)
            }
        );
    }

    // -- EnvPrimitive --

    #[test]
    fn env_parse_perc() {
        let e: EnvPrimitive = "perc(attack: 0.01, release: 0.5)".parse().unwrap();
        assert_eq!(
            e,
            EnvPrimitive::Perc {
                attack: 0.01,
                release: 0.5
            }
        );
    }

    #[test]
    fn env_parse_adsr() {
        let e: EnvPrimitive = "adsr(a: 0.01, d: 0.1, s: 0.8, r: 0.3)".parse().unwrap();
        assert_eq!(
            e,
            EnvPrimitive::Adsr {
                attack: 0.01,
                decay: 0.1,
                sustain: 0.8,
                release: 0.3
            }
        );
    }

    #[test]
    fn env_parse_perc_defaults() {
        let e: EnvPrimitive = "perc".parse().unwrap();
        assert_eq!(
            e,
            EnvPrimitive::Perc {
                attack: 0.01,
                release: 0.5
            }
        );
    }

    // -- DistortPrimitive --

    #[test]
    fn distort_parse_tanh() {
        let d: DistortPrimitive = "tanh(drive: 2.0)".parse().unwrap();
        assert_eq!(d, DistortPrimitive::Tanh { drive: Value::Fixed(2.0) });
    }

    #[test]
    fn distort_parse_bitcrush() {
        let d: DistortPrimitive = "bitcrush(bits: 8)".parse().unwrap();
        assert_eq!(d, DistortPrimitive::Bitcrush { bits: 8 });
    }

    // -- FxPrimitive --

    #[test]
    fn fx_parse_reverb() {
        let fx: FxPrimitive = "reverb(mix: 0.7, room: 0.95)".parse().unwrap();
        assert_eq!(fx, FxPrimitive::Reverb { mix: Value::Fixed(0.7), room: Value::Fixed(0.95) });
    }

    #[test]
    fn fx_parse_delay() {
        let fx: FxPrimitive = "delay(time: 0.3, feedback: 0.5)".parse().unwrap();
        assert_eq!(
            fx,
            FxPrimitive::Delay {
                time: Value::Fixed(0.3),
                feedback: Value::Fixed(0.5)
            }
        );
    }

    #[test]
    fn fx_parse_allpass() {
        let fx: FxPrimitive = "allpass(time: 0.3, feedback: 0.6)".parse().unwrap();
        assert_eq!(
            fx,
            FxPrimitive::Allpass {
                time: Value::Fixed(0.3),
                feedback: Value::Fixed(0.6)
            }
        );
    }

    #[test]
    fn fx_parse_allpass_with_range() {
        let fx: FxPrimitive = "allpass(time: 0.3~0.7, feedback: 0.6)".parse().unwrap();
        assert_eq!(
            fx,
            FxPrimitive::Allpass {
                time: Value::Range(0.3, 0.7),
                feedback: Value::Fixed(0.6)
            }
        );
    }

    // -- PanPrimitive --

    #[test]
    fn pan_parse_center() {
        let p: PanPrimitive = "center".parse().unwrap();
        assert_eq!(p, PanPrimitive::Center);
    }

    #[test]
    fn pan_parse_noise_with_range() {
        let p: PanPrimitive = "noise(rate: 0.1, range: -0.5~0.5)".parse().unwrap();
        assert_eq!(
            p,
            PanPrimitive::Noise {
                rate: Value::Fixed(0.1),
                range: Value::Range(-0.5, 0.5)
            }
        );
    }

    #[test]
    fn pan_parse_noise_with_value_range() {
        let p: PanPrimitive = "noise(rate: 0.04, range: -1.0~1.0)".parse().unwrap();
        assert_eq!(
            p,
            PanPrimitive::Noise {
                rate: Value::Fixed(0.04),
                range: Value::Range(-1.0, 1.0)
            }
        );
    }

    #[test]
    fn pan_parse_lfo() {
        let p: PanPrimitive = "lfo(rate: 0.05)".parse().unwrap();
        assert_eq!(p, PanPrimitive::Lfo { rate: Value::Fixed(0.05) });
    }

    #[test]
    fn pan_parse_lfo_default_rate() {
        let p: PanPrimitive = "lfo".parse().unwrap();
        assert_eq!(p, PanPrimitive::Lfo { rate: Value::Fixed(0.05) });
    }

    // -- SynthBlock serde --

    #[test]
    fn synthblock_deserialize_minimal() {
        let yaml = "osc: sine\namp: 0.1\n";
        let block: SynthBlock = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(block.osc, Some(OscLayer(vec![OscPrimitive::Sine { freq: None }])));
        assert_eq!(block.amp, Some(Value::Fixed(0.1)));
        assert!(block.filter.is_none());
        assert!(block.env.is_none());
        assert!(block.notes.is_none());
        assert!(block.sample.is_none());
        assert!(block.loop_mode.is_none());
    }

    #[test]
    fn synthblock_deserialize_sample() {
        let yaml = "sample: samples/kick.wav\nloop: true\namp: 0.8\n";
        let block: SynthBlock = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(block.sample, Some("samples/kick.wav".to_string()));
        assert_eq!(block.loop_mode, Some(true));
        assert_eq!(block.amp, Some(Value::Fixed(0.8)));
        assert!(block.osc.is_none());
    }

    #[test]
    fn synthblock_deserialize_full() {
        let yaml = r#"
notes: [D4, Eb4, "-"]
osc: "pulse(width: 0.5)"
filter: "lpf(cutoff: 800)"
env: "perc(attack: 0.01, release: 0.5)"
distort: "tanh(drive: 2.0)"
fx: "reverb(mix: 0.7, room: 0.95)"
pan: center
amp: 0.1
tempo: "0.35s/note"
"#;
        let block: SynthBlock = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(block.osc, Some(OscLayer(vec![OscPrimitive::Pulse { width: Value::Fixed(0.5), detune: None }])));
        assert_eq!(block.filter, Some(FilterPrimitive::Lpf { cutoff: Value::Fixed(800.0) }));
        assert_eq!(
            block.env,
            Some(EnvPrimitive::Perc {
                attack: 0.01,
                release: 0.5
            })
        );
        assert_eq!(block.distort, Some(DistortPrimitive::Tanh { drive: Value::Fixed(2.0) }));
        assert_eq!(
            block.fx,
            Some(FxPrimitive::Reverb {
                mix: Value::Fixed(0.7),
                room: Value::Fixed(0.95)
            })
        );
        assert_eq!(block.pan, Some(PanPrimitive::Center));
        assert_eq!(block.amp, Some(Value::Fixed(0.1)));
        assert_eq!(block.tempo, Some("0.35s/note".to_string()));
        assert_eq!(
            block.notes,
            Some(vec!["D4".into(), "Eb4".into(), "-".into()])
        );
    }

    #[test]
    fn synthblock_deserialize_range_syntax() {
        let yaml = r#"
osc: "pulse(width: 0.03~0.08)"
filter: "lpf(cutoff: 180~900)"
fx: "allpass(time: 0.3~0.7, feedback: 0.6)"
pan: "noise(rate: 0.04, range: -1.0~1.0)"
amp: 0.04
"#;
        let block: SynthBlock = serde_saphyr::from_str(yaml).unwrap();
        assert_eq!(block.osc, Some(OscLayer(vec![OscPrimitive::Pulse { width: Value::Range(0.03, 0.08), detune: None }])));
        assert_eq!(block.filter, Some(FilterPrimitive::Lpf { cutoff: Value::Range(180.0, 900.0) }));
        assert_eq!(block.fx, Some(FxPrimitive::Allpass { time: Value::Range(0.3, 0.7), feedback: Value::Fixed(0.6) }));
        assert_eq!(block.amp, Some(Value::Fixed(0.04)));
    }

    #[test]
    fn synthblock_unknown_field_rejected() {
        let yaml = "osc: sine\nunknown_field: x\n";
        let result: Result<SynthBlock, _> = serde_saphyr::from_str(yaml);
        assert!(result.is_err(), "Expected error for unknown field");
    }

    // -- Primitive call parser --

    #[test]
    fn parse_primitive_call_bare() {
        let (name, params) = parse_primitive_call("sine");
        assert_eq!(name, "sine");
        assert!(params.is_empty());
    }

    #[test]
    fn parse_primitive_call_with_params() {
        let (name, params) = parse_primitive_call("pulse(width: 0.5)");
        assert_eq!(name, "pulse");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], ("width", "0.5"));
    }

    #[test]
    fn parse_primitive_call_multi_params() {
        let (name, params) = parse_primitive_call("bpf(freq: 2000, q: 0.3)");
        assert_eq!(name, "bpf");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], ("freq", "2000"));
        assert_eq!(params[1], ("q", "0.3"));
    }
}
