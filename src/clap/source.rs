//! Clap Command Line Argument Parser source for config crate.
//! This module provides a source to generate configuration structures
//! from clap command line parses.
//! Provide a source for clap command line arguments.
//! This source lets you integrate the Clap command-line parsing crate
//! with the config crate.
//!
//! # Examples
//!
//!```
//! use clap::{Command, Arg, ArgAction};
//! use config::Config;
//! use config::clap::source::ClapConfig;
//!
//! let m = Command::new("myapp")
//!     .arg(Arg::new("input")
//!          .short('i')
//!          .num_args(1)
//!          .action(ArgAction::Set),
//!     )
//!     .get_matches_from(
//!         vec!["app", "-i", "filename.txt"]);
//!
//! let clap_config = ClapConfig::new(m);
//!
//! let config = Config::builder()
//!     .add_source(clap_config)
//!     .build().unwrap();
//!
//! // Test the built config
//! assert_eq!(config.get_string("input").unwrap(), "filename.txt");
//!
//!```
use crate::error::Result;
use crate::map::Map;
use crate::source::Source;
use crate::value::{Value, ValueKind};

use crate::ConfigError;
use clap::parser::ArgMatches;
use clap::parser::ValuesRef;

use std::collections::HashMap;

use std::fmt::{Debug, Display, Formatter};

/// ClapConfig is a wrapper for ArgMatches that includes optional
/// additional metadata about the command line parameters.
#[derive(Clone)]
pub struct ClapConfig {
    pub arg_matches: ArgMatches,
    /// metadata is a map from command line options to ValueKinds.
    /// Each entry indicates the type of a command line argument.
    /// It is optional.  If it's not included, the defaults are
    /// outlined in the documentation.
    pub metadata: Option<HashMap<String, ValueKind>>,
}

impl ClapConfig {
    /// Create a new ClapConfig with an ArgMatches and no additional
    /// type hints.
    pub fn new(arg_matches: ArgMatches) -> ClapConfig {
        ClapConfig {
            arg_matches,
            metadata: None,
        }
    }

    /// Create a new ClapConfig with an ArgMatches and additional type
    /// hints.
    pub fn new_with_metadata(
        arg_matches: ArgMatches,
        metadata: HashMap<String, ValueKind>,
    ) -> ClapConfig {
        ClapConfig {
            arg_matches,
            metadata: Some(metadata),
        }
    }

    /// Get all the command line arguments options as Strings.
    pub fn get_keys(&self) -> Vec<String> {
        self.arg_matches.get_keys()
    }

    /// Parse a command line argument with multiple values as an array
    /// if there are multiple values.  Parse it as a string if there
    /// is a single value.
    fn parse_multiple_values_default(
        &self,
        uri: &String,
        key: &str,
        values: ValuesRef<String>,
    ) -> crate::error::Result<Value> {
        // The default behavior treats a single multiple-value
        // argument as a string.  Multiple multiple-value
        // arguments are treated as an array.
        let kind = if values.len() == 1 {
            ValueKind::String(self.arg_matches.get_one::<String>(key).unwrap().to_string())
        } else {
            let v: Vec<Value> = values
                .map(|s| Value::new(Some(uri), ValueKind::String(s.to_string())))
                .collect();
            ValueKind::Array(v)
        };
        Ok(Value::new(Some(uri), kind))
    }

    /// Parse a command line argument with multiple values as an array
    fn parse_multiple_values_as_array(
        &self,
        uri: &String,
        key: &str,
        values: ValuesRef<String>,
        metadata: &HashMap<String, ValueKind>,
    ) -> crate::error::Result<Value> {
        let hash_res = metadata.get(key).expect("Expected a metadata key");
        let vk = match hash_res {
            ValueKind::Array(_) => {
                let v: Vec<Value> = values
                    .map(|s| Value::new(Some(uri), ValueKind::String(s.to_string())))
                    .collect();
                ValueKind::Array(v)
            }
            ValueKind::String(_) => {
                ValueKind::String(self.arg_matches.get_one::<String>(key).unwrap().to_string())
            }
            _ => {
                return Err(ConfigError::Message(String::from("Unsupported ValueKind")));
            }
        };

        Ok(Value::new(Some(uri), vk))
    }

    /// Parse the command line arguments into config values.
    fn parse_arguments(&self, key: &str, type_id: std::any::TypeId) -> crate::error::Result<Value> {
        let uri = String::from("clap");

        let res = if type_id == std::any::TypeId::of::<bool>() {
            Value::new(
                Some(&uri),
                ValueKind::Boolean(
                    *self
                        .arg_matches
                        .get_one::<bool>(key)
                        .expect("Couldn't get boolean"),
                ),
            )
        } else if type_id == std::any::TypeId::of::<String>() {
            let values = self.arg_matches.get_many::<String>(key).unwrap();
            if let Some(metadata) = &self.metadata {
                if metadata.contains_key(key) {
                    self.parse_multiple_values_as_array(&uri, key, values, metadata)?
                } else {
                    self.parse_multiple_values_default(&uri, key, values)?
                }
            } else {
                self.parse_multiple_values_default(&uri, key, values)?
            }
        } else {
            Value::new(
                Some(&uri),
                ValueKind::String(self.arg_matches.get_one::<String>(key).unwrap().to_string()),
            )
        };
        Ok(res)
    }

    /// Get the value or values of an individual command line option
    pub fn get_item(&self, key: &str) -> crate::error::Result<Value> {
        let key_string = String::from(key);
        let result = self.arg_matches.get_arg_by_name(&key_string);

        match result {
            None => Err(ConfigError::Message(String::from(
                "Error retrieving clap arguments",
            ))),

            Some(argument_value) => {
                if let Some(type_id) = argument_value.type_id() {
                    self.parse_arguments(key, type_id.type_id())
                } else {
                    Err(ConfigError::Message(String::from("No type information")))
                }
            }
        }
    }
}

impl Display for ClapConfig {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.arg_matches)?;
        write!(f, "{:?}", self.metadata)
    }
}

impl Debug for ClapConfig {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.arg_matches)?;
        write!(f, "{:?}", self.metadata)
    }
}

impl Source for ClapConfig {
    fn clone_into_box(&self) -> Box<dyn Source + Send + Sync> {
        Box::new((*self).clone())
    }

    fn collect(&self) -> Result<Map<String, Value>> {
        let mut clap_args: Map<String, Value> = Map::new();

        for key in self.get_keys() {
            let value = self.get_item(&key)?;
            clap_args.insert(key, value);
        }
        Ok(clap_args)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        clap::source::ClapConfig, config::Config, error::Unexpected, ConfigError, ValueKind,
    };
    use clap::{Arg, ArgAction, Command};

    use std::collections::HashMap;

    /// Test that basic types such as string, bool and array work.
    #[test]
    fn basic_types_work() {
        let m = Command::new("myapp")
            .arg(
                Arg::new("input")
                    .short('i')
                    .num_args(1)
                    .action(ArgAction::Set),
            )
            .arg(
                Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("debug")
                    .short('d')
                    .long("debug")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("tag")
                    .short('t')
                    .long("tag")
                    .action(ArgAction::Append),
            )
            .get_matches_from(vec![
                "myapp", "-v", "-i", "filename", "-t", "tagone", "-t", "tagtwo",
            ]);

        let clap_config = ClapConfig::new(m);

        let config = Config::builder().add_source(clap_config).build().unwrap();

        assert_eq!(config.get_string("input").unwrap(), "filename");
        assert_eq!(config.get_bool("verbose").unwrap(), true);
        assert_eq!(config.get_bool("debug").unwrap(), false);
        let mut vi = config.get_array("tag").unwrap().into_iter();
        let tagone = vi.next().unwrap().into_string().unwrap();
        let tagtwo = vi.next().unwrap().into_string().unwrap();
        assert_eq!(tagone, "tagone");
        assert_eq!(tagtwo, "tagtwo");
    }

    /// Test that without metadata hinting a single multiple value is
    /// interpreted as a string.
    #[test]
    fn single_multiple_values_isnt_array() {
        let m = Command::new("myapp")
            .arg(
                Arg::new("tag")
                    .short('t')
                    .long("tag")
                    .action(ArgAction::Append),
            )
            .get_matches_from(vec!["myapp", "-t", "tagone"]);

        let clap_config = ClapConfig::new(m);

        let config = Config::builder().add_source(clap_config).build().unwrap();

        let res = config.get_array("tag");
        match res {
            Ok(_) => {
                panic!("Should be an error");
            }
            Err(e) => match e {
                ConfigError::Type {
                    origin: _,
                    unexpected,
                    expected: _,
                    key: _,
                } => match unexpected {
                    Unexpected::Str(u) => {
                        assert_eq!(u, "tagone");
                    }
                    _ => {
                        panic!("Should be an unexpected string");
                    }
                },
                _ => {
                    panic!("Should be a ConfigError::Type");
                }
            },
        }
    }

    /// Test that a command line argument with multiple raw values is
    /// interpreted as an array.
    #[test]
    fn single_multiple_values_is_string() {
        let m = Command::new("myapp")
            .arg(
                Arg::new("tag")
                    .short('t')
                    .long("tag")
                    .action(ArgAction::Append),
            )
            .get_matches_from(vec!["myapp", "-t", "tagone"]);

        let clap_config = ClapConfig::new(m);

        let config = Config::builder().add_source(clap_config).build().unwrap();

        let tag = config.get_string("tag").unwrap();
        assert_eq!(tag, "tagone");
    }

    /// Test with metadata specifying an array argument.  This
    /// provides a hint that the command line argument is a
    /// multiple-value argument that should be interpreted as an
    /// array.
    #[test]
    fn single_multiple_values_with_metadata_is_array() {
        let m = Command::new("myapp")
            .arg(
                Arg::new("tag")
                    .short('t')
                    .long("tag")
                    .action(ArgAction::Append),
            )
            .get_matches_from(vec!["myapp", "-t", "tagone"]);

        let mut ht: HashMap<String, ValueKind> = HashMap::new();
        ht.insert(String::from("tag"), ValueKind::Array(Vec::new()));

        let clap_config = ClapConfig::new_with_metadata(m, ht);

        let config = Config::builder().add_source(clap_config).build().unwrap();

        let tag = config.get_array("tag").unwrap();
        assert_eq!(tag.len(), 1);
        let mut vi = config.get_array("tag").unwrap().into_iter();
        let tagone = vi.next().unwrap().into_string().unwrap();
        assert_eq!(tagone, "tagone");
    }
}
