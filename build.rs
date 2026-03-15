use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use librojo::cli;

fn main() {
    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out_path = PathBuf::from(&out_dir);

    // ── 1. Build the Roblox Studio plugin (.rbxm) ────────────────────────────
    let plugin_dest = out_path.join("MCPStudioPlugin.rbxm");
    eprintln!("Rebuilding plugin: {plugin_dest:?}");

    let options = cli::Options {
        global: cli::GlobalOptions {
            verbosity: 1,
            color: cli::ColorChoice::Always,
        },
        subcommand: cli::Subcommand::Build(cli::BuildCommand {
            project: PathBuf::from("plugin"),
            output: Some(plugin_dest),
            plugin: None,
            watch: false,
        }),
    };
    options.run().unwrap();

    // Recursively watch ALL files under plugin/ so that any .luau/.json change
    // triggers a rebuild of the embedded .rbxm
    fn emit_rerun(dir: &Path) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                println!("cargo:rerun-if-changed={}", path.display());
                if path.is_dir() {
                    emit_rerun(&path);
                }
            }
        }
    }
    emit_rerun(Path::new("plugin"));
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=electron/package.json");

    // ── 2. Generate electron-builder config ──────────────────────────────────
    let electron_builder_config = serde_json::json!({
        "appId": "com.chat-toolkit.gemini-studio",
        "productName": "GeminiStudio",
        "electronDist": "node_modules/electron/dist",
        "directories": {
            "output": "../dist"
        },
        "publish": serde_json::Value::Null,
        "files": [
            "**/*",
            "!user-themes",
            "!oauth_config.json",
            "!node_modules/electron",
            "!node_modules/electron-builder"
        ],
        // ── Windows (.exe) NSIS Installer ────────────────────────────────
        "win": {
            "target": [{
                "target": "nsis",
                "arch": ["x64"]
            }],
            "icon": "icons/AppLogo.ico",
            "extraResources": [
                {
                    "from": "../target/release/rbx-studio-mcp.exe",
                    "to": "server.exe"
                }
            ]
        },
        "nsis": {
            "oneClick": false,
            "allowToChangeInstallationDirectory": true,
            "createDesktopShortcut": "always",
            "createStartMenuShortcut": true,
            "shortcutName": "GeminiStudio",
            "uninstallDisplayName": "GeminiStudio",
            "installerIcon": "icons/AppLogo.ico",
            "uninstallerIcon": "icons/AppLogo.ico",
            "installerHeader": "icons/AppLogo.png",
            "installerSidebar": serde_json::Value::Null,
            "license": "../LICENSE",
            "perMachine": false,
            "allowElevation": true,
            "runAfterFinish": true,
            "deleteAppDataOnUninstall": false,
            "include": "installer/nsis-custom.nsh"
        },
        // ── macOS (.dmg) ─────────────────────────────────────────────────
        "mac": {
            "target": [{
                "target": "dir",
                "arch": ["x64"]
            }],
            "icon": "icons/AppLogoWithoutBG.png",
            "category": "public.app-category.developer-tools",
            "darkModeSupport": true,
            "hardenedRuntime": false,
            "identity": serde_json::Value::Null,
            "gatekeeperAssess": false,
            "extraResources": [
                {
                    "from": "../target/release/rbx-studio-mcp",
                    "to": "server"
                }
            ]
        },
        "dmg": {
            "background": serde_json::Value::Null,
            "iconSize": 128,
            "contents": [
                { "x": 176, "y": 192, "type": "file" },
                { "x": 432, "y": 192, "type": "link", "path": "/Applications" }
            ],
            "window": {
                "width": 660,
                "height": 400
            },
            "title": "GeminiStudio"
        },
        // ── Linux (.AppImage / .deb) ─────────────────────────────────────
        "linux": {
            "target": ["AppImage", "deb"],
            "icon": "icons/AppLogo.png",
            "category": "Development",
            "desktop": {
                "entry": {
                    "Name": "GeminiStudio",
                    "Comment": "Gemini-powered agent for Roblox Studio",
                    "Type": "Application",
                    "Categories": "Development;IDE;"
                }
            }
        }
    });

    let electron_dir = Path::new("electron");
    let builder_config_path = electron_dir.join("electron-builder.json");
    fs::write(
        &builder_config_path,
        serde_json::to_string_pretty(&electron_builder_config).unwrap(),
    )
    .expect("Failed to write electron-builder.json");
    eprintln!("Generated electron-builder config: {builder_config_path:?}");

    // ── 3. Generate NSIS custom script (Windows installer extras) ─────────
    let nsis_dir = electron_dir.join("installer");
    fs::create_dir_all(&nsis_dir).ok();

    let nsis_script = r##"!macro customHeader
!macroend

!macro customInit
!macroend

!macro customInstallMode
!macroend

!macro customPageAfterChangeDir
  Page custom OpenSourcePage

  Function OpenSourcePage
    nsDialogs::Create 1018
    Pop $0

    ${If} $0 == error
      Abort
    ${EndIf}

    ${NSD_CreateLabel} 0 10u 100% 24u "GeminiStudio is free and open source software (MIT License)."
    Pop $1

    ${NSD_CreateLabel} 0 40u 100% 20u "You have the right to use, modify, and distribute this software."
    Pop $2

    ${NSD_CreateLabel} 0 70u 100% 20u "Source Code && Support:"
    Pop $3

    ${NSD_CreateLink} 0 95u 100% 15u "https://github.com/studio-toolkit/chat-toolkit-rust-mcp"
    Pop $4
    ${NSD_OnClick} $4 OpenGitHub

    ${NSD_CreateLabel} 0 125u 100% 20u "If you find this useful, please give us a star on GitHub!"
    Pop $5

    nsDialogs::Show
  FunctionEnd

  Function OpenGitHub
    Pop $0
    ExecShell "open" "https://github.com/studio-toolkit/chat-toolkit-rust-mcp"
  FunctionEnd
!macroend

!macro customUnInit
!macroend

!macro customUnInstall
  DeleteRegKey HKCU "Software\GeminiStudio"
!macroend
"##;

    fs::write(nsis_dir.join("nsis-custom.nsh"), nsis_script)
        .expect("Failed to write NSIS custom script");
    eprintln!("Generated NSIS custom script");

    // ── 4. Update electron package.json with build scripts ───────────────
    let pkg_path = electron_dir.join("package.json");
    if pkg_path.exists() {
        let pkg_raw = fs::read_to_string(&pkg_path).unwrap();
        if let Ok(mut pkg) = serde_json::from_str::<serde_json::Value>(&pkg_raw) {
            if let Some(scripts) = pkg.get_mut("scripts").and_then(|s| s.as_object_mut()) {
                scripts.insert(
                    "build:win".to_string(),
                    serde_json::Value::String("electron-builder --win --config electron-builder.json".to_string()),
                );
                scripts.insert(
                    "build:mac".to_string(),
                    serde_json::Value::String("electron-builder --mac --config electron-builder.json".to_string()),
                );
                scripts.insert(
                    "build:linux".to_string(),
                    serde_json::Value::String("electron-builder --linux --config electron-builder.json".to_string()),
                );
                scripts.insert(
                    "build:all".to_string(),
                    serde_json::Value::String("electron-builder --win --mac --linux --config electron-builder.json".to_string()),
                );
            }

            // Add electron-builder as devDependency if missing
            if let Some(dev_deps) = pkg.get_mut("devDependencies").and_then(|d| d.as_object_mut()) {
                if !dev_deps.contains_key("electron-builder") {
                    dev_deps.insert(
                        "electron-builder".to_string(),
                        serde_json::Value::String("^26.8.1".to_string()),
                    );
                }
            }

            // Add author field
            if pkg.get("author").is_none() {
                pkg.as_object_mut().unwrap().insert(
                    "author".to_string(),
                    serde_json::json!({
                        "name": "Chat Toolkit Community",
                        "url": "https://github.com/studio-toolkit/chat-toolkit-rust-mcp"
                    }),
                );
            }

            // Add repository field
            if pkg.get("repository").is_none() {
                pkg.as_object_mut().unwrap().insert(
                    "repository".to_string(),
                    serde_json::json!({
                        "type": "git",
                        "url": "https://github.com/studio-toolkit/chat-toolkit-rust-mcp.git"
                    }),
                );
            }

            // Add license
            if pkg.get("license").is_none() {
                pkg.as_object_mut().unwrap().insert(
                    "license".to_string(),
                    serde_json::Value::String("MIT".to_string()),
                );
            }

            let updated = serde_json::to_string_pretty(&pkg).unwrap();
            fs::write(&pkg_path, updated).expect("Failed to update package.json");
            eprintln!("Updated electron/package.json with build scripts");
        }
    }
}
