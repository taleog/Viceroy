use arboard::Clipboard;
use std::io::{self, Write};
use std::thread;
use tokio::runtime::Runtime;
use viceroy::search_engine::{self, SearchResult};
use viceroy::{
    app_launcher, clipboard, database, dictionary, sync, system_commands, updater, usage,
    web_search,
};

pub fn run() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_cli_help();
        return;
    }

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    if let Err(err) = database::init() {
        eprintln!("Database init error: {err}");
        return;
    }
    if let Err(err) = sync::init() {
        eprintln!("Sync init error: {err:#}");
    }
    if let Err(err) = sync::start_background_worker() {
        eprintln!("Sync worker start error: {err:#}");
    }

    start_clipboard_monitor();

    let runtime = Runtime::new().expect("failed to create tokio runtime");
    maybe_check_for_updates(&runtime, &args);

    let query_args = extract_query_args(&args);
    if !query_args.is_empty() {
        let query = query_args.join(" ");
        run_single_query(&runtime, &query);
        return;
    }

    run_repl(&runtime);
}

#[cfg(target_os = "windows")]
const PREVIEW_LABEL: &str = "Windows preview";

#[cfg(not(target_os = "windows"))]
const PREVIEW_LABEL: &str = "Cross-platform preview";

fn print_cli_help() {
    println!("Viceroy v{}", env!("CARGO_PKG_VERSION"));
    println!("{PREVIEW_LABEL} launcher");
    println!();
    println!("Usage:");
    println!("  viceroy [query]");
    println!("  viceroy --help");
    println!();
    println!("Interactive commands:");
    println!("  :open N      Run the selected result");
    println!("  :copy N      Copy a result payload to the clipboard");
    println!("  :history     Show recent clipboard items");
    println!("  :quit        Exit");
}

fn maybe_check_for_updates(runtime: &Runtime, args: &[String]) {
    if updater::update_check_disabled(args) {
        return;
    }

    let silent = updater::silent_update_check(args);
    if let Err(err) = runtime.block_on(updater::check_for_updates(silent)) {
        log::error!("Update check failed: {err:#}");
    }
}

fn extract_query_args(args: &[String]) -> Vec<String> {
    args.iter()
        .skip(1)
        .filter(|arg| !arg.starts_with("--"))
        .cloned()
        .collect()
}

fn start_clipboard_monitor() {
    thread::spawn(|| {
        let runtime = Runtime::new().expect("failed to create clipboard runtime");
        runtime.block_on(async {
            if let Err(err) = clipboard::start_monitor().await {
                eprintln!("Clipboard monitor error: {err}");
            }
        });
    });
}

fn run_single_query(runtime: &Runtime, query: &str) {
    match runtime.block_on(search_engine::search(query)) {
        Ok(results) => {
            if results.is_empty() {
                println!("No results for \"{query}\"");
                return;
            }
            print_results(&results);
        }
        Err(err) => eprintln!("Search failed: {err:#}"),
    }
}

fn run_repl(runtime: &Runtime) {
    println!("Viceroy {PREVIEW_LABEL}");
    println!("Type a query, then use :open N or :copy N. :quit exits.");

    let mut last_results = Vec::new();

    loop {
        print!("viceroy> ");
        let _ = io::stdout().flush();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(0) => break,
            Ok(_) => {}
            Err(err) => {
                eprintln!("Input error: {err}");
                break;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }
        if input == ":quit" || input == ":q" {
            break;
        }
        if input == ":history" {
            show_clipboard_history(runtime);
            continue;
        }
        if let Some(index) = parse_index_command(input, ":open") {
            run_result(runtime, &last_results, index);
            continue;
        }
        if let Some(index) = parse_index_command(input, ":copy") {
            copy_result(runtime, &last_results, index);
            continue;
        }

        match runtime.block_on(search_engine::search(input)) {
            Ok(results) => {
                if results.is_empty() {
                    println!("No results");
                } else {
                    print_results(&results);
                }
                last_results = results;
            }
            Err(err) => eprintln!("Search failed: {err:#}"),
        }
    }
}

fn parse_index_command(input: &str, prefix: &str) -> Option<usize> {
    let rest = input.strip_prefix(prefix)?.trim();
    let parsed = rest.parse::<usize>().ok()?;
    parsed.checked_sub(1)
}

fn show_clipboard_history(runtime: &Runtime) {
    match runtime.block_on(clipboard::get_history(10)) {
        Ok(entries) if entries.is_empty() => println!("Clipboard history is empty"),
        Ok(entries) => {
            for (index, entry) in entries.iter().enumerate() {
                let label = if entry.content_type == "image" {
                    entry
                        .custom_name
                        .clone()
                        .unwrap_or_else(|| "[image]".to_string())
                } else {
                    entry.content.chars().take(60).collect::<String>()
                };
                println!("{:>2}. {}", index + 1, label);
            }
        }
        Err(err) => eprintln!("Failed to read clipboard history: {err:#}"),
    }
}

fn print_results(results: &[SearchResult]) {
    for (index, result) in results.iter().take(12).enumerate() {
        println!("{:>2}. {}", index + 1, render_result(result));
    }
}

fn render_result(result: &SearchResult) -> String {
    match result {
        SearchResult::App { name, path, .. } => format!("[app] {name} ({path})"),
        SearchResult::File { name, path, .. } => format!("[file] {name} ({path})"),
        SearchResult::Clipboard {
            preview,
            content_type,
            custom_name,
            ..
        } => {
            let title = custom_name.as_deref().unwrap_or(preview);
            format!("[clipboard:{content_type}] {title}")
        }
        SearchResult::Command {
            name, description, ..
        } => format!("[command] {name} - {description}"),
        SearchResult::Calculator {
            expression, result, ..
        } => format!("[calc] {expression} = {result}"),
        SearchResult::Emoji { emoji, name, .. } => format!("[emoji] {emoji} {name}"),
        SearchResult::Dictionary { word, .. } => format!("[dictionary] {word}"),
        SearchResult::WebSearch { engine, query, .. } => {
            format!("[web:{engine}] {query}")
        }
    }
}

fn run_result(runtime: &Runtime, results: &[SearchResult], index: usize) {
    let Some(result) = results.get(index) else {
        eprintln!("No result {}", index + 1);
        return;
    };

    match execute_result(runtime, result) {
        Ok(message) => println!("{message}"),
        Err(err) => eprintln!("Action failed: {err:#}"),
    }
}

fn copy_result(runtime: &Runtime, results: &[SearchResult], index: usize) {
    let Some(result) = results.get(index) else {
        eprintln!("No result {}", index + 1);
        return;
    };

    match copy_result_payload(runtime, result) {
        Ok(message) => println!("{message}"),
        Err(err) => eprintln!("Copy failed: {err:#}"),
    }
}

fn execute_result(runtime: &Runtime, result: &SearchResult) -> anyhow::Result<String> {
    match result {
        SearchResult::App { name, path, .. } => {
            usage::record_app_launch(path);
            app_launcher::launch(path)?;
            Ok(format!("Launched {name}"))
        }
        SearchResult::File { name, path, .. } => {
            app_launcher::open_file(path)?;
            Ok(format!("Opened {name}"))
        }
        SearchResult::Clipboard {
            content,
            content_type,
            image_width,
            image_height,
            ..
        } => {
            runtime.block_on(clipboard::restore_history_entry_to_clipboard(
                content,
                content_type,
                *image_width,
                *image_height,
            ))?;
            Ok("Clipboard entry restored to the system clipboard".to_string())
        }
        SearchResult::Command { command, .. } => {
            runtime.block_on(system_commands::execute(command))
        }
        SearchResult::Calculator { result, .. } => {
            copy_text(result)?;
            Ok("Calculator result copied to the clipboard".to_string())
        }
        SearchResult::Emoji { emoji, .. } => {
            copy_text(emoji)?;
            Ok("Emoji copied to the clipboard".to_string())
        }
        SearchResult::Dictionary { word, .. } => {
            dictionary::open_dictionary(word)?;
            Ok(format!("Opened a definition for {word}"))
        }
        SearchResult::WebSearch { url, .. } => {
            web_search::open_web_search(url)?;
            Ok("Opened web search".to_string())
        }
    }
}

fn copy_result_payload(runtime: &Runtime, result: &SearchResult) -> anyhow::Result<String> {
    match result {
        SearchResult::App { path, .. } | SearchResult::File { path, .. } => {
            copy_text(path)?;
            Ok("Path copied to the clipboard".to_string())
        }
        SearchResult::Clipboard {
            content,
            content_type,
            image_width,
            image_height,
            ..
        } => {
            runtime.block_on(clipboard::restore_history_entry_to_clipboard(
                content,
                content_type,
                *image_width,
                *image_height,
            ))?;
            Ok("Clipboard entry restored to the system clipboard".to_string())
        }
        SearchResult::Command { command, .. } => {
            copy_text(command)?;
            Ok("Command identifier copied to the clipboard".to_string())
        }
        SearchResult::Calculator { result, .. } => {
            copy_text(result)?;
            Ok("Calculator result copied to the clipboard".to_string())
        }
        SearchResult::Emoji { emoji, .. } => {
            copy_text(emoji)?;
            Ok("Emoji copied to the clipboard".to_string())
        }
        SearchResult::Dictionary { word, .. } => {
            copy_text(word)?;
            Ok("Dictionary term copied to the clipboard".to_string())
        }
        SearchResult::WebSearch { url, .. } => {
            copy_text(url)?;
            Ok("Search URL copied to the clipboard".to_string())
        }
    }
}

fn copy_text(value: &str) -> anyhow::Result<()> {
    let mut clipboard = Clipboard::new()?;
    clipboard.set_text(value.to_string())?;
    Ok(())
}
