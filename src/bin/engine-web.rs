//! 命令行：`engine-web tools`、`engine-web describe <工具>`、`engine-web call <工具> '<JSON>'`（engine-core 的骨架）。

fn main() -> std::process::ExitCode {
    let web = engine_web::tools::Web {
        app: "命令行".into(),
        ..Default::default()
    };
    engine_core::cli::run(&engine_web::tools::toolkit_with(web))
}
