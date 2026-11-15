//! The `grant` command.

use grant::{Error, LexError, Loader, Output, Session, lex, show};
use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, SystemTime};

const HELP: &str = "\
Grant — a language for describing worlds and changing them.

Usage:
  grant <world>           run a world and show its questions
  grant run <world>       the same
  grant check <world>…    run worlds quietly and report only errors
  grant show <world>      print everything stated in a world, parents included
  grant watch <world>     run again whenever the world or anything it builds on changes
  grant repl [world]      state lines one at a time, on a world or from nothing
  grant help | version

A world is a file such as town.grant; `grant town` finds it too.";

const REPL_HELP: &str = "\
Every line is a statement, as in a file. Open brackets continue on the next line.
  :check    check what must hold when the world ends
  :show     print everything stated so far
  :save f   write the lines stated so far to f.grant
  :quit     leave";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let ok = match args.as_slice() {
        [] | ["help" | "--help" | "-h"] => {
            println!("{HELP}");
            true
        }
        ["version" | "--version" | "-V"] => {
            println!("grant {} (Foundation 1)", env!("CARGO_PKG_VERSION"));
            true
        }
        ["check", worlds @ ..] if !worlds.is_empty() => worlds.iter().filter(|world| !check(world)).count() == 0,
        ["show", world] => report(Loader::new().run(&file(world), &mut Vec::new()).map(|world| print!("{}", show::source(&world)))),
        ["watch", world] => watch(world),
        ["repl"] => repl(None),
        ["repl", world] => repl(Some(world)),
        ["run", world] => run(&mut Loader::new(), &file(world)),
        [world] if !world.starts_with('-') && !["run", "check", "show", "watch"].contains(world) => run(&mut Loader::new(), &file(world)),
        _ => {
            eprintln!("{HELP}");
            false
        }
    };
    if ok { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// `town`, `town.grant` and `towns/town` all name a file.
fn file(world: &str) -> PathBuf {
    let path = PathBuf::from(world);
    if path.extension().is_some() || path.exists() { path } else { path.with_extension("grant") }
}

fn paint(code: &str, text: &str, on: bool) -> String {
    if on { format!("\x1b[{code}m{text}\x1b[0m") } else { text.into() }
}

fn report<T>(result: Result<T, Error>) -> bool {
    if let Err(error) = &result {
        let text = error.to_string();
        let colour = std::io::stderr().is_terminal();
        eprintln!("{}", text.replace("error ", &format!("{} ", paint("1;31", "error", colour))));
    }
    result.is_ok()
}

/// Shows each line that said something, with what it said beneath it.
fn print(outputs: &[Output], echo: bool) {
    let colour = std::io::stdout().is_terminal();
    for output in outputs {
        if echo {
            output.source.lines().for_each(|line| println!("{}", paint("2", line, colour)));
        }
        output.text.lines().for_each(|line| println!("  {line}"));
    }
}

/// Runs a world and shows what it showed, up to its first error if it has one.
fn run(loader: &mut Loader, path: &Path) -> bool {
    let mut shown = Vec::new();
    let result = loader.run(path, &mut shown);
    print(&shown, true);
    report(result)
}

fn check(world: &str) -> bool {
    report(Loader::new().run(&file(world), &mut Vec::new()).map(|world| println!("{}: {}", world.name, show::summary(&world))))
}

fn watch(world: &str) -> bool {
    let path = file(world);
    let stamp =
        |files: &[PathBuf]| -> Vec<Option<SystemTime>> { files.iter().map(|f| f.metadata().and_then(|m| m.modified()).ok()).collect() };
    loop {
        let mut loader = Loader::new();
        if std::io::stdout().is_terminal() {
            print!("\x1b[2J\x1b[H");
        }
        run(&mut loader, &path);
        // A file that isn't there yet is watched for too.
        let files = if loader.files().is_empty() { vec![path.clone()] } else { loader.files().to_vec() };
        let before = stamp(&files);
        while stamp(&files) == before {
            std::thread::sleep(Duration::from_millis(250));
        }
    }
}

fn repl(parent: Option<&str>) -> bool {
    let mut loader = Loader::new();
    let dir = std::env::current_dir().unwrap_or_default();
    let mut session = Session::new("repl", &dir, None);
    let mut stated: Vec<String> = Vec::new();
    let interactive = std::io::stdin().is_terminal();
    let state = |session: &mut Session, loader: &mut Loader, text: &str, stated: &mut Vec<String>| -> bool {
        let lines = match lex(text) {
            Ok(lines) => lines,
            Err(LexError::Unclosed(_)) => return false,
            Err(LexError::Bad(_, message)) => {
                eprintln!("error: {message}");
                return true;
            }
        };
        for line in &lines {
            // A line that fails leaves the world as it was.
            let before = session.clone();
            match session.run(loader, line) {
                Ok(outputs) => {
                    print(&outputs, false);
                    stated.push(line.text.clone());
                }
                Err(error) => {
                    eprintln!("error: {}", error.messages.join("; "));
                    if let Some(cause) = &error.cause {
                        eprintln!("{cause}");
                    }
                    *session = before;
                }
            }
        }
        true
    };
    if let Some(parent) = parent {
        let name = file(parent).with_extension("");
        state(&mut session, &mut loader, &format!("from {}", name.display()), &mut stated);
        if stated.is_empty() {
            return false;
        }
    }
    if interactive {
        println!("Grant {} — :help for help, :quit to leave", env!("CARGO_PKG_VERSION"));
    }
    let mut pending = String::new();
    loop {
        if interactive {
            print!("{}", if pending.is_empty() { "grant> " } else { "     … " });
            std::io::stdout().flush().ok();
        }
        let mut input = String::new();
        if std::io::stdin().lock().read_line(&mut input).unwrap_or(0) == 0 {
            return true;
        }
        let command: Vec<&str> = input.split_whitespace().collect();
        match command.as_slice() {
            [":quit" | ":q"] => return true,
            [":help"] => println!("{REPL_HELP}"),
            [":show"] => print!("{}", show::source(&session.world)),
            [":check"] => match grant::check(&session.world) {
                Ok(problems) if problems.is_empty() => println!("  everything holds"),
                Ok(problems) => problems.iter().for_each(|p| println!("  - {p}")),
                Err(message) => eprintln!("error: {message}"),
            },
            [":save", name] => {
                let path = file(name);
                match std::fs::write(&path, stated.join("\n") + "\n") {
                    Ok(()) => println!("  saved {}", path.display()),
                    Err(e) => eprintln!("error: can't write {}: {e}", path.display()),
                }
            }
            [first, ..] if first.starts_with(':') && pending.is_empty() => eprintln!("error: no command {first}; try :help"),
            _ => {
                pending.push_str(&input);
                if state(&mut session, &mut loader, &pending, &mut stated) {
                    pending.clear();
                }
            }
        }
    }
}
