use colored::{ColoredString, Colorize};
use inflector::Inflector;
use std::cell::RefCell;
use std::fmt::{Debug, Display};
use std::rc::Rc;
use strum::IntoEnumIterator;

pub mod bat_dialoguer;
pub mod command_line;
pub mod git;
pub mod markdown;
pub mod metadata;
pub mod miro;
pub mod parser;
pub mod path;
pub mod silicon;
pub mod sonar;
pub mod templates;

pub type ShareableDataType<T> = Rc<RefCell<T>>;
pub struct ShareableData<T>
where
    T: Sized,
{
    pub original: ShareableDataType<T>,
    pub cloned: ShareableDataType<T>,
}

impl<T> ShareableData<T>
where
    T: Sized,
{
    pub fn new(data_to_share: T) -> Self {
        let original = Rc::new(RefCell::new(data_to_share));
        let cloned = Rc::clone(&original);
        Self { original, cloned }
    }
}

pub trait BatEnumerator
where
    Self: ToString + Display + IntoEnumIterator + Clone + Sized + Debug,
{
    fn to_snake_case(&self) -> String {
        self.to_string().to_snake_case()
    }

    fn to_sentence_case(&self) -> String {
        self.to_string().to_sentence_case()
    }

    fn from_str(type_str: &str) -> Self {
        let structs_type_vec = Self::get_type_vec();

        structs_type_vec
            .into_iter()
            .find(|struct_type| struct_type.to_snake_case() == type_str.to_snake_case())
            .unwrap()
    }

    fn from_index(index: usize) -> Self {
        Self::get_type_vec()[index].clone()
    }

    fn get_type_vec() -> Vec<Self> {
        Self::iter().collect::<Vec<_>>()
    }

    fn get_colored_name(&self, to_plural: bool) -> ColoredString {
        let self_name = if to_plural {
            self.to_string().to_plural()
        } else {
            self.to_string()
        };

        let colorized_vec = Self::get_colorized_type_vec(to_plural);

        colorized_vec
            .into_iter()
            .find(|color| color.contains(&self_name))
            .unwrap()
    }

    fn colored_from_index(type_str: &str, idx: usize) -> ColoredString {
        match idx {
            0 => type_str.bright_green(),
            1 => type_str.bright_blue(),
            2 => type_str.bright_yellow(),
            3 => type_str.bright_cyan(),
            4 => type_str.bright_purple(),
            _ => type_str.bright_white(),
        }
    }

    fn get_colorized_type_vec(to_plural: bool) -> Vec<ColoredString> {
        let metadata_type_vec = Self::get_type_vec();
        let metadata_type_colorized = metadata_type_vec
            .iter()
            .enumerate()
            .map(|metadata_type| {
                if to_plural {
                    Self::colored_from_index(
                        &(*metadata_type.1).to_string().to_plural(),
                        metadata_type.0,
                    )
                } else {
                    Self::colored_from_index(&(*metadata_type.1).to_string(), metadata_type.0)
                }
            })
            .collect::<Vec<_>>();
        metadata_type_colorized
    }
}
