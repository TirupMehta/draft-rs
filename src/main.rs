const VERSION: &str = env!("CARGO_PKG_VERSION");

mod text;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        None | Some("app") => {
            println!("Draft {VERSION}");
            println!("The native shell is not built yet. See docs/ROADMAP.md.");
        }
        Some("clean") => {
            let input = args[1..].join(" ");
            println!("{}", text::clean_text(&input, false));
        }
        Some("--version" | "-V") => println!("draft {VERSION}"),
        Some("--help" | "-h") => print_help(),
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_help();
            std::process::exit(2);
        }
    }
}

fn print_help() {
    println!(
        "Draft {VERSION}

Usage:
  draft              Open the app (not built yet)
  draft clean <text> Print the cleaned transcript
  draft --version    Print the version
  draft --help       Show this help"
    );
}
