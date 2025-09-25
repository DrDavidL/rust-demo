# clinical-scrubber

A command-line helper that redacts common protected health information (PHI) patterns from clinical notes. It replaces matches with easily spotted tokens such as `[EMAIL]` or `[MRN]` so the text can be shared for downstream work (QA, analytics, demos) with less manual cleanup.

## Features
- Detects and redacts emails (including obfuscated forms), phone numbers, dates, MRNs, SSNs, ZIP codes, addresses, facilities, coordinates, URLs, and names via dictionaries plus heuristics (common surnames, first-name/last-name pairs, and an expanded honorific list), all after Unicode/punctuation normalization.
- Optional `--safe-harbor` mode layers in additional HIPAA Safe Harbor identifiers (insurance/policy numbers, licenses, VINs, device serials, IPs) for stricter de-identification.
- Optional JSON configuration lets you extend the built-in dictionaries or override MRN lengths.
- Prints a redaction summary (text or JSON) to stderr so you can review what changed.
- Works with files or standard input/output for quick command-line piping.
- Uses consistent placeholders like `[EMAIL]`, `[PHONE]`, `[PERSON]`, `[FACILITY]`, `[ADDRESS]`, `[COORD]`, `[URL]`, `[INSURANCE]`, `[LICENSE]`, `[VEHICLE]`, `[DEVICE]`, `[IP]`, `[DATE]`, `[REL_DATE]`, `[MRN]`, and `[SSN]` while tracking counts for each category.

## Getting Started
1. Install the Rust toolchain if needed (`https://rustup.rs`).
2. Build and run the CLI:
   ```bash
   cargo run -- --input note.txt --output scrubbed.txt
   ```
   Omit `--input`/`--output` to read from stdin and write to stdout.

## Configuration
Provide a JSON file with any fields you need. All fields are optional.
```json
{
  "names": ["Meredith Grey", "Derek Shepherd"],
  "keywords": ["Seattle Grace"],
  "mrn_min_length": 5,
  "mrn_max_length": 12
}
```
Use it via `--config custom.json`. Names and keywords are matched case-insensitively; spaces match any amount of whitespace.

## Examples
Read from stdin, skip person redactions, and emit stats as JSON:
```bash
echo "Met with Jane Doe on 04/02/2024" | cargo run -- --skip person --stats-json
```

Skip address masking while keeping other PHI redactions:
```bash
echo "Visited 128 Elmwood Drive for follow-up" | cargo run -- --skip address
```

Enable Safe Harbor scrubbing to catch insurance IDs, licenses, VINs, and IPs:
```bash
echo "Member # 8392-77-551 with VIN 1HGCM82633A004352" | cargo run -- --safe-harbor
```

## Testing
Run the unit tests with:
```bash
cargo test
```

## Notes
This utility applies heuristic patterns and cannot guarantee complete PHI removal. Always review the output before sharing externally.
