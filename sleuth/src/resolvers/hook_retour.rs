// use retour::static_detour;
// use std::collections::HashMap;
// use std::error::Error;
// use std::os::raw::c_void;
// use std::mem;


// CREATE_HOOK!(TESTFKT, (engine:*mut c_void, delta:f32, state:u8), {
//     println!("rust UGameEngineTick delta: {}", delta);
// });

// use once_cell::sync::Lazy;

// fn get_base_address
// fn get_offsets() -> HashMap<String, u64> {
//   HashMap<String, u64>()
// }

// static _GAME_ENGINE_TICK_HOOK: Lazy<Result<(), Box<dyn std::error::Error>>> = Lazy::new(|| {
//     // Provide the actual base address and offsets here
//     let base_address = get_base_address(); // <-- define this globally
//     let offsets = get_offsets();           // <-- define this globally
//     unsafe { attach_TESTFKT(base_address, offsets) }
// });

// // Force initialization at compile time (optional, ensures it runs)
// #[used]
// static _FORCE_GAME_ENGINE_TICK_HOOK: &Lazy<Result<(), Box<dyn std::error::Error>>> = &_GAME_ENGINE_TICK_HOOK;


// CREATE_HOOK!(UGameEngineTick, HOOK_ATTACH, c_void, (engine:c_void, delta:f32, state:u8));

// unsafe fn attach_GameEngineTick(base_address: usize, offsets: HashMap<String, u64>)  -> Result<(), Box<dyn Error>>{

//   let address = base_address + offsets["UGameEngineTick"] as usize; 
//   let target: FnUGameEngineTick = mem::transmute(address);
  
//   type FnUGameEngineTick = unsafe extern "C" fn(*mut c_void, f32, u8);

//   static_detour! {
//     static UGameEngineTick: unsafe extern "C" fn(*mut c_void, f32, u8);
//   }

//   fn detour_fkt(engine:*mut c_void, delta:f32, state:u8) {
//       println!("rust UGameEngineTick delta: {}", delta);
//       unsafe { UGameEngineTick.call( engine, delta, state) }
//   }
  
//   UGameEngineTick
//     .initialize(target, detour_fkt)?
//     .enable()?;

//   Ok(())
// }

// pub unsafe fn attach_hooks(base_address: usize, offsets: HashMap<String, u64>) -> Result<(), Box<dyn Error>> {

//   // attach_GameEngineTick(base_address, offsets).unwrap();
//   attach_TESTFKT(base_address, offsets).unwrap();
//   Ok(())
// }
