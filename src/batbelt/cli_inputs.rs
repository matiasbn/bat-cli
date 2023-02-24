use dialoguer::{console::Term, theme::ColorfulTheme, Input, MultiSelect, Select};
use error_stack::Result;

use crate::commands::CommandError;

#[derive(Debug, Clone)]
pub struct BatDialoguer;

impl BatDialoguer {
    pub fn multiselect<T>(
        prompt_text: String,
        items: Vec<T>,
        default: Option<&Vec<bool>>,
    ) -> Result<Vec<usize>, CommandError>
    where
        T: ToString + Clone,
    {
        let mut waiting_response = true;
        while waiting_response {
            let colorful_theme = &ColorfulTheme::default();
            let mut multi_select = MultiSelect::with_theme(colorful_theme);
            let mut dialog = multi_select.with_prompt(&prompt_text).items(&items);

            if let Some(def) = default {
                dialog = dialog.defaults(def);
            }
            let result = dialog.interact_on_opt(&Term::stderr()).unwrap();

            match result {
                Some(vec_options) => {
                    return Ok(vec_options);
                }
                None => {
                    println!("No option selected, you should pick at least 1 by hitting space bar");
                    waiting_response = true;
                }
            }
        }
        Ok((vec![]))
    }

    pub fn select<T>(
        prompt_text: String,
        items: Vec<T>,
        default: Option<usize>,
    ) -> Result<usize, CommandError>
    where
        T: ToString + Clone,
    {
        let colorful_theme = &ColorfulTheme::default();
        let mut select = Select::with_theme(colorful_theme);
        let mut dialog = select.with_prompt(&prompt_text).items(&items);

        if let Some(def) = default {
            dialog = dialog.default(def);
        } else {
            dialog = dialog.default(0);
        }

        Ok(dialog.interact_on_opt(&Term::stderr()).unwrap().unwrap())
    }

    pub fn select_yes_or_no(prompt_text: String) -> Result<bool, CommandError> {
        let colorful_theme = &ColorfulTheme::default();
        let mut select = Select::with_theme(colorful_theme);
        let dialog = select
            .with_prompt(&prompt_text)
            .item("yes")
            .item("no")
            .default(0);
        let opt = dialog
            .interact_on_opt(&Term::stderr())
            .ok()
            .ok_or(CommandError)?
            .ok_or(CommandError)?;

        Ok(opt == 0)
    }

    pub fn input(prompt_text: String) -> Result<String, CommandError> {
        let colorful_theme = &ColorfulTheme::default();
        let mut input = Input::with_theme(colorful_theme);
        let dialog: String = input
            .with_prompt(&prompt_text)
            .interact_text()
            .ok()
            .ok_or(CommandError)?;

        Ok(dialog)
    }
}

pub fn multiselect<T>(
    prompt_text: &str,
    items: Vec<T>,
    default: Option<&Vec<bool>>,
) -> Result<Vec<usize>, CommandError>
where
    T: ToString + Clone,
{
    Ok(BatDialoguer::multiselect(
        prompt_text.to_string(),
        items,
        default,
    )?)
}

pub fn select<T>(
    prompt_text: &str,
    items: Vec<T>,
    default: Option<usize>,
) -> Result<usize, CommandError>
where
    T: ToString + Clone,
{
    Ok(BatDialoguer::select(
        prompt_text.to_string(),
        items,
        default,
    )?)
}

pub fn select_yes_or_no(prompt_text: &str) -> Result<bool, CommandError> {
    Ok(BatDialoguer::select_yes_or_no(prompt_text.to_string())?)
}

pub fn input(prompt_text: &str) -> Result<String, CommandError> {
    Ok(BatDialoguer::input(prompt_text.to_string())?)
}
