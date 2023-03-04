use crate::batbelt::metadata::functions_source_code_metadata::FunctionSourceCodeMetadata;
use crate::batbelt::metadata::traits_source_code_metadata::{
    TraitMetadataType, TraitSourceCodeMetadata,
};
use crate::batbelt::parser::{ParserError, ParserResult};

use crate::batbelt::metadata::trait_metadata::{TraitMetadata, TraitMetadataFunction};
use crate::batbelt::metadata::{BatMetadata, BatMetadataParser};
use error_stack::{Result, ResultExt};
use regex::Regex;

#[derive(Clone, Debug)]
pub struct TraitParser {
    pub name: String,
    pub impl_from: String,
    pub impl_to: String,
    pub external_trait: bool,
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
            external_trait: false,
            trait_source_code_metadata: trait_source_code_metadata.clone(),
            impl_functions: vec![],
        };
        if let Ok(trait_metadata) = bat_metadata
            .get_trait_metadata_by_trait_source_code_metadata_id(
                trait_source_code_metadata.metadata_id,
            )
        {
            new_parser.impl_functions = trait_metadata
                .impl_functions
                .clone()
                .into_iter()
                .map(|trait_meta_function| {
                    bat_metadata
                        .source_code
                        .get_function_by_id(trait_meta_function.function_source_code_metadata_id)
                })
                .collect::<Result<Vec<_>, _>>()
                .change_context(ParserError)?;
            new_parser.external_trait = trait_metadata.external_trait;
            new_parser.impl_from = trait_metadata.impl_from;
            new_parser.impl_to = trait_metadata.impl_to;
            Ok(new_parser)
        } else {
            new_parser.get_impl_function()?;
            new_parser.get_from_to()?;
            new_parser.get_external_trait_value()?;
            let new_trait_metadata = TraitMetadata::new(
                BatMetadata::create_metadata_id(),
                new_parser.clone().name,
                new_parser.clone().trait_source_code_metadata.metadata_id,
                new_parser
                    .clone()
                    .impl_functions
                    .into_iter()
                    .map(|function_meta| {
                        let trait_signature =
                            format!("{}::{}", new_parser.impl_to, function_meta.name);
                        TraitMetadataFunction {
                            function_source_code_metadata_id: function_meta.metadata_id,
                            trait_signature,
                        }
                    })
                    .collect::<Vec<_>>(),
                new_parser.clone().trait_source_code_metadata.trait_type,
                new_parser.clone().external_trait,
                new_parser.clone().impl_from,
                new_parser.clone().impl_to,
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

    fn get_external_trait_value(&mut self) -> ParserResult<()> {
        if self.trait_source_code_metadata.trait_type == TraitMetadataType::Definition
            || self.impl_from.is_empty()
        {
            self.external_trait = false;
            return Ok(());
        };
        let bat_metadata = BatMetadata::read_metadata().change_context(ParserError)?;
        let traits_sc_metadata = bat_metadata.source_code.traits_source_code;
        let mut definition_traits = traits_sc_metadata
            .into_iter()
            .filter(|trait_sc| trait_sc.trait_type == TraitMetadataType::Definition);
        // if not Definition trait, and the definition is not present, then is an external trait
        match definition_traits.find(|def_trait| def_trait.name == self.impl_from) {
            None => self.external_trait = true,
            Some(_) => self.external_trait = false,
        }
        Ok(())
    }
}
