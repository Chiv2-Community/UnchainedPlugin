use super::PlatformType;

/// Platforms declared by a pattern resolver; OTHER means a shared signature.
pub struct ResolverPlatforms {
    pub name: &'static str,
    pub platforms: &'static [PlatformType],
}

inventory::collect!(ResolverPlatforms);

pub fn supports_platform(name: &str, platform: PlatformType) -> bool {
    inventory::iter::<ResolverPlatforms>
        .into_iter()
        .filter(|resolver| resolver.name == name)
        .all(|resolver| {
            resolver.platforms.contains(&platform)
                || resolver.platforms.contains(&PlatformType::OTHER)
        })
}
