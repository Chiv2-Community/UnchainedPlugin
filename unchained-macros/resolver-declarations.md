Declare a named resolver from a pattern list or selector dictionary.
Qualified dictionary keys identify the enum and its `ResolverSelector` provider.

```
#[unchained_macros::resolver_selection_key(require = [First, Second], fallback = Other)]
enum Key { First, Second, Other }
impl unchained_macros::ResolverSelector for Key {
    fn current() -> Self { Self::First }
}
unchained_macros::define_pattern_resolver!(Example, {
    Key::First: ["AA BB"], Key::Second: None, Key::Other: ["CC DD"],
});
fn main() {
    let snapshot = unchained_macros::SelectorSnapshot::capture_registered().unwrap();
    assert!(snapshot.supports_resolver("Example"));
    assert!(snapshot.chain::<Key>().unwrap() == [Key::First, Key::Other]);
}
```

The first declared entry on the frozen fallback chain wins. `None` disables
resolution and stops fallback. A nonempty pattern list is scanned as-is: missing
binary matches or ambiguity do not cause fallback. Empty lists are forbidden.
Required keys must be explicit even when the enum declares a fallback.
See the [resolver reference](resolvers.md) for scan modes and inline bodies.

Missing required keys fail compilation:

```compile_fail
# fn main() {}
#[unchained_macros::resolver_selection_key(require = [First, Second])]
enum Key { First, Second }
# impl unchained_macros::ResolverSelector for Key { fn current() -> Self { Self::First } }
unchained_macros::define_pattern_resolver!(Invalid, { Key::First: ["AA BB"] });
```

Empty entries fail compilation:

```compile_fail
# fn main() {}
#[unchained_macros::resolver_selection_key]
enum Key { First }
# impl unchained_macros::ResolverSelector for Key { fn current() -> Self { Self::First } }
unchained_macros::define_pattern_resolver!(Invalid, { Key::First: [] });
```

Duplicate entries fail compilation, including `None`:

```compile_fail
# fn main() {}
#[unchained_macros::resolver_selection_key]
enum Key { First }
# impl unchained_macros::ResolverSelector for Key { fn current() -> Self { Self::First } }
unchained_macros::define_pattern_resolver!(Invalid, {
    Key::First: None, Key::First: ["AA BB"],
});
```

Unknown variants and mixed enum types fail compilation:

```compile_fail
# fn main() {}
#[unchained_macros::resolver_selection_key]
enum Key { First }
# impl unchained_macros::ResolverSelector for Key { fn current() -> Self { Self::First } }
unchained_macros::define_pattern_resolver!(Invalid, { Key::Missing: ["AA BB"] });
```

```compile_fail
# fn main() {}
#[unchained_macros::resolver_selection_key]
enum Key { First }
#[unchained_macros::resolver_selection_key]
enum Other { First }
# impl unchained_macros::ResolverSelector for Key { fn current() -> Self { Self::First } }
# impl unchained_macros::ResolverSelector for Other { fn current() -> Self { Self::First } }
unchained_macros::define_pattern_resolver!(Invalid, {
    Key::First: ["AA BB"], Other::First: ["CC DD"],
});
```
