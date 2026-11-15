//! Finding worlds: each world is one `.grant` file, named by it, and is run on whatever its
//! parents are now.

use crate::error::{Error, Fail};
use crate::exec::{Output, Session};
use crate::lexer::{LexError, lex};
use crate::world::World;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Loads worlds from files, once each, and refuses loops.
#[derive(Default)]
pub struct Loader {
    done: HashMap<PathBuf, Result<Rc<World>, Rc<Error>>>,
    /// The worlds being run right now, outermost first.
    running: Vec<PathBuf>,
    files: Vec<PathBuf>,
}

fn name_of(path: &Path) -> String {
    path.file_stem().unwrap_or(path.as_os_str()).to_string_lossy().into_owned()
}

/// An error in a file that no statement of it was run for.
fn error(file: &Path, line: Option<u32>, message: String) -> Error {
    Error { world: name_of(file), file: Some(file.into()), line: line.map(|n| (n, String::new())), messages: vec![message], cause: None }
}

impl Loader {
    pub fn new() -> Loader {
        Loader::default()
    }

    /// Every file read so far: the world that was run and all it builds on or holds.
    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Runs the world in a file. What its own questions show is added to `shown`, line by
    /// line, so everything shown before an error is still there.
    pub fn run(&mut self, file: &Path, shown: &mut Vec<Output>) -> Result<Rc<World>, Error> {
        match file.canonicalize() {
            Ok(path) => self.load(&path, shown),
            Err(_) => Err(error(file, None, format!("there is no file {}", file.display()))),
        }
    }

    /// The world a name stands for, seen from a folder. Its questions are not repeated.
    pub(crate) fn named(&mut self, dir: &Path, name: &str) -> Result<Rc<World>, Fail> {
        let Ok(path) = dir.join(format!("{name}.grant")).canonicalize() else {
            return Err(format!("no world called `{name}` here").into());
        };
        if let Some(at) = self.running.iter().position(|p| *p == path) {
            let chain: Vec<String> = self.running[at..].iter().chain([&path]).map(|p| name_of(p)).collect();
            return Err(chain.join(" → ").into());
        }
        let result = match self.done.get(&path) {
            Some(result) => result.clone(),
            None => self.load(&path, &mut Vec::new()).map_err(Rc::new),
        };
        result.map_err(|cause| Fail(format!("`{}` has errors; fix it first", name_of(&path)), Some(cause)))
    }

    fn load(&mut self, path: &Path, shown: &mut Vec<Output>) -> Result<Rc<World>, Error> {
        self.files.push(path.into());
        let text = std::fs::read_to_string(path).map_err(|e| error(path, None, format!("can't read the file: {e}")))?;
        let lines = lex(&text).map_err(|e| match e {
            LexError::Unclosed(number) => error(path, Some(number), "a bracket opened here is never closed".into()),
            LexError::Bad(number, message) => error(path, Some(number), message),
        })?;
        let mut session = Session::new(&name_of(path), path.parent().unwrap_or(Path::new(".")), Some(path));
        self.running.push(path.into());
        let ran = lines.iter().try_for_each(|line| session.run(self, line).map(|outputs| shown.extend(outputs)));
        self.running.pop();
        let result = ran.and_then(|()| session.finish()).map(Rc::new);
        self.done.insert(path.into(), result.clone().map_err(Rc::new));
        result
    }
}
