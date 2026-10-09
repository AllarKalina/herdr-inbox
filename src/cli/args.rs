//! Command-line arguments: named flags first, then exactly the positionals a command expects.

use crate::store::Result;

pub struct Args(Vec<String>);

impl Args {
    pub fn new(args: Vec<String>) -> Self {
        Self(args)
    }

    /// Removes `name VALUE` and returns the value.
    pub fn flag(&mut self, name: &str) -> Result<Option<String>> {
        let Some(index) = self.0.iter().position(|arg| arg == name) else {
            return Ok(None);
        };
        self.0.remove(index);
        if index >= self.0.len() {
            return Err(format!("{name} needs a value").into());
        }
        Ok(Some(self.0.remove(index)))
    }

    /// Removes every `name VALUE` pair, in order.
    pub fn flags(&mut self, name: &str) -> Result<Vec<String>> {
        let mut values = Vec::new();
        while let Some(value) = self.flag(name)? {
            values.push(value);
        }
        Ok(values)
    }

    /// Removes a valueless flag and reports whether it was there.
    pub fn switch(&mut self, name: &str) -> bool {
        match self.0.iter().position(|arg| arg == name) {
            Some(index) => {
                self.0.remove(index);
                true
            }
            None => false,
        }
    }

    /// Takes the next positional argument, if any; used to pick a subcommand.
    pub fn next(&mut self) -> Option<String> {
        (!self.0.is_empty()).then(|| self.0.remove(0))
    }

    /// Ends parsing: what remains must be exactly the `N` positionals the command takes.
    pub fn positionals<const N: usize>(self) -> Result<[String; N]> {
        self.0
            .try_into()
            .map_err(|_| "Wrong number of arguments; run herdr-inbox help".into())
    }
}
