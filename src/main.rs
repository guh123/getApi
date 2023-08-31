use std::future::Future;
use std::pin::Pin;
use std::process::Output;
use std::result;
use reqwest::get;
use scraper::{Html, Selector};
use regex::Regex;
use std::io::{self, BufRead};
use clap::{App, Arg};
use futures::future::join_all;
use std::fs::File;
use std::io::BufReader;
use tokio::sync::Mutex;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let matches = App::new("My Program")
        .arg(Arg::new("threads")
             .short('t')
             .long("threads")
             .takes_value(true)
             .default_value("1"))
        .arg(Arg::new("file")
             .short('f')
             .long("file")
             .takes_value(true)
             .required(true))
        .arg(Arg::new("depth")
             .short('d')
             .long("depth")
             .takes_value(true)
             .default_value("3"))
        .get_matches();

    let file_path = matches.value_of("file").unwrap();
    let depth: usize = matches.value_of_t("depth").unwrap_or_else(|e| e.exit());

    run(file_path, depth).await;
}

async fn run(file_path: &str, depth: usize) {
    let file = File::open(file_path).expect("Unable to open file");
    let reader = BufReader::new(file);
    let regex_patterns: Vec<String> = reader.lines().filter_map(Result::ok).collect();

    let stdin = io::stdin();
    let mut tasks = vec![];

    let current = Arc::new(Mutex::new(0));

    for line in stdin.lock().lines().filter_map(Result::ok) {
        let current_clone = Arc::clone(&current);
        let task = process_url(line, regex_patterns.clone(), depth, current_clone);
        tasks.push(task);
    }

    join_all(tasks).await;
}

fn process_url(url: String, regex_patterns: Vec<String>, depth: usize, current: Arc<Mutex<usize>>) -> Pin<Box<dyn Future<Output= ()>>> {
    Box::pin(async move {

        let mut guard = current.lock().await;
        if *guard >= depth {
            return;
        }
        *guard += 1;
        drop(guard);

        match get(&url).await {
            Ok(resp) => {
                if let Ok(body) = resp.text().await {
                    if url.ends_with(".js") {
                        extract_api(body.clone(), &regex_patterns).await;
                    } else {
                        let current_clone = Arc::clone(&current);
                        extract_links_and_api(body.clone(), regex_patterns.clone(), depth, current_clone).await;
                    }
                }
            }
            Err(_) => println!("Failed to fetch {}", url),
        }
    })
}

fn extract_links_and_api(html: String, regex_patterns: Vec<String>, depth: usize, current: Arc<Mutex<usize>>) -> Pin<Box<dyn Future<Output = ()>>> {
    Box::pin(async move {
        let parsed_html = Html::parse_document(html.as_str());

        let a_selector = Selector::parse("a").unwrap();

        let mut tasks = Vec::new();
        eprintln!("extract links");
        for element in parsed_html.select(&a_selector) {
            let mut result_link:  String = String::new();
            if let Some(link_value) = element.value().attr("href") {
                if !link_value.is_empty() {
                    if link_value.starts_with("//"){
                        result_link =  format!("http:{}", link_value);
                    }  else if !link_value.starts_with("http")  {
                        result_link =  format!("http://{}",link_value);
                    } else {
                        result_link = link_value.to_string();
                    }
                    println!("{}", result_link.trim());
                    let current_clone = Arc::clone(&current);
                    let task = process_url(result_link, regex_patterns.clone(), depth, current_clone);
                    tasks.push(task);
                }
            }
        }

        join_all(tasks).await;

        extract_api(html, &regex_patterns).await;
    })
}

async fn extract_api(content: String, regex_patterns: &[String]) {
    for pattern in regex_patterns {
        let re = Regex::new(pattern).unwrap();
        for cap in re.captures_iter(content.as_str()) {
            if !cap[0].trim().is_empty() {
                println!("{}", cap[0].trim());
            }
        }
    }
}
