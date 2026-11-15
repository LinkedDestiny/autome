use std::os::unix::{fs::PermissionsExt, process::CommandExt};
use std::path::PathBuf;
use std::process::Command;

pub fn run(args: &[String]) -> i32 {
    if args.iter().any(|arg| arg == "--help" || arg == "-h")
        || args.first().is_some_and(|arg| arg == "help")
    {
        println!(
            "用法：autome web [命令] [选项]\n\n\
             命令：\n\
               open                    打开面板（缺省）\n\
               start                   启动面板\n\
               status [--json]          查看面板状态\n\
               stop [--force] [--wait 秒] 停止面板\n\
               service install|uninstall 安装或卸载自启动服务\n\n\
             选项：\n\
               --port <端口>           指定面板端口\n\
               --help, -h              显示此帮助\n\n\
             环境变量：\n\
               AUTOME_WEB_PORT         面板端口\n\
               AUTOME_HOME             Autome 数据目录"
        );
        return 0;
    }
    let node = std::env::var_os("AUTOME_NODE")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
                .map(|dir| dir.join("node"))
                .find(|path| {
                    path.metadata()
                        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
                })
        });
    let Some(node) = node else {
        eprintln!(
            "没有找到 Node.js（面板需要 22.22 或更高版本）。安装：brew install node，或从 https://nodejs.org 下载。"
        );
        return 1;
    };
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("无法定位 autome：{error}");
            return 1;
        }
    };
    let dir = exe.parent().expect("executable has a parent");
    let root = std::env::var_os("AUTOME_WEB_ROOT")
        .map(PathBuf::from)
        .into_iter()
        .chain([
            dir.join("web"),
            dir.join("../lib/autome/web"),
            dir.join("../../apps/web"),
        ])
        .find(|path| path.join("bin/autome-web.js").is_file());
    let Some(root) = root else {
        eprintln!("没有找到面板：只装了命令行。装好 Node ≥ 22.22 后重新运行 install-cli.sh");
        return 1;
    };
    let error = Command::new(node)
        .arg(root.join("bin/autome-web.js"))
        .args(args)
        .env("AUTOME_BUNDLED_CORE", exe)
        .exec();
    eprintln!("无法运行 Node.js：{error}；安装：brew install node，或从 https://nodejs.org 下载。");
    1
}
