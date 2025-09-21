use std::collections::HashSet;

use anyhow::{anyhow, Result};
use once_cell::sync::Lazy;
use regex::{Captures, Regex, RegexBuilder};
use unicode_normalization::UnicodeNormalization;
use serde::Serialize;

use crate::config::ScrubberConfig;
use crate::Category;

const EMAIL_TOKEN: &str = "[EMAIL]";
const PHONE_TOKEN: &str = "[PHONE]";
const DATE_TOKEN: &str = "[DATE]";
const REL_DATE_TOKEN: &str = "[REL_DATE]";
const SSN_TOKEN: &str = "[SSN]";
const MRN_TOKEN: &str = "[MRN]";
const ADDRESS_TOKEN: &str = "[ADDRESS]";
const PERSON_TOKEN: &str = "[PERSON]";
const FACILITY_TOKEN: &str = "[FACILITY]";
const ZIP_TOKEN: &str = "[ZIP]";
const COORD_TOKEN: &str = "[COORD]";

const DEFAULT_NAMES: &[&str] = &[
    "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis",
    "Rodriguez", "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson",
    "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee", "Perez", "Thompson",
    "White", "Harris", "Sanchez", "Clark", "Ramirez", "Lewis", "Robinson", "Walker",
    "Young", "Allen", "King", "Wright", "Scott", "Torres", "Nguyen", "Hill", "Flores",
    "Green", "Adams", "Nelson", "Baker", "Hall", "Rivera", "Campbell", "Mitchell",
    "Carter", "Roberts", "Gomez", "Phillips", "Turner", "Parker", "Evans", "Edwards",
    "Collins", "Stewart", "Sanchez", "Morris", "Murphy", "Cook", "Rogers", "Morgan",
    "Patel", "Singh", "Khan", "Ali", "Mohammed", "Mohammad", "Abdullah", "Hussain", "Kim", "Park",
    "Chen", "Wang", "Zhang", "Lin", "Tran", "Ng", "Chaudhry", "Ahmad", "Iqbal", "Rahman",
];

const COMMON_FIRST_NAMES: &[&str] = &[
    "James", "Mary", "Robert", "Patricia", "John", "Jennifer", "Michael", "Linda", "William", "Elizabeth",
    "David", "Barbara", "Richard", "Susan", "Joseph", "Jessica", "Thomas", "Sarah", "Charles", "Karen",
    "Christopher", "Nancy", "Daniel", "Lisa", "Matthew", "Betty", "Anthony", "Margaret", "Mark", "Sandra",
    "Donald", "Ashley", "Steven", "Kimberly", "Paul", "Emily", "Andrew", "Donna", "Joshua", "Michelle",
    "Kenneth", "Dorothy", "Kevin", "Carol", "Brian", "Amanda", "George", "Melissa", "Timothy", "Deborah",
    "Ronald", "Stephanie", "Edward", "Rebecca", "Jason", "Sharon", "Jeffrey", "Laura", "Ryan", "Cynthia",
    "Jacob", "Kathleen", "Gary", "Amy", "Nicholas", "Shirley", "Eric", "Angela", "Jonathan", "Helen",
    "Stephen", "Anna", "Larry", "Brenda", "Justin", "Pamela", "Scott", "Nicole", "Brandon", "Samantha",
    "Frank", "Katherine", "Benjamin", "Emma", "Gregory", "Ruth", "Samuel", "Christine", "Patrick", "Catherine",
    "Alexander", "Debra", "Jack", "Rachel", "Dennis", "Carolyn", "Jerry", "Janet", "Tyler", "Maria",
    "Mohammed", "Muhammad", "Ahmed", "Ahmad", "Omar", "Hassan", "Hussein", "Abdullah", "Fatima", "Aisha",
    "Amelia", "Priya", "Anjali", "Sofia", "Noor", "Amina", "Li", "Wei", "Min", "Hao",
    "Jin", "Sang", "Hye", "Yuki", "Mei", "Ravi", "Imran", "Farah", "Leila", "Zara",
];
const DEFAULT_FACILITY_TERMS: &[&str] = &[
    "General Hospital",
    "Medical Center",
    "Children's Hospital",
    "Urgent Care",
    "Cardiology Clinic",
    "Dialysis Center",
    "Health System",
    "Cancer Institute",
    "Family Practice",
    "Primary Care",
    "Internal Medicine",
];


const NAME_STOPLIST: &[&str] = &[
    "CKD", "ESBL", "ICU", "BKA", "IDDM", "MRSA", "ASTHMA", "DIALYSIS", "MEROPENEM", "SEPSIS",
    "HYPERTENSION", "DIABETES", "E COLI", "E. COLI", "HGB", "HCT", "POC", "IV",
];

static MULTISPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^\S\r\n]+").expect("multispace regex"));
static SPACE_AROUND_PUNCT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+([.,;:!?])").expect("punct regex"));
static DUP_PUNCT_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"([.,;:!?]){2,}").expect("dup punct regex"));

#[derive(Debug, Default, Serialize)]
pub struct ScrubStats {
    pub emails: usize,
    pub phones: usize,
    pub dates: usize,
    pub relative_dates: usize,
    pub ssn: usize,
    pub mrn: usize,
    pub zip_codes: usize,
    pub persons: usize,
    pub facilities: usize,
    pub addresses: usize,
    pub coordinates: usize,
}

impl ScrubStats {
    pub fn total(&self) -> usize {
        self.emails
            + self.phones
            + self.dates
            + self.relative_dates
            + self.ssn
            + self.mrn
            + self.zip_codes
            + self.persons
            + self.facilities
            + self.addresses
            + self.coordinates
    }
}

pub struct Scrubber {
    email_regex: Regex,
    phone_regex: Regex,
    ssn_regex: Regex,
    mrn_regex: Regex,
    mrn_label_regex: Regex,
    zip_regex: Regex,
    facility_regex: Regex,
    custom_facility_regex: Option<Regex>,
    address_regex: Regex,
    coordinate_regex: Regex,
    name_dictionary_regex: Option<Regex>,
    titled_name_regex: Regex,
    first_last_regex: Regex,
    capital_sequence_regex: Regex,
    date_regex: Regex,
    relative_date_regex: Regex,
}

impl Scrubber {
    pub fn new(config: ScrubberConfig) -> Result<Self> {
        let mrn_min = config.mrn_min_length.unwrap_or(6);
        let mrn_max = config.mrn_max_length.unwrap_or(10);
        if mrn_min == 0 || mrn_max == 0 || mrn_min > mrn_max {
            return Err(anyhow!("invalid MRN length range: {}-{}", mrn_min, mrn_max));
        }

        let email_regex = RegexBuilder::new(r"(?xi)\b[\w.+%-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
            .case_insensitive(true)
            .build()?;

        let phone_regex = Regex::new(
            r"(?xi)
            \b(?:\+?1[-.\s•·]?)?
            (?:\(?\d{3}\)?|\d{3})[-.\s•·]?
            \d{3}[-.\s•·]?\d{4}
            (?:\s*(?:x|ext\.?|extension)\s*\d{1,6})?
            \b",
        )?;

        let ssn_regex = Regex::new(r"\b(?:\d{3}-\d{2}-\d{4}|xxx-xx-\d{4})\b")?;
        let mrn_regex = Regex::new(&format!(r"\b\d{{{},{}}}\b", mrn_min, mrn_max))?;
        let mrn_label_regex = Regex::new(r"(?i)\b(?:MRN|Acct|Account|Patient\s*ID|Chart)\s*[:#]?\s*-?\s*[A-Za-z0-9-]{4,}\b")?;
        let zip_regex = Regex::new(r"\b\d{5}(?:-\d{4})?\b")?;

        let facility_regex = Regex::new(
            r"(?xi)
            \b(?:St\.|Saint|Mt\.|Mount|Univ\.|University|Memorial|Children'?s|General|County)\s+
            (?:[A-Z][\p{L}\p{M}\p{N}’'\.-]+(?:\s+[A-Z][\p{L}\p{M}\p{N}’'\.-]+){0,4})
            (?:\s+(?:Hospital|Med(?:ical)?\s*Center|Clinic|Health(?:care)?|Infirmary))?
            \b",
        )?;

        let address_regex = Regex::new(
            r"(?xi)
            \b\d{1,6}\s+(?:[A-Z][\w\.-]*\s+){1,5}
            (?:St\.|Street|Ave(?:nue)?|Rd\.?|Road|Dr\.?|Drive|Blvd\.?|Boulevard|Ln\.?|Lane|Ct\.?|Court|Pl\.?|Place|Ter(?:race)?|Way)\b
            (?:\s*(?:Apt|Unit|\#)\s*\w+)?",
        )?;

        let coordinate_regex = Regex::new(
            r"(?xi)
            \b-?\d{1,3}\.\d+\s*(?:°|º)?\s*[NS]\b[,\s]*-?\d{1,3}\.\d+\s*(?:°|º)?\s*[EW]\b
        ",
        )?;

        let facility_terms = build_dictionary(DEFAULT_FACILITY_TERMS, &config.keywords);
        let custom_facility_regex = build_dictionary_regex(&facility_terms)?;

        let names = build_dictionary(DEFAULT_NAMES, &config.names);
        let name_dictionary_regex = build_dictionary_regex(&names)?;
        let titled_name_regex = build_titled_name_regex()?;
        let first_last_regex = build_first_last_regex()?;
        let capital_sequence_regex = build_capital_sequence_regex()?;

        let date_regex = Regex::new(
            r"(?xi)
            \b(
                \d{1,2}[/-]\d{1,2}(?:[/-]\d{2,4})|
                \d{4}-\d{2}-\d{2}|
                (?:Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Sept|Oct|Nov|Dec)[a-z]*\s+\d{1,2},?\s+\d{2,4}
            )\b",
        )?;

        let relative_date_regex = Regex::new(
            r"(?xi)
            \b(
                yesterday|today|tomorrow|
                last\s+(?:night|week|month|year|Monday|Tuesday|Wednesday|Thursday|Friday|Saturday|Sunday)|
                this\s+(?:morning|afternoon|evening|week|month)|
                \d+\s+(?:day|days|week|weeks|month|months|year|years)\s+ago
            )\b",
        )?;

        Ok(Self {
            email_regex,
            phone_regex,
            ssn_regex,
            mrn_regex,
            mrn_label_regex,
            zip_regex,
            facility_regex,
            custom_facility_regex,
            address_regex,
            coordinate_regex,
            name_dictionary_regex,
            titled_name_regex,
            first_last_regex,
            capital_sequence_regex,
            date_regex,
            relative_date_regex,
        })
    }

    pub fn scrub(&self, input: &str, skip: &HashSet<Category>) -> (String, ScrubStats) {
        let normalized = normalize_input(input);
        let mut output = normalized.clone();
        let mut stats = ScrubStats::default();

        if !skip.contains(&Category::Email) {
            let (next, count) = replace_all(&self.email_regex, &output, EMAIL_TOKEN);
            output = next;
            stats.emails = count;
        }

        if !skip.contains(&Category::Phone) {
            let (next, count) = replace_all(&self.phone_regex, &output, PHONE_TOKEN);
            output = next;
            stats.phones = count;
        }

        if !skip.contains(&Category::Ssn) {
            let (next, count) = replace_all(&self.ssn_regex, &output, SSN_TOKEN);
            output = next;
            stats.ssn = count;
        }

        if !skip.contains(&Category::Mrn) {
            let (next, count_a) = replace_all(&self.mrn_label_regex, &output, MRN_TOKEN);
            output = next;
            let (next, count_b) = replace_all(&self.mrn_regex, &output, MRN_TOKEN);
            output = next;
            stats.mrn = count_a + count_b;
        }

        if !skip.contains(&Category::Zip) {
            let (next, count) = replace_all(&self.zip_regex, &output, ZIP_TOKEN);
            output = next;
            stats.zip_codes = count;
        }

        if !skip.contains(&Category::Facility) {
            let (next, count_a) = replace_all(&self.facility_regex, &output, FACILITY_TOKEN);
            output = next;
            let mut facility_total = count_a;
            if let Some(regex) = &self.custom_facility_regex {
                let (next, count_b) = replace_all(regex, &output, FACILITY_TOKEN);
                output = next;
                facility_total += count_b;
            }
            stats.facilities = facility_total;
        }

        if !skip.contains(&Category::Address) {
            let (next, count) = replace_all(&self.address_regex, &output, ADDRESS_TOKEN);
            output = next;
            stats.addresses = count;
        }

        if !skip.contains(&Category::Coordinate) {
            let (next, count) = replace_all(&self.coordinate_regex, &output, COORD_TOKEN);
            output = next;
            stats.coordinates = count;
        }

        if !skip.contains(&Category::Person) {
            let mut person_total = 0;
            if let Some(regex) = &self.name_dictionary_regex {
                let (next, count) = replace_names(regex, &output, PERSON_TOKEN);
                output = next;
                person_total += count;
            }

            let (next, count) = replace_names(&self.titled_name_regex, &output, PERSON_TOKEN);
            output = next;
            person_total += count;

            let (next, count) = replace_names(&self.first_last_regex, &output, PERSON_TOKEN);
            output = next;
            person_total += count;

            let (next, count) = replace_names(&self.capital_sequence_regex, &output, PERSON_TOKEN);
            output = next;
            person_total += count;

            stats.persons = person_total;
        }

        if !skip.contains(&Category::Date) {
            let (next, count) = replace_all(&self.date_regex, &output, DATE_TOKEN);
            output = next;
            stats.dates = count;
        }

        if !skip.contains(&Category::RelativeDate) {
            let (next, count) = replace_all(&self.relative_date_regex, &output, REL_DATE_TOKEN);
            output = next;
            stats.relative_dates = count;
        }

        output = tidy_punctuation(&output);
        (output, stats)
    }
}

fn replace_all(regex: &Regex, input: &str, replacement: &str) -> (String, usize) {
    let mut count = 0;
    let result = regex.replace_all(input, |_: &Captures| {
        count += 1;
        replacement
    });
    (result.into_owned(), count)
}

fn replace_names(regex: &Regex, input: &str, replacement: &str) -> (String, usize) {
    replace_all_filtered(regex, input, replacement, |candidate| !is_name_stopword(candidate))
}

fn replace_all_filtered<F>(regex: &Regex, input: &str, replacement: &str, mut should_replace: F) -> (String, usize)
where
    F: FnMut(&str) -> bool,
{
    let mut count = 0;
    let result = regex.replace_all(input, |caps: &Captures| {
        let mat = caps.get(0).map(|m| m.as_str()).unwrap_or("");
        if should_replace(mat) {
            count += 1;
            replacement.to_string()
        } else {
            mat.to_string()
        }
    });
    (result.into_owned(), count)
}

fn build_dictionary(defaults: &[&str], overrides: &[String]) -> Vec<String> {
    let mut set: HashSet<String> = defaults.iter().map(|s| s.to_string()).collect();
    for entry in overrides {
        if entry.trim().is_empty() {
            continue;
        }
        set.insert(entry.trim().to_string());
    }
    let mut list: Vec<String> = set.into_iter().collect();
    list.sort();
    list
}

fn build_dictionary_regex(entries: &[String]) -> Result<Option<Regex>> {
    if entries.is_empty() {
        return Ok(None);
    }

    let patterns: Vec<String> = entries
        .iter()
        .map(|value| {
            let mut escaped = regex::escape(value);
            escaped = escaped.replace('\u{2019}', "[\u{2019}']");
            escaped = escaped.replace("'", "[\u{2019}']");
            escaped = escaped.replace(r"\ ", r"\s+");
            escaped
        })
        .collect();

    let joined = patterns.join("|");
    let pattern = format!("(?i)\\b(?:{})\\b", joined);
    let regex = Regex::new(&pattern)?;
    Ok(Some(regex))
}

fn build_first_last_regex() -> Result<Regex> {
    let firsts: Vec<String> = COMMON_FIRST_NAMES.iter().map(|name| regex::escape(name)).collect();
    let pattern = format!(
        r"(?xi)\b(?:{})\s+[A-Z][\p{{L}}\u{{2019}}'-]+(?:\s+[A-Z][\p{{L}}\u{{2019}}'-]+)?",
        firsts.join("|")
    );
    Ok(Regex::new(&pattern)?)
}

fn build_titled_name_regex() -> Result<Regex> {
    let pattern = r"(?xi)\b(?:Drs?\.?|Prof\.?|Mr\.?|Mrs\.?|Ms\.?|Mx\.?|Capt\.?|Captain|Lt\.?|Lieutenant|Sgt\.?|Sergeant|Officer|Chief|Judge|Sir|Dame|Madam|Rev\.?|Reverend|Father|Fr\.?|Sister|Brother|Pastor|Chaplain|Rabbi|Imam)\s+[A-Z][\p{L}\u{2019}'-]+(?:\s+[A-Z][\p{L}\u{2019}'-]+)?";
    Ok(Regex::new(pattern)?)
}

fn build_capital_sequence_regex() -> Result<Regex> {
    let pattern = r"(?x)
        \b
        [A-Z][\p{L}\u{2019}']+\s+[A-Z][\p{L}\u{2019}']+
        (?:\s+[A-Z][\p{L}\u{2019}']+)?
        \b";
    Ok(Regex::new(pattern)?)
}

fn is_name_stopword(candidate: &str) -> bool {
    let trimmed = candidate.trim();
    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("st. ") || lower.starts_with("st ") {
        return true;
    }

    let upper: String = trimmed
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ')
        .flat_map(|c| c.to_uppercase())
        .collect();
    NAME_STOPLIST.contains(&upper.trim())
}

fn normalize_input(input: &str) -> String {
    let nfkc: String = input.nfkc().collect();
    let mut normalized = nfkc
        .replace(['\u{2018}', '\u{2019}', '\u{201B}', '\u{2032}'], "'")
        .replace(['\u{201C}', '\u{201D}', '\u{2033}'], "\"")
        .replace(['\u{2013}', '\u{2014}', '\u{2212}'], "-")
        .replace(['\u{2022}', '\u{00B7}', '\u{2027}', '\u{2043}', '\u{30FB}'], " ");

    normalized = MULTISPACE_RE.replace_all(&normalized, " ").into_owned();
    normalized
}

fn tidy_punctuation(input: &str) -> String {
    let mut text = SPACE_AROUND_PUNCT_RE.replace_all(input, "$1").into_owned();
    text = DUP_PUNCT_RE
        .replace_all(&text, |caps: &Captures| caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string())
        .into_owned();
    text.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Category;
    use std::collections::HashSet;

    #[test]
    fn redacts_email_and_phone() {
        let scrubber = Scrubber::new(ScrubberConfig::default()).expect("scrubber");
        let input = "Reach me at jane.doe@example.com or (555) 867-5309.";
        let (output, stats) = scrubber.scrub(input, &HashSet::new());
        assert!(output.contains(EMAIL_TOKEN));
        assert!(output.contains(PHONE_TOKEN));
        assert_eq!(stats.emails, 1);
        assert_eq!(stats.phones, 1);
    }

    #[test]
    fn honors_skip_categories() {
        let scrubber = Scrubber::new(ScrubberConfig::default()).expect("scrubber");
        let input = "Call 555-111-2222 and email foo@bar.com.";
        let mut skip = HashSet::new();
        skip.insert(Category::Phone);
        let (output, stats) = scrubber.scrub(input, &skip);
        assert!(output.contains("555-111-2222"));
        assert!(output.contains(EMAIL_TOKEN));
        assert_eq!(stats.phones, 0);
        assert_eq!(stats.emails, 1);
    }

    #[test]
    fn redacts_custom_names() {
        let config = ScrubberConfig {
            names: vec!["Zelda Fitzgerald".to_string()],
            ..Default::default()
        };
        let scrubber = Scrubber::new(config).expect("scrubber");
        let input = "Discussed plan with Zelda Fitzgerald today.";
        let (output, stats) = scrubber.scrub(input, &HashSet::new());
        assert!(output.contains(PERSON_TOKEN));
        assert_eq!(stats.persons, 1);
    }

    #[test]
    fn redacts_common_first_last_pair() {
        let scrubber = Scrubber::new(ScrubberConfig::default()).expect("scrubber");
        let input = "David Harmon discussed the plan.";
        let (output, stats) = scrubber.scrub(input, &HashSet::new());
        assert!(output.contains(PERSON_TOKEN));
        assert_eq!(stats.persons, 1);
    }

    #[test]
    fn redacts_extended_honorifics() {
        let scrubber = Scrubber::new(ScrubberConfig::default()).expect("scrubber");
        let input = "Rev. O'Connor provided counseling.";
        let (output, stats) = scrubber.scrub(input, &HashSet::new());
        assert!(output.contains(PERSON_TOKEN));
        assert_eq!(stats.persons, 1);
    }

    #[test]
    fn redacts_coordinates() {
        let scrubber = Scrubber::new(ScrubberConfig::default()).expect("scrubber");
        let input = "Coordinates 41.8781° N, 87.6298° W were logged.";
        let (output, stats) = scrubber.scrub(input, &HashSet::new());
        assert!(output.contains(COORD_TOKEN));
        assert_eq!(stats.coordinates, 1);
    }

    #[test]
    fn redacts_titles_and_addresses() {
        let scrubber = Scrubber::new(ScrubberConfig::default()).expect("scrubber");
        let input = "Dr. Harmon visited 128 Elmwood Drive.";
        let (output, stats) = scrubber.scrub(input, &HashSet::new());
        assert!(output.contains(PERSON_TOKEN));
        assert!(output.contains(ADDRESS_TOKEN));
        assert_eq!(stats.persons, 1);
        assert_eq!(stats.addresses, 1);
    }

    #[test]
    fn redacts_saint_facilities_with_curly_apostrophe() {
        let scrubber = Scrubber::new(ScrubberConfig::default()).expect("scrubber");
        let input = "Transferred from St. John\u{2019}s Medical Center.";
        let (output, stats) = scrubber.scrub(input, &HashSet::new());
        assert!(output.contains(FACILITY_TOKEN));
        assert_eq!(stats.facilities, 1);
    }

    #[test]
    fn relative_dates_detected() {
        let scrubber = Scrubber::new(ScrubberConfig::default()).expect("scrubber");
        let input = "Symptoms started 3 days ago and worsened yesterday.";
        let (output, stats) = scrubber.scrub(input, &HashSet::new());
        assert!(output.contains(REL_DATE_TOKEN));
        assert_eq!(stats.relative_dates, 2);
    }
}
