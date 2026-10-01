use clap::Parser;
use eframe::egui;
use retro_cardmaker::cli::{self, Cli};
use retro_cardmaker::gui::RetroCardMakerApp;

fn main() -> eframe::Result<()> {
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
            .with_inner_size([1160.0, 760.0])
            .with_min_inner_size([920.0, 640.0])
            .with_title("Retro CardMaker - Handheld SD Card Setup & ROM Curating QuickInstaller"),
        ..Default::default()
    };

    eframe::run_native(
        "Retro CardMaker",
        options,
        Box::new(|cc| Ok(Box::new(RetroCardMakerApp::new(cc)))),
    )
}
