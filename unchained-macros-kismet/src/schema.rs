//! Copied reflection schema; native layout access stays in the host adapter.

#[derive(Clone, Debug, PartialEq, Eq)]

pub enum PropertyKind {
    Bool,
    Byte,
    Int,
    Float,
    Object,
    String,
    Text,
    Struct(String),
    Unsupported(String),
}

#[derive(Clone, Debug)]

pub struct PropertyMetadata {
    pub identity: usize,
    pub kind: PropertyKind,
    pub size: usize,
    pub array_dim: i32,
    pub flags: u64,
    pub offset: usize,
}

#[derive(Clone, Debug)]

pub struct FunctionSchema {
    pub function_identity: usize,
    /// False when another reflected UFunction shares this wrapper. Shared
    /// wrappers require a separately verified frame-to-callee identity adapter.
    pub unique_exec_thunk: bool,
    pub parameters: Vec<PropertyMetadata>,
}

/// Declaration-side VM layout, independent of the native C++ calling convention.
#[derive(Clone, Copy, Debug)]

pub struct Parameter {
    /// UE property class name, or `StructProperty:StructName`.
    pub kind: &'static str,
    pub size: usize,
}

pub const PARM: u64 = 0x80;

pub const OUT_PARM: u64 = 0x100;

pub const RETURN_PARM: u64 = 0x400;

pub const REFERENCE_PARM: u64 = 0x0800_0000;

pub const CONST_PARM: u64 = 0x2;

fn matches(kind: &PropertyKind, expected: &str) -> bool {

    match kind {
        PropertyKind::Bool => expected == "BoolProperty",
        PropertyKind::Byte => expected == "ByteProperty",
        PropertyKind::Int => expected == "IntProperty",
        PropertyKind::Float => expected == "FloatProperty",
        PropertyKind::Object => expected == "ObjectProperty",
        PropertyKind::String => expected == "StrProperty",
        PropertyKind::Text => expected == "TextProperty",
        PropertyKind::Struct(name) => expected.strip_prefix("StructProperty:") == Some(name.as_str()),
        PropertyKind::Unsupported(_) => false,
    }
}

/// Validate before attaching; never infer a dummy from schema/decoder failure.

pub fn validate(
    function: &crate::FunctionMetadata,
    schema: &FunctionSchema,
    arguments: &[Parameter],
    result: Option<Parameter>,
) -> Result<Vec<usize>, String> {

    let count = function.arguments()? as usize;

    if !schema.unique_exec_thunk {

        return Err("Shared exec wrappers are unsupported without verified frame-to-function identity".into());
    }

    if schema.function_identity == 0
        || count != arguments.len()
        || schema.parameters.len() != function.num_parms as usize
    {

        return Err("Exec observation function identity/parameter count mismatch".into());
    }

    let mut properties = Vec::new();

    let mut returned = false;

    for property in &schema.parameters {

        if property.identity == 0 || property.flags & PARM == 0 || property.array_dim != 1 {

            return Err("Exec observation requires scalar reflected parameter properties".into());
        }

        let is_result = property.flags & RETURN_PARM != 0;

        let expected = if is_result {

            if returned || property.offset != function.return_value_offset as usize {

                return Err("Exec observation return metadata mismatch".into());
            }

            returned = true;

            result.ok_or("Exec observation declaration omits reflected result")?
        } else {

            if property.flags & OUT_PARM != 0
                || (property.flags & REFERENCE_PARM != 0 && property.flags & CONST_PARM == 0)
            {

                return Err("Exec observation does not support mutable reference/out parameters".into());
            }

            *arguments
                .get(properties.len())
                .ok_or("Exec observation has excess parameters")?
        };

        // A VM bool input uses bool32, but reflected property/result size is bool8.
        let size = if !is_result && matches!(property.kind, PropertyKind::Bool) {

            1
        } else {

            expected.size
        };

        if !matches(&property.kind, expected.kind) || property.size != size {

            return Err(format!(
                "Exec observation parameter type/layout mismatch: expected {}, reflected {:?}",
                expected.kind, property.kind
            ));
        }

        if !is_result {

            properties.push(property.identity);
        }
    }

    if properties.len() != arguments.len() || returned != result.is_some() {

        return Err("Exec observation argument/result schema mismatch".into());
    }

    Ok(properties)
}

#[cfg(test)]

mod tests {

    use super::*;

    fn fixture() -> (crate::FunctionMetadata, FunctionSchema) {

        (
            crate::FunctionMetadata {
                is_native: true,
                exec_thunk: 1,
                num_parms: 2,
                return_value_offset: 8,
            },
            FunctionSchema {
                function_identity: 2,
                unique_exec_thunk: true,
                parameters: vec![
                    PropertyMetadata {
                        identity: 3,
                        kind: PropertyKind::Bool,
                        size: 1,
                        array_dim: 1,
                        flags: PARM,
                        offset: 0,
                    },
                    PropertyMetadata {
                        identity: 4,
                        kind: PropertyKind::Bool,
                        size: 1,
                        array_dim: 1,
                        flags: PARM | RETURN_PARM,
                        offset: 8,
                    },
                ],
            },
        )
    }

    const BOOL: Parameter = Parameter {
        kind: "BoolProperty",
        size: 1,
    };

    #[test]

    fn return_is_not_stepped_and_bool_input_uses_separate_vm_layout() {

        let (function, schema) = fixture();

        assert_eq!(validate(&function, &schema, &[BOOL], Some(BOOL)).unwrap(), [3]);
    }

    #[test]

    fn mismatched_counts_results_types_and_layouts_fail_closed() {

        let (function, schema) = fixture();

        assert!(validate(&function, &schema, &[], Some(BOOL)).is_err());

        assert!(validate(&function, &schema, &[BOOL], None).is_err());

        let mut wrong = schema.clone();

        wrong.parameters[0].kind = PropertyKind::Int;

        assert!(validate(&function, &wrong, &[BOOL], Some(BOOL)).is_err());

        wrong = schema.clone();

        wrong.parameters[1].offset = 9;

        assert!(validate(&function, &wrong, &[BOOL], Some(BOOL)).is_err());

        wrong = schema.clone();

        wrong.parameters[1].size = 4;

        assert!(validate(&function, &wrong, &[BOOL], Some(BOOL)).is_err());
    }

    #[test]

    fn mutable_references_out_parameters_and_arrays_are_rejected() {

        let (function, schema) = fixture();

        for flags in [PARM | OUT_PARM, PARM | REFERENCE_PARM, PARM | OUT_PARM | CONST_PARM] {

            let mut wrong = schema.clone();

            wrong.parameters[0].flags = flags;

            assert!(validate(&function, &wrong, &[BOOL], Some(BOOL)).is_err());
        }

        let mut wrong = schema.clone();

        wrong.parameters[0].array_dim = 2;

        assert!(validate(&function, &wrong, &[BOOL], Some(BOOL)).is_err());

        let mut constant = schema.clone();

        constant.parameters[0].flags = PARM | REFERENCE_PARM | CONST_PARM;

        assert!(validate(&function, &constant, &[BOOL], Some(BOOL)).is_ok());
    }

    #[test]

    fn owned_results_require_exact_native_storage_size_and_kind() {

        for (kind, name, size) in [
            (PropertyKind::String, "StrProperty", 16),
            (PropertyKind::Text, "TextProperty", 24),
            (
                PropertyKind::Struct("LinearColor".into()),
                "StructProperty:LinearColor",
                16,
            ),
        ] {

            let function = crate::FunctionMetadata {
                is_native: true,
                exec_thunk: 1,
                num_parms: 1,
                return_value_offset: 0,
            };

            let mut schema = FunctionSchema {
                function_identity: 2,
                unique_exec_thunk: true,
                parameters: vec![PropertyMetadata {
                    identity: 3,
                    kind,
                    size,
                    array_dim: 1,
                    flags: PARM | RETURN_PARM,
                    offset: 0,
                }],
            };

            let result = Parameter { kind: name, size };

            assert!(validate(&function, &schema, &[], Some(result)).is_ok());

            schema.parameters[0].size -= 1;

            assert!(validate(&function, &schema, &[], Some(result)).is_err());
        }
    }
}
