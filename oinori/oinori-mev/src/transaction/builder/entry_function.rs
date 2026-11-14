use super::function_args::FunctionArg;
use super::type_tag::FunctionTypeTag;
use crate::account_address::hex_to_account_address;
use anyhow::Result;
use aptos_types::transaction::{EntryFunction, TransactionPayload};
use move_core_types::identifier::Identifier;
use move_core_types::language_storage::{ModuleId, TypeTag};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct EntryFunctionCall {
    pub module_hex_address: String,
    pub module_name: String,
    pub function_name: String,
}

impl EntryFunctionCall {
    fn module_id(&self) -> Result<ModuleId> {
        Ok(ModuleId::new(
            hex_to_account_address(&self.module_hex_address)?,
            Identifier::new(self.module_name.to_string())?,
        ))
    }

    fn function_name_identifier(&self) -> Result<Identifier> {
        let id = Identifier::new(self.function_name.clone())?;
        Ok(id)
    }

    fn into_entry_function(
        &self,
        args: Option<Vec<FunctionArg>>,
        types: Option<Vec<FunctionTypeTag>>,
    ) -> Result<EntryFunction> {
        let module_id = self.module_id()?;
        let function = self.function_name_identifier()?;

        let mut arg_values: Vec<Vec<u8>> = vec![];
        let mut type_tags: Vec<TypeTag> = vec![];

        if let Some(args) = args {
            for each_arg in args {
                let (_, arg) = each_arg.as_type_and_value()?;
                arg_values.push(arg);
            }
        }

        if let Some(types) = types {
            for each_type in types {
                let type_tag = each_type.to_type_tag()?;
                type_tags.push(type_tag);
            }
        }

        Ok(EntryFunction::new(
            module_id,
            function,
            vec![], //not need ty_args,ty_args,
            arg_values,
        ))
    }

    pub fn as_transaction_payload(
        &self,
        args: Option<Vec<FunctionArg>>,
        types: Option<Vec<FunctionTypeTag>>,
    ) -> Result<TransactionPayload> {
        let entry_function = self.into_entry_function(args, types)?;
        Ok(TransactionPayload::EntryFunction(entry_function))
    }
}
