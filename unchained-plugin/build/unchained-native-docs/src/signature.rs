//! Conservative source-overload selection from verified Rust binding declarations.

use quote::ToTokens;
use syn::{FnArg, GenericArgument, PathArguments, Signature, Type};

#[derive(Debug)]
enum Parameter {
    Value(String),
    Indirect { name: String, mutable: bool },
    IndirectValue(String),
}

/// Type identity for documentation selection, not proof of native ABI correctness.
#[derive(Debug)]
pub struct BindingSignature {
    parameters: Vec<Parameter>,
    receiver_const: Option<bool>,
}

impl BindingSignature {
    pub fn from_rust(signature: &Signature, owner: &str) -> Option<Self> {
        let mut parameters = Vec::new();
        let mut receiver_const = None;
        for argument in &signature.inputs {
            match argument {
                FnArg::Receiver(receiver) => {
                    let Parameter::Indirect { mutable, .. } = parameter(&receiver.ty, owner)?
                    else {
                        return None;
                    };
                    receiver_const = Some(!mutable);
                }
                FnArg::Typed(argument) => {
                    if argument
                        .attrs
                        .iter()
                        .any(|attribute| attribute.path().is_ident("native_by_value"))
                    {
                        let Type::Ptr(pointer) = argument.ty.as_ref() else {
                            return None;
                        };
                        if pointer.mutability.is_none() {
                            return None;
                        }
                        parameters.push(Parameter::IndirectValue(type_name(&pointer.elem, owner)?));
                    } else {
                        parameters.push(parameter(&argument.ty, owner)?);
                    }
                }
            }
        }
        Some(Self {
            parameters,
            receiver_const,
        })
    }

    pub(crate) fn matches(&self, cpp: &[Vec<String>]) -> bool {
        let Some((qualifiers, parameters)) = cpp.split_last() else {
            return false;
        };
        let parameters = if parameters.len() == 1 && parameters[0] == ["void"] {
            &[][..]
        } else {
            parameters
        };
        if self.parameters.len() != parameters.len() {
            return false;
        }
        let is_const = qualifiers.iter().any(|token| token == "const");
        if self.receiver_const.unwrap_or(false) != is_const {
            return false;
        }
        self.parameters
            .iter()
            .zip(parameters)
            .all(|(rust, cpp)| parameter_matches(rust, cpp))
    }
}

fn parameter(ty: &Type, owner: &str) -> Option<Parameter> {
    Some(match ty {
        Type::Ptr(pointer) => Parameter::Indirect {
            name: type_name(&pointer.elem, owner)?,
            mutable: pointer.mutability.is_some(),
        },
        Type::Reference(reference) => Parameter::Indirect {
            name: type_name(&reference.elem, owner)?,
            mutable: reference.mutability.is_some(),
        },
        _ => Parameter::Value(type_name(ty, owner)?),
    })
}

fn type_name(ty: &Type, owner: &str) -> Option<String> {
    match ty {
        Type::Path(path) if path.qself.is_none() => {
            let segment = path.path.segments.last()?;
            let mut name = segment.ident.to_string();
            if name == "Self" {
                name = owner.to_owned();
            }
            if let PathArguments::AngleBracketed(arguments) = &segment.arguments {
                let arguments = arguments
                    .args
                    .iter()
                    .map(|argument| match argument {
                        GenericArgument::Type(ty) => type_name(ty, owner),
                        GenericArgument::Const(value) => {
                            Some(value.to_token_stream().to_string().replace(' ', ""))
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()?;
                name = format!("{name}<{}>", arguments.join(","));
            } else if !matches!(segment.arguments, PathArguments::None) {
                return None;
            }
            Some(canonical(&name).to_owned())
        }
        Type::Ptr(pointer) => {
            let pointee = type_name(&pointer.elem, owner)?;
            Some(if pointer.mutability.is_some() {
                format!("{pointee}*")
            } else if pointee.ends_with('*') {
                format!("{pointee}const*")
            } else {
                format!("const{pointee}*")
            })
        }
        Type::Paren(ty) => type_name(&ty.elem, owner),
        Type::Group(ty) => type_name(&ty.elem, owner),
        _ => None,
    }
}

fn canonical(name: &str) -> &str {
    match name {
        "float" => "f32",
        "double" => "f64",
        "int" | "int32" => "i32",
        "unsignedint" | "uint32" => "u32",
        "short" | "int16" => "i16",
        "unsignedshort" | "uint16" => "u16",
        "int8" | "signedchar" => "i8",
        "uint8" | "unsignedchar" => "u8",
        "int64" | "longlong" => "i64",
        "uint64" | "unsignedlonglong" => "u64",
        "SIZE_T" | "size_t" => "usize",
        "void" => "c_void",
        _ => name,
    }
}

fn parameter_matches(rust: &Parameter, cpp: &[String]) -> bool {
    let mut cpp = cpp;
    // A by-value pointer's own const qualifier is not pointee constness.
    if cpp.last().is_some_and(|token| token == "const")
        && cpp
            .get(cpp.len().saturating_sub(2))
            .is_some_and(|token| token == "*")
    {
        cpp = &cpp[..cpp.len() - 1];
    }
    let indirect = cpp
        .last()
        .is_some_and(|token| matches!(token.as_str(), "*" | "&" | "&&"));
    if indirect {
        cpp = &cpp[..cpp.len() - 1];
    }
    let mut is_const = false;
    let mut depth = 0usize;
    let nested_pointer = cpp.iter().any(|token| match token.as_str() {
        "<" => {
            depth += 1;
            false
        }
        ">" => {
            depth = depth.saturating_sub(1);
            false
        }
        ">>" => {
            depth = depth.saturating_sub(2);
            false
        }
        "*" => depth == 0,
        _ => false,
    });
    if !nested_pointer && cpp.first().is_some_and(|token| token == "const") {
        is_const = true;
        cpp = &cpp[1..];
    }
    if cpp.last().is_some_and(|token| token == "const") {
        is_const = true;
        cpp = &cpp[..cpp.len() - 1];
    }
    let name = cpp.join("");
    let name = canonical(&name);
    match rust {
        Parameter::Value(rust) => !indirect && rust == name,
        Parameter::Indirect {
            name: rust,
            mutable,
        } => {
            if rust != name {
                return false;
            }
            indirect && *mutable != is_const
        }
        Parameter::IndirectValue(rust) => !indirect && rust == name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(parameters: &[&str], qualifiers: &str) -> Vec<Vec<String>> {
        parameters
            .iter()
            .chain([&qualifiers])
            .map(|parameter| parameter.split_whitespace().map(str::to_owned).collect())
            .collect()
    }

    #[test]
    fn callback_and_constness_select_the_native_overload() {
        let signature = BindingSignature::from_rust(&syn::parse_quote! {
            unsafe fn UDPEcho(endpoint: *const FString, timeout: f32, #[native_by_value] callback: *mut FIcmpEchoResultCallback)
        }, "FUDPPing").unwrap();
        assert!(signature.matches(&key(
            &["const FString &", "float", "FIcmpEchoResultCallback"],
            ""
        )));
        assert!(!signature.matches(&key(
            &["const FString &", "float", "FIcmpEchoResultDelegate"],
            ""
        )));
        assert!(!signature.matches(&key(&["FString &", "float", "FIcmpEchoResultCallback"], "")));
        assert!(!signature.matches(&key(
            &["const FString &", "double", "FIcmpEchoResultCallback"],
            ""
        )));
    }

    #[test]
    fn hidden_storage_is_not_silently_removed() {
        let signature = BindingSignature::from_rust(
            &syn::parse_quote! {
                unsafe fn Construct(output: *mut Self, string: &FString) -> *mut Self
            },
            "FText",
        )
        .unwrap();
        assert!(!signature.matches(&key(&["const FString &"], "")));
    }

    #[test]
    fn ordinary_pointers_are_not_guessed_to_be_indirect_values() {
        let signature = BindingSignature::from_rust(
            &syn::parse_quote! {
                unsafe fn Call(callback: *mut Callback)
            },
            "Native",
        )
        .unwrap();
        assert!(signature.matches(&key(&["Callback *"], "")));
        assert!(!signature.matches(&key(&["Callback"], "")));
    }

    #[test]
    fn nested_pointers_keep_constness_at_the_correct_level() {
        let signature = BindingSignature::from_rust(
            &syn::parse_quote! {
                unsafe fn Call(value: *mut *const FString)
            },
            "Native",
        )
        .unwrap();
        assert!(signature.matches(&key(&["const FString * *"], "")));
        assert!(!signature.matches(&key(&["FString * const *"], "")));
        let signature = BindingSignature::from_rust(
            &syn::parse_quote! {
                unsafe fn Call(value: *mut FString)
            },
            "Native",
        )
        .unwrap();
        assert!(signature.matches(&key(&["FString * const"], "")));
        assert!(!signature.matches(&key(&["const FString *"], "")));
    }

    #[test]
    fn receiver_constness_and_generic_parameter_types_are_preserved() {
        let signature = BindingSignature::from_rust(
            &syn::parse_quote! {
                unsafe fn Call(self: *const Self, output: *mut TArray<FString>)
            },
            "Native",
        )
        .unwrap();
        assert!(signature.matches(&key(&["TArray < FString > &"], "const")));
        assert!(!signature.matches(&key(&["TArray < FString > &"], "")));
        assert!(!signature.matches(&key(&["TArray < FName > &"], "const")));
    }
}
