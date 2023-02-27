use std::cell::RefCell;
use std::rc::Rc;

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
        let original = Rc::new(RefCell::new((data_to_share)));
        let cloned = Rc::clone(&original);
        Self { original, cloned }
    }
}
