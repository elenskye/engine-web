//! 命令行：`engine-web tools`、`engine-web describe <工具>`、`engine-web call <工具> '<JSON>'`（engine-core 的骨架）。

fn main() -> std::process::ExitCode {
    engine_core::cli::run(&engine_web::tools::toolkit())
}
