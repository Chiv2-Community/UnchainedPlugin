use object::SectionKind;
use patternsleuth::{
    image::{pe::PEImage, Image, ImageType},
    Memory, NamedMemorySection,
};
use unchained_macros::signature::{DefaultResult, Signature, Simple_signature};

// No plugin modules or logging macros are needed.
unchained_macros::define_signature_fn!(literal_signature, |ctx, patterns| {
    let _ = ctx;
    Ok::<_, patternsleuth::resolvers::ResolveError>(DefaultResult(patterns[0].parse::<u64>().unwrap()))
});

fn image() -> Image<'static> {
    Image::from_memory(
        0x1000,
        Memory::from_sections(vec![NamedMemorySection::new(
            ".text".into(),
            0x2000,
            SectionKind::Text,
            vec![0x90, 0xAA, 0xBB, 0x90],
        )]),
        ImageType::PEImage(PEImage {
            exception_directory_range: 0..0,
            exception_children_cache: Default::default(),
        }),
    )
}

#[test]
fn signatures_resolve_and_clone_without_plugin_state() {
    patternsleuth::resolvers::eval(&image(), |ctx| Box::pin(async move {
        let signature = Simple_signature("AA BB");
        let cloned = signature.clone();
        assert_eq!(cloned.as_ref(), "AA BB");
        assert!(std::sync::Arc::ptr_eq(
            &signature.offset_calculator, &cloned.offset_calculator,
        ));
        assert_eq!(signature.calculate_offset(ctx).await.unwrap(), DefaultResult(0x2001));
        assert_eq!(cloned.calculate_offset(ctx).await.unwrap().offset(), 0x2001);
        assert_eq!(literal_signature("42").calculate_offset(ctx).await.unwrap().offset(), 42);
    }));
}

#[test]
fn missing_signature_returns_a_resolution_error() {
    patternsleuth::resolvers::eval(&image(), |ctx| Box::pin(async move {
        assert!(Simple_signature("CC DD").calculate_offset(ctx).await.is_err());
    }));
}

#[test]
fn all_existing_signature_factories_remain_available() {
    use unchained_macros::signature::{
        Call_signature, First_signature, XrefFirst_signature, XrefLast_signature,
    };
    for factory in [
        Simple_signature, Call_signature, First_signature, XrefFirst_signature, XrefLast_signature,
    ] {
        let signature: Signature<'_> = factory("AA BB");
        assert_eq!(signature.as_ref(), "AA BB");
    }
}
