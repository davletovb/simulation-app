//! Shows what a new campaign would freeze into its record: fetches the configured real-news feed and prints every entry with
//! the keyword rule that matched it (decision 0013). Run it to check that the feed is reachable and that the rules read it
//! the way you expect; it changes nothing.
//!
//!     cargo run -p myth-local-api --bin myth-news
//!     MYTH_REAL_NEWS=file:feed.xml cargo run -p myth-local-api --bin myth-news
use myth_local_api::real_news::{NewsSource, Rules, build_batch, parse_atom};
use std::collections::BTreeSet;
use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let rules = match Rules::load() {
        Ok(rules) => rules,
        Err(reason) => {
            eprintln!("The rules could not be read: {reason}");
            return ExitCode::FAILURE;
        }
    };
    let source = NewsSource::from_env();
    match &source {
        NewsSource::Off => {
            println!("Real news is off (MYTH_REAL_NEWS=off). Nothing to fetch.");
            return ExitCode::SUCCESS;
        }
        NewsSource::File(path) => println!("Reading the feed file {}", path.display()),
        NewsSource::Http(url) => println!("Fetching {url}"),
    }
    let xml = match source.fetch().await {
        Ok(xml) => xml,
        Err(reason) => {
            eprintln!("Could not get the feed: {reason}");
            eprintln!(
                "A campaign started now would have no real news. If this machine is online, check that www.gov.uk can be reached."
            );
            return ExitCode::FAILURE;
        }
    };
    let entries = parse_atom(&xml);
    println!("\n{} entries read:\n", entries.len());
    for entry in &entries {
        let rule = rules.classify(&entry.title).map_or("-", |r| r.id.as_str());
        println!("  {}  {:<22} {}", entry.published, rule, entry.title);
    }
    let batch = build_batch(rules, &entries, &BTreeSet::new());
    println!("\nA new campaign would freeze these, newest first:\n");
    if batch.is_empty() {
        println!("  (nothing: no entry could be used)");
    }
    for item in &batch {
        match &item.rule {
            Some(rule) => println!(
                "  CHANGES SUPPORT  {} [{}] {}",
                item.published, rule.id, item.title
            ),
            None => println!("  only informs     {} {}", item.published, item.title),
        }
    }
    ExitCode::SUCCESS
}
