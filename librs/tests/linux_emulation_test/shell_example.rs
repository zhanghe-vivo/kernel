// #![feature(start)]
use std::{
    io::{self, Write},
    thread,
};

#[no_mangle]
fn main() {
    thread::Builder::new()
        .name("shell".to_string())
        .stack_size(65536)
        .spawn(move || {
            println!("Hello, shell!");
            shell_loop();
        })
        .unwrap()
        .join()
        .unwrap();
}

fn shell_loop() {
    loop {
        print!("> ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();
        let input = input.trim();

        if input == "exit" {
            break;
        }

        if input.is_empty() {
            continue;
        }

        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts[0];
        let args = &parts[1..];

        match cmd {
            "echo" => println!("{}", args.join(" ")),
            "help" => {
                println!("可用命令:");
                println!("  exit      - 退出 Shell");
                println!("  echo <..> - 打印参数");
                println!("  cd <dir>  - 切换目录");
                println!("  pwd       - 显示当前目录");
                println!("  help      - 显示帮助");
            }
            _ => println!("未知命令: {}", cmd),
        }
    }
}
