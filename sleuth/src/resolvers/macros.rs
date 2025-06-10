
// Testing SomeTestFkt
// EGS: ["40 53 FF", "40 53 48"]
// Testing SomeTestFkt
// OTHER: ["11 53 B8", "22 53 FF", "40 53 4F", "40 53 A8"]
// combined ["40 53 FF", "40 53 48", "11 53 B8", "22 53 FF", "40 53 4F", "40 53 A8"]
// define_pattern_resolver![SomeTestFkt, {
//     OTHER : ["11 53 B8", "22 53 FF", "40 53 4F", "40 53 A8"],
//     EGS : ["40 53 FF", "40 53 48"],
//     XBOX : ["40 53 48", "40 53 48"],
//     STEAM : ["40 53 48", "40 53 48", "40 53 48", "40 53 48"],
// }];

// #[macro_use]
// extern crate paste; // concat strings

// unsafe fn attach_TestFunction(base_address: usize, offsets: HashMap<String, u64>)  -> Result<(), Box<dyn Error>>{
//   let address = base_address + offsets["TestFunction"] as usize; 
//   let target: FnTestFunction = mem::transmute(address);
//   type FnTestFunction = unsafe extern "C" fn(*mut c_void, f32, u8);
//   static_detour! {
//     static TestFunction: unsafe extern "C" fn(*mut c_void, f32, u8);
//   }
//   fn detour_fkt(engine:*mut c_void, delta:f32, state:u8) {
//       println!("rust TestFunction delta: {}", delta);
//       unsafe { TestFunction.call( engine, delta, state) }
//   }
//   TestFunction
//     .initialize(target, detour_fkt)?
//     .enable()?;
//   Ok(())
// }
/// ```rust
/// unsafe fn attach_GameEngineTick(base_address: usize, offsets: HashMap<String, u64>)  -> Result<(), Box<dyn Error>>{
///   let address = base_address + offsets["UGameEngineTick"] as usize; 
///   let target: FnUGameEngineTick = mem::transmute(address);
///   type FnUGameEngineTick = unsafe extern "C" fn(*mut c_void, f32, u8);
///   static_detour! {
///     static UGameEngineTick: unsafe extern "C" fn(*mut c_void, f32, u8);
///   }
///   fn detour_fkt(engine:*mut c_void, delta:f32, state:u8) {
///       println!("rust UGameEngineTick delta: {}", delta);
///       unsafe { UGameEngineTick.call( engine, delta, state) }
///   }
///   UGameEngineTick
///     .initialize(target, detour_fkt)?
///     .enable()?;
///   Ok(())
/// }
/// ```
/// 
/// 
// TODO: replace this with registration similar to resolvers
pub type HookFn = unsafe fn(usize, HashMap<String, u64>) -> Result<Option<usize>, Box<dyn std::error::Error>>;
// macro_rules! attach_hooks_list {
//     ( [ $( $pattern:ident ),+ $(,)? ]) => {
//         paste::paste!{
//             {
//                 let hooks: HashMap<String, u64>= [ $( ( stringify![$pattern], [<attach_ $pattern>] ) ),+ ]
//                                     .into_iter().map(|(name, func)| (name.to_string(), func)).collect();
//                 Ok(hooks)
//             }
//         }
//     };
// }

// #[macro_export]
// macro_rules! attach_hooks_list {
//     ( [ $( $pattern:ident ),+ $(,)? ]) => {{
//         use std::collections::HashMap;
//         use crate::resolvers::macros::HookFn;
//         paste::paste! {
//             let hooks: HashMap<&'static str, HookFn> = [
//                 $((stringify!($pattern), [<attach_ $pattern>] as HookFn)),+
//             ]
//             .into_iter()
//             .collect();

//             hooks
//         }
//     }};
// }
#[macro_export]
macro_rules! attach_hooks_list {
    ( [ $(
        $(#[$attr:meta])*
        $pattern:ident
    ),+ $(,)? ]) => {{
        use std::collections::HashMap;
        use $crate::resolvers::macros::HookFn;
        paste::paste! {
            #[allow(dead_code)]
            enum ActiveHooks {
                $(
                    $(#[$attr])*
                    $pattern
                ),+
            }

            let hooks: HashMap<&'static str, HookFn> = [
                $(
                    $(#[$attr])*
                    (stringify!($pattern), [<attach_ $pattern>] as HookFn)
                ),+
            ]
            .into_iter()
            .collect();

            hooks
        }
    }};
}


// generate_stub!(StubName);
#[macro_export]
macro_rules! CREATE_HOOK {
    
    // ($name:ident, $mode:ident, $rettype:ident, ( $( $call_type:ident:  $pattern:expr  ),+ $(,)? )) => {
    // ($name:ident, $mode:ident, $rettype:ident, ( $( $call_type:ident:  $pattern:expr  ),+ $(,)? )) => {
    //     println!($name);
    //     println!($mode);
    //     println!($rettype);

    //     [ $( $call_type ($pattern) ),+ ];
    // };
    
    ($name:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:block) => {
        CREATE_HOOK!($name, ::std::ffi::c_void, ( $( $arg: $ty ),+ ), $body);
    };

    ($name:ident, $out_type:ty, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:block) => {
        
      paste::paste! {

        ::retour::static_detour! {
          pub static [<o_ $name>]: unsafe extern "C" fn ($( $ty ),+ ) -> $out_type;
        }

        #[allow(non_snake_case)]
        pub fn [<$name _detour_fkt>]( $( $arg: $ty ),+ ) -> $out_type {
            // println!("rust $name delta: {}", delta);
            $body
            unsafe { [<o_ $name>].call ( $( $arg ),+ ) }
        }

        #[allow(non_snake_case)]
        pub unsafe fn [<attach_ $name>](base_address: usize, offsets: std::collections::HashMap<String, u64>)  -> Result<Option<usize>, Box<dyn std::error::Error>>{
        // pub unsafe fn [<attach_ $name>](base_address: usize, offsets: HashMap<String, u64>)  -> Result<(), Box<dyn Error> {
        
        // TODO: propagate error? why panic
          match offsets.get(stringify![$name]) {
            None => {
                Err("No address found.".into())// Err("No Address found."),//log::error!["Failed to attach: {}", stringify![$name]],
            },
            Some(_) => {
                // log::info!["attached"];
                // ( $( $arg: $ty ),+ );
                let rel_address = offsets[stringify![$name]] as usize;
                let target: [<Fn $name>] = std::mem::transmute(base_address + rel_address);
      
                type [<Fn $name>] = unsafe extern "C" fn ($( $ty ),+ ) -> $out_type;
      
                [<o_ $name>]
                  .initialize(target, [<$name _detour_fkt>])?
                  .enable()?;;
      
                // crate::debug_where!("Attached [ 0x{:#x?} ]", rel_address);
                $crate::sdebug!(f; "Attached [ 0x{:#x?} ]", rel_address);
                Ok(Some(rel_address as usize))
            },
          }
        }
      }
    };
}

// #[macro_export]
// macro_rules! CREATE_HOOK {
    
//     // ($name:ident, $mode:ident, $rettype:ident, ( $( $call_type:ident:  $pattern:expr  ),+ $(,)? )) => {
//     // ($name:ident, $mode:ident, $rettype:ident, ( $( $call_type:ident:  $pattern:expr  ),+ $(,)? )) => {
//     //     println!($name);
//     //     println!($mode);
//     //     println!($rettype);

//     //     [ $( $call_type ($pattern) ),+ ];
//     // };
//     ($name:ident, ( $( $arg:ident: $ty:ty ),+ $(,)? ), $body:block) => {
//       paste::paste! {
//         #[allow(non_snake_case)]
//         pub unsafe fn [<attach_ $name>](base_address: usize, offsets: HashMap<String, u64>)  -> Result<(), Box<dyn Error>>{
        
//           // ( $( $arg: $ty ),+ );
//           let address = base_address + offsets[stringify![$name]] as usize; 
//           let target: [<Fn $name>] = mem::transmute(address);

//           type [<Fn $name>] = unsafe extern "C" fn ($( $ty ),+ );

//           static_detour! {
//             pub static [<o_ $name>]: unsafe extern "C" fn ($( $ty ),+ );
//           }

//           pub fn detour_fkt( $( $arg: $ty ),+ ) {
//               // println!("rust $name delta: {}", delta);
//               $body
//               unsafe { [<o_ $name>].call ( $( $arg ),+ ) }
//           }

//           [<o_ $name>]
//             .initialize(target, detour_fkt)?
//             .enable()?;

//           Ok(())
//         }
//       }


//     };
// }


use std::{collections::HashMap, future::Future, pin::Pin};
use patternsleuth::resolvers::{AsyncContext, ResolveError};


#[macro_export]
macro_rules! define_pocess {
    // Note: After adding a new process, add a MultiCall handler below    
    // e.g. wrap_process_macro!(Simple);

    // Simple sig pattern lookup (returns start address)
    (@emit_body $name:ident, Simple, $ctx:ident, $patterns:ident) => {{
        define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter()
                    .map(|p| $ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;
    
            ::patternsleuth::resolvers::ensure_one(futures.into_iter().flatten())?
        })
    }};

    (@emit_body $name:ident, First, $ctx:ident, $patterns:ident) => {{
        define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            let futures = ::patternsleuth::resolvers::futures::future::join_all(
                $patterns.iter()
                    .map(|p| $ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
            ).await;
    
            // FIXME
            futures.into_iter().flatten().collect::<Vec<usize>>()[0]
        })
    }};

    // Scan for a function call, extract 4 bytes and return address of the called function
    // e.g. "E8 | ?? ?? ?? ?? 4C 39 ?8 74 3?" returns address of the function after |
    (@emit_body $name:ident, Call, $ctx:ident, $patterns:ident) => {{
        use patternsleuth::MemoryTrait;
        define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            let res = futures::future::join_all($patterns.iter().map(|p| $ctx.scan(patternsleuth::scanner::Pattern::new(p).unwrap()))).await;
            
            patternsleuth::resolvers::try_ensure_one(
                res.iter()
                    .flatten()
                    .map(|a| -> patternsleuth::resolvers::Result<usize> { Ok($ctx.image().memory.rip4(*a)?) }))?
        })
    }};
    
    (@emit_body $name:ident, MultiCall, $ctx:ident, $patterns:ident) => {{
        // use patternsleuth::MemoryTrait;
        define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            let mut results = Vec::new();
            // FIXME: Nihi: group sigs by type, run Signature func on multiple
            for pat in $patterns {
                // match pat.kind {
                //     SignatureKind::Call => println!("call"),
                //     SignatureKind::Function => println!("function"),
                //     _ => print!("NO MATCH")
                // }
                let offset = pat.calculate_offset($ctx).await.map(|r| r.offset());
                // print!("PART RESULT: {:?}", offset);
                results.push(offset);
            }
            ensure_one(results.into_iter().flatten())?
        })
    }};

    // Scan for Xrefs, return last
    // FIXME: Nihi: expects max. 2 results
    // FIXME: Nihi: handles only one pattern
    (@emit_body $name:ident, XrefLast, $ctx:ident, $patterns:ident) => {{
        use patternsleuth::resolvers::unreal::util;
        use patternsleuth::resolvers::ensure_one;
        define_pocess!(@emit_process_inline $name, |$ctx, $patterns| {
            // let strings = futures::future::join_all(patterns.iter().map(|p| ctx.scan(p.clone()))).await;
            let strings = $ctx.scan($patterns.first().unwrap().clone()).await;
            let refs = util::scan_xrefs($ctx, &strings).await;
            let mut fns = util::root_functions($ctx, &refs)?;
            if fns.len() == 2 {
                fns[0] = fns[1]; // FIXME: Nihi: deque? last?
                fns.pop();
            }
            ensure_one(fns)?
        })
    }};

    // Wrap code and define_pattern_resolver
    (@emit_process_inline $name:ident, |$ctx:ident, $patterns:ident| $body:block) => {{
        let result = $body;
        log::debug![target:"sig_scan", "[ 0x{:#x?} ]: {}", result, stringify![$name]];
        Ok($name(result))
    }};
}

#[allow(unused_macros)]
macro_rules! generate_stub {
    ( $name:ident ) => {
        // $name
        #[cfg(feature="dev")]
        define_pattern_resolver!($name, [
            "4C"
        ]);
        
        #[cfg(feature="dev")]
        CREATE_HOOK!($name, (arg0: *mut c_void), {
            crate::sinfo![f; "Triggered!"];
        });
    };
}

// inline this
// generate_stub!(StubName);
#[macro_export]
macro_rules! define_pattern_resolver {

    // Internal: produce header
    (@emit_header $name:ident) => {
        #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
        #[allow(non_camel_case_types)]
        pub struct $name(pub usize);
    };

    // Called with Mode (Process) specified
    //  (NAME, MODE, LIST) 
    ($name:ident, $mode:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let patterns = [ $( $pattern ),+ ];

            define_pocess!(@emit_body $name, $mode, ctx, patterns)
        });
    };

    // Called with a body block
    // (NAME, LIST, |ctx, patterns| { CODE } )
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ], |$ctx:ident, $patterns:ident| $body:block) => {
        define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let $patterns = [ $( $pattern ),+ ];

            define_pocess!(@emit_process_inline $name, |ctx, patterns| $body)
        });
    };

    // Called with Mode (Process) specified
    //  (NAME, MODE, DICT) 
    ($name:ident, $mode:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        define_pattern_resolver!(@emit_header $name);

        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |ctx| async {
            let platform = super::current_platform();
            let mut patterns: Vec<_> = Vec::new();
            for var in [platform, super::PlatformType::OTHER] {
                // println!("Testing {}", stringify!($name));                
                let pattern_part: &[_] = match var {
                    $(
                        super::PlatformType::$platform => &[ $( $pattern ),+ ],
                    )+
                    _ => &[],
                };                
                // println!("{}: {:?}", var, pattern_part);
                patterns.extend_from_slice(pattern_part);
            }            
            // println!("combined {:?}", patterns);            
            define_pocess!(@emit_body $name, $mode, ctx, patterns)
        });
    };

    // Called with a body block
    //  (NAME, MODE, DICT) 
    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }, |$ctx:ident, $patterns:ident| $body:block ) => {
        define_pattern_resolver!(@emit_header $name);
        
        ::patternsleuth::resolvers::impl_resolver_singleton!(all, $name, |$ctx| async {
            let platform = super::current_platform();
            let mut $patterns: Vec<_> = Vec::new();
            for var in [platform, super::PlatformType::OTHER] {
                // println!("Testing {}", stringify!($name));                
                let pattern_part: &[_] = match var {
                    $(
                        super::PlatformType::$platform => &[ $( $pattern ),+ ],
                    )+
                    _ => &[],
                };                
                // println!("{}: {:?}", var, pattern_part);
                $patterns.extend_from_slice(pattern_part);
            }            
            // println!("combined {:?}", patterns);            

            define_pocess!(@emit_process_inline $name, |ctx, patterns| $body)
        });
    };

    // Multicall Fallbacks (No keyword specified)
    
    //  (NAME, LIST) 
    // define_pattern_resolver![GetMotdTwoS2, [
    //     Simple_signature("4C 89"),
    //     function_first_signature("4C 89")
    // ]];
    ($name:ident,[ $( $call_type:ident ( $pattern:expr ) ),+ $(,)? ]) => {
            define_pattern_resolver![$name, MultiCall,
                [ $( $call_type ($pattern) ),+ ]
            ];
    };

    // //  (NAME, DICT) 
    // define_pattern_resolver!(GetMotdTwoS, {
    //     EGS: [
    //         Simple_signature("4C 89"),
    //         function_first_signature("4C 89")
    //     ],
    //     STEAM: [call_signature("4C 89 | ?? ?? ?? ?? BE EF")]
    // });
    ($name:ident, { $( $platform:ident : [ $( $call_type:ident ( $pattern:expr ) ),+ $(,)? ] ),+ $(,)? }) => {
        define_pattern_resolver!($name, MultiCall, {
            $( $platform : [ $( $call_type ($pattern) ),+ ] ),+
        });
    };

    // Fallbacks for cases with no mode specified (assuming Simple)
    
    //  (NAME, LIST) 
    // define_pattern_resolver!(GetMotdTwoS, [
    //         "4C 89",
    //         "4C 89"
    //     ]);
    ($name:ident, [ $( $pattern:expr ),+ $(,)? ]) => {
        define_pattern_resolver![$name, Simple,
            [ $( $pattern ),+ ]
        ];
    };

    //  (NAME, DICT) 
    // define_pattern_resolver!(GetMotdTwoS, {
    //     EGS: [
    //         "4C 89",
    //         "4C 89"
    //     ],
    //     STEAM: ["4C 89 | ?? ?? ?? ?? BE EF"]
    // });
    ($name:ident, { $( $platform:ident : [ $( $pattern:expr ),+ $(,)? ] ),+ $(,)? }) => {
        define_pattern_resolver!($name, Simple, {
            $( $platform : [ $( $pattern ),+ ] ),+
        });
    };
}

#[macro_export]
macro_rules! define_process {
    ($src:ident, $name:ident, $body:block) => {
        async fn $name(
            ctx: &patternsleuth::resolvers::AsyncContext<'_>,
            patterns: Vec<patternsleuth::scanner::Pattern>
        ) -> patternsleuth::resolvers::Result<$src> {
            let result: Vec<usize> = Vec::new();
            $body
            Ok($src(ensure_one(result)?))
        }
    };
}

// custom handlers
define_pattern_resolver!(@emit_header DefaultResult);

#[allow(dead_code)]
impl DefaultResult {
    pub fn offset(&self) -> usize {
        self.0 // assuming it's something like `pub struct DefaultResult(pub usize)`
    }
}

// #[derive(Debug, Clone, Copy)]
// pub enum SignatureKind {
//     Call,
//     Function,
// }

type OffsetFuture<'a> = Pin<Box<dyn Future<Output = Result<DefaultResult, ResolveError>> + Send + 'a>>;
pub type OffsetCalculator<'a> = std::sync::Arc<
    dyn Fn(&'a AsyncContext<'a>) -> OffsetFuture<'a> + Send + Sync + 'a,
>;

pub struct Signature<'a> {
    // pub kind: SignatureKind,
    pub offset_calculator: OffsetCalculator<'a>,
    pub signature_string: String,
}

#[allow(dead_code)]
impl<'a> Signature<'a> {
    pub async fn calculate_offset(&self, ctx: &'a AsyncContext<'a>) -> Result<DefaultResult, ResolveError> {
        (self.offset_calculator)(ctx).await
    }
}

impl<'a> AsRef<str> for Signature<'a> {
    fn as_ref(&self) -> &str {
        &self.signature_string
    }
}
// for extend_from_slice
impl<'a> Clone for Signature<'a> {
    fn clone(&self) -> Self {
        Signature {
            // kind: self.kind,
            offset_calculator: self.offset_calculator.clone(),
            signature_string: self.signature_string.clone(),
        }
    }
}

// // Constructor for FunctionSignature — async closure with some logic
// pub fn function_signature<'a>(s: &'a str) -> Signature<'a> {
//     // let sig_clone = s.to_string();
//     let calc: Arc<
//     dyn Fn(&AsyncContext<'a>) -> Pin<Box<dyn Future<Output = Result<DefaultResult, ResolveError>> + Send + 'a>>
//         + Send
//         + Sync
//         + 'a,
//     > = Arc::new(move |ctx: &AsyncContext<'a>| {
//         let sig = s.to_string();
//         let ctx = ctx.clone();
//         let fut = async move {
//             // TODO: this takes only one sig rn, should be grouped first instead
//             // println!("CALCULATING OFFSET FOR {}", sig);
//             let v = &[sig];
//             define_pocess!(@emit_body DefaultResult, Simple, ctx, v)
//         };
//         Box::pin(fut)
//     });
//     Signature {
//         kind: SignatureKind::Function,
//         offset_calculator: calc,
//         signature_string: s.to_string(),
//     }
// }

// Example: create new handlers using macro below
// Wrap existing Mode implementation
// define_signature_fn!(call_signature, SignatureKind::Call, 
//     | ctx, patterns | { define_pocess!(@emit_body DefaultResult, Call, ctx, patterns)}
// );
// Wrap Custom implementation
// define_signature_fn!(function_first_signature, SignatureKind::Call, 
//     | ctx, patterns | {
//         let futures = ::patternsleuth::resolvers::futures::future::join_all(
//             patterns.iter()
//                 .map(|p| ctx.scan(::patternsleuth::scanner::Pattern::new(p).unwrap()))
//         ).await;
//         // FIXME
//         Ok(DefaultResult(futures.into_iter().flatten().collect::<Vec<usize>>()[0]))
//     }
// );
#[macro_export]
macro_rules! define_signature_fn {
    (
        $fn_name:ident,
        // $kind:expr,
        | $ctx:ident, $patterns:ident| $body:block
    ) => {
        #[allow(non_snake_case)]
        #[allow(dead_code)]
        pub fn $fn_name<'a>(s: &'a str) -> $crate::resolvers::macros::Signature<'a> {
            let calc: std::sync::Arc<
                dyn Fn(&AsyncContext<'a>) -> Pin<Box<dyn Future<Output = Result<DefaultResult, ResolveError>> + Send + 'a>>
                    + Send
                    + Sync
                    + 'a,
            > = std::sync::Arc::new(move |ctx: &AsyncContext<'a>| {
                let sig = s.to_string();
                let ctx = ctx.clone();
                let fut = async move {
                    let v = &[sig];
                    let $ctx = ctx;
                    let $patterns = v;
                    $body
                };
                Box::pin(fut)
            });

            $crate::resolvers::macros::Signature {
                // kind: $kind,
                offset_calculator: calc,
                signature_string: s.to_string(),
            }
        }
    };
}

// macro to wrap process definition like this
// wrap_process_macro!(Simple);
// creates Simple_signature fn
#[macro_export]
macro_rules! wrap_process_macro {
    ($fn_name:ident) => {
        paste::paste! {
            define_signature_fn!(
                [<$fn_name _signature>],
                // SignatureKind::Function,
                |ctx, patterns| {
                    define_pocess!(@emit_body DefaultResult, Simple, ctx, patterns)
                }
            );
        }
    };
}

// FIXME: Nihi: move somewhere so those are not buried?
// Possible to auto generate from macro?
wrap_process_macro!(Simple);
wrap_process_macro!(Call);
wrap_process_macro!(First);
wrap_process_macro!(XrefLast);

// Simpler version

// // Define a type alias for the function signature, to improve readability
// type OffsetCalculator = fn(address: usize) -> usize;

// pub struct Signature {
//     kind: SignatureKind,
//     offset_calculator: OffsetCalculator,
//     signature_string: String,
// }

// #[derive(Debug, Clone, Copy)]
// pub enum SignatureKind {
//     Call,
//     Function,
// }

// impl Signature {

//     pub fn calculate_offset(&self, address: usize) -> usize {
//         (self.offset_calculator)(address)
//     }
// }

// impl Clone for Signature {
//     fn clone(&self) -> Self {
//         Signature {
//             kind: self.kind,
//             offset_calculator: self.offset_calculator,
//             signature_string: self.signature_string.clone(),
//         }
//     }
// }

// // Implement a constructor function for CallSignature
// pub fn call_signature(s: &str) -> Signature {
//     fn default_offset_calculator(address: usize) -> usize {
//         address
//     }

//     Signature {
//         kind: SignatureKind::Call,
//         offset_calculator: default_offset_calculator,
//         signature_string: s.to_string(),
//     }
// }

// pub fn sc(s: &str) -> Signature {
//     call_signature(s)
// }

// // Implement a constructor function for FunctionSignature
// pub fn function_signature(s: &str) -> Signature {
//     //  -> patternsleuth::resolvers::Result<usize>
//     // define_pattern_resolver!(@emit_header DefaultResult);
//     async fn adjusted_offset_calculator(ctx: patternsleuth::resolvers::AsyncContext<'_>, patterns: Vec<&str>) -> Result<DefaultResult, patternsleuth::resolvers::ResolveError>{
//         define_pocess!(@emit_body DefaultResult, Simple, ctx, patterns)
//     }

//     fn default_offset_calculator(address: usize) -> usize {
//         address
//     }

//     Signature {
//         kind: SignatureKind::Function,
//         offset_calculator: adjusted_offset_calculator,
//         // offset_calculator: adjusted_offset_calculator,
//         signature_string: s.to_string(),
//     }
// }

// pub fn sf(s: &str) -> Signature {
//     function_signature(s)
// }