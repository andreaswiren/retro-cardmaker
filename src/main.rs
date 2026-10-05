#![cfg_attr(windows, windows_subsystem = "windows")]

use clap::Parser;
use eframe::egui;
use retro_cardmaker::cli::{self, Cli};
use retro_cardmaker::gui::RetroCardMakerApp;

#[cfg(windows)]
fn attach_console() {
    unsafe {
        unsafe extern "system" {
            fn AttachConsole(dw_process_id: u32) -> i32;
            fn CreateFileW(
                lp_file_name: *const u16,
                dw_desired_access: u32,
                dw_share_mode: u32,
                lp_security_attributes: *const std::ffi::c_void,
                dw_creation_disposition: u32,
                dw_flags_and_attributes: u32,
                h_template_file: *const std::ffi::c_void,
            ) -> isize;
            fn SetStdHandle(n_std_handle: u32, h_handle: isize) -> i32;
        }

        if AttachConsole(0xFFFFFFFF) != 0 {
            let conout: Vec<u16> = "CONOUT$\0".encode_utf16().collect();
            let conin: Vec<u16> = "CONIN$\0".encode_utf16().collect();

            let h_out = CreateFileW(
                conout.as_ptr(),
                0x40000000 | 0x80000000,
                1 | 2,
                std::ptr::null(),
                3,
                0,
                std::ptr::null(),
            );
            if h_out != -1 && h_out != 0 {
                SetStdHandle(0xFFFFFFF5, h_out);
                SetStdHandle(0xFFFFFFF4, h_out);
            }

            let h_in = CreateFileW(
                conin.as_ptr(),
                0x80000000,
                1 | 2,
                std::ptr::null(),
                3,
                0,
                std::ptr::null(),
            );
            if h_in != -1 && h_in != 0 {
                SetStdHandle(0xFFFFFFF6, h_in);
            }
        }
    }
}

#[cfg(not(windows))]
fn attach_console() {}

fn main() -> eframe::Result<()> {
    std::panic::set_hook(Box::new(|info| {
        let _ = std::fs::write("panic.txt", format!("Panic: {:?}", info));
    }));

    attach_console();
    let args = Cli::parse();

    // If explicit CLI subcommand was requested, run command
    if let Some(cmd) = args.command {
        cli::run_cli_command(cmd);
        return Ok(());
    }

    // If --cli interactive wizard was requested
    if args.cli {
        cli::run_interactive_wizard();
        return Ok(());
    }

    // Default: Launch rich desktop GUI
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 780.0])
            .with_min_inner_size([960.0, 680.0])
            .with_title("Retro CardMaker - Windows 11"),
        ..Default::default()
    };

    let screenshot_path = args.screenshot.clone();
    let screenshot_tab = args.screenshot_tab.clone();
    let screenshot_modal = args.screenshot_modal;
    let screenshot_step = args.screenshot_step;
    let res = eframe::run_native(
        "Retro CardMaker",
        options,
        Box::new(move |cc| Ok(Box::new(RetroCardMakerApp::new(cc, screenshot_path, screenshot_tab, screenshot_modal, screenshot_step)))),
    );
    if let Err(ref e) = res {
        let _ = std::fs::write("crash.txt", format!("run_native error: {:?}", e));
    }
    res
}
