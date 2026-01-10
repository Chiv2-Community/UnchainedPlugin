
use std::{ffi::CStr, mem::zeroed, thread::sleep, time::Duration};

use tokio::time::Sleep;
use winapi::vc::excpt::EXCEPTION_CONTINUE_SEARCH;
use windows::Win32::{Foundation::EXCEPTION_ACCESS_VIOLATION, System::{Diagnostics::Debug::{AddVectoredExceptionHandler, EXCEPTION_POINTERS, SYMBOL_INFO, SymFromAddr, SymInitialize}, Threading::{GetCurrentProcess, GetCurrentThreadId}}};



use windows::Win32::{
    Foundation::*,
    System::{
        Diagnostics::Debug::*,
    },
};

static mut SYMBOLS_READY: bool = false;

unsafe fn ensure_symbols() {
    if SYMBOLS_READY {
        return;
    }

    let process = GetCurrentProcess();

    // Load symbols for all modules
    SymInitialize(process, None, TRUE);
    SymSetOptions(
        SYMOPT_DEFERRED_LOADS
            | SYMOPT_UNDNAME
            | SYMOPT_LOAD_LINES,
    );

    SYMBOLS_READY = true;
}

unsafe fn resolve_symbol(addr: u64) -> (Option<String>, Option<String>) {
    ensure_symbols();

    let process = GetCurrentProcess();

    // ---------- Function name ----------
    let mut buffer = [0u8; std::mem::size_of::<SYMBOL_INFO>() + 256];
    let sym = buffer.as_mut_ptr() as *mut SYMBOL_INFO;

    (*sym).SizeOfStruct = std::mem::size_of::<SYMBOL_INFO>() as u32;
    (*sym).MaxNameLen = 255;

    let mut name = None;
    if SymFromAddr(process, addr, None, sym).is_ok() {
        // let cstr = CStr::from_ptr((*sym).Name.as_ptr());
        let cstr = CStr::from_ptr((*sym).Name.as_ptr() as *const i8);
        name = Some(cstr.to_string_lossy().into_owned());
    }

    // ---------- File + line ----------
    let mut line: IMAGEHLP_LINE64 = zeroed();
    line.SizeOfStruct = std::mem::size_of::<IMAGEHLP_LINE64>() as u32;

    let mut displacement = 0u32;
    let mut file_line = None;

    if SymGetLineFromAddr64(
        process,
        addr,
        &mut displacement,
        &mut line,
    )
    .is_ok()
    {
        let file = CStr::from_ptr(line.FileName.as_ptr() as *const i8);

        file_line = Some(format!(
            "{}:{} (+0x{:X})",
            file.to_string_lossy(),
            line.LineNumber,
            displacement
        ));
    }

    (name, file_line)
}
unsafe extern "system" fn sf_table_access64(hprocess: HANDLE, addrbase: u64) -> *mut std::ffi::c_void {
    SymFunctionTableAccess64(hprocess, addrbase)
}

unsafe extern "system" fn get_module_base64(hprocess: HANDLE, addr: u64) -> u64 {
    SymGetModuleBase64(hprocess, addr)
}

use winapi::um::winnt::IMAGE_FILE_MACHINE_AMD64;

use crate::{discord::notifications::CrashEvent, dispatch};
unsafe fn print_stack(ctx: *mut CONTEXT) -> Vec<String> {
    let process = GetCurrentProcess();
    let thread: HANDLE = HANDLE(-1); // current thread
    let mut frame: STACKFRAME64 = zeroed();
    let mut trace: Vec<String> = Vec::new();

    frame.AddrPC.Mode = AddrModeFlat;
    frame.AddrStack.Mode = AddrModeFlat;
    frame.AddrFrame.Mode = AddrModeFlat;

    frame.AddrPC.Offset = (*ctx).Rip;
    frame.AddrStack.Offset = (*ctx).Rsp;
    frame.AddrFrame.Offset = (*ctx).Rbp;

    for i in 0..10 {
        let ok = StackWalk64::<HANDLE, HANDLE>(
            IMAGE_FILE_MACHINE_AMD64.into(),
            process,
            thread,
            &mut frame,
            ctx as *mut _,
            None,
            Some(sf_table_access64),
            Some(get_module_base64),
            None,
        )
        .as_bool();

        if !ok || frame.AddrPC.Offset == 0 {
            break;
        }

        let mut sym_buffer = [0u8; std::mem::size_of::<SYMBOL_INFO>() + 256];
        let sym = sym_buffer.as_mut_ptr() as *mut SYMBOL_INFO;
        (*sym).SizeOfStruct = std::mem::size_of::<SYMBOL_INFO>() as u32;
        (*sym).MaxNameLen = 255;

        if SymFromAddr(process, frame.AddrPC.Offset, None, sym).is_ok() {
            let func_name =
                std::ffi::CStr::from_ptr((*sym).Name.as_ptr() as *const i8)
                    .to_string_lossy();
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
    if info.is_null() {
        return EXCEPTION_CONTINUE_SEARCH;
    }

    let record = (*info).ExceptionRecord;
    let ctx = (*info).ContextRecord;

    if (*record).ExceptionCode == EXCEPTION_ACCESS_VIOLATION {
        let rip = (*ctx).Rip;
        let rsp = (*ctx).Rsp;

        let av_type = (*record).ExceptionInformation[0];
        let av_addr = (*record).ExceptionInformation[1];

        let rw = match av_type {
            0 => "READ",
            1 => "WRITE",
            8 => "EXECUTE",
            _ => "UNKNOWN",
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
        let trace = print_stack(ctx);
        dispatch!(CrashEvent{
            event_type: format!("ACCESS VIOLATION ({rw})"),
            event_trace: trace,
        });

        sleep(Duration::from_secs(5));
    }

    EXCEPTION_CONTINUE_SEARCH
}

pub unsafe fn install() {
    AddVectoredExceptionHandler(1, Some(veh));
}
