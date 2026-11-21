use object::SectionKind;
use patternsleuth::{
    image::{pe::PEImage, Image, ImageType},
    Memory, NamedMemorySection,
};
use unchained_macros::signature::{DefaultResult, Signature, Simple_signature};

unchained_macros::define_pattern_resolver!(CallMode, Call, ["E8 | ?? ?? ?? ??"]);

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

#[test]
fn typed_call_decodes_the_rel32_target() {
    use unchained_macros::signature::Call_signature;
    let image = Image::from_memory(
        0x1000,
        Memory::from_sections(vec![NamedMemorySection::new(
            ".text".into(), 0x2000, SectionKind::Text,
            vec![0xE8, 0x0B, 0, 0, 0, 0x90],
        )]),
        ImageType::PEImage(PEImage {
            exception_directory_range: 0..0,
            exception_children_cache: Default::default(),
        }),
    );
    patternsleuth::resolvers::eval(&image, |ctx| Box::pin(async move {
        assert_eq!(Call_signature("E8 | ?? ?? ?? ??")
            .calculate_offset(ctx).await.unwrap().offset(), 0x2010);
    }));
}

#[test]
fn typed_first_accepts_multiple_matches() {
    use unchained_macros::signature::First_signature;
    let image = Image::from_memory(
        0x1000,
        Memory::from_sections(vec![NamedMemorySection::new(
            ".text".into(), 0x2000, SectionKind::Text, vec![0xAA, 0xBB, 0xAA, 0xBB],
        )]),
        ImageType::PEImage(PEImage {
            exception_directory_range: 0..0,
            exception_children_cache: Default::default(),
        }),
    );
    patternsleuth::resolvers::eval(&image, |ctx| Box::pin(async move {
        assert_eq!(First_signature("AA BB")
            .calculate_offset(ctx).await.unwrap().offset(), 0x2000);
    }));
}

#[test]
fn typed_xrefs_return_enclosing_functions_not_the_matched_data() {
    use unchained_macros::signature::{XrefFirst_signature, XrefLast_signature};
    let mut code = vec![0x90; 0x40];
    for start in [2usize, 0x22] {
        code[start..start + 3].copy_from_slice(&[0x48, 0x8D, 0x05]);
        let displacement = (0x3000i32 - (0x2000 + start as i32 + 7)).to_le_bytes();
        code[start + 3..start + 7].copy_from_slice(&displacement);
    }
    let functions: Vec<u8> = [0x1000u32, 0x1020, 0x2100, 0x1020, 0x1040, 0x2100]
        .into_iter().flat_map(u32::to_le_bytes).collect();
    let image = Image::from_memory(
        0x1000,
        Memory::from_sections(vec![
            NamedMemorySection::new(".text".into(), 0x2000, SectionKind::Text, code),
            NamedMemorySection::new(".rdata".into(), 0x3000, SectionKind::ReadOnlyData, vec![0xAA, 0xBB, 0xCC, 0xDD]),
            NamedMemorySection::new(".xdata".into(), 0x3100, SectionKind::ReadOnlyData, vec![0; 4]),
            NamedMemorySection::new(".pdata".into(), 0x4000, SectionKind::ReadOnlyData, functions),
        ]),
        ImageType::PEImage(PEImage {
            exception_directory_range: 0x4000..0x4018,
            exception_children_cache: Default::default(),
        }),
    );
    patternsleuth::resolvers::eval(&image, |ctx| Box::pin(async move {
        assert_eq!(XrefFirst_signature("AA BB CC DD")
            .calculate_offset(ctx).await.unwrap().offset(), 0x2000);
        assert_eq!(XrefLast_signature("AA BB CC DD")
            .calculate_offset(ctx).await.unwrap().offset(), 0x2020);
    }));
}
