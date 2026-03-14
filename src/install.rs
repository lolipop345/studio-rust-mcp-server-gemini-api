use color_eyre::eyre::{Result, WrapErr};
use roblox_install::RobloxStudio;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::{fs, io};

fn get_message() -> String {
    "Roblox Studio Gemini Agent plugin installed successfully.\n\
     Please restart Roblox Studio to apply the changes.\n\
     \n\
     Available tools:\n\
     - run_code\n\
     - insert_model\n\
     - get_console_output\n\
     - start_stop_play\n\
     - run_script_in_play_mode\n\
     - get_studio_mode\n\
     - read_file / read_lines\n\
     - get_hierarchy / get_hierarchy_of\n\
     - get_properties / find_instances\n\
     - add_instance / add_json_instance\n\
     - remove_instance / move_instance / clone_instance\n\
     - replace_lines_with / replace_with\n\
     - import_from_http / import_from_wally\n\
     - create_script\n\
     - debug_script\n\
     \n\
     To use the agent, set the GEMINI_API_KEY environment variable and run the binary without flags.\n\
     To uninstall, delete the MCPStudioPlugin.rbxm from your Plugins directory."
        .to_string()
}

pub async fn do_install() -> Result<String> {
    let plugin_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/MCPStudioPlugin.rbxm"));
    let studio = RobloxStudio::locate()?;
    let plugins = studio.plugins_path();
    if let Err(err) = fs::create_dir(plugins) {
        if err.kind() != io::ErrorKind::AlreadyExists {
            return Err(err.into());
        }
    }
    let output_plugin = Path::new(&plugins).join("MCPStudioPlugin.rbxm");
    {
        let mut file = File::create(&output_plugin).wrap_err_with(|| {
            format!(
                "Could not write Roblox Plugin file at {}",
                output_plugin.display()
            )
        })?;
        file.write_all(plugin_bytes)?;
    }
    println!(
        "Installed Roblox Studio plugin to {}",
        output_plugin.display()
    );

    let msg = get_message();
    println!("\n{msg}");
    Ok(msg)
}

#[cfg(target_os = "windows")]
pub async fn install() -> Result<()> {
    use std::process::Command;
    if let Err(e) = do_install().await {
        tracing::error!("Failed to install Roblox Studio plugin: {:#}", e);
    }
    let _ = Command::new("cmd.exe").arg("/c").arg("pause").status();
    Ok(())
}

#[cfg(target_os = "macos")]
pub async fn install() -> Result<()> {
    use native_dialog::{DialogBuilder, MessageLevel};
    let alert_builder = match do_install().await {
        Err(e) => DialogBuilder::message()
            .set_level(MessageLevel::Error)
            .set_text(format!("Errors occurred: {e:#}")),
        Ok(msg) => DialogBuilder::message()
            .set_level(MessageLevel::Info)
            .set_text(msg),
    };
    let _ = alert_builder
        .set_title("Roblox Studio Gemini Agent")
        .alert()
        .show();
    Ok(())
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub async fn install() -> Result<()> {
    do_install().await?;
    Ok(())
}
