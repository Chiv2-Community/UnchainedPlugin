
use std::{ffi::CStr, mem::zeroed, thread::sleep, time::Duration};

// use tokio::time::Sleep;
use winapi::vc::excpt::EXCEPTION_CONTINUE_SEARCH;
use windows::Win32::{Foundation::EXCEPTION_ACCESS_VIOLATION, System::{Diagnostics::Debug::{AddVectoredExceptionHandler, EXCEPTION_POINTERS, SYMBOL_INFO, SymFromAddr, SymInitialize}, Threading::GetCurrentProcess}};



use windows::Win32::{
    Foundation::*,
    System::{
        Diagnostics::Debug::*,
    },
};

use std::sync::Once;
static SYMBOLS_INIT: Once = Once::new();

#[repr(C, align(8))]
struct SymBuffer {
    sym: SYMBOL_INFO,
    name: [u8; 256],
}

fn ensure_symbols() {
    SYMBOLS_INIT.call_once(|| {
        let process = unsafe { GetCurrentProcess() };

        // Load symbols for all modules
        match unsafe { SymInitialize(process, None, TRUE) } {
            Ok(_) => {
                unsafe {
                    SymSetOptions(
                        SYMOPT_DEFERRED_LOADS
                            | SYMOPT_UNDNAME
                            | SYMOPT_LOAD_LINES,
                    );
                }
            }
            Err(e) => {
                log::error!("SymInitialize failed: {}", e);
            }
        }
    });
}

type NameAndLocation = (Option<String>, Option<String>);

fn resolve_symbol(addr: u64) -> NameAndLocation {
    ensure_symbols();

    unsafe {
        let process = GetCurrentProcess();

        // ---------- Function name ----------
        let mut buffer: SymBuffer = zeroed();
        let sym = &mut buffer.sym;

        sym.SizeOfStruct = std::mem::size_of::<SYMBOL_INFO>() as u32;
        sym.MaxNameLen = 255;

        let mut name = None;
        if SymFromAddr(process, addr, None, sym).is_ok() {
            let name_ptr = sym.Name.as_ptr() as *const i8;
            if !name_ptr.is_null() {
                let cstr = CStr::from_ptr(name_ptr);
                name = Some(cstr.to_string_lossy().into_owned());
            }
        }

        // ---------- File + line ----------
        let mut line: IMAGEHLP_LINE64 = zeroed();
        line.SizeOfStruct = std::mem::size_of::<IMAGEHLP_LINE64>() as u32;

        let mut displacement = 0u32;
        let mut file_line = None;

        if SymGetLineFromAddr64(process, addr, &mut displacement, &mut line).is_ok() {
            if !line.FileName.is_null() {
                let file = CStr::from_ptr(line.FileName.0 as *const i8);
                file_line = Some(format!(
                    "{}:{} (+0x{:X})",
                    file.to_string_lossy(),
                    line.LineNumber,
                    displacement
                ));
            }
        }

        (name, file_line)
    }
}
unsafe extern "system" fn sf_table_access64(hprocess: HANDLE, addrbase: u64) -> *mut std::ffi::c_void {
    SymFunctionTableAccess64(hprocess, addrbase)
}

unsafe extern "system" fn get_module_base64(hprocess: HANDLE, addr: u64) -> u64 {
    SymGetModuleBase64(hprocess, addr)
}

use winapi::um::winnt::IMAGE_FILE_MACHINE_AMD64;

use crate::{discord::notifications::CrashEvent, dispatch};
fn print_stack(ctx: &mut CONTEXT) -> Vec<String> {
    let process = unsafe { GetCurrentProcess() };
    let thread: HANDLE = HANDLE(-1); // current thread
    let mut frame: STACKFRAME64 = unsafe { zeroed() };
    let mut trace: Vec<String> = Vec::new();

    frame.AddrPC.Mode = AddrModeFlat;
    frame.AddrStack.Mode = AddrModeFlat;
    frame.AddrFrame.Mode = AddrModeFlat;

    frame.AddrPC.Offset = ctx.Rip;
    frame.AddrStack.Offset = ctx.Rsp;
    frame.AddrFrame.Offset = ctx.Rbp;

    for i in 0..10 {
        let ok = unsafe {
            StackWalk64::<HANDLE, HANDLE>(
                IMAGE_FILE_MACHINE_AMD64.into(),
                process,
                thread,
                &mut frame,
                ctx as *mut _ as *mut _,
                None,
                Some(sf_table_access64),
                Some(get_module_base64),
                None,
            )
        }
        .as_bool();

        if !ok || frame.AddrPC.Offset == 0 {
            break;
        }

        let mut buffer: SymBuffer = unsafe { zeroed() };
        let sym = &mut buffer.sym;

        sym.SizeOfStruct = std::mem::size_of::<SYMBOL_INFO>() as u32;
        sym.MaxNameLen = 255;

        if unsafe { SymFromAddr(process, frame.AddrPC.Offset, None, sym).is_ok() } {
            let func_name = unsafe {
                let name_ptr = sym.Name.as_ptr() as *const i8;
                if !name_ptr.is_null() {
                    std::ffi::CStr::from_ptr(name_ptr).to_string_lossy()
                } else {
                    "<unknown>".into()
                }
            };
            let trace_entry = format!("  frame {}: {}", i, func_name);
            log::error!("{trace_entry}");
            trace.push(trace_entry.clone());
        }
    }
    trace
}

unsafe extern "system" fn veh(
    info: *mut EXCEPTION_POINTERS,
) -> i32 {
    let Some(info_ref) = (unsafe { info.as_ref() }) else {
        return EXCEPTION_CONTINUE_SEARCH;
    };

    let Some(record) = (unsafe { info_ref.ExceptionRecord.as_ref() }) else {
        return EXCEPTION_CONTINUE_SEARCH;
    };

    if record.ExceptionCode == EXCEPTION_ACCESS_VIOLATION {
        let av_type = record.ExceptionInformation[0];
        let av_addr = record.ExceptionInformation[1];

        let rw = match av_type {
            0 => "READ",
            1 => "WRITE",
            8 => "EXECUTE",
            _ => "UNKNOWN",
        };

        let (rip, rsp) = unsafe { info_ref.ContextRecord.as_ref() }
            .map_or((0, 0), |ctx| (ctx.Rip, ctx.Rsp));
        let trace = match unsafe { info_ref.ContextRecord.as_mut() } {
            Some(ctx) => print_stack(ctx),
            None => vec!["<null context>".to_string()],
        };

        let (func, file) = resolve_symbol(rip);

        crate::serror!(
            f;
            "[veh] ACCESS VIOLATION\n\
             rip=0x{:X} rsp=0x{:X}\n\
             {} addr=0x{:X}\n\
             function={}\n\
             location={}",
            rip,
            rsp,
            rw,
            av_addr,
            func.as_deref().unwrap_or("<unknown>"),
            file.as_deref().unwrap_or("<no line info>")
        );
        dispatch!(CrashEvent{
            event_type: format!("ACCESS VIOLATION ({rw})"),
            event_trace: trace,
        });

        sleep(Duration::from_secs(5));
    }

    EXCEPTION_CONTINUE_SEARCH
}

pub fn install() {
    unsafe { AddVectoredExceptionHandler(1, Some(veh)) };
}
