use librojo::cli;

fn main() {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    let dest_path = std::path::PathBuf::from(&out_dir).join("MCPStudioPlugin.rbxm");
    eprintln!("Rebuilding plugin: {dest_path:?}");
    let options = cli::Options {
        global: cli::GlobalOptions {
            verbosity: 1,
            color: cli::ColorChoice::Always,
        },
        subcommand: cli::Subcommand::Build(cli::BuildCommand {
            project: std::path::PathBuf::from("plugin"),
            output: Some(dest_path),
            plugin: None,
            watch: false,
        }),
    };
    options.run().unwrap();

    // Recursively watch ALL files under plugin/ so that any .luau/.json change
    // triggers a rebuild of the embedded .rbxm
    fn emit_rerun(dir: &std::path::Path) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                println!("cargo:rerun-if-changed={}", path.display());
                if path.is_dir() {
                    emit_rerun(&path);
                }
            }
        }
    }
    emit_rerun(std::path::Path::new("plugin"));
}
