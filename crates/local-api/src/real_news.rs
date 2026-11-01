//! Real headlines (decision 0013): fetching GOV.UK's Department for Education news, matching it against the published keyword
//! rules, and turning what matches into a batch the game freezes into a campaign's record.
//!
//! Nothing here touches the rules of the game. The game never fetches anything and never sees a feed: it is handed a small
//! batch of already-chosen items, once at the start of a term, and replays that batch for ever. If the network is down, or the
//! feed changes shape, the batch is empty and the term simply has no real news.
//!
//! The rules are in `data/england-school-pilot-v1/real-news-rules.json`, in words, and are the whole mapping from a headline to
//! its effect. Only the title is matched (a summary carries too many incidental words), so a headline that happens to contain a
//! rule's phrase in the wrong sense is mis-read the way a keyword rule would misread it; the screen shows which rule matched.

use myth_contracts::{RealItemWire, RealRuleWire, RealToneWire};
use myth_domain as d;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

const RULES: &str = include_str!("../../../data/england-school-pilot-v1/real-news-rules.json");
/// A feed bigger than this is not a feed.
const MAX_FEED_BYTES: usize = 1_048_576;
/// How long a campaign may wait for the news before starting without it.
const FETCH_SECONDS: u64 = 4;

#[derive(Deserialize)]
struct SourceFile {
    feed_url: String,
    allowed_link_prefix: String,
}
#[derive(Deserialize)]
struct RuleFile {
    id: String,
    label: String,
    tone: String,
    any: Vec<String>,
    /// Group id to exposure, in percent of the worst case (bad news).
    exposure_pct: Option<BTreeMap<String, u32>>,
    /// Group id to gain, in ppm (good news).
    gain_ppm: Option<BTreeMap<String, u32>>,
    /// A bad rule may also help some groups (a trade-off story).
    winners_ppm: Option<BTreeMap<String, u32>>,
}
#[derive(Deserialize)]
struct RulesFile {
    source: SourceFile,
    max_items: usize,
    max_with_effect: usize,
    rules: Vec<RuleFile>,
}
/// One rule, ready to match: its phrases normalised, its numbers checked.
pub struct Rule {
    pub id: String,
    pub label: String,
    pub tone: RealToneWire,
    /// Group id to number: exposure in percent for bad news, gain in ppm for good news.
    pub effect: BTreeMap<String, u32>,
    pub winners: BTreeMap<String, u32>,
    phrases: Vec<String>,
}
pub struct Rules {
    pub feed_url: String,
    allowed_link_prefix: String,
    max_items: usize,
    max_with_effect: usize,
    pub rules: Vec<Rule>,
}

/// Lower-case, letters and digits only, single spaces, a space at each end: so a phrase matches whole words only.
fn normalise(s: &str) -> String {
    let mut out = String::from(" ");
    for word in s
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        out.push_str(word);
        out.push(' ');
    }
    if out.len() == 1 { String::new() } else { out }
}

impl Rules {
    /// The published rules, parsed and checked once.
    pub fn load() -> Result<&'static Rules, String> {
        static LOADED: OnceLock<Result<Rules, String>> = OnceLock::new();
        LOADED
            .get_or_init(|| Rules::parse(RULES))
            .as_ref()
            .map_err(Clone::clone)
    }
    fn parse(raw: &str) -> Result<Rules, String> {
        let file: RulesFile =
            serde_json::from_str(raw).map_err(|_| "real news rules unreadable".to_string())?;
        if !file
            .source
            .feed_url
            .starts_with(&file.source.allowed_link_prefix)
            || !file.source.allowed_link_prefix.starts_with("https://")
            || file.max_with_effect > d::news::MAX_REAL_PER_TERM
            || file.max_items > d::news::MAX_REAL_ITEMS
            || file.max_with_effect > file.max_items
        {
            return Err("real news rules out of bounds".into());
        }
        let mut rules = vec![];
        for r in file.rules {
            let (tone, effect) = match (r.tone.as_str(), r.exposure_pct, r.gain_ppm) {
                ("bad", Some(e), None) => (RealToneWire::Bad, e),
                ("good", None, Some(g)) if r.winners_ppm.is_none() => (RealToneWire::Good, g),
                _ => return Err(format!("real news rule {} is malformed", r.id)),
            };
            let phrases: Vec<String> = r.any.iter().map(|p| normalise(p)).collect();
            if phrases.is_empty() || phrases.iter().any(String::is_empty) {
                return Err(format!("real news rule {} has an empty phrase", r.id));
            }
            rules.push(Rule {
                id: r.id,
                label: r.label,
                tone,
                effect,
                winners: r.winners_ppm.unwrap_or_default(),
                phrases,
            });
        }
        // The same limits as for the batch the game accepts.
        let probe = RealItemWire {
            title: "probe".into(),
            summary: "probe".into(),
            url: file.source.feed_url.clone(),
            published: "2026-01-01".into(),
            rule: None,
        };
        if !d::news::RealItem::from(probe.clone()).is_valid() || rules.is_empty() {
            return Err("real news rules out of bounds".into());
        }
        let population = myth_hiyal_adapter::population()?;
        for r in &rules {
            let item = RealItemWire {
                rule: Some(wire_rule(r)),
                ..probe.clone()
            };
            let item = d::news::RealItem::from(item);
            if !item.is_valid() {
                return Err(format!("real news rule {} is out of bounds", r.id));
            }
            // Every group a rule names must exist in the scenario's population, and no voters may be both hurt and helped.
            if !d::news::valid_import_for(&[item], population) {
                return Err(format!(
                    "real news rule {} does not fit the population",
                    r.id
                ));
            }
        }
        Ok(Rules {
            feed_url: file.source.feed_url,
            allowed_link_prefix: file.source.allowed_link_prefix,
            max_items: file.max_items,
            max_with_effect: file.max_with_effect,
            rules,
        })
    }
    /// The first rule one of whose phrases is in the title, as whole words.
    pub fn classify(&self, title: &str) -> Option<&Rule> {
        let text = normalise(title);
        self.rules
            .iter()
            .find(|r| r.phrases.iter().any(|p| text.contains(p.as_str())))
    }
}
fn wire_rule(r: &Rule) -> RealRuleWire {
    RealRuleWire {
        id: r.id.clone(),
        label: r.label.clone(),
        tone: r.tone,
        effect: r.effect.clone(),
        winners: r.winners.clone(),
    }
}
/// One entry of an Atom feed, as published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeedEntry {
    pub title: String,
    pub summary: String,
    pub link: String,
    /// `YYYY-MM-DD`.
    pub published: String,
}

fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';').filter(|e| *e <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|n| n.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut inside = false;
    for c in s.chars() {
        match c {
            '<' => inside = true,
            '>' if inside => {
                inside = false;
                out.push(' ');
            }
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out
}
/// Entities decoded, markup removed, whitespace collapsed, control characters dropped.
fn clean_text(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix("<![CDATA[")
        .and_then(|x| x.strip_suffix("]]>"))
        .unwrap_or(s);
    // A feed may carry HTML escaped once (or twice): decode, strip, decode again for what the markup hid.
    let text = decode_entities(&strip_tags(&decode_entities(s)));
    text.split_whitespace()
        .map(|w| w.chars().filter(|c| !c.is_control()).collect::<String>())
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_owned();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", cut.trim_end())
}
/// The text between `<tag ...>` and `</tag>` in the first element called `tag`.
fn element<'a>(block: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}");
    let mut from = 0;
    while let Some(i) = block[from..].find(&open) {
        let start = from + i + open.len();
        let next = block[start..].chars().next()?;
        if next == '>' || next == ' ' || next == '/' || next == '\n' || next == '\t' {
            let tag_end = block[start..].find('>')? + start;
            if block[..tag_end].ends_with('/') {
                return Some("");
            }
            let close = format!("</{tag}>");
            let end = block[tag_end + 1..].find(&close)? + tag_end + 1;
            return Some(&block[tag_end + 1..end]);
        }
        from = start;
    }
    None
}
/// The `href` of the entry's alternate link: the first `<link>` with no `rel`, or `rel="alternate"`.
fn link_href(block: &str) -> Option<String> {
    let mut from = 0;
    while let Some(i) = block[from..].find("<link") {
        let start = from + i + "<link".len();
        let end = block[start..].find('>')? + start;
        let attrs = &block[start..end];
        let attr = |name: &str| -> Option<String> {
            let key = format!("{name}=");
            let at = attrs.find(&key)? + key.len();
            let quote = attrs[at..]
                .chars()
                .next()
                .filter(|c| *c == '"' || *c == '\'')?;
            let rest = &attrs[at + 1..];
            Some(decode_entities(&rest[..rest.find(quote)?]))
        };
        if attr("rel").is_none_or(|r| r == "alternate")
            && let Some(href) = attr("href")
        {
            return Some(href);
        }
        from = end;
    }
    None
}
/// `2026-11-02T09:30:00+00:00` to `2026-11-02`, if it looks like a date.
fn day_of(stamp: &str) -> Option<String> {
    let day = stamp.trim().get(..10)?;
    let b = day.as_bytes();
    (b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()))
    .then(|| day.to_owned())
}
/// Every entry of an Atom feed that has a title, a link and a date. A tolerant scan, not a validating parser: whatever
/// cannot be read is left out, and the game never depends on any one entry.
pub fn parse_atom(xml: &str) -> Vec<FeedEntry> {
    let mut entries = vec![];
    let mut rest = xml;
    while let Some(i) = rest.find("<entry") {
        let after = &rest[i + "<entry".len()..];
        let Some(end) = after.find("</entry>") else {
            break;
        };
        let block = &after[..end];
        rest = &after[end + "</entry>".len()..];
        let title = element(block, "title").map(clean_text).unwrap_or_default();
        let link = link_href(block).unwrap_or_default();
        let published = element(block, "updated")
            .or_else(|| element(block, "published"))
            .and_then(day_of);
        let summary = element(block, "summary")
            .or_else(|| element(block, "content"))
            .map(clean_text)
            .unwrap_or_default();
        if let (false, false, Some(published)) = (title.is_empty(), link.is_empty(), published) {
            entries.push(FeedEntry {
                title,
                summary,
                link,
                published,
            });
        }
    }
    entries
}

/// Choose the batch for a term: the newest stories first, at most two that a rule matched (they will change support) and up to
/// two more that matched nothing (shown, changing nothing). A story already used in this game, from outside the allowed
/// links, or twice in the feed, is left out.
pub fn build_batch(
    rules: &Rules,
    entries: &[FeedEntry],
    used_links: &BTreeSet<String>,
) -> Vec<RealItemWire> {
    let mut seen = BTreeSet::new();
    let mut fresh: Vec<&FeedEntry> = entries
        .iter()
        .filter(|e| {
            e.link.starts_with(&rules.allowed_link_prefix)
                && !used_links.contains(&e.link)
                && seen.insert(e.link.clone())
        })
        .collect();
    // Newest first; the feed's own order breaks ties (a stable sort).
    fresh.sort_by(|a, b| b.published.cmp(&a.published));
    let mut matched = vec![];
    let mut informational = vec![];
    for entry in fresh {
        let rule = rules.classify(&entry.title);
        let item = RealItemWire {
            title: truncate_chars(&entry.title, 200),
            summary: truncate_chars(
                if entry.summary.is_empty() {
                    "No summary was published."
                } else {
                    &entry.summary
                },
                280,
            ),
            url: entry.link.clone(),
            published: entry.published.clone(),
            rule: rule.map(wire_rule),
        };
        // The game's own check, so what leaves here is always accepted there.
        if !d::news::RealItem::from(item.clone()).is_valid() {
            continue;
        }
        if rule.is_some() && matched.len() < rules.max_with_effect {
            matched.push(item);
        } else if rule.is_none() && informational.len() < 2 {
            informational.push(item);
        }
    }
    matched.extend(informational);
    matched.truncate(rules.max_items);
    matched
}

/// Where the feed comes from.
#[derive(Clone, Debug)]
pub enum NewsSource {
    /// No real news: the game plays on invented headlines alone.
    Off,
    /// A feed file on disk, for testing and for playing offline.
    File(PathBuf),
    /// A feed on the web, GOV.UK only.
    Http(String),
}
impl NewsSource {
    /// `MYTH_REAL_NEWS`: `off`, `file:<path>`, or (the default) GOV.UK's Department for Education news.
    pub fn from_env() -> Self {
        match std::env::var("MYTH_REAL_NEWS").ok().as_deref() {
            Some("off") => Self::Off,
            Some(spec) if spec.starts_with("file:") => Self::File(PathBuf::from(&spec[5..])),
            _ => Rules::load().map_or(Self::Off, |r| Self::Http(r.feed_url.clone())),
        }
    }
    pub fn is_on(&self) -> bool {
        !matches!(self, Self::Off)
    }
    /// The feed's text, or why it could not be had. Never panics and never waits more than a few seconds.
    pub async fn fetch(&self) -> Result<String, String> {
        match self {
            Self::Off => Err("real news is off".into()),
            Self::File(path) => {
                let bytes =
                    std::fs::read(path).map_err(|e| format!("cannot read feed file: {e}"))?;
                if bytes.len() > MAX_FEED_BYTES {
                    return Err("feed file too large".into());
                }
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            }
            Self::Http(url) => {
                let rules = Rules::load()?;
                if !url.starts_with(&rules.allowed_link_prefix) {
                    return Err("feed is not from an allowed site".into());
                }
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(FETCH_SECONDS))
                    .redirect(reqwest::redirect::Policy::limited(3))
                    .user_agent("myth-maker-local/0.1 (+offline game; fetches headlines only)")
                    .build()
                    .map_err(|e| format!("cannot start a request: {e}"))?;
                let response = client
                    .get(url)
                    .send()
                    .await
                    .map_err(|e| format!("cannot reach the feed: {e}"))?;
                if !response.status().is_success() {
                    return Err(format!("the feed answered {}", response.status()));
                }
                if response
                    .content_length()
                    .is_some_and(|n| n as usize > MAX_FEED_BYTES)
                {
                    return Err("feed too large".into());
                }
                let bytes = response
                    .bytes()
                    .await
                    .map_err(|e| format!("cannot read the feed: {e}"))?;
                if bytes.len() > MAX_FEED_BYTES {
                    return Err("feed too large".into());
                }
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            }
        }
    }
    /// The batch to freeze into a campaign at the start of a term: empty when real news is off or anything goes wrong.
    pub async fn batch(&self, used_links: &BTreeSet<String>) -> Vec<RealItemWire> {
        if !self.is_on() {
            return vec![];
        }
        let Ok(rules) = Rules::load() else {
            return vec![];
        };
        match self.fetch().await {
            Ok(xml) => build_batch(rules, &parse_atom(&xml), used_links),
            Err(reason) => {
                eprintln!("Real news unavailable, so this term has none: {reason}");
                vec![]
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// What a rule does to each cell of the scenario's population.
    fn rule_effect(rule: &Rule) -> d::news::Effect {
        let wire = wire_rule(rule);
        let item = d::news::RealItem::from(RealItemWire {
            title: "t".into(),
            summary: "s".into(),
            url: "https://www.gov.uk/x".into(),
            published: "2026-01-01".into(),
            rule: Some(wire),
        });
        item.rule
            .unwrap()
            .effect
            .resolve(myth_hiyal_adapter::population().unwrap())
            .unwrap()
    }

    /// A feed written by hand for these tests, in the Atom layout GOV.UK's finders use (title, updated, an alternate link, an
    /// HTML summary). It is NOT a capture of the real feed and says nothing about real events: the headlines are made up so
    /// that each rule, and the cases around the rules, can be exercised.
    pub(crate) const FIXTURE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>Test fixture: not a real feed</title>
  <entry>
    <id>tag:test,2026:1</id>
    <updated>2026-11-02T09:30:00+00:00</updated>
    <link rel="alternate" type="text/html" href="https://www.gov.uk/government/news/teacher-pay-award-confirmed"/>
    <title type="text">Teacher pay award confirmed for next year</title>
    <summary type="html">&lt;p&gt;Schools will be funded for the award &amp;amp; given notice.&lt;/p&gt;</summary>
  </entry>
  <entry>
    <id>tag:test,2026:2</id>
    <updated>2026-11-01T16:00:00+00:00</updated>
    <link rel="alternate" type="text/html" href="https://www.gov.uk/government/news/raac-survey-finds-more-unsafe-buildings"/>
    <title type="text">RAAC survey finds more unsafe buildings</title>
    <summary type="text">Surveyors have found more affected sites.</summary>
  </entry>
  <entry>
    <id>tag:test,2026:3</id>
    <updated>2026-10-31T10:00:00+00:00</updated>
    <link rel="alternate" type="text/html" href="https://www.gov.uk/government/news/strike-ballot-opens"/>
    <title type="text">Strike ballot opens among teachers</title>
    <summary type="text">A ballot opens today.</summary>
  </entry>
  <entry>
    <id>tag:test,2026:4</id>
    <updated>2026-10-30T10:00:00+00:00</updated>
    <link rel="alternate" type="text/html" href="https://www.gov.uk/government/news/new-guidance-published"/>
    <title type="text">New guidance published for school governors</title>
    <summary type="text">Guidance for governing boards.</summary>
  </entry>
  <entry>
    <id>tag:test,2026:5</id>
    <updated>2026-10-29T10:00:00+00:00</updated>
    <link rel="alternate" type="text/html" href="https://example.com/not-gov-uk"/>
    <title type="text">Extra funding for schools announced</title>
    <summary type="text">Not from the allowed site.</summary>
  </entry>
  <entry>
    <id>tag:test,2026:6</id>
    <updated>2026-10-28T10:00:00+00:00</updated>
    <link rel="alternate" type="text/html" href="https://www.gov.uk/government/news/consultation-opens"/>
    <title type="text">Consultation opens on the curriculum</title>
    <summary type="text">A consultation.</summary>
  </entry>
  <entry>
    <id>tag:test,2026:7</id>
    <updated>not a date</updated>
    <link rel="alternate" href="https://www.gov.uk/government/news/undated"/>
    <title type="text">Undated story with extra funding</title>
  </entry>
</feed>"#;

    #[test]
    fn the_published_rules_load_and_stay_within_the_games_limits() {
        let rules = Rules::load().unwrap();
        assert!(rules.rules.len() >= 8);
        assert!(rules.feed_url.starts_with("https://www.gov.uk/"));
        let mut ids: Vec<_> = rules.rules.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), rules.rules.len(), "every rule has its own id");
        // Both tones are covered, so real news can help as well as hurt.
        assert!(rules.rules.iter().any(|r| r.tone == RealToneWire::Good));
        assert!(rules.rules.iter().any(|r| r.tone == RealToneWire::Bad));
    }

    #[test]
    fn the_atom_feed_is_read_tolerantly_and_what_cannot_be_read_is_left_out() {
        let entries = parse_atom(FIXTURE);
        // The undated story is dropped; the rest are read in feed order.
        assert_eq!(entries.len(), 6);
        assert_eq!(
            entries[0].title,
            "Teacher pay award confirmed for next year"
        );
        assert_eq!(entries[0].published, "2026-11-02");
        assert_eq!(
            entries[0].link,
            "https://www.gov.uk/government/news/teacher-pay-award-confirmed"
        );
        // Escaped HTML in the summary is turned into plain text, entities included.
        assert_eq!(
            entries[0].summary,
            "Schools will be funded for the award & given notice."
        );
        assert!(parse_atom("not xml at all").is_empty());
        assert!(
            parse_atom("<entry><title>x</title>").is_empty(),
            "an entry left open is not read"
        );
    }

    #[test]
    fn a_title_matches_a_rule_by_whole_words_and_the_first_rule_wins() {
        let rules = Rules::load().unwrap();
        let id = |t: &str| rules.classify(t).map(|r| r.id.as_str());
        assert_eq!(
            id("Teacher pay award confirmed for next year"),
            Some("teacher_pay_good")
        );
        assert_eq!(
            id("RAAC survey finds more unsafe buildings"),
            Some("school_buildings_bad")
        );
        assert_eq!(
            id("Strike ballot opens among teachers"),
            Some("teacher_supply_bad")
        );
        assert_eq!(id("New guidance published for school governors"), None);
        // Two rules are trade-offs: a story that helps some groups while it hurts others.
        assert_eq!(
            id("Funding formula reform confirmed"),
            Some("funding_shift_mixed")
        );
        assert_eq!(
            id("Small schools to be merged"),
            Some("small_schools_mixed")
        );
        for title in [
            "Funding formula reform confirmed",
            "Small schools to be merged",
        ] {
            let rule = rules.classify(title).unwrap();
            let cells = rule_effect(rule);
            assert!(cells.exposure.iter().any(|e| *e > 0) && cells.winners.iter().any(|w| *w > 0));
            assert!(
                cells
                    .exposure
                    .iter()
                    .zip(&cells.winners)
                    .all(|(loses, gains)| *loses == 0 || *gains == 0),
                "no voters are both hurt and helped"
            );
        }
        // Whole words: "strikes" is not "strike", and a phrase inside a longer word does not count.
        assert_eq!(id("Education secretary strikes a deal"), None);
        assert_eq!(id("Preaching about raacs"), None);
        // Capitals, punctuation and extra spaces do not matter.
        assert_eq!(id("  STRIKE!  "), Some("teacher_supply_bad"));
        // A title with both a good and a bad phrase is decided by rule order, which is the published order.
        assert_eq!(id("Pay award ends strike"), Some("teacher_pay_good"));
    }

    #[test]
    fn a_batch_is_the_newest_matches_then_the_newest_that_matched_nothing_and_never_repeats_a_story()
     {
        let rules = Rules::load().unwrap();
        let entries = parse_atom(FIXTURE);
        let batch = build_batch(rules, &entries, &BTreeSet::new());
        let titles: Vec<_> = batch.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                // At most two that change support, newest first...
                "Teacher pay award confirmed for next year",
                "RAAC survey finds more unsafe buildings",
                // ...then the newest that match nothing. The strike story is a third match, so it waits (and is not shown).
                "New guidance published for school governors",
                "Consultation opens on the curriculum",
            ]
        );
        assert!(
            batch[0]
                .rule
                .as_ref()
                .is_some_and(|r| r.id == "teacher_pay_good" && r.tone == RealToneWire::Good)
        );
        assert!(batch[2].rule.is_none() && batch[3].rule.is_none());
        // The link that is not from GOV.UK never appears.
        assert!(
            batch
                .iter()
                .all(|i| i.url.starts_with("https://www.gov.uk/"))
        );
        // The game accepts exactly what is sent.
        let items: Vec<d::news::RealItem> = batch.iter().cloned().map(Into::into).collect();
        assert!(d::news::valid_import(&items));
        // A story already used earlier in the game is skipped, so the next newest takes its place.
        let used: BTreeSet<String> = [batch[0].url.clone()].into();
        let next = build_batch(rules, &entries, &used);
        assert_eq!(next[0].title, "RAAC survey finds more unsafe buildings");
        assert_eq!(next[1].title, "Strike ballot opens among teachers");
        assert!(next.iter().all(|i| i.url != batch[0].url));
        // Nothing usable gives an empty batch, never an error.
        assert!(build_batch(rules, &[], &BTreeSet::new()).is_empty());
    }

    #[test]
    fn long_text_is_cut_and_markup_and_control_characters_never_reach_the_game() {
        let long = "word ".repeat(200);
        let xml = format!(
            "<entry><updated>2026-11-02</updated><link href=\"https://www.gov.uk/x\"/><title>Extra funding \u{7} for schools</title><summary>&lt;script&gt;alert(1)&lt;/script&gt;{long}</summary></entry>"
        );
        let entries = parse_atom(&xml);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].title, "Extra funding for schools");
        assert!(!entries[0].summary.contains('<'));
        let batch = build_batch(Rules::load().unwrap(), &entries, &BTreeSet::new());
        assert_eq!(batch.len(), 1);
        assert!(batch[0].summary.chars().count() <= 280);
        assert!(batch[0].summary.ends_with('…'));
    }

    #[tokio::test]
    async fn a_missing_file_or_a_site_that_is_not_gov_uk_gives_no_news_and_no_panic() {
        let none = NewsSource::File(PathBuf::from("/nonexistent/feed.xml"));
        assert!(none.fetch().await.is_err());
        assert!(none.batch(&BTreeSet::new()).await.is_empty());
        assert!(NewsSource::Off.batch(&BTreeSet::new()).await.is_empty());
        let elsewhere = NewsSource::Http("https://example.com/feed.atom".into());
        assert!(elsewhere.fetch().await.is_err());
    }

    #[tokio::test]
    async fn a_feed_file_gives_the_same_batch_as_parsing_it() {
        let dir = std::env::temp_dir().join(format!("myth-news-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("feed.xml");
        std::fs::write(&path, FIXTURE).unwrap();
        let from_file = NewsSource::File(path).batch(&BTreeSet::new()).await;
        let direct = build_batch(
            Rules::load().unwrap(),
            &parse_atom(FIXTURE),
            &BTreeSet::new(),
        );
        assert_eq!(from_file, direct);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
