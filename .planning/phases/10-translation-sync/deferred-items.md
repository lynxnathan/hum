# Deferred Items - Phase 10

## Pre-existing Test Failure

**Test:** `dict::tests::add_entry_overwrites_existing_term`
**Source:** Plan 1 (10-1) dict add feature
**Issue:** `DictStore::add_entry` serializes SynthBlock fields using Debug format (e.g., `Lpf { cutoff: 800.0 }`) instead of the display format the parser expects (e.g., `lpf(cutoff: 800)`). The written YAML cannot be re-parsed.
**Impact:** `hum dict add` writes entries that cannot be loaded back. Does not affect Plan 2 features.
**Fix needed:** Use Display trait implementations (already added in Plan 1) instead of Debug when serializing synth block fields in `add_entry`.
