// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // 设置日志过滤，完全屏蔽Tao事件循环警告
    std::env::set_var("RUST_LOG", "warn,tauri_app_lib=info,tao::platform_impl::platform::event_loop::runner=off");

    tauri_app_lib::run()
}
