use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{parse::{Parse, ParseStream}, FnArg, Pat, Type};

// Read the existing cpp_struct declarations, not a second signature inventory.
struct VtableMethods(Vec<syn::ImplItemFn>);

impl Parse for VtableMethods {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut methods = Vec::new();
        while !input.is_empty() {
            let attrs = input.call(syn::Attribute::parse_outer)?;
            let name: syn::Ident = input.parse()?;
            let arguments;
            syn::parenthesized!(arguments in input);
            if name == "pad" || name == "offset" {
                let _: TokenStream = arguments.parse()?;
                input.parse::<syn::Token![;]>()?;
                continue;
            }
            let inputs = arguments.parse_terminated(FnArg::parse, syn::Token![,])?;
            let output: syn::ReturnType = input.parse()?;
            input.parse::<syn::Token![;]>()?;
            let mut method: syn::ImplItemFn = syn::parse_quote! {
                pub unsafe fn #name() #output { unreachable!() }
            };
            method.attrs = attrs;
            method.sig.inputs = inputs;
            methods.push(method);
        }
        Ok(Self(methods))
    }
}

pub fn expand(item: TokenStream) -> syn::Result<TokenStream> {
    match syn::parse2::<syn::Item>(item)? {
        syn::Item::Struct(item) => {
            if !item.generics.params.is_empty() {
                return Err(syn::Error::new_spanned(item.generics, "global targets must be concrete native types"));
            }
            let name = &item.ident;
            let proxy = format_ident!("{}Global", name);
            let vis = &item.vis;
            let cfg: Vec<_> = item.attrs.iter().filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr")).collect();
            let mut methods = Vec::new();
            for field in &item.fields {
                for attribute in &field.attrs {
                    if attribute.path().is_ident("vtable") {
                        for method in attribute.parse_args::<VtableMethods>()?.0 {
                            methods.push(forward(&method, name)?);
                        }
                    }
                }
            }
            Ok(quote! {
                #item
                #(#cfg)*
                /// Generated native-global forwarding. Does not own native storage.
                #vis struct #proxy { pointer: ::unchained_macros::NativeGlobalPointer }
                #(#cfg)*
                impl ::unchained_macros::NativeGlobalTarget for #name { type Global = #proxy; }
                #(#cfg)*
                impl #proxy {
                    #[doc(hidden)]
                    pub const fn new(binding: &'static ::unchained_macros::ValueBinding, indirect: bool) -> Self {
                        Self { pointer: ::unchained_macros::NativeGlobalPointer::new(binding, indirect) }
                    }
                    /// Load the current native pointer without borrowing its slot.
                    ///
                    /// # Safety
                    /// Native storage, lifetime, and synchronization rules apply.
                    pub unsafe fn as_ptr(&self) -> *mut #name {
                        unsafe { self.pointer.as_ptr() }
                    }
                    #(#methods)*
                }
            })
        }
        syn::Item::Impl(item) => {
            let Type::Path(path) = item.self_ty.as_ref() else {
                return Err(syn::Error::new_spanned(&item.self_ty, "expected concrete native type"));
            };
            let name = path.path.get_ident().ok_or_else(|| syn::Error::new_spanned(path, "global forwarding impl must use an unqualified native type"))?;
            let proxy = format_ident!("{}Global", name);
            let methods = item.items.iter().filter_map(|i| match i {
                syn::ImplItem::Fn(method) if !matches!(method.vis, syn::Visibility::Inherited) => Some(forward(method, name)),
                _ => None,
            }).collect::<syn::Result<Vec<_>>>()?;
            let cfg: Vec<_> = item.attrs.iter()
                .filter(|a| a.path().is_ident("cfg") || a.path().is_ident("cfg_attr"))
                .cloned().collect();
            let item = super::detour::expand_impl(TokenStream::new(), quote!(#item))?;
            Ok(quote!(#item #(#cfg)* impl #proxy { #(#methods)* }))
        }
        item => Err(syn::Error::new_spanned(item, "global forwarding requires a native struct or inherent impl")),
    }
}

fn forward(method: &syn::ImplItemFn, target: &syn::Ident) -> syn::Result<TokenStream> {
    let Some(FnArg::Receiver(receiver)) = method.sig.inputs.first() else {
        return Err(syn::Error::new_spanned(&method.sig, "global methods require a receiver"));
    };
    let pointer = syn::Ident::new("__unchained_global_pointer", Span::mixed_site());
    let native_receiver = match receiver.ty.as_ref() {
        Type::Ptr(ty) if ty.mutability.is_some() => quote!(#pointer),
        Type::Ptr(_) => quote!(#pointer.cast_const()),
        Type::Reference(ty) if ty.mutability.is_some() => quote!(&mut *#pointer),
        Type::Reference(_) => quote!(&*#pointer),
        _ => return Err(syn::Error::new_spanned(receiver, "global receivers must be borrowed or raw pointers")),
    };
    let mut signature = method.sig.clone();
    signature.unsafety = Some(syn::token::Unsafe::default());
    let mut proxy_receiver: syn::Receiver = syn::parse_quote!(&self);
    // Preserve explicit receiver lifetime relationships on returned guards/borrows.
    if let Some((_, lifetime)) = &receiver.reference {
        proxy_receiver.reference.as_mut().unwrap().1 = lifetime.clone();
        if let Type::Reference(ty) = proxy_receiver.ty.as_mut() {
            ty.lifetime = lifetime.clone();
        }
    }
    signature.inputs[0] = FnArg::Receiver(proxy_receiver);
    super::consume_native_parameter_metadata(&mut signature)?;
    struct ReplaceSelf<'a>(&'a syn::Ident);
    impl syn::visit_mut::VisitMut for ReplaceSelf<'_> {
        fn visit_type_mut(&mut self, ty: &mut Type) {
            if matches!(ty, Type::Path(p) if p.path.is_ident("Self")) {
                let target = self.0;
                *ty = syn::parse_quote!(#target);
            } else {
                syn::visit_mut::visit_type_mut(self, ty);
            }
        }
    }
    // Substitute native Self outside the proxy receiver.
    use syn::visit_mut::VisitMut;
    for argument in signature.inputs.iter_mut().skip(1) {
        if let FnArg::Typed(argument) = argument {
            ReplaceSelf(target).visit_type_mut(&mut argument.ty);
        }
    }
    ReplaceSelf(target).visit_return_type_mut(&mut signature.output);
    let arguments = signature.inputs.iter().skip(1).map(|arg| {
        let FnArg::Typed(arg) = arg else { unreachable!() };
        let Pat::Ident(name) = arg.pat.as_ref() else { return Err(syn::Error::new_spanned(arg, "global arguments must use plain identifiers")); };
        Ok(&name.ident)
    }).collect::<syn::Result<Vec<_>>>()?;
    let name = &method.sig.ident;
    let attrs = method.attrs.iter().filter(|attribute| {
        !attribute.path().segments.last().is_some_and(|segment|
            segment.ident == "bind_resolved_func" || segment.ident == "bind_resolved_request_function")
    });
    let vis = &method.vis;
    Ok(quote! {
        #(#attrs)*
        /// Forwards to the current native global on every call.
        ///
        /// # Safety
        /// Native storage, lifetime, synchronization, and method contracts apply.
        #[inline(always)]
        #vis #signature {
            let #pointer: *mut #target = unsafe { self.as_ptr() };
            unsafe { #target::#name(#native_receiver, #(#arguments),*) }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vtable_forwarding_uses_existing_signatures_and_raw_receiver() {
        let expanded = expand(quote! {
            pub struct Allocator {
                #[vtable(
                    offset(16);
                    Allocate(self: *mut Self, count: usize) -> *mut Self;
                    #[cfg(any())]
                    Inspect(self: *const Self) -> bool;
                )]
                table: *const (),
            }
        }).unwrap();
        syn::parse2::<syn::File>(expanded.clone()).unwrap();
        let expanded = expanded.to_string();
        assert!(expanded.contains("self . as_ptr ()"));
        assert!(expanded.contains("Allocator :: Allocate (__unchained_global_pointer , count)"));
        assert!(expanded.contains("-> * mut Allocator"));
        assert!(expanded.contains("cast_const"));
        assert!(!expanded.contains("Deref"));
    }
}
