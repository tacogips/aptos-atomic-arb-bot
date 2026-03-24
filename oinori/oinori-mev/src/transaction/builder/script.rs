use super::function_args::FunctionArg;
use super::package_builder::BuildPackageInfo;
use super::type_tag::FunctionTypeTag;
use anyhow::{anyhow, Result};

use move_core_types::language_storage::TypeTag;

use aptos_types::transaction::{Script, TransactionArgument, TransactionPayload};

#[derive(Debug)]
pub struct ScriptCall {
    code_bytes: Vec<u8>,
}

impl ScriptCall {
    pub fn build_script(build_package_info: BuildPackageInfo) -> Result<ScriptCall> {
        let built_package = build_package_info.build_package()?;
        let scripts_count = built_package.script_count();
        if scripts_count != 1 {
            return Err(anyhow!(
                "Only one script can be prepared a time. Make sure one and only one script file \
                is included in the Move package. Found {} scripts.",
                scripts_count
            ));
        }

        let code_bytes = built_package
            .extract_script_code()
            .pop()
            .ok_or(anyhow!("no script code"))?;

        Ok(ScriptCall { code_bytes })
    }

    pub fn as_transaction_payload(
        &self,
        args: Option<Vec<FunctionArg>>,
        types: Option<Vec<FunctionTypeTag>>,
    ) -> Result<TransactionPayload> {
        let mut arg_values: Vec<TransactionArgument> = vec![];
        if let Some(args) = args {
            for each_arg in args {
                let arg = each_arg.as_transaction_arg()?;
                arg_values.push(arg);
            }
        }

        let mut type_tags: Vec<TypeTag> = vec![];
        if let Some(types) = types {
            for each_type in types {
                let type_tag = each_type.to_type_tag()?;
                type_tags.push(type_tag);
            }
        }

        Ok(TransactionPayload::Script(Script::new(
            self.code_bytes.clone(),
            type_tags, //vec![], //not need ty_args,ty_args,
            arg_values,
        )))
    }
}
