use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;
use crate::batbelt::metadata::traits_source_code_metadata::TraitSourceCodeMetadata;
use crate::batbelt::parser::ParserError;

use crate::batbelt::metadata::trait_metadata::TraitMetadata;
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser};
use error_stack::{Result, ResultExt};
use regex::Regex;

#[derive(Clone, Debug)]
pub struct TraitParser {
    pub name: String,
    pub impl_from: String,
    pub impl_to: String,
    pub trait_source_code_metadata: TraitSourceCodeMetadata,
    pub impl_functions: Vec<FunctionSourceCodeMetadata>,
}

impl TraitParser {
    pub fn new_from_metadata(
        trait_source_code_metadata: TraitSourceCodeMetadata,
    ) -> Result<Self, ParserError> {
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        let name = trait_source_code_metadata.name.clone();
        let mut new_parser = Self {
            name,
            impl_from: "".to_string(),
            impl_to: "".to_string(),
            trait_source_code_metadata: trait_source_code_metadata.clone(),
            impl_functions: vec![],
        };
        if let Ok(trait_metadata) = bat_metadata
            .get_trait_metadata_by_trait_source_code_metadata_id(
                trait_source_code_metadata.metadata_id.clone(),
            )
        {
            new_parser.impl_functions = trait_metadata
                .impl_functions_id
                .clone()
                .into_iter()
                .map(|f_meta_id| bat_metadata.source_code.get_function_by_id(f_meta_id))
                .collect::<Result<Vec<_>, _>>()
                .change_context(ParserError)?;
            new_parser.get_from_to()?;
            log::debug!("new_trait_parser:\n{:#?}", new_parser);
            Ok(new_parser)
        } else {
            new_parser.get_impl_function()?;
            new_parser.get_from_to()?;
            let new_trait_metadata = TraitMetadata::new(
                BatMetadata::create_metadata_id(),
                new_parser.clone().name,
                new_parser.clone().trait_source_code_metadata.metadata_id,
                new_parser
                    .clone()
                    .impl_functions
                    .clone()
                    .into_iter()
                    .map(|function_meta| function_meta.metadata_id.clone())
                    .collect::<Vec<_>>(),
            );
            new_trait_metadata
                .update_metadata_file()
                .change_context(ParserError)?;
            log::debug!("new_trait_parser:\n{:#?}", new_parser);
            Ok(new_parser)
        }
    }

    fn get_impl_function(&mut self) -> Result<(), ParserError> {
        let functions_metadata_vec = BatMetadata::read_metadata()
            .change_context(ParserError)?
            .source_code
            .functions_source_code;
        let filtered_metadata_vec = functions_metadata_vec
            .into_iter()
            .filter(|f_metadata| {
                f_metadata.path == self.trait_source_code_metadata.path
                    && f_metadata.start_line_index
                        > self.trait_source_code_metadata.start_line_index
                    && f_metadata.end_line_index < self.trait_source_code_metadata.end_line_index
            })
            .collect::<Vec<_>>();
        self.impl_functions = filtered_metadata_vec;
        Ok(())
    }

    fn get_from_to(&mut self) -> Result<(), ParserError> {
        let name = self.name.clone();
        let match_regex = Regex::new(r"[A-Za-z0-9]+<[<A-Za-z0-9>]+> for [A-Za-z0-9]+").unwrap();
        let _generic_type_regex = Regex::new(r"").unwrap();
        if match_regex.is_match(&name) {
            let mut splitted = name.split(" for ");
            self.impl_from = splitted.next().unwrap().to_string();
            self.impl_to = splitted.next().unwrap().to_string();
        } else {
            self.impl_to = name;
        }
        Ok(())
    }
}
